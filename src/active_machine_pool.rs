use std::{ffi::OsStr, path::PathBuf, sync::{Arc, Mutex}};

use log::{as_serde, error, info};
use moka::{future::{Cache, FutureExt}, notification::ListenerFuture, policy::EvictionPolicy};
use rustcracker::{components::machine::{Config, Machine, MachineCore}, model::{memory_backend::{BackendType, MemoryBackend}, snapshot_load_params::SnapshotLoadParams}};
use sqlx::PgPool;
use chrono::prelude::*;

use crate::{error::{VmManageError, VmManageResult}, models::{SnapshotInfo, VmViewInfo, DEFAULT_MACHINE_CORE_TABLE, DEFAULT_SNAPSHOT_TABLE, DEFAULT_VM_CONFIG_TABLE, MACHINE_CORE_TABLE_NAME, SNAPSHOT_TABLE_NAME, VM_CONFIG_TABLE_NAME}};


/// Serve as a cache and manage connection with Postgresql
/// if a Machine is active
pub struct ActiveMachinePool {
    pool_id: String,
    machines: Cache<String, Arc<Mutex<Machine>>>,
    conn: PgPool,
}

#[derive(sqlx::FromRow, sqlx::Type)]
#[sqlx(type_name = "pg_machine_core_element")]
pub struct PgMachineCoreElement {
    vmid: String,
    machine_core: sqlx::types::Json<MachineCore>,
}

#[derive(sqlx::FromRow, sqlx::Type)]
#[sqlx(type_name = "pg_vm_config_element")]
pub struct PgVmConfigElement {
    vmid: String,
    config: sqlx::types::Json<Config>,
}

#[derive(sqlx::FromRow, sqlx::Type)]
#[sqlx(type_name = "pg_snapshot_element")]
pub struct PgSnapshotElement {
    vmid: String,
    snapshot_id: String,
    snapshot_info: sqlx::types::Json<SnapshotInfo>,
}

// Private methods
impl ActiveMachinePool {
    async fn delete_core(&self, vmid: &String) -> VmManageResult<()> {
        let machine_core_storage_table = self.machine_core_storage_table();
        if let Err(_e) = sqlx::query("DELETE * FROM $1 WHERE vmid = $2")
            .bind(&machine_core_storage_table)
            .bind(vmid)
            .execute(&self.conn)
            .await
        {
            ()
        }
        Ok(())
    }

    async fn store_core(&self, vmid: &String, core: &MachineCore) -> VmManageResult<()> {
        let machine_core_storage_table = self.machine_core_storage_table();
        if let Err(e) =
            sqlx::query("INSERT INTO $1 (vmid, machine_core) VALUES ($2, $3)")
                .bind(&machine_core_storage_table)
                .bind(vmid)
                .bind(sqlx::types::Json(core.to_owned()))
                .execute(&self.conn)
                .await
        {
            error!(target: "ActiveMachinePool", "Fail to insert machine core into the database: {}", e);
            info!(target = "MachineCore", core = as_serde!(core); "Fail to store this core");
        }
        Ok(())
    }

    async fn delete_config(&self, vmid: &String) -> VmManageResult<()> {
        let config_storage_table = self.config_storage_table();
        if let Err(_e) = sqlx::query("DELETE * FROM $1 WHERE vmid = $2")
            .bind(&config_storage_table)
            .bind(vmid)
            .execute(&self.conn)
            .await
        {
            ()
        }
        Ok(())
    }

    async fn store_config(&self, vmid: &String, config: &Config) -> VmManageResult<()> {
        let config_storage_table = self.config_storage_table();
        if let Err(e) = sqlx::query("INSERT INTO $1 (vmid, config) VALUES ($2, $3)")
            .bind(&config_storage_table)
            .bind(vmid)
            .bind(sqlx::types::Json(config.clone()))
            .execute(&self.conn)
            .await
        {
            error!(target: "ActiveMachinePool", "Fail to insert vm config into the database: {}", e);
            info!(target = "Config", config = as_serde!(config); "Fail to store this config");
        }
        Ok(())
    }

    async fn delete_snapshot(&self, vmid: &String, snapshot_id: &String) -> VmManageResult<()> {
        let snapshot_table = self.snapshot_storage_table();
        if let Err(_e) = sqlx::query("DELETE * FROM $1 WHERE vmid = $2 AND snapshot_id = $3")
            .bind(&snapshot_table)
            .bind(vmid)
            .bind(snapshot_id)
            .execute(&self.conn)
            .await
        {
            ()
        }
        Ok(())
    }

    async fn store_snapshot(&self, vmid: &String, snapshot_id: &String, info: &SnapshotInfo) -> VmManageResult<()> {
        let snapshot_table = self.snapshot_storage_table();
        if let Err(e) = sqlx::query("INSERT INTO $1 (vmid, snapshot_id, snapshot_info) VALUES ($2, $3, $4)")
            .bind(&snapshot_table)
            .bind(vmid)
            .bind(snapshot_id)
            .bind(sqlx::types::Json(info.clone()))
            .execute(&self.conn)
            .await
        {
            error!(target: "ActiveMachinePool", "Fail to insert vm config into the database: {}", e);
            info!(target = "Snapshot", info = as_serde!(info); "Fail to store this snapshot info");
        }
        Ok(())
    }

    fn machine_core_storage_table(&self) -> String {
        format!("{}_{}", std::env::var(MACHINE_CORE_TABLE_NAME)
            .unwrap_or(DEFAULT_MACHINE_CORE_TABLE.to_string()), self.pool_id)
    }

    fn config_storage_table(&self) -> String {
        format!("{}_{}", std::env::var(VM_CONFIG_TABLE_NAME).unwrap_or(DEFAULT_VM_CONFIG_TABLE.to_string()), self.pool_id)
    }

    fn snapshot_storage_table(&self) -> String {
        format!("{}_{}", std::env::var(SNAPSHOT_TABLE_NAME).unwrap_or(DEFAULT_SNAPSHOT_TABLE.to_string()), self.pool_id)
    }
}

// Public methods
// Error handling:
// 1. vmid duplication
// 2. rebuild failure
// 3. 
impl ActiveMachinePool {
    pub async fn new(id: &str, size: u64, conn: PgPool) -> Self {
        let conn_back = conn.clone();
        let id_clone = id.to_string();
        let conn1 = Arc::new(conn);

        // eviction listener.
        // When the Machine agent is evicted then refresh the core and config into the database.
        let eviction_listener =
            move |k: Arc<String>, v: Arc<Mutex<Machine>>, cause| -> ListenerFuture {
                info!(target: "cache eviction", "A Machine has been evicted. vmid: {k:?}, cause: {cause:?}");
                let conn2 = Arc::clone(&conn1);
                let core = v.lock().unwrap().dump_into_core().unwrap();
                let config = v.lock().unwrap().get_config();
                let machine_core_storage_table = format!("{}_{}", std::env::var(MACHINE_CORE_TABLE_NAME)
                    .unwrap_or(DEFAULT_MACHINE_CORE_TABLE.to_string()), id_clone);
                let config_storage_table =
                    format!("{}_{}", std::env::var(VM_CONFIG_TABLE_NAME).unwrap_or(DEFAULT_VM_CONFIG_TABLE.to_string()), id_clone);
                async move {
                    /* Refresh MachineCore */
                    // First delete existing entry
                    if let Err(_e) = sqlx::query("DELETE * FROM $1 WHERE vmid = $2")
                        .bind(&machine_core_storage_table)
                        .bind(k.as_ref())
                        .execute(conn2.as_ref())
                        .await
                    {
                        ()
                    }
                    // Then store.
                    if let Err(e) =
                        sqlx::query("INSERT INTO $1 (vmid, machine_core) VALUES ($2, $3)")
                            .bind(&machine_core_storage_table)
                            .bind(k.as_ref())
                            .bind(sqlx::types::Json(core.to_owned()))
                            .execute(conn2.as_ref())
                            .await
                    {
                        error!(target: "cache eviction", "Fail to insert machine core into the database: {}", e);
                        info!(target = "MachineCore", core = as_serde!(core); "Fail to store this core");
                    }

                    /* Refresh Config */
                    // First delete existing entry
                    if let Err(_e) = sqlx::query("DELETE * FROM $1 WHERE vmid = $2")
                        .bind(&config_storage_table)
                        .bind(k.as_ref())
                        .execute(conn2.as_ref())
                        .await
                    {
                        ()
                    }

                    // Then store
                    if let Err(e) = 
                        sqlx::query("INSERT INTO $1 (vmid, config) VALUES ($2, $3)")
                            .bind(&config_storage_table)
                            .bind(k.as_ref())
                            .bind(sqlx::types::Json(config.clone()))
                            .execute(conn2.as_ref())
                            .await
                    {
                        error!(target: "cache eviction", "Fail to insert vm config into the database: {}", e);
                        info!(target = "Config", config = as_serde!(config); "Fail to store this config");
                    }
                    
                }
                .boxed()
            };

        let machines = Cache::builder()
            .max_capacity(size)
            .async_eviction_listener(eviction_listener)
            .eviction_policy(EvictionPolicy::tiny_lfu())
            .build();
        Self {
            pool_id: id.to_owned(),
            machines,
            conn: conn_back,
        }
    }

    /// Create a machine into the pool
    /// dump the core and configuration into database
    pub async fn create_machine(&self, vmid: &String, cfg: &Config) -> VmManageResult<()> {
        let (machine, _exit_ch) = Machine::new(cfg.clone())?;
        let core = machine.dump_into_core()?;

        // cache the machine
        self.machines
            .insert(vmid.to_string(), Arc::new(Mutex::new(machine)))
            .await;
        // store the machine core
        self.store_core(vmid, &core).await?;
        // store the config
        self.store_config(vmid, cfg).await?;

        Ok(())
    }

    /// Start machine by vmid, if not exists then error.
    pub async fn start_machine(&self, vmid: &String) -> VmManageResult<()> {
        let machine = self
            .machines
            .get(vmid)
            .await
            .ok_or(VmManageError::NotFound)?;
        machine.lock().unwrap().start().await?;
        Ok(())
    }

    /// Pause machine by vmid, if not exists then error.
    /// Useful then the user want to pause the machine and could easily resume
    /// it without cpu, memory and other status of machine being changed.
    pub async fn pause_machine(&self, vmid: &String) -> VmManageResult<()> {
        let machine = self
            .machines
            .get(vmid)
            .await
            .ok_or(VmManageError::NotFound)?;
        machine.lock().unwrap().pause().await?;
        Ok(())
    }

    /// Resume machine by vmid, if not exists then error.
    /// Resume from `Pause`
    pub async fn resume_machine(&self, vmid: &String) -> VmManageResult<()> {
        let machine = self
            .machines
            .get(vmid)
            .await
            .ok_or(VmManageError::NotFound)?;
        machine.lock().unwrap().resume().await?;
        Ok(())
    }

    /// Stop the machine by vmid.
    /// Useful when the user want to stop the machine and expect to re-use it.
    pub async fn stop_machine(&self, vmid: &String) -> VmManageResult<()> {
        if let Some(machine) = self
            .machines
            .remove(vmid)
            .await
        {
            // if the machine exists in memory
            // stop the firecracker process
            machine.lock().unwrap().shutdown().await?;
            machine.lock().unwrap().stop_vmm().await?;
        } else {
            // if not, rebuild it and stop it.
            let mut machine = self.rebuild_from_core(vmid).await?;
            machine.shutdown().await?;
            machine.stop_vmm().await?;
        }
        Ok(())
    }

    /// Delete machine by vmid (as well as MachineCore and Config stored in database)
    /// Useful when the user want to totally delete the machine.
    /// the machine will disappear along with its core and its config.
    /// if not exists then error.
    pub async fn delete_machine(&self, vmid: &String) -> VmManageResult<()> {
        // stop the machine
        self.stop_machine(vmid).await?;
        
        // delete core
        self.delete_core(vmid).await?;
        
        // delete config
        self.delete_config(vmid).await?;
        Ok(())
    }

    /// Rebuild machine by vmid from config.
    /// Useful when the user have `Stop`ped the machine and want to restart it again
    /// without storage(disk) and other devices configured with `Config` changed
    pub async fn rebuild_from_config(&self, vmid: &String) -> VmManageResult<()> {
        let config_storage_table = self.config_storage_table();
        let element = sqlx::query_as::<_, PgVmConfigElement>("SELECT * FROM $1 WHERE vmid = $2")
            .bind(config_storage_table)
            .bind(vmid)
            .fetch_one(&self.conn)
            .await?;

        // create the machine
        let config = element.config.0;
        self.create_machine(vmid, &config).await?;

        Ok(())
    }

    /// Rebuild machine by vmid from config.
    /// Hot rebuild. Re-attach the agent to the Vm if needed after the Machine instance
    /// is evicted from the cache or after the Vm manage process's corruption and restoration.
    async fn rebuild_from_core(&self, vmid: &String) -> VmManageResult<Machine> {
        let machine_core_storage_table = self.machine_core_storage_table();
        let element = sqlx::query_as::<_, PgMachineCoreElement>("SELETE * FROM $1 WHERE vmid = $2")
            .bind(machine_core_storage_table)
            .bind(vmid)
            .fetch_one(&self.conn)
            .await?;
        let core = element.machine_core.0;
        let (machine, _exit_ch) = Machine::rebuild(core)?;
        Ok(machine)
    }

    /// Rebuild all machine according to the cores stored in the database.
    pub async fn restore_all(&self) -> VmManageResult<()> {
        let machine_core_storage_table = self.machine_core_storage_table();
        let elements = sqlx::query_as::<_, PgMachineCoreElement>("SELECT * FROM $1")
            .bind(machine_core_storage_table)
            .fetch_all(&self.conn).await?;
        
        for element in elements {
            let core = element.machine_core.0;
            let (machine, _exit_ch) = Machine::rebuild(core)?;
            self.machines.insert(element.vmid, Arc::new(Mutex::new(machine))).await;
        }

        Ok(())
    }

    /// Query machine status.
    pub async fn get_status(&self, vmid: &String) -> VmManageResult<VmViewInfo> {
        let machine_core_storage_table = self.machine_core_storage_table();
        if let Some(machine) = self.machines.get(vmid).await {
            let mut machine = machine.lock().unwrap();
            let info = machine.describe_instance_info().await?;
            let config = machine.get_config();
            let full_config = machine.get_export_vm_config().await?;
            let res = VmViewInfo { user_id: String::new(), vmid: vmid.to_owned(), vm_info: info, full_config, boot_config: config };
            Ok(res)
        } else {
            let element = sqlx::query_as::<_, PgMachineCoreElement>("SELECT * FROM $1 WHERE vmid = $2")
                .bind(machine_core_storage_table)
                .bind(vmid)
                .fetch_one(&self.conn)
                .await?;
            let core = element.machine_core.0;
            let (mut machine, _exit_ch) = Machine::rebuild(core)?;
            let info = machine.describe_instance_info().await?;
            let config = machine.get_config();
            let full_config = machine.get_export_vm_config().await?;
            let res = VmViewInfo { user_id: String::new(), vmid: vmid.to_owned(), vm_info: info, full_config, boot_config: config };
            Ok(res)
        }
    }

    /// Get snapshot detail
    pub async fn get_snapshot_detail(&self, vmid: &String, snapshot_id: &String) -> VmManageResult<SnapshotInfo> {
        let snapshot_storage_table = self.snapshot_storage_table();
        let element = sqlx::query_as::<_, PgSnapshotElement>("SELECT * FROM $1 WHERE vmid = $2 AND snapshot_id = $3")
            .bind(&snapshot_storage_table)
            .bind(vmid)
            .bind(snapshot_id)
            .fetch_one(&self.conn)
            .await?;
        let info = element.snapshot_info.0;
        Ok(info)
    }

    /// Create snapshot at given path
    pub async fn create_snapshot(&self, vmid: &String, snapshot_id: &String, snapshot_path: &PathBuf, memory_path: &PathBuf) -> VmManageResult<()> {
        let machine_core_storage_table = self.machine_core_storage_table();
        if let Some(machine) = self.machines.get(vmid).await {
            let machine = machine.lock().unwrap();
            machine.create_snapshot(memory_path, snapshot_path).await?;
        } else {
            let element = sqlx::query_as::<_, PgMachineCoreElement>("SELECT * FROM $1 WHERE vmid = $2")
            .bind(machine_core_storage_table)
            .bind(vmid)
            .fetch_one(&self.conn)
            .await?;
            let core = element.machine_core.0; 
            let (machine, _exit_ch) = Machine::rebuild(core)?;
            machine.create_snapshot(memory_path, snapshot_path).await?;
        }

        // delete original snapshots (fs)

        // create current info
        let local: DateTime<Local> = Local::now();
        let info = SnapshotInfo {
            memory_path: memory_path.to_owned(),
            snapshot_path: snapshot_path.to_owned(),
            created_at: local,
        };
        self.delete_snapshot(vmid, snapshot_id).await?;
        self.store_snapshot(vmid, snapshot_id, &info).await?;

        Ok(())
    }

    /// Restore machine from given snapshot
    pub async fn restore_from_snapshot(&self, vmid: &String, snapshot_id: &String) -> VmManageResult<()> {
        let machine_core_storage_table = self.machine_core_storage_table();
        let snapshot_storage_table = self.snapshot_storage_table();
        let element = sqlx::query_as::<_, PgSnapshotElement>("SELECT * FROM $1 WHERE vmid = $2 AND snapshot_id = $3").bind(snapshot_storage_table).bind(vmid).bind(snapshot_id).fetch_one(&self.conn).await?;
        let info = element.snapshot_info.0;
        let snapshot_load_params = SnapshotLoadParams {
            enable_diff_snapshots: Some(true),
            mem_backend: Some(MemoryBackend{
                backend_path: info.memory_path,
                backend_type: BackendType::File,
            }),
            mem_file_path: None,
            resume_vm: Some(true),
            snapshot_path: info.snapshot_path,
        };
        if let Some(machine) = self.machines.get(vmid).await {
            let mut machine = machine.lock().unwrap();
            machine.load_from_snapshot(&snapshot_load_params).await?;
        } else {
            let element = sqlx::query_as::<_, PgMachineCoreElement>("SELECT * FROM $1 WHERE vmid = $2")
            .bind(machine_core_storage_table)
            .bind(vmid)
            .fetch_one(&self.conn)
            .await?;
            let core = element.machine_core.0; 
            let (mut machine, _exit_ch) = Machine::rebuild(core)?;
            machine.load_from_snapshot(&snapshot_load_params).await?;
        }

        Ok(())
    }

    /// Modify metadata of given machine
    pub async fn modify_metadata(&self, vmid: &String, metadata: &String) -> VmManageResult<()> {
        let machine_core_storage_table = self.machine_core_storage_table();
        if let Some(machine) = self.machines.get(vmid).await {
            let machine = machine.lock().unwrap();
            machine.update_metadata(metadata).await?;
        } else {
            let element = sqlx::query_as::<_, PgMachineCoreElement>("SELECT * FROM $1 WHERE vmid = $2")
            .bind(machine_core_storage_table)
            .bind(vmid)
            .fetch_one(&self.conn)
            .await?;
            let core = element.machine_core.0; 
            let (machine, _exit_ch) = Machine::rebuild(core)?;
            machine.update_metadata(metadata).await?;
        }

        Ok(())
    }

}

#[cfg(test)]
mod tests {

}