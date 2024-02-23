use actix_web::web;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    error::VmManageResult,
    machine_pool::{self, VmManagePool},
    models::{MachineCreateConfig, VmViewInfo},
};

/// Require global lock
pub async fn create_vm_op(
    pool: web::Data<Mutex<VmManagePool>>,
    config: &MachineCreateConfig,
) -> VmManageResult<Uuid> {
    let mut pool = pool.lock().await;

    let mut pool_guard = pool.lock_global().await?;
    let pool = pool_guard.pool();
    machine_pool::create_vm(pool, config).await
}

pub async fn start_vm_op(pool: web::Data<Mutex<VmManagePool>>, vmid: Uuid) -> VmManageResult<()> {
    let mut pool = pool.lock().await;
    let mut pool_guard = pool.lock(&vmid).await?;
    let pool = pool_guard.pool();
    machine_pool::start_vm(pool, vmid).await
}

pub async fn pause_vm_op(pool: web::Data<Mutex<VmManagePool>>, vmid: Uuid) -> VmManageResult<()> {
    let mut pool = pool.lock().await;
    let mut pool_guard = pool.lock(&vmid).await?;
    let pool = pool_guard.pool();
    machine_pool::pause_vm(pool, vmid).await
}

pub async fn resume_vm_op(pool: web::Data<Mutex<VmManagePool>>, vmid: Uuid) -> VmManageResult<()> {
    let mut pool = pool.lock().await;
    let mut pool_guard = pool.lock(&vmid).await?;
    let pool = pool_guard.pool();
    machine_pool::resume_vm(pool, vmid).await
}

pub async fn stop_vm_op(pool: web::Data<Mutex<VmManagePool>>, vmid: Uuid) -> VmManageResult<()> {
    let mut pool = pool.lock().await;
    let mut pool_guard = pool.lock(&vmid).await?;
    let pool = pool_guard.pool();
    machine_pool::stop_vm(pool, vmid).await
}

/// Require global lock
pub async fn delete_vm_op(pool: web::Data<Mutex<VmManagePool>>, vmid: Uuid) -> VmManageResult<()> {
    let mut pool = pool.lock().await;
    let mut pool_guard = pool.lock_global().await?;
    let pool = pool_guard.pool();
    machine_pool::delete_vm(pool, vmid).await
}

pub async fn get_vm_status_op(pool: web::Data<Mutex<VmManagePool>>, vmid: Uuid) -> VmManageResult<VmViewInfo> {
    let mut pool = pool.lock().await;
    let mut pool_guard = pool.lock(&vmid).await?;
    let pool = pool_guard.pool();
    machine_pool::get_status(pool, vmid).await
}

pub async fn modify_metadata_op(pool: web::Data<Mutex<VmManagePool>>, vmid: Uuid, metadata: &String) -> VmManageResult<()> {
    let mut pool = pool.lock().await;
    let mut pool_guard = pool.lock(&vmid).await?;
    let pool = pool_guard.pool();
    machine_pool::modify_metadata(pool, vmid, metadata).await
}

/// Require global lock
pub async fn restore_all_op(pool: web::Data<Mutex<VmManagePool>>) -> VmManageResult<Vec<Uuid>> {
    let mut pool = pool.lock().await;
    let mut pool_guard = pool.lock_global().await?;
    let pool = pool_guard.pool();
    machine_pool::restore_all(pool).await
}