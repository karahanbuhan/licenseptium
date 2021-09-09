use std::{cmp::Ordering, str::FromStr};

use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use deadpool_postgres::Pool;
use licenseptium::{config::Config, database::create_tables, date::DateTimePlus};
use tokio_postgres::NoTls;
use uuid::Uuid;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Read environment variables and save to config for postgres
    let cfg = Config::from_env().unwrap();
    let pool = cfg.pg.create_pool(NoTls).unwrap();

    // Create default tables if not exists
    create_tables(pool.get().await.unwrap()).await.unwrap();

    // Start actix server
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .service(validate)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

#[get("validate/{key}")]
async fn validate(
    req: HttpRequest,
    key: web::Path<String>,
    pool: web::Data<Pool>,
) -> impl Responder {
    // Get IPv4 Address
    if req.peer_addr().is_none() {
        return HttpResponse::BadRequest();
    }
    let ipv4_addr = req.peer_addr().unwrap();
    if !ipv4_addr.is_ipv4() {
        return HttpResponse::BadRequest();
    }
    let ipv4_addr = ipv4_addr.ip();

    // Check if key format is UUID
    let key = Uuid::from_str(&key.replace("-", ""));
    if key.is_err() {
        return HttpResponse::BadRequest();
    }
    let key = key.unwrap();

    // Get new client from database pool
    let client = pool.get().await;
    if client.is_err() {
        return HttpResponse::InternalServerError();
    }
    let client = client.unwrap();

    // Get license row by key
    let rows = client
        .query("SELECT id, expiry_date FROM licenses WHERE key=$1", &[&key])
        .await
        .unwrap();
    // Return forbidden if license does not exist
    if rows.len() == 0 {
        return HttpResponse::Forbidden();
    }

    let row = &rows[0];
    let id: i32 = row.get("id");
    let expiry_date: DateTimePlus = row.get("expiry_date");

    // Return forbidden if license has expired
    if expiry_date.0.cmp(&chrono::offset::Utc::now()) == Ordering::Less {
        return HttpResponse::Forbidden();
    }

    // Insert validations into table
    client
            .execute(
                "INSERT INTO validations(ipv4_address, license_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                &[&ipv4_addr, &id],
            )
            .await
            .unwrap();

    HttpResponse::Ok()
}
