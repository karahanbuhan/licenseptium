pub mod config {
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
}

pub mod database {
    use deadpool_postgres::tokio_postgres::error::Error;
    use deadpool_postgres::Client;

    pub async fn create_tables(client: Client) -> Result<(), Error> {
        client
            .simple_query(
                r#"
                CREATE TABLE IF NOT EXISTS licenses (
                    id              SERIAL PRIMARY KEY,
                    key             UUID UNIQUE NOT NULL,                
                    comment         TEXT NOT NULL,
                    expiry_date     TIMESTAMPTZ NOT NULL
                )"#,
            )
            .await?;
        client
            .simple_query(
                r#"
                CREATE TABLE IF NOT EXISTS validations (
                    ipv4_address    INET NOT NULL,
                    license_id      SERIAL NOT NULL,
                    PRIMARY KEY (ipv4_address, license_id)
                )"#,
            )
            .await?;

        Ok(())
    }
}

pub mod date {
    use std::error::Error;

    use chrono::{DateTime, NaiveDateTime, Utc};
    use tokio_postgres::types::{accepts, FromSql, Type};

    // https://github.com/sfackler/rust-postgres/issues/816
    #[derive(Debug)]
    pub struct DateTimePlus(pub DateTime<Utc>);

    impl<'a> FromSql<'a> for DateTimePlus {
        fn from_sql(
            type_: &Type,
            raw: &[u8],
        ) -> Result<DateTimePlus, Box<dyn Error + Sync + Send>> {
            let naive = match raw {
                [128, 0, 0, 0, 0, 0, 0, 0] => chrono::naive::MIN_DATETIME,
                [127, 255, 255, 255, 255, 255, 255, 255] => chrono::naive::MAX_DATETIME,
                _ => NaiveDateTime::from_sql(type_, raw)?,
            };
            Ok(DateTimePlus(DateTime::from_utc(naive, Utc)))
        }

        accepts!(TIMESTAMPTZ);
    }
}

pub mod error {
    use actix_web::{http::StatusCode, HttpResponse, ResponseError};
    use serde::Serialize;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum ValidationError {
        #[error("IP address of the validator was not found")]
        IPAddressNotFound,
        #[error("IP address of the validator is not IPv4")]
        BadIPVersion,
        #[error("Key sent by the validator is not UUID")]
        MalformedKey,
        #[error("Cannot access to the database")]
        DatabaseError,
        #[error("This license key is invalid")]
        InvalidKey,
        #[error("This license key has expired")]
        ExpiredKey,
    }

    impl ValidationError {
        pub fn name(&self) -> String {
            match self {
                Self::IPAddressNotFound => "IPAddressNotFound",
                Self::BadIPVersion => "BadIPVersion",
                Self::MalformedKey => "MalformedKey",
                Self::DatabaseError => "DatabaseError",
                Self::InvalidKey => "InvalidKey",
                Self::ExpiredKey => "ExpiredKey",
            }
            .to_owned()
        }
    }

    impl ResponseError for ValidationError {
        fn status_code(&self) -> StatusCode {
            match *self {
                Self::IPAddressNotFound => StatusCode::BAD_REQUEST,
                Self::BadIPVersion => StatusCode::BAD_REQUEST,
                Self::MalformedKey => StatusCode::BAD_REQUEST,
                Self::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
                Self::InvalidKey => StatusCode::FORBIDDEN,
                Self::ExpiredKey => StatusCode::FORBIDDEN,
            }
        }

        fn error_response(&self) -> HttpResponse {
            let status_code = self.status_code();
            let error_response = ErrorResponse {
                code: status_code.as_u16(),
                message: self.to_string(),
                error: self.name(),
            };
            HttpResponse::build(status_code).json(error_response)
        }
    }

    #[derive(Serialize)]
    struct ErrorResponse {
        code: u16,
        error: String,
        message: String,
    }
}
