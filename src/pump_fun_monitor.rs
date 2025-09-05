use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, debug};
use reqwest::Client;
use chrono;
use crate::config::SniperConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PumpFunToken {
    pub address: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: u64,
    pub creator: String,
    pub created_at: u64,
    pub liquidity_sol: f64,
    pub holder_count: u32,
    pub market_cap_sol: f64,
    pub price_sol: f64,
    pub volume_24h: f64,
    pub price_change_24h: f64,
    pub is_verified: bool,
    pub is_mutable: bool,
}

#[derive(Debug, Clone)]
pub struct TokenMonitor {
    config: Arc<SniperConfig>,
    http_client: Client,
    known_tokens: Arc<RwLock<HashMap<String, PumpFunToken>>>,
    #[allow(dead_code)]
    last_check: Arc<RwLock<u64>>,
    #[allow(dead_code)]
    websocket_url: String,
    api_base_url: String,
}

impl TokenMonitor {
    pub fn new(config: Arc<SniperConfig>) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            config,
            http_client,
            known_tokens: Arc::new(RwLock::new(HashMap::new())),
            last_check: Arc::new(RwLock::new(0)),
            websocket_url: "wss://pump.fun/ws".to_string(),
            api_base_url: "https://pump.fun/api".to_string(),
        }
    }

    /// Start monitoring for new tokens
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting Pump.fun token monitoring...");
        
        // Initial token discovery
        self.discover_existing_tokens().await?;
        
        // Start continuous monitoring
        self.continuous_monitoring().await?;
        
        Ok(())
    }

    /// Discover existing tokens on Pump.fun
    async fn discover_existing_tokens(&self) -> Result<()> {
        info!("Discovering existing tokens...");
        
        let url = format!("{}/tokens", self.api_base_url);
        
        match self.http_client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    if let Ok(tokens) = response.json::<Vec<PumpFunToken>>().await {
                        let mut known = self.known_tokens.write().await;
                        for token in tokens {
                            known.insert(token.address.clone(), token);
                        }
                        info!("Discovered {} existing tokens", known.len());
                    }
                }
            }
            Err(e) => {
                warn!("Failed to discover existing tokens: {}", e);
            }
        }
        
        Ok(())
    }

    /// Continuous monitoring loop
    async fn continuous_monitoring(&self) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
            // Check for new tokens
            if let Err(e) = self.check_for_new_tokens().await {
                warn!("Error checking for new tokens: {}", e);
            }
            
            // Update existing token data
            if let Err(e) = self.update_token_data().await {
                warn!("Error updating token data: {}", e);
            }
            
            // Clean up old tokens
            self.cleanup_old_tokens().await;
        }
    }

    /// Check for new tokens
    async fn check_for_new_tokens(&self) -> Result<()> {
        let url = format!("{}/tokens/recent", self.api_base_url);
        
        let response = self.http_client
            .get(&url)
            .header("User-Agent", "MEV-Bot/1.0")
            .send()
            .await?;
        
        if response.status().is_success() {
            if let Ok(tokens) = response.json::<Vec<PumpFunToken>>().await {
                let mut known = self.known_tokens.write().await;
                let mut new_tokens = Vec::new();
                
                for token in tokens {
                    if !known.contains_key(&token.address) {
                        // Apply filtering criteria
                        if self.passes_token_filters(&token).await? {
                            known.insert(token.address.clone(), token.clone());
                            new_tokens.push(token);
                        }
                    }
                }
                
                if !new_tokens.is_empty() {
                    info!("Found {} new tokens", new_tokens.len());
                    for token in new_tokens {
                        debug!("New token: {} ({}) - Liquidity: {} SOL, Holders: {}", 
                               token.symbol, token.address, token.liquidity_sol, token.holder_count);
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Update data for existing tokens
    async fn update_token_data(&self) -> Result<()> {
        let known = self.known_tokens.read().await;
        let token_addresses: Vec<String> = known.keys().cloned().collect();
        drop(known);
        
        for address in token_addresses {
            if let Ok(updated_token) = self.fetch_token_data(&address).await {
                let mut known = self.known_tokens.write().await;
                if let Some(existing) = known.get_mut(&address) {
                    // Update only certain fields to preserve historical data
                    existing.liquidity_sol = updated_token.liquidity_sol;
                    existing.holder_count = updated_token.holder_count;
                    existing.market_cap_sol = updated_token.market_cap_sol;
                    existing.price_sol = updated_token.price_sol;
                    existing.volume_24h = updated_token.volume_24h;
                    existing.price_change_24h = updated_token.price_change_24h;
                }
            }
            
            // Rate limiting
            sleep(Duration::from_millis(100)).await;
        }
        
        Ok(())
    }

    /// Fetch updated data for a specific token
    async fn fetch_token_data(&self, address: &str) -> Result<PumpFunToken> {
        let url = format!("{}/token/{}", self.api_base_url, address);
        
        let response = self.http_client
            .get(&url)
            .header("User-Agent", "MEV-Bot/1.0")
            .send()
            .await?;
        
        if response.status().is_success() {
            let token = response.json::<PumpFunToken>().await?;
            Ok(token)
        } else {
            Err(anyhow!("Failed to fetch token data: {}", response.status()))
        }
    }

    /// Check if token passes all filtering criteria
    async fn passes_token_filters(&self, token: &PumpFunToken) -> Result<bool> {
        // Check liquidity range
        if token.liquidity_sol < self.config.min_liquidity_sol 
            || token.liquidity_sol > self.config.max_liquidity_sol {
            return Ok(false);
        }

        // Check holder count range
        if token.holder_count < self.config.min_holder_count 
            || token.holder_count > self.config.max_holder_count {
            return Ok(false);
        }

        // Check market cap range
        if token.market_cap_sol < self.config.min_market_cap_sol 
            || token.market_cap_sol > self.config.max_market_cap_sol {
            return Ok(false);
        }

        // Check token age
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let token_age_minutes = (current_time - token.created_at) / 60;
        
        if token_age_minutes < self.config.min_token_age_minutes 
            || token_age_minutes > self.config.max_token_age_hours * 60 {
            return Ok(false);
        }

        // Check blacklisted creators
        if self.config.blacklisted_creators.contains(&token.creator) {
            return Ok(false);
        }

        // Check whitelisted creators (if any specified)
        if !self.config.whitelisted_creators.is_empty() 
            && !self.config.whitelisted_creators.contains(&token.creator) {
            return Ok(false);
        }

        Ok(true)
    }

    /// Get tokens that match current strategy criteria
    pub async fn get_strategy_tokens(&self) -> Result<Vec<PumpFunToken>> {
        let known = self.known_tokens.read().await;
        let mut strategy_tokens = Vec::new();
        
        for token in known.values() {
            if self.matches_buy_strategy(token).await? {
                strategy_tokens.push(token.clone());
            }
        }
        
        // Sort by potential (you can customize this)
        strategy_tokens.sort_by(|a, b| {
            b.liquidity_sol.partial_cmp(&a.liquidity_sol).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        Ok(strategy_tokens)
    }

    /// Check if token matches current buy strategy
    async fn matches_buy_strategy(&self, token: &PumpFunToken) -> Result<bool> {
        match &self.config.buy_strategy {
            crate::config::BuyStrategy::Immediate => Ok(true),
            crate::config::BuyStrategy::VolumeSpike => {
                Ok(token.volume_24h > self.config.volume_multiplier * 1000.0) // Example threshold
            }
            crate::config::BuyStrategy::PriceBreakout => {
                Ok(token.price_change_24h > 20.0) 
            }
            crate::config::BuyStrategy::LiquidityAddition => {
                Ok(token.liquidity_sol > self.config.min_liquidity_sol * 1.5)
            }
            crate::config::BuyStrategy::HolderGrowth => {
                Ok(token.holder_count > self.config.min_holder_count * 2)
            }
            crate::config::BuyStrategy::Custom(_) => Ok(true),
        }
    }

    async fn cleanup_old_tokens(&self) {
        let mut known = self.known_tokens.write().await;
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut to_remove = Vec::new();
        
        for (address, token) in known.iter() {
            let token_age_hours = (current_time - token.created_at) / 3600;
            
            // Remove tokens older than max age or with very low liquidity
            if token_age_hours > self.config.max_token_age_hours 
                || token.liquidity_sol < 0.1 {
                to_remove.push(address.clone());
            }
        }
        
        for address in &to_remove {
            known.remove(address.as_str());
        }
        
        if !to_remove.is_empty() {
            debug!("Cleaned up {} old tokens", to_remove.len());
        }
    }

    /// Get monitoring statistics
    pub async fn get_stats(&self) -> MonitorStats {
        let known = self.known_tokens.read().await;
        let total_tokens = known.len();
        let total_liquidity: f64 = known.values().map(|t| t.liquidity_sol).sum();
        let avg_holders: f64 = known.values().map(|t| t.holder_count as f64).sum::<f64>() / total_tokens.max(1) as f64;
        
        MonitorStats {
            total_tokens,
            total_liquidity,
            avg_holders,
            last_update: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonitorStats {
    pub total_tokens: usize,
    pub total_liquidity: f64,
    pub avg_holders: f64,
    pub last_update: u64,
}

impl MonitorStats {
    pub fn to_string(&self) -> String {
        format!(
            "📊 **Monitor Stats**\n\
            • Total Tokens: {}\n\
            • Total Liquidity: {:.2} SOL\n\
            • Avg Holders: {:.0}\n\
            • Last Update: {}",
            self.total_tokens,
            self.total_liquidity,
            self.avg_holders,
            chrono::DateTime::from_timestamp(self.last_update as i64, 0)
                .unwrap()
                .format("%H:%M:%S")
        )
    }
}
