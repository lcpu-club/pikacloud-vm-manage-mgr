mod handler;
mod error;
mod models;
mod database;
mod active_machine_pool;

use active_machine_pool::ActiveMachinePool;
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // get config from .env file
    dotenv().ok();

    let conn = PgPoolOptions::new().connect("postgres://xuehaonan:xuehaonan@localhost/vm_manage").await?;
    let pool = ActiveMachinePool::new(Uuid::new_v4(), 256, conn).await;
    
    Ok(())
}