use localmux_gateway::{serve, GatewayState};
use sha2::{Digest, Sha256};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key = std::env::var("LOCALMUX_TEST_API_KEY")?;
    if !key.starts_with("lmx_test_") {
        anyhow::bail!("Only synthetic test keys are accepted");
    }
    let port: u16 = std::env::var("LOCALMUX_TEST_PORT")?.parse()?;
    let pool = localmux_storage::open_memory().await?;
    let hash = format!("{:x}", Sha256::digest(key.as_bytes()));
    localmux_storage::insert_api_key(
        &pool,
        "compat",
        &hash,
        &[
            "models:read".into(),
            "chat:invoke".into(),
            "responses:invoke".into(),
        ],
    )
    .await?;
    serve(GatewayState::mock(pool), port).await?;
    Ok(())
}
