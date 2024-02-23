use std::{default, env, sync::Mutex};

use actix_web::{web, App, HttpServer};
use dotenv::dotenv;
use pikacloud_vm_manage_mgr::{handler::*, machine_pool::FirecrackerVmManagePool};

use uuid::Uuid;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // get config from .env file
    dotenv().ok();

    log4rs::init_file("log4rs.yaml", default::Default::default()).unwrap();

    let pool = FirecrackerVmManagePool::new(Uuid::new_v4())
        .await
        .map_err(|e| {
            eprintln!("Fail to build a vm pool: {}", e.to_string());
            panic!();
        });

    let listen_address = match env::var("LISTENING_ADDR") {
        Ok(address) => address,
        Err(_) => {
            log::warn!("No bind address found in .env file, using 0.0.0.0");
            "0.0.0.0".to_owned()
        }
    };
    let port = match env::var("LISTENING_PORT") {
        Ok(port) => port.parse::<u16>().expect("Failed to parse port"),
        Err(_) => {
            log::warn!("No port found in .env file, using 58890");
            58890
        }
    };

    HttpServer::new(move || {
        App::new()
            .service(index)
            .service(get_root_vm_page_handler)
            .service(create_machine_handler)
            .service(get_vm_status_handler)
            .service(modify_metadata_handler)
            .service(operate_machine_handler)
            .service(delete_machine_handler)
            .service(get_snapshot_detail_handler)
            .service(create_snapshot_handler)
            .service(attach_volume_to_machine_handler)
            .service(delete_volume_from_machine_handler)
            .service(restore_all_machines_from_core)
            .app_data(web::Data::new(Mutex::new(pool.clone())))
    })
    .bind((listen_address, port))?
    .run()
    .await

}
