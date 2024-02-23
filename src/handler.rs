use crate::{
    active_machine_pool::ActiveMachinePool,
    machine_pool::{FirecrackerVmManagePool, VmManagePool},
    models::*,
};
/// handler for the routes
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use rustcracker::components::machine::Config;
use tokio::sync::Mutex;
use uuid::Uuid;

#[get("/api/v1")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body("Hello, world!")
}

#[get("/api/v1/vm")]
async fn get_root_vm_page_handler() -> impl Responder {
    HttpResponse::Ok().body("Index page of Vm management")
}

#[post("/api/v1/vm")]
async fn create_machine_handler(
    request: web::Json<VmCreateRequest>,
    pool: web::Data<Mutex<FirecrackerVmManagePool>>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool.lock().await.create_machine(&request.config).await;
    match res {
        Ok(vmid) => HttpResponse::Ok().json(VmCreateResponse {
            vmid,
            created_at: chrono::Local::now(),
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/api/v1/vm/{vmid}")]
async fn get_vm_status_handler(
    request: web::Json<VmQueryStatusRequest>,
    pool: web::Data<Mutex<FirecrackerVmManagePool>>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool.lock().await.get_status(&request.vmid).await;
    match res {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[put("/api/v1/vm/{vmid}")]
async fn modify_metadata_handler(
    request: web::Json<VmModifyMetadataRequest>,
    pool: web::Data<Mutex<FirecrackerVmManagePool>>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool
        .lock()
        .await
        .modify_metadata(&request.vmid, &request.metadata)
        .await;
    match res {
        Ok(_) => HttpResponse::Ok().json(VmModifyMetadataResponse {
            vmid: request.vmid,
            time: chrono::Local::now(),
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[put("/api/v1/vm/{vmid}/power_state")]
async fn operate_machine_handler(
    request: web::Json<VmOperateRequest>,
    pool: web::Data<Mutex<FirecrackerVmManagePool>>,
) -> impl Responder {
    let request = request.into_inner();
    let res = match request.operation {
        Operation::Start => pool.lock().await.start_machine(&request.vmid).await,
        Operation::Pause => pool.lock().await.pause_machine(&request.vmid).await,
        Operation::Resume => pool.lock().await.resume_machine(&request.vmid).await,
        Operation::Stop => pool.lock().await.stop_machine(&request.vmid).await,
    };
    match res {
        Ok(_) => HttpResponse::Ok().json(VmOperateResponse {
            vmid: request.vmid,
            time: chrono::Local::now(),
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[delete("/api/v1/vm/delete")]
async fn delete_machine_handler(
    request: web::Json<VmDeleteRequest>,
    pool: web::Data<Mutex<FirecrackerVmManagePool>>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool.lock().await.delete_machine(&request.vmid).await;
    match res {
        Ok(_) => HttpResponse::Ok().json(VmDeleteResponse {
            vmid: request.vmid,
            time: chrono::Local::now(),
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/api/v1/vm/{vmid}/snapshots/{snapshot_id}")]
async fn get_snapshot_detail_handler(
    request: web::Json<VmSnapshotDetailRequest>,
    pool: web::Data<Mutex<FirecrackerVmManagePool>>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool.
        lock().await
        .get_snapshot_detail(&request.vmid, &request.snapshot_id)
        .await;
    match res {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[post("/api/v1/vm/{vmid}/snapshots")]
async fn create_snapshot_handler(
    request: web::Json<VmCreateSnapshotRequest>,
    pool: web::Data<Mutex<FirecrackerVmManagePool>>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool.
        lock().await
        .create_snapshot(
            &request.vmid,
        )
        .await;
    match res {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

// #[post("/api/v1/vm/{vmid}/snapshots/{snapshot_id}/restore")]
// async fn restore_from_snapshot_handler(
//     request: web::Json<VmLoadSnapshotRequest>,
//     pool: web::Data<Mutex<FirecrackerVmManagePool>>,
// ) -> impl Responder {
//     let request = request.into_inner();
//     let res = pool.
//         lock().await
//         .restore_from_snapshot(
//             &request.vmid,
//             &request.snapshot_id,
//         )
//         .await;
//     match res {
//         Ok(_) => HttpResponse::Ok().finish(),
//         Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
//     }
// }

/// Attach to the machine with specified device (root file system / virtio device)
#[post("/api/v1/vm/{vmid}/volume/attach")]
async fn attach_volume_to_machine_handler(
    request: web::Json<()>,
    pool: web::Data<Mutex<FirecrackerVmManagePool>>,
) -> impl Responder {
    HttpResponse::Ok().finish()
}

/// Delete Volume from the machine of specified device (root file system / virtio device)
#[delete("/api/v1/vm/{vmid}/volume/delete")]
async fn delete_volume_from_machine_handler(
    request: web::Json<()>,
    pool: web::Data<Mutex<FirecrackerVmManagePool>>,
) -> impl Responder {
    HttpResponse::Ok().finish()
}

#[post("/api/v1/vm/restoreall")]
async fn restore_all_machines_from_core(
    request: web::Json<()>,
    pool: web::Data<Mutex<FirecrackerVmManagePool>>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool.lock().await.restore_all().await;
    match res {
        Ok(infos) => HttpResponse::Ok().json(VmRestoreAllResponse { infos }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
