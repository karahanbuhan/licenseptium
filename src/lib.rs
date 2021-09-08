use actix_web::Result;
use deadpool_postgres::Client;

use deadpool_postgres::tokio_postgres::error::Error;

pub async fn create_tables(client: Client) -> Result<(), Error> {
    client
        .simple_query(
            "CREATE TABLE IF NOT EXISTS licenses (
                id              SERIAL PRIMARY KEY,
                key             UUID UNIQUE NOT NULL,
                comment         TEXT NOT NULL
            )",
        )
        .await?;

    client
        .simple_query(
            "CREATE TABLE IF NOT EXISTS validations (
                ipv4_address    INET NOT NULL,
                license_id      SERIAL NOT NULL,
                PRIMARY KEY (ipv4_address, license_id)
            )",
        )
        .await?;

    Ok(())
}
