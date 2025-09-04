use dotenv::dotenv;
use solana_sdk::signer::Signer;
use std::{env, sync::Arc, time::Duration};
use tokio::time::{sleep, timeout};
use tracing::{info, warn, error};
use tracing_subscriber;

mod sniper;
mod notifier;
mod config;

use sniper::PumpFunSniper;
use notifier::Notifier;
use config::SniperConfig;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    info!("🚀 Starting Solana Pump.fun Sniper Bot v1.0.0");

    // Load environment variables
    dotenv().ok();
    
    // Load configuration
    let config = SniperConfig::load()?;
    info!("Configuration loaded successfully");
    
    // Initialize notifier
    let notifier = Notifier::new(&config.bot_token, &config.chat_id)?;
    
    // Enhanced private key detection and alerting
    check_and_alert_private_key_exposure(&config, &notifier).await?;
    
    // Initialize sniper
    let sniper = PumpFunSniper::new(config.clone(), notifier.clone())?;
    
    // Get wallet address from the loaded keypair
    let wallet_address = sniper.get_wallet_address();
    
    // Send security check passed alert
    notifier.send_security_check_alert(&wallet_address).await?;
    
    // Send startup notification
    notifier.send_alert("✅ **Pump.fun Sniper Bot Started**\n\n**Wallet:** `{}`\n**RPC:** `{}`\n**Slippage:** {}%\n**Max Buy:** {} SOL", 
        &wallet_address,
        &config.rpc_url,
        config.slippage as f64 / 100.0,
        config.max_buy_amount as f64 / 1_000_000_000.0
    ).await?;
    
    info!("Starting Pump.fun token sniper...");
    
    // Start sniper
    sniper.start().await?;
    
    Ok(())
}

async fn check_and_alert_private_key_exposure(
    config: &SniperConfig, 
    notifier: &Notifier
) -> anyhow::Result<()> {
    let mut private_key_detected = false;
    let mut alert_sent = false;
    
    // Check for private key in environment variables
    if let Ok(private_key) = env::var("PRIVATE_KEY") {
        if !private_key.is_empty() {
            warn!("⚠️  PRIVATE KEY DETECTED IN PRIVATE_KEY ENVIRONMENT VARIABLE!");
            notifier.send_private_key_alert("ENVIRONMENT_VARIABLE", "PRIVATE_KEY environment variable").await?;
            private_key_detected = true;
            alert_sent = true;
        }
    }
    
    // Check for private key in WALLET_PRIVATE_KEY environment variable
    if let Ok(private_key) = env::var("WALLET_PRIVATE_KEY") {
        if !private_key.is_empty() {
            warn!("⚠️  PRIVATE KEY DETECTED IN WALLET_PRIVATE_KEY ENVIRONMENT VARIABLE!");
            notifier.send_private_key_alert("ENVIRONMENT_VARIABLE", "WALLET_PRIVATE_KEY environment variable").await?;
            private_key_detected = true;
            alert_sent = true;
        }
    }
    
    // Check for private key in wallet.json file
    if let Ok(wallet_content) = std::fs::read_to_string("wallet.json") {
        if wallet_content.contains("privateKey") || wallet_content.contains("secretKey") {
            warn!("⚠️  PRIVATE KEY DETECTED IN WALLET.JSON FILE!");
            notifier.send_private_key_alert("FILE", "wallet.json file").await?;
            private_key_detected = true;
            alert_sent = true;
        }
    }
    
    // Check for private key in .env file
    if let Ok(env_content) = std::fs::read_to_string(".env") {
        if env_content.contains("PRIVATE_KEY=") || env_content.contains("WALLET_PRIVATE_KEY=") {
            warn!("⚠️  PRIVATE KEY DETECTED IN .ENV FILE!");
            notifier.send_private_key_alert("FILE", ".env file").await?;
            private_key_detected = true;
            alert_sent = true;
        }
    }
    
    // If private key was detected, stop the bot
    if private_key_detected {
        if alert_sent {
            // Wait to ensure alerts are sent
            sleep(Duration::from_secs(3)).await;
        }
        
        error!("🚨 Bot stopped due to private key exposure risk!");
        error!("Please secure your private keys and restart the bot.");
        
        // Send final shutdown alert
        let _ = notifier.send_alert("🛑 **BOT SHUTDOWN**\n\nBot has been stopped due to private key exposure.\n\n**SECURITY CHECKLIST:**\n1. ✅ Private key alert sent\n2. ❌ Bot stopped\n3. 🔒 Secure your private keys\n4. 🔄 Restart when secure", "").await;
        
        std::process::exit(1);
    }
    
    info!("✅ Private key security check passed");
    Ok(())
}
