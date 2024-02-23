

use crate::{
    machine_pool::{self, VmManagePool},
    models::*, operation::*,
};
/// handler for the routes
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use tokio::sync::Mutex;

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
    pool: web::Data<Mutex<VmManagePool>>,
    request: web::Json<VmCreateRequest>,
) -> impl Responder {
    let request = request.into_inner();
    let res = create_vm_op(pool, &request.config).await;
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
    pool: web::Data<Mutex<VmManagePool>>,
    request: web::Json<VmQueryStatusRequest>,
) -> impl Responder {
    let request = request.into_inner();
    let res = get_vm_status_op(pool, request.vmid).await;
    match res {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[put("/api/v1/vm/{vmid}")]
async fn modify_metadata_handler(
    pool: web::Data<Mutex<VmManagePool>>,
    request: web::Json<VmModifyMetadataRequest>,
) -> impl Responder {
    let request = request.into_inner();
    let res = modify_metadata_op(pool, request.vmid, &request.metadata).await;
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
    pool: web::Data<Mutex<VmManagePool>>,
    request: web::Json<VmOperateRequest>,
) -> impl Responder {
    let request = request.into_inner();
    let res = match request.operation {
        Operation::Start => start_vm_op(pool, request.vmid).await,
        Operation::Pause => pause_vm_op(pool, request.vmid).await,
        Operation::Resume => resume_vm_op(pool, request.vmid).await,
        Operation::Stop => stop_vm_op(pool, request.vmid).await,
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
    pool: web::Data<Mutex<VmManagePool>>,
    request: web::Json<VmDeleteRequest>,
) -> impl Responder {
    let request = request.into_inner();
    let res = delete_vm_op(pool, request.vmid).await;
    match res {
        Ok(_) => HttpResponse::Ok().json(VmDeleteResponse {
            vmid: request.vmid,
            time: chrono::Local::now(),
        }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// Attach to the machine with specified device (root file system / virtio device)
#[post("/api/v1/vm/{vmid}/volume/attach")]
async fn attach_volume_to_machine_handler(
    request: web::Json<()>,
    pool: web::Data<Mutex<VmManagePool>>,
) -> impl Responder {
    HttpResponse::Ok().finish()
}

/// Delete Volume from the machine of specified device (root file system / virtio device)
#[delete("/api/v1/vm/{vmid}/volume/delete")]
async fn delete_volume_from_machine_handler(
    request: web::Json<()>,
    pool: web::Data<Mutex<VmManagePool>>,
) -> impl Responder {
    HttpResponse::Ok().finish()
}

#[post("/api/v1/vm/restoreall")]
async fn restore_all_machines_from_core(
    request: web::Json<()>,
    pool: web::Data<Mutex<VmManagePool>>,
) -> impl Responder {
    let request = request.into_inner();
    let res = restore_all_op(pool).await;
    match res {
        Ok(infos) => HttpResponse::Ok().json(VmRestoreAllResponse { infos }),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
