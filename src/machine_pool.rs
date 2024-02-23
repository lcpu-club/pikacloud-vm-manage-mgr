use std::{collections::BTreeMap, env, io::Read, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    error::{VmManageError, VmManageResult},
    models::*,
    storage_models::{
        SnapshotCreateRequest, SnapshotCreateResponse, SnapshotDeleteRequest, VolumeAttachRequest,
        VolumeAttachResponse, VolumeCreateRequest, VolumeCreateResponse, VolumeDeleteRequest,
        VolumeDeleteResponse, VolumeDetachRequest, VolumeDetachResponse,
    },
};
use rustcracker::{
    components::machine::{Config, Machine, MachineCore},
    model::{
        drive::Drive, full_vm_configuration::FullVmConfiguration, instance_info::InstanceInfo,
        logger::LogLevel, machine_configuration::MachineConfiguration,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KernelItem {
    kernel_name: String,
    kernel_version: String,
    path: String,
}

// Get kernel image paths from a configuration file
// re-get every time to ensure dynamic modification
fn get_kernel_image_path(kernel_name: &String, kernel_version: &String) -> VmManageResult<PathBuf> {
    let kernel_list_file =
        env::var("KERNEL_LIST_FILE").map_err(|_| VmManageError::EnvironVarError("KERNEL_LIST_FILE"))?;
    let mut f = std::fs::File::open(kernel_list_file)?;
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;

    let list: Vec<KernelItem> = serde_json::from_str(&buf)?;
    let mut path: Option<String> = None;
    for e in &list {
        if &e.kernel_name == kernel_name && &e.kernel_version == kernel_version {
            path = Some(e.path.to_owned());
            break;
        }
    }
    match path {
        Some(path) => Ok(PathBuf::from(path)),
        None => Err(VmManageError::NotFound),
    }
}

/// Regulated basic functions that a Vm managing agent must have
#[async_trait]
pub trait VmManagePool {
    /// Configuration type that is used to boot the machine
    type ConfigType;
    type MachineIdentifier;
    async fn create_machine(
        &mut self,
        config: &Self::ConfigType,
    ) -> VmManageResult<Self::MachineIdentifier>;

    async fn start_machine(&self, vmid: &Self::MachineIdentifier) -> VmManageResult<()>;

    async fn pause_machine(&self, vmid: &Self::MachineIdentifier) -> VmManageResult<()>;

    async fn resume_machine(&self, vmid: &Self::MachineIdentifier) -> VmManageResult<()>;

    async fn stop_machine(&self, vmid: &Self::MachineIdentifier) -> VmManageResult<()>;

    async fn delete_machine(&mut self, vmid: &Self::MachineIdentifier) -> VmManageResult<()>;
}

#[allow(unused)]
#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone)]
pub struct PgMachineCoreElement {
    vmid: Uuid,
    machine_core: sqlx::types::Json<MachineCore>,
}

#[allow(unused)]
#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone)]
pub struct PgVmConfigElement {
    vmid: Uuid,
    config: sqlx::types::Json<Config>,
}

#[allow(unused)]
#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone)]
pub struct PgSnapshotElement {
    vmid: Uuid,
    snapshot_id: Uuid,
    mem_file_path: String,
    snapshot_path: String,
}

#[derive(Clone)]
pub struct FirecrackerVmManagePool {
    pool_id: Uuid,
    machines: BTreeMap<Uuid, Arc<Mutex<Machine>>>,
    conn: PgPool,
    storage_mgr_addr: String,
    storage_client: reqwest::Client,
}

#[async_trait]
impl VmManagePool for FirecrackerVmManagePool {
    type ConfigType = MachineCreateConfig;
    type MachineIdentifier = Uuid;
    async fn create_machine(&mut self, config: &MachineCreateConfig) -> VmManageResult<Uuid> {
        /* Create a new vmid for the machine */
        let vmid = Uuid::new_v4();
        let socket_path = PathBuf::from(format!(
            "{}{}.socket",
            env::var("SOCKETS_DIR").map_err(|_| VmManageError::EnvironVarError("SOCKETS_DIR"))?,
            vmid.to_string()
        ));
        let log_fifo = PathBuf::from(format!(
            "{}{}.log",
            env::var("LOGS_DIR").map_err(|_| VmManageError::EnvironVarError("LOGS_DIR"))?,
            vmid.to_string()
        ));
        let metrics_fifo = PathBuf::from(format!(
            "{}{}-metrics",
            env::var("METRICS_DIR").map_err(|_| VmManageError::EnvironVarError("METRICS_DIR"))?,
            vmid.to_string()
        ));
        let agent_init_timeout = env::var("AGENT_INIT_TIMEOUT")
            .map_err(|_| VmManageError::EnvironVarError("AGENT_INIT_TIMEOUT"))?
            .parse::<f64>()
            .map_err(|_| VmManageError::EnvironVarError("AGENT_INIT_TIMEOUT"))?;
        let agent_request_timeout = env::var("AGENT_INIT_TIMEOUT")
            .map_err(|_| VmManageError::EnvironVarError("AGENT_REQUEST_TIMEOUT"))?
            .parse::<f64>()
            .map_err(|_| VmManageError::EnvironVarError("AGENT_REQUEST_TIMEOUT"))?;

        /* Machine configuration for the microVM */
        let machine_cfg = MachineConfiguration {
            cpu_template: None,
            ht_enabled: config.enable_hyperthreading,
            mem_size_mib: config.memory_size_in_mib as isize,
            track_dirty_pages: None,
            vcpu_count: config.vcpu_count as isize,
        };
        let kernel_image_path = get_kernel_image_path(&config.kernel_name, &config.kernel_version)?;
        
        /* Request for a volume from storage manager */
        let volume_id = self.create_volume(config.volume_size_in_mib, None).await?;
        let volume_path = self.attach_volume(volume_id).await?;
        /* Config the root device */
        let root_device = Drive {
            drive_id: "rootfs".to_string(),
            partuuid: Some(volume_id.to_string()),
            is_root_device: true,
            cache_type: None,
            is_read_only: false,
            path_on_host: PathBuf::from(volume_path),
            rate_limiter: None,
            io_engine: None,
            socket: None,
        };

        /* Add the creating config to database */
        self.add_vm_config(&vmid, config).await?;

        /* Build the config */
        let config = Config {
            socket_path: Some(socket_path),
            log_fifo: Some(log_fifo),
            log_path: None,
            log_level: Some(LogLevel::Info),                    // Set log level to Info
            log_clear: Some(false),                             // Keep log fifo
            metrics_fifo: Some(metrics_fifo),
            metrics_path: None,
            metrics_clear: Some(false),                         // Keep metrics fifo
            kernel_image_path: Some(kernel_image_path),
            initrd_path: None,
            kernel_args: None,
            drives: Some(vec![root_device]),                    // Root device
            network_interfaces: None,                           // TODO: network interface
            vsock_devices: None,
            machine_cfg: Some(machine_cfg),
            disable_validation: true,                           // Enable validation
            enable_jailer: false,                               // Disable jailer
            jailer_cfg: None,
            vmid: None,
            net_ns: None,
            network_clear: Some(true),
            forward_signals: None,
            seccomp_level: None,
            mmds_address: None,
            balloon: None,
            init_metadata: config.initial_metadata.to_owned(),  // Initial metadata
            stderr: None,
            stdin: None,
            stdout: None,
            agent_init_timeout: Some(agent_init_timeout),
            agent_request_timeout: Some(agent_request_timeout),
        }; // Config

        /* Create the machine */
        let machine = Machine::new(config.to_owned())?;

        /* Dump to machine core */
        let core = machine.dump_into_core().map_err(|e| {
            VmManageError::MachineError(format!("Fail to dump into MachineCore: {}", e))
        })?;

        /* add to memory */
        self.machines.insert(vmid, Arc::new(Mutex::new(machine)));

        /* add core to database */
        self.add_core(&vmid, &core).await?;
        
        Ok(vmid)
    }

    async fn start_machine(&self, vmid: &Uuid) -> VmManageResult<()> {
        let machine = self.machines.get(&vmid).ok_or(VmManageError::NotFound)?;
        machine.lock().await.start().await?;
        Ok(())
    }

    async fn pause_machine(&self, vmid: &Uuid) -> VmManageResult<()> {
        let machine = self.machines.get(&vmid).ok_or(VmManageError::NotFound)?;
        machine.lock().await.pause().await?;
        Ok(())
    }

    async fn resume_machine(&self, vmid: &Uuid) -> VmManageResult<()> {
        let machine = self.machines.get(&vmid).ok_or(VmManageError::NotFound)?;
        machine.lock().await.resume().await?;
        Ok(())
    }

    async fn stop_machine(&self, vmid: &Uuid) -> VmManageResult<()> {
        let machine = self.machines.get(&vmid).ok_or(VmManageError::NotFound)?;
        machine.lock().await.shutdown().await?;
        machine.lock().await.stop_vmm().await?;
        self.delete_core(vmid).await?;
        Ok(())
    }

    async fn delete_machine(&mut self, vmid: &Uuid) -> VmManageResult<()> {
        let machine = self.machines.remove(&vmid).ok_or(VmManageError::NotFound)?;
        machine.lock().await.shutdown().await?;
        machine.lock().await.stop_vmm().await?;
        self.delete_core(vmid).await?;
        Ok(())
    }
}

impl FirecrackerVmManagePool {
    pub async fn new(pool_id: Uuid) -> VmManageResult<Self> {
        dotenv::dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let database_user = env::var("DATABASE_USER").expect("DATABASE_USER must be set");
        let database_password =
            env::var("DATABASE_PASSWORD").expect("DATABASE_PASSWORD must be set");
        let database_name = env::var("DATABASE_NAME").expect("DATABASE_NAME must be set");
        let storage_mgr_addr = env::var("STORAGE_MGR_ADDR").expect("STORAGE_MGR_ADDR must be set");

        let database_url = format!(
            "postgres://{}:{}@{}/{}",
            database_user, database_password, database_url, database_name
        );
        log::debug!("Database URL: {}", database_url);

        let conn = PgPoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await?;

        let storage_client = reqwest::Client::new();

        let pool = Self {
            pool_id,
            machines: BTreeMap::new(),
            conn,
            storage_mgr_addr,
            storage_client,
        };

        /* Create tables for storing */
        // machine core
        sqlx::query(DROP_MAHCINE_CORE_TABLE_SQL)
            .bind(pool.machine_core_storage_table())
            .execute(&pool.conn)
            .await?;
        sqlx::query(CREATE_MACHINE_CORE_TABLE_SQL)
            .bind(pool.machine_core_storage_table())
            .execute(&pool.conn)
            .await?;
        // vm, mem snapshots
        sqlx::query(DROP_VM_MEM_SNAPSHOT_TABLE_SQL)
            .bind(pool.vm_mem_snapshot_storage_table())
            .execute(&pool.conn)
            .await?;
        sqlx::query(CREATE_VM_MEM_SNAPSHOT_TABLE_SQL)
            .bind(pool.vm_mem_snapshot_storage_table())
            .execute(&pool.conn)
            .await?;
        // vm configs
        sqlx::query(DROP_VMVIEWCONFIGS_TABLE_SQL)
            .bind(pool.config_storage_table())
            .execute(&pool.conn)
            .await?;
        sqlx::query(CREATE_VMVIEWCONFIGS_TABLE_SQL)
            .bind(pool.config_storage_table())
            .execute(&pool.conn)
            .await?;
        Ok(pool)
    }
}

/// Postgresql related methods
impl FirecrackerVmManagePool {
    #[inline]
    fn machine_core_storage_table(&self) -> String {
        format!(
            "{}_{}",
            std::env::var(MACHINE_CORE_TABLE_NAME)
                .unwrap_or(DEFAULT_MACHINE_CORE_TABLE.to_string()),
            self.pool_id
        )
    }

    #[inline]
    fn config_storage_table(&self) -> String {
        format!(
            "{}_{}",
            std::env::var(VM_CONFIG_TABLE_NAME).unwrap_or(DEFAULT_VM_CONFIG_TABLE.to_string()),
            self.pool_id
        )
    }

    #[inline]
    fn vm_mem_snapshot_storage_table(&self) -> String {
        format!(
            "{}_{}",
            std::env::var(VM_MEM_SNAPSHOT_TABLE_NAME)
                .unwrap_or(DEFAULT_VM_MEM_SNAPSHOT_TABLE.to_string()),
            self.pool_id
        )
    }

    async fn add_core(&self, vmid: &Uuid, core: &MachineCore) -> VmManageResult<()> {
        let machine_core_storage_table = self.machine_core_storage_table();
        sqlx::query(INSERT_MACHINE_CORE_BY_VMID)
            .bind(machine_core_storage_table)
            .bind(vmid)
            .bind(sqlx::types::Json(core.to_owned()))
            .execute(&self.conn)
            .await?;

        Ok(())
    }

    async fn delete_core(&self, vmid: &Uuid) -> VmManageResult<()> {
        let machine_core_storage_table = self.machine_core_storage_table();
        sqlx::query(DELETE_MACHINE_CORE_BY_VMID)
            .bind(machine_core_storage_table)
            .bind(vmid)
            .execute(&self.conn)
            .await?;
        Ok(())
    }

    async fn add_vm_mem_snapshot(
        &self,
        vmid: &Uuid,
        snapshot_id: &Uuid,
        mem_file_path: &String,
        snapshot_path: &String,
    ) -> VmManageResult<()> {
        let vm_mem_snapshot_storage_table = self.vm_mem_snapshot_storage_table();
        sqlx::query(INSERT_VM_MEM_SNAPSHOT_BY_ID)
            .bind(vm_mem_snapshot_storage_table)
            .bind(vmid)
            .bind(snapshot_id)
            .bind(mem_file_path)
            .bind(snapshot_path)
            .execute(&self.conn)
            .await?;

        Ok(())
    }

    async fn delete_vm_mem_snapshot(&self, vmid: &Uuid, snapshot_id: &Uuid) -> VmManageResult<()> {
        let vm_mem_snapshot_storage_table = self.vm_mem_snapshot_storage_table();
        sqlx::query(DELETE_VM_MEM_SNAPSHOT_BY_ID)
            .bind(vm_mem_snapshot_storage_table)
            .bind(vmid)
            .bind(snapshot_id)
            .execute(&self.conn)
            .await?;

        Ok(())
    }

    async fn get_vm_mem_snapshot_detail(
        &self,
        vmid: &Uuid,
        snapshot_id: &Uuid,
    ) -> VmManageResult<Vec<PgSnapshotElement>> {
        let vm_mem_snapshot_storage_table = self.vm_mem_snapshot_storage_table();
        let list = sqlx::query_as::<_, PgSnapshotElement>(GET_VM_MEM_SNAPSHOT_BY_ID)
            .bind(vm_mem_snapshot_storage_table)
            .bind(vmid)
            .bind(snapshot_id)
            .fetch_all(&self.conn)
            .await?;

        Ok(list)
    }

    async fn add_vm_config(&self, vmid: &Uuid, config: &MachineCreateConfig) -> VmManageResult<()> {
        let config_storage_table = self.config_storage_table();
        sqlx::query(INSERT_VMVIEWCONFIGS_BY_VMID)
        .bind(config_storage_table)
        .bind(vmid)
        .bind(sqlx::types::Json(config))
        .execute(&self.conn)
        .await?;

        Ok(())
    }

    async fn delete_vm_config(&self, vmid: &Uuid) -> VmManageResult<()> {
        let config_storage_table = self.config_storage_table();
        sqlx::query(DELETE_VMVIEWCONFIGS_BY_VMID)
        .bind(config_storage_table)
        .bind(vmid)
        .execute(&self.conn)
        .await?;

        Ok(())
    }

    async fn get_vm_config_detail(&self, vmid: &Uuid) -> VmManageResult<Vec<PgVmConfigElement>> {
        let config_storage_table = self.config_storage_table();
        let elements = sqlx::query_as::<_, PgVmConfigElement>(GET_VMVIEWCONFIGS_BY_VMID)
        .bind(config_storage_table)
        .bind(vmid)
        .fetch_all(&self.conn)
        .await?;

        Ok(elements)
    }
}

impl FirecrackerVmManagePool {
    pub async fn restore_all(&mut self) -> VmManageResult<Vec<VmViewInfo>> {
        let machine_core_storage_table = self.machine_core_storage_table();
        let elements = sqlx::query_as::<_, PgMachineCoreElement>("SELECT * FROM $1")
            .bind(machine_core_storage_table)
            .fetch_all(&self.conn)
            .await?;

        let mut res = Vec::new();

        for element in elements {
            let vmid = element.vmid;
            let core = element.machine_core.0;
            let mut machine = Machine::rebuild(core)?;

            // Check whether the machine is still alive
            let vm_info = machine.describe_instance_info().await;
            let full_config = machine.get_export_vm_config().await;
            let config = machine.get_config();

            if vm_info.is_ok() && full_config.is_ok() {
                // machine still alive
                res.push(VmViewInfo {
                    vmid,
                    vm_info: vm_info.unwrap(),
                    full_config: full_config.unwrap(),
                    boot_config: config,
                })
            } else {
                // machine crushed
                res.push(VmViewInfo {
                    vmid,
                    vm_info: InstanceInfo {
                        app_name: String::new(),
                        id: String::new(),
                        state: rustcracker::model::instance_info::State::NotStarted,
                        vmm_version: String::new(),
                    },
                    full_config: FullVmConfiguration::default(),
                    boot_config: Config::default(),
                })
            }

            self.machines
                .insert(element.vmid, Arc::new(Mutex::new(machine)));
        }

        Ok(res)
    }

    pub async fn get_status(&self, vmid: &Uuid) -> VmManageResult<VmViewInfo> {
        if let Some(machine) = self.machines.get(vmid) {
            let mut machine = machine.lock().await;
            let vm_info = machine.describe_instance_info().await?;
            let config = machine.get_config();
            let full_config = machine.get_export_vm_config().await?;

            Ok(VmViewInfo {
                vmid: vmid.to_owned(),
                vm_info,
                full_config,
                boot_config: config,
            })
        } else {
            Err(VmManageError::NotFound)
        }
    }

    pub async fn modify_metadata(&self, vmid: &Uuid, metadata: &String) -> VmManageResult<()> {
        if let Some(machine) = self.machines.get(vmid) {
            let machine = machine.lock().await;
            machine.update_metadata(metadata).await?;

            Ok(())
        } else {
            Err(VmManageError::NotFound)
        }
    }
}

/// RPC to storage management (naive with http)
impl FirecrackerVmManagePool {
    async fn create_volume(&self, size: i32, parent: Option<Uuid>) -> VmManageResult<Uuid> {
        let url = format!("{}{}", self.storage_mgr_addr, "/api/v1/volume");
        let req = VolumeCreateRequest { size, parent };
        let res = self
            .storage_client
            .post(url)
            .json(&req)
            .send()
            .await?
            .json::<VolumeCreateResponse>()
            .await?;

        Ok(res.volume)
    }

    async fn delete_volume(&self, volume: Uuid) -> VmManageResult<Uuid> {
        let url = format!("{}{}", self.storage_mgr_addr, "/api/v1/volume");
        let req = VolumeDeleteRequest { volume };
        let res = self
            .storage_client
            .delete(url)
            .json(&req)
            .send()
            .await?
            .json::<VolumeDeleteResponse>()
            .await?;

        Ok(res.volume)
    }

    /// Get the volume path
    async fn attach_volume(&self, volume: Uuid) -> VmManageResult<String> {
        let url = format!("{}{}", self.storage_mgr_addr, "/api/v1/volume/attach");
        let req = VolumeAttachRequest { volume };
        let res = self
            .storage_client
            .post(url)
            .json(&req)
            .send()
            .await?
            .json::<VolumeAttachResponse>()
            .await?;

        Ok(res.device)
    }

    async fn detach_volume(&self, volume: Uuid) -> VmManageResult<Uuid> {
        let url = format!("{}{}", self.storage_mgr_addr, "/api/v1/volume/detach");
        let req = VolumeDetachRequest { volume };
        let res = self
            .storage_client
            .post(url)
            .json(&req)
            .send()
            .await?
            .json::<VolumeDetachResponse>()
            .await?;

        Ok(res.volume)
    }

    async fn volume_detail(&self, volume: Uuid) -> VmManageResult<()> {
        todo!()
    }

    async fn create_volume_snapshot(&self, volume: Uuid) -> VmManageResult<Uuid> {
        let url = format!("{}{}", self.storage_mgr_addr, "/api/v1/snapshot");
        let req = SnapshotCreateRequest { volume };
        let res = self
            .storage_client
            .post(url)
            .json(&req)
            .send()
            .await?
            .json::<SnapshotCreateResponse>()
            .await?;

        Ok(res.volume)
    }

    async fn delete_volume_snapshot(&self, volume: Uuid, snapshot: Uuid) -> VmManageResult<()> {
        let url = format!("{}{}", self.storage_mgr_addr, "/api/v1/snapshot");
        let req = SnapshotDeleteRequest { volume, snapshot };
        let _ = self.storage_client.post(url).json(&req).send().await?;

        Ok(())
    }
}

/// RPC to network management (naive with http)
impl FirecrackerVmManagePool {

}

/// Implementations for creating vm status and memory snapshots
impl FirecrackerVmManagePool {
    pub async fn create_snapshot(&self, vmid: &Uuid) -> VmManageResult<Uuid> {
        let vm_mem_snapshot_dir = env::var("MEMORY_SNAPSHOT_DIR")
            .map_err(|_| VmManageError::EnvironVarError("MEMORY_SNAPSHOT_DIR"))?;
        if let Some(machine) = self.machines.get(vmid) {
            let snapshot_id = Uuid::new_v4();
            let machine = machine.lock().await;
            let cur_dir = PathBuf::from(vm_mem_snapshot_dir)
                .join(vmid.to_string())
                .join(snapshot_id.to_string());
            let mem_file_path = &cur_dir.join("mem");
            let snapshot_path = &cur_dir.join("vm");
            machine
                .create_snapshot(mem_file_path, snapshot_path)
                .await?;
            // store into database
            self.add_vm_mem_snapshot(
                &vmid,
                &snapshot_id,
                &mem_file_path.to_string_lossy().to_string(),
                &snapshot_path.to_string_lossy().to_string(),
            )
            .await?;
            Ok(snapshot_id)
        } else {
            Err(VmManageError::NotFound)
        }
    }

    pub async fn delete_snapshot(&self, vmid: &Uuid, snapshot_id: &Uuid) -> VmManageResult<()> {
        let vm_mem_snapshot_dir = env::var("MEMORY_SNAPSHOT_DIR")
            .map_err(|_| VmManageError::EnvironVarError("MEMORY_SNAPSHOT_DIR"))?;
        let cur_dir = PathBuf::from(vm_mem_snapshot_dir)
            .join(vmid.to_string())
            .join(snapshot_id.to_string());
        if let Ok(_) = tokio::fs::remove_dir_all(cur_dir).await {
            // delete from database
            self.delete_vm_mem_snapshot(vmid, &snapshot_id).await?;
            Ok(())
        } else {
            Err(VmManageError::NotFound)
        }
    }

    pub async fn get_snapshot_detail(
        &self,
        vmid: &Uuid,
        snapshot_id: &Uuid,
    ) -> VmManageResult<Vec<PgSnapshotElement>> {
        self.get_vm_mem_snapshot_detail(vmid, snapshot_id).await
    }
}
