use std::{
    error::Error,
    io::{stdin, stdout, Write},
    process,
};

use licenseptium::{config::Config, database::create_tables, print_logo};
use tokio_postgres::NoTls;
use uuid::Uuid;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    print_logo();

    let key = read_key();
    if let Err(e) = key {
        println!("{}", e);
        process::exit(0);
    }
    let key = key.unwrap();

    // Read environment variables and save to config for postgres
    let cfg = Config::from_env().unwrap();
    let pool = cfg.pg.create_pool(NoTls).unwrap();
    let client = pool.get().await.unwrap();

    // Create default tables if not exists
    create_tables(&client).await.unwrap();

    client
        .execute(r#"DELETE FROM licenses WHERE key=$1"#, &[&key])
        .await
        .unwrap();

    println!("License deleted successfully!");
}

fn read_key() -> Result<Uuid, Box<dyn Error>> {
    fn flush() -> Result<(), Box<dyn Error>> {
        stdout().flush()?;
        Ok(())
    }

    fn remove_crlf(buf: &mut String) {
        *buf = buf.replace("\r\n", "");
    }

    println!("License keys are formatted as UUID, you can use show_licenses to see the keys.");
    print!("What is the key of the license you want to delete? ");
    flush()?;
    let mut key = String::new();
    stdin()
        .read_line(&mut key)
        .map_err(|_| "Cannot not read comment.")?;
    remove_crlf(&mut key);
    if key.is_empty() {
        Err("Key cannot be empty.")?;
    }
    let key = Uuid::parse_str(&key).map_err(|_| "Key format must be UUID.")?;

    println!("Deleting a license is irreversible!");
    print!("Are you sure? (n) ");
    flush()?;
    let mut confirm = String::new();
    stdin()
        .read_line(&mut confirm)
        .map_err(|_| "Cannot read confirmation.")?;
    remove_crlf(&mut confirm);
    if confirm.to_lowercase() != "y" && !confirm.to_lowercase().contains("yes") {
        Err("Terminating delete process.")?;
    }

    Ok(key)
}
