use crate::{active_machine_pool::ActiveMachinePool, models::*};
/// handler for the routes
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};

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
    pool: web::Data<ActiveMachinePool>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool.create_machine(request.vmid, &request.config).await;
    match res {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/api/v1/vm/{vmid}")]
async fn get_vm_status_handler(
    request: web::Json<VmQueryStatusRequest>,
    pool: web::Data<ActiveMachinePool>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool.get_status(request.vmid).await;
    match res {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[put("/api/v1/vm/{vmid}")]
async fn modify_metadata_handler(
    request: web::Json<VmModifyMetadataRequest>,
    pool: web::Data<ActiveMachinePool>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool.modify_metadata(request.vmid, &request.metadata).await;
    match res {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[put("/api/v1/vm/{vmid}/power_state")]
async fn operate_machine_handler(
    request: web::Json<VmOperateRequest>,
    pool: web::Data<ActiveMachinePool>,
) -> impl Responder {
    let request = request.into_inner();
    let res = match request.operation {
        Operation::Start => pool.start_machine(request.vmid).await,
        Operation::Pause => pool.pause_machine(request.vmid).await,
        Operation::Resume => pool.resume_machine(request.vmid).await,
        Operation::Stop => pool.stop_machine(request.vmid).await,
    };
    match res {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[delete("/api/v1/vm/delete")]
async fn delete_machine_handler(
    request: web::Json<VmDeleteRequest>,
    pool: web::Data<ActiveMachinePool>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool.delete_machine(request.vmid).await;
    match res {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/api/v1/vm/{vmid}/snapshots/{snapshot_id}")]
async fn get_snapshot_detail_handler(
    request: web::Json<VmSnapshotDetailRequest>,
    pool: web::Data<ActiveMachinePool>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool
        .get_snapshot_detail(request.vmid, request.snapshot_id)
        .await;
    match res {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[post("/api/v1/vm/{vmid}/snapshots")]
async fn create_snapshot_handler(
    request: web::Json<VmCreateSnapshotRequest>,
    pool: web::Data<ActiveMachinePool>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool
        .create_snapshot(
            request.vmid,
            &request.snapshot_path,
            &request.memory_path,
        )
        .await;
    match res {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[post("/api/v1/vm/{vmid}/snapshots/{snapshot_id}/restore")]
async fn restore_from_snapshot_handler(
    request: web::Json<VmLoadSnapshotRequest>,
    pool: web::Data<ActiveMachinePool>,
) -> impl Responder {
    let request = request.into_inner();
    let res = pool
        .restore_from_snapshot(
            request.vmid,
            request.snapshot_id,
        )
        .await;
    match res {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
