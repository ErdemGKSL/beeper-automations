use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    beeper_automations::run_service().await
}
