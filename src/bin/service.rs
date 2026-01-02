use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    beeper_auotmations::run_service().await
}
