use actix_web::{
    get,
    web::{self},
    App, HttpRequest, HttpResponse, HttpServer,
};
use deadpool_postgres::Pool;
use licenseptium::{
    config::Config, database::create_tables, date::DateTimePlus, error::ValidationError,
};
use serde::Deserialize;
use serde_json::json;
use tokio_postgres::NoTls;
use uuid::Uuid;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Read environment variables and save to config for postgres
    let cfg = Config::from_env().unwrap();
    let pool = cfg.pg.create_pool(NoTls).unwrap();

    // Create default tables if not exists
    create_tables(&pool.get().await.unwrap()).await.unwrap();

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

#[derive(Deserialize)]
struct ValidationArgs {
    key: String,
    checksum: String,
}

#[get("/validate/{key}/{checksum}")]
async fn validate(
    req: HttpRequest,
    args: web::Path<ValidationArgs>,
    pool: web::Data<Pool>,
) -> Result<HttpResponse, ValidationError> {
    let sock_addr = req.peer_addr().ok_or(ValidationError::IPAddressNotFound)?;
    let ipv4_addr = sock_addr
        .is_ipv4()
        .then(|| sock_addr.ip())
        .ok_or(ValidationError::BadIPVersion)?;

    let key = Uuid::parse_str(&args.key).or(Err(ValidationError::MalformedKey))?;

    let client = pool
        .get()
        .await
        .map_err(|_| ValidationError::DatabaseError)?;

    let rows = client
        .query(
            "SELECT id, ip_limit, expires_at, checksum FROM licenses WHERE key=$1",
            &[&key],
        )
        .await
        .map_err(|_| ValidationError::DatabaseError)?;
    let row = rows.first().ok_or(ValidationError::InvalidKey)?;
    let id: i32 = row.get("id");
    let ip_limit: i32 = row.get("ip_limit");
    let expires_at: DateTimePlus = row.get("expires_at");
    let checksum: Vec<u8> = row.get("checksum");

    let rows = client
        .query(
            "SELECT COUNT(*) FROM validations WHERE ipv4_address!=$1 AND license_id=$2",
            &[&ipv4_addr, &id],
        )
        .await
        .map_err(|_| ValidationError::DatabaseError)?;
    let count: i64 = rows
        .first()
        .ok_or(ValidationError::DatabaseError)?
        .get("count");

    if count >= ip_limit as i64 {
        return Err(ValidationError::ReachedActivationLimit);
    }

    if expires_at.0.cmp(&chrono::offset::Utc::now()).is_lt() {
        return Err(ValidationError::ExpiredKey);
    }

    if checksum != hex::decode(&args.checksum).map_err(|_| ValidationError::MalformedChecksum)? {
        return Err(ValidationError::InvalidChecksum);
    }

    client
        .execute(
            "INSERT INTO validations(ipv4_address, license_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            &[&ipv4_addr, &id],
        )
        .await.map_err(|_| ValidationError::DatabaseError)?;

    Ok(HttpResponse::Ok().json(json!({ "key": &args.key, "checksum": &args.checksum })))
}
