use vark_api::{server::router, Engine};
use vark_config::VarkConfig;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = match std::env::var("VARK_CONFIG") {
        Ok(path) => VarkConfig::load(path)?,
        Err(_) => VarkConfig::default(),
    };
    let engine = Engine::new(config)?;
    let app = router(engine);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    println!("VARK HTTP listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await?;
    Ok(())
}
