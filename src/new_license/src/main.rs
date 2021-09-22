use std::{error::Error, io, process};

use io::{stdin, stdout, Write};
use licenseptium::{config::Config, database::create_tables, print_logo};
use tokio_postgres::NoTls;
use uuid::Uuid;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    print_logo();

    let input = read_input().unwrap_or_else(|e| {
        println!("{}", e);
        process::exit(0);
    });

    let key = Uuid::new_v4();

    // Read environment variables and save to config for postgres
    let cfg = Config::from_env().unwrap();
    let pool = cfg.pg.create_pool(NoTls).unwrap();
    let client = pool.get().await.unwrap();

    // Create default tables if not exists
    create_tables(&client).await.unwrap();

    if input.expiration_date == -1f64 {
        client
            .execute(
                r#"
    INSERT INTO licenses(
        key, comment, ip_limit, checksum) 
    VALUES ($1, $2, $3, SHA512(CONVERT_TO($4, 'UTF-8')))"#,
                &[&key, &input.comment, &input.ip_limit, &input.checksum],
            )
            .await
            .unwrap();
    } else {
        client
            .execute(
                r#"
    INSERT INTO licenses(
        key, comment, ip_limit, checksum, expires_at) 
    VALUES ($1, $2, $3, SHA512(CONVERT_TO($4, 'UTF-8')), TO_TIMESTAMP($5))"#,
                &[
                    &key,
                    &input.comment,
                    &input.ip_limit,
                    &input.checksum,
                    &input.expiration_date,
                ],
            )
            .await
            .unwrap();
    }

    println!("New license successfully created!");
    println!("License key: {}", &key);
}

struct Input {
    comment: String,
    ip_limit: i32,
    expiration_date: f64,
    checksum: String,
}

fn read_input() -> Result<Input, Box<dyn Error>> {
    fn flush() -> Result<(), Box<dyn Error>> {
        stdout().flush()?;
        Ok(())
    }

    fn remove_crlf(buf: &mut String) {
        *buf = buf.replace("\r\n", "");
    }

    println!("License comments are designed to describe license owner and has no effect.");
    print!("What do you want to comment? ");
    flush()?;
    let mut comment = String::new();
    stdin()
        .read_line(&mut comment)
        .map_err(|_| "Cannot not read comment.")?;
    remove_crlf(&mut comment);
    if comment.is_empty() {
        Err("Comment cannot be empty.")?;
    }

    print!("How many different IP addresses will activate with? (1) ");
    flush()?;
    let mut ip_limit = String::new();
    stdin()
        .read_line(&mut ip_limit)
        .map_err(|_| "Cannot read ip limit.")?;
    remove_crlf(&mut ip_limit);
    let ip_limit = if ip_limit.is_empty() {
        1
    } else {
        ip_limit.parse().map_err(|_| "IP limit must be a number.")?
    };

    println!("Format for expiration date is Unix time.");
    print!("When will the key expire? (infinity) ");
    flush()?;
    let mut expiration_date = String::new();
    stdin()
        .read_line(&mut expiration_date)
        .map_err(|_| "Cannot read expiration date.")?;
    remove_crlf(&mut expiration_date);
    let expiration_date =
        if expiration_date.is_empty() || expiration_date.to_lowercase() == "infinity" {
            -1f64
        } else {
            expiration_date.parse::<f64>().map_err(|_| {
                "Expiration date format is not Unix time (64 bit integer) or 'infinity'."
            })?
        };

    println!("Checksums are used for validation and validator will send it with the key. Checksums are case sensitive.");
    print!("What will be the checksum? ");
    flush()?;
    let mut checksum = String::new();
    stdin()
        .read_line(&mut checksum)
        .map_err(|_| "Cannot read checksum.")?;
    remove_crlf(&mut checksum);
    if checksum.is_empty() {
        Err("Checksum cannot be empty.")?;
    }

    Ok(Input {
        comment,
        ip_limit,
        expiration_date,
        checksum,
    })
}
