use crate::pump_fun_monitor::{TokenMonitor, PumpFunToken};
use crate::config::SniperConfig;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize config
    let config = SniperConfig::load()?;
    let config_arc = Arc::new(config);
    
    // Create token monitor
    let monitor = TokenMonitor::new(config_arc);
    
    println!("Starting Pump.fun token monitor test...");
    
    // Start monitoring
    let monitor_clone = monitor.clone();
    tokio::spawn(async move {
        if let Err(e) = monitor_clone.start_monitoring().await {
            eprintln!("Monitoring failed: {}", e);
        }
    });
    
    // Wait for initial discovery
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    
    // Get stats
    let stats = monitor.get_stats().await;
    println!("Monitor stats: {:?}", stats);
    
    // Get strategy tokens
    let strategy_tokens = monitor.get_strategy_tokens().await?;
    println!("Found {} strategy tokens", strategy_tokens.len());
    
    for token in strategy_tokens.iter().take(5) {
        println!("Token: {} ({}) - Liquidity: {} SOL, Holders: {}", 
                token.symbol, token.address, token.liquidity_sol, token.holder_count);
    }
    
    Ok(())
}
