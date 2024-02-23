use std::env;

use pikacloud_vm_manage_mgr::models::{
    Operation, VmCreateResponse, VmDeleteRequest, VmDeleteResponse, VmOperateRequest, VmOperateResponse
};
use tokio::time::{sleep, Duration};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let target_addr = env::var("LISTENING_ADDR").expect("LISTENING_ADDR must be set");
    let target_port = env::var("LISTENING_PORT").expect("LISTENING_PORT must be set");
    let target_addr = format!("{}{}", target_addr, target_port);
    let client = reqwest::Client::new();

    // User want to get the root page
    let index_page = client
        .get(format!("{}{}", target_addr, "/api/v1/vm"))
        .send()
        .await?
        .json::<String>()
        .await?;
    println!("Get index page: {}", index_page);
    sleep(Duration::from_secs(3)).await;

    // User want to create a new machine
    let create_machine_response = client
        .post(format!("{}{}", target_addr, "api/v1/vm"))
        .send()
        .await?
        .json::<VmCreateResponse>()
        .await?;
    let machine_vmid = create_machine_response.vmid;
    let create_at = create_machine_response.created_at;
    println!(
        "Created machine with vmid {} created at {}",
        machine_vmid, create_at
    );
    sleep(Duration::from_secs(3)).await;

    // User want to start the machine
    let start_machine_response = client
        .put(format!(
            "{}{}",
            target_addr,
            format!("/api/v1/vm/{machine_vmid}/power_state")
        ))
        .json(&VmOperateRequest {
            vmid: machine_vmid,
            operation: Operation::Start,
        })
        .send()
        .await?
        .json::<VmOperateResponse>()
        .await?;
    let start_at = start_machine_response.time;
    println!("Started the machine at {}", start_at);
    sleep(Duration::from_secs(3)).await;

    // User want to pause the machine
    let pause_machine_response = client
        .put(format!(
            "{}{}",
            target_addr,
            format!("/api/v1/vm/{machine_vmid}/power_state")
        ))
        .json(&VmOperateRequest {
            vmid: machine_vmid,
            operation: Operation::Pause,
        })
        .send()
        .await?
        .json::<VmOperateResponse>()
        .await?;
    let paused_at = pause_machine_response.time;
    println!("Paused the machine at {}", paused_at);
    sleep(Duration::from_secs(3)).await;

    // User want to resume the machine
    let resume_machine_response = client
        .put(format!(
            "{}{}",
            target_addr,
            format!("/api/v1/vm/{machine_vmid}/power_state")
        ))
        .json(&VmOperateRequest {
            vmid: machine_vmid,
            operation: Operation::Resume,
        })
        .send()
        .await?
        .json::<VmOperateResponse>()
        .await?;
    let resumed_at = resume_machine_response.time;
    println!("Resumed the machine at {}", resumed_at);
    sleep(Duration::from_secs(3)).await;

    // User want to stop the machine
    let stop_machine_response = client
        .put(format!(
            "{}{}",
            target_addr,
            format!("/api/v1/vm/{machine_vmid}/power_state")
        ))
        .json(&VmOperateRequest {
            vmid: machine_vmid,
            operation: Operation::Stop,
        })
        .send()
        .await?
        .json::<VmOperateResponse>()
        .await?;
    let stopped_at = stop_machine_response.time;
    println!("Resumed the machine at {}", stopped_at);
    sleep(Duration::from_secs(3)).await;

    // User want to discard the machine
    let delete_machine_response =  client
        .delete(format!(
            "{}{}",
            target_addr,
            format!("/api/v1/vm/delete")
        ))
        .json(&VmDeleteRequest {
            vmid: machine_vmid,
        })
        .send()
        .await?
        .json::<VmDeleteResponse>()
        .await?;
    let deleted_at = delete_machine_response.time;
    println!("Resumed the machine at {}", deleted_at);
    sleep(Duration::from_secs(3)).await;

    println!("User exiting...");
    Ok(())
}
