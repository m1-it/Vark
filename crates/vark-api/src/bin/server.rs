use std::{env, io};
use vark_api::{server::router, ApiAuth, Engine};
use vark_config::VarkConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = match env::var("VARK_CONFIG") {
        Ok(path) => VarkConfig::load(path)?,
        Err(_) => VarkConfig::default(),
    };
    let auth = ApiAuth::from_env(&config.user_id)?;
    let storage_key = env::var("VARK_STORAGE_KEY").map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "VARK_STORAGE_KEY wajib disetel untuk menjalankan server API",
        )
    })?;
    if storage_key.len() < 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "VARK_STORAGE_KEY harus memiliki setidaknya 32 byte",
        )
        .into());
    }
    let bind_address = env::var("VARK_API_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let engine = Engine::new(config)?;
    let app = router(engine, auth);
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    println!("VARK HTTP listening on http://{bind_address}");
    axum::serve(listener, app).await?;
    Ok(())
}
