use std::env;
use anyhow::{Result, anyhow};

#[derive(Clone)]
pub struct SniperConfig {
    pub rpc_url: String,
    pub slippage: u64, // in basis points (100 = 1%)
    pub max_buy_amount: u64, // in lamports
    pub bot_token: String,
    pub chat_id: String,
}

impl SniperConfig {
    pub fn load() -> Result<Self> {
        let rpc_url = env::var("RPC_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        
        let slippage = env::var("SLIPPAGE")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<u64>()
            .unwrap_or(100); // Default 1%
        
        let max_buy_amount = env::var("MAX_BUY_AMOUNT_SOL")
            .unwrap_or_else(|_| "0.1".to_string())
            .parse::<f64>()
            .unwrap_or(0.1) * 1_000_000_000.0; // Convert SOL to lamports
        
        let bot_token = env::var("BOT_TOKEN")
            .map_err(|_| anyhow!("BOT_TOKEN environment variable is required"))?;
        
        let chat_id = env::var("CHAT_ID")
            .map_err(|_| anyhow!("CHAT_ID environment variable is required"))?;
        
        Ok(Self {
            rpc_url,
            slippage,
            max_buy_amount,
            bot_token,
            chat_id,
        })
    }
    
    pub fn validate(&self) -> Result<()> {
        if self.slippage > 1000 {
            return Err(anyhow!("Slippage too high: {}% (max 10%)", self.slippage as f64 / 100.0));
        }
        
        if self.max_buy_amount > 10_000_000_000 {
            return Err(anyhow!("Max buy amount too high: {} SOL (max 10 SOL)", 
                self.max_buy_amount as f64 / 1_000_000_000.0));
        }
        
        Ok(())
    }
}
