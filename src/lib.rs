use actix_web::Result;
use deadpool_postgres::tokio_postgres::error::Error;
use deadpool_postgres::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub pg: deadpool_postgres::Config,
}

impl Config {
    pub fn from_env() -> Result<Self, ::config::ConfigError> {
        let mut cfg = ::config::Config::new();
        cfg.set_default("pg.dbname", "licenseptium")?;
        cfg.merge(::config::Environment::new().separator("_"))?;
        cfg.try_into()
    }
}

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
