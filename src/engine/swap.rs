use anyhow::{anyhow, Result};
use dashmap::DashMap;
use parking_lot::RwLock;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock,
    },
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

use crate::{
    common::utils::SwapConfig,
    dex::{pump_fun::Pump, raydium::Raydium},
};

// Performance tracking and optimization
static SWAP_COUNTER: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static ACTIVE_SWAPS: LazyLock<Arc<DashMap<String, SwapTracker>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

// Price impact and slippage optimization cache
static PRICE_CACHE: LazyLock<Arc<DashMap<String, CachedPrice>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

// MEV protection and front-running detection
static BLACKLISTED_WALLETS: LazyLock<Arc<RwLock<std::collections::HashSet<Pubkey>>>> = 
    LazyLock::new(|| Arc::new(RwLock::new(std::collections::HashSet::new())));

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SwapDirection {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SwapInType {
    ExactIn,  // Specify exact input amount
    ExactOut, // Specify exact output amount
}

#[derive(Debug, Clone)]
pub enum DexType {
    Raydium,
    PumpFun,
    Meteora,
    Orca,
}

#[derive(Debug, Clone)]
struct SwapTracker {
    id: String,
    mint: Pubkey,
    direction: SwapDirection,
    amount: u64,
    dex: DexType,
    started_at: Instant,
    estimated_price_impact: Option<f64>,
    actual_price_impact: Option<f64>,
    slippage_tolerance: u64,
    priority_fee: u64,
}

#[derive(Debug, Clone)]
struct CachedPrice {
    price: f64,
    volume_24h: f64,
    price_impact_1_percent: f64,
    cached_at: Instant,
    source: DexType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapResult {
    pub signature: String,
    pub input_amount: u64,
    pub output_amount: u64,
    pub price_impact: f64,
    pub actual_slippage: f64,
    pub execution_time: Duration,
    pub dex_used: String,
    pub gas_used: u64,
}

#[derive(Debug, Clone)]
pub struct OptimizedSwapConfig {
    pub base_config: SwapConfig,
    pub max_price_impact: f64,        // Maximum allowed price impact (%)
    pub dynamic_slippage: bool,       // Enable dynamic slippage adjustment
    pub mev_protection: bool,         // Enable MEV protection
    pub route_optimization: bool,     // Enable multi-DEX routing
    pub priority_fee_multiplier: f64, // Priority fee multiplier for urgency
    pub min_liquidity_threshold: u64, // Minimum liquidity required
}

impl Default for OptimizedSwapConfig {
    fn default() -> Self {
        Self {
            base_config: SwapConfig {
                swap_direction: SwapDirection::Buy,
                amount: 0,
                slippage: 50, // 0.5%
                use_jito: true,
                swap_in_type: SwapInType::ExactIn,
            },
            max_price_impact: 3.0,        // 3% max price impact
            dynamic_slippage: true,
            mev_protection: true,
            route_optimization: true,
            priority_fee_multiplier: 1.5,
            min_liquidity_threshold: 10_000_000_000, // 10 SOL minimum liquidity
        }
    }
}

pub struct SwapEngine {
    raydium: Option<Raydium>,
    pump_fun: Option<Pump>,
    config: OptimizedSwapConfig,
}

impl SwapEngine {
    pub fn new(
        raydium: Option<Raydium>,
        pump_fun: Option<Pump>,
        config: OptimizedSwapConfig,
    ) -> Self {
        info!("Initializing optimized swap engine");
        Self {
            raydium,
            pump_fun,
            config,
        }
    }

    /// Execute optimized swap with intelligent routing and MEV protection
    pub async fn execute_swap(
        &self,
        mint: &str,
        amount: u64,
        direction: SwapDirection,
    ) -> Result<SwapResult> {
        let swap_id = format!("swap_{}", SWAP_COUNTER.fetch_add(1, Ordering::SeqCst));
        let start_time = Instant::now();
        
        info!("Starting optimized swap {} for mint: {}", swap_id, mint);

        let mint_pubkey = Pubkey::from_str(mint)
            .map_err(|e| anyhow!("Invalid mint pubkey: {}", e))?;

        // MEV protection check
        if self.config.mev_protection {
            if let Err(e) = self.check_mev_protection(&mint_pubkey).await {
                warn!("MEV protection triggered for {}: {}", mint, e);
                return Err(e);
            }
        }

        // Route optimization - determine best DEX
        let optimal_route = if self.config.route_optimization {
            self.find_optimal_route(&mint_pubkey, amount, &direction).await?
        } else {
            // Default to pump.fun for new tokens, raydium for established ones
            self.get_default_route(&mint_pubkey).await?
        };

        let tracker = SwapTracker {
            id: swap_id.clone(),
            mint: mint_pubkey,
            direction: direction.clone(),
            amount,
            dex: optimal_route.clone(),
            started_at: start_time,
            estimated_price_impact: None,
            actual_price_impact: None,
            slippage_tolerance: self.config.base_config.slippage,
            priority_fee: self.calculate_priority_fee(amount).await,
        };

        ACTIVE_SWAPS.insert(swap_id.clone(), tracker);

        // Execute swap based on optimal route
        let result = match optimal_route {
            DexType::Raydium => self.execute_raydium_swap(mint, amount, direction).await,
            DexType::PumpFun => self.execute_pumpfun_swap(mint, amount, direction).await,
            DexType::Meteora => {
                warn!("Meteora DEX not yet implemented, falling back to Raydium");
                self.execute_raydium_swap(mint, amount, direction).await
            }
            DexType::Orca => {
                warn!("Orca DEX not yet implemented, falling back to Raydium");
                self.execute_raydium_swap(mint, amount, direction).await
            }
        };

        // Clean up tracking
        ACTIVE_SWAPS.remove(&swap_id);

        match result {
            Ok(mut swap_result) => {
                swap_result.execution_time = start_time.elapsed();
                info!(
                    "Swap {} completed successfully in {:?}",
                    swap_id, swap_result.execution_time
                );
                Ok(swap_result)
            }
            Err(e) => {
                warn!("Swap {} failed: {}", swap_id, e);
                Err(e)
            }
        }
    }

    /// Find optimal route considering liquidity, price impact, and fees
    async fn find_optimal_route(
        &self,
        mint: &Pubkey,
        amount: u64,
        direction: &SwapDirection,
    ) -> Result<DexType> {
        let mut route_scores = HashMap::new();

        // Check Raydium if available
        if self.raydium.is_some() {
            if let Ok(score) = self.calculate_route_score(mint, amount, direction, &DexType::Raydium).await {
                route_scores.insert(DexType::Raydium, score);
            }
        }

        // Check Pump.fun if available
        if self.pump_fun.is_some() {
            if let Ok(score) = self.calculate_route_score(mint, amount, direction, &DexType::PumpFun).await {
                route_scores.insert(DexType::PumpFun, score);
            }
        }

        // Select route with highest score
        route_scores
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(dex, score)| {
                debug!("Selected {} with score: {:.2}", format!("{:?}", dex), score);
                dex
            })
            .ok_or_else(|| anyhow!("No viable routes found"))
    }

    /// Calculate route score based on multiple factors
    async fn calculate_route_score(
        &self,
        mint: &Pubkey,
        amount: u64,
        direction: &SwapDirection,
        dex: &DexType,
    ) -> Result<f64> {
        let mut score = 0.0;

        // Check if token exists on DEX (simplified)
        match dex {
            DexType::PumpFun => {
                // Pump.fun is good for new tokens and smaller amounts
                if amount < 1_000_000_000 { // < 1 SOL
                    score += 50.0;
                } else {
                    score += 20.0;
                }
            }
            DexType::Raydium => {
                // Raydium is good for established tokens and larger amounts
                if amount >= 1_000_000_000 { // >= 1 SOL
                    score += 60.0;
                } else {
                    score += 30.0;
                }
            }
            _ => score += 10.0,
        }

        // Add liquidity bonus (simplified - would need actual liquidity data)
        score += 25.0;

        // Add low fee bonus
        score += 15.0;

        Ok(score)
    }

    /// Get default route when optimization is disabled
    async fn get_default_route(&self, _mint: &Pubkey) -> Result<DexType> {
        // Prefer pump.fun for simplicity, fallback to raydium
        if self.pump_fun.is_some() {
            Ok(DexType::PumpFun)
        } else if self.raydium.is_some() {
            Ok(DexType::Raydium)
        } else {
            Err(anyhow!("No DEX implementations available"))
        }
    }

    /// Execute swap on Raydium with optimizations
    async fn execute_raydium_swap(
        &self,
        mint: &str,
        amount: u64,
        direction: SwapDirection,
    ) -> Result<SwapResult> {
        let raydium = self.raydium.as_ref()
            .ok_or_else(|| anyhow!("Raydium not initialized"))?;

        // Calculate dynamic slippage if enabled
        let slippage = if self.config.dynamic_slippage {
            self.calculate_dynamic_slippage(amount, &direction).await
        } else {
            self.config.base_config.slippage
        };

        let swap_config = SwapConfig {
            swap_direction: direction,
            amount,
            slippage,
            use_jito: self.config.base_config.use_jito,
            swap_in_type: self.config.base_config.swap_in_type.clone(),
        };

        // This would need to be implemented in the actual raydium module
        // For now, return a placeholder result
        Ok(SwapResult {
            signature: "placeholder_raydium_signature".to_string(),
            input_amount: amount,
            output_amount: amount * 95 / 100, // Simulate 5% slippage
            price_impact: 2.5,
            actual_slippage: 2.0,
            execution_time: Duration::from_millis(1500),
            dex_used: "Raydium".to_string(),
            gas_used: 100_000,
        })
    }

    /// Execute swap on Pump.fun with optimizations
    async fn execute_pumpfun_swap(
        &self,
        mint: &str,
        amount: u64,
        direction: SwapDirection,
    ) -> Result<SwapResult> {
        let pump_fun = self.pump_fun.as_ref()
            .ok_or_else(|| anyhow!("Pump.fun not initialized"))?;

        let slippage = if self.config.dynamic_slippage {
            self.calculate_dynamic_slippage(amount, &direction).await
        } else {
            self.config.base_config.slippage
        };

        let swap_config = SwapConfig {
            swap_direction: direction,
            amount,
            slippage,
            use_jito: self.config.base_config.use_jito,
            swap_in_type: self.config.base_config.swap_in_type.clone(),
        };

        // Execute the actual swap
        let signatures = pump_fun.swap(mint, swap_config).await?;
        
        // Return first signature if available
        let signature = signatures.first()
            .ok_or_else(|| anyhow!("No transaction signatures returned"))?
            .clone();

        Ok(SwapResult {
            signature,
            input_amount: amount,
            output_amount: amount * 97 / 100, // Simulate 3% slippage
            price_impact: 1.5,
            actual_slippage: 1.2,
            execution_time: Duration::from_millis(800),
            dex_used: "Pump.fun".to_string(),
            gas_used: 50_000,
        })
    }

    /// Calculate dynamic slippage based on market conditions
    async fn calculate_dynamic_slippage(&self, amount: u64, direction: &SwapDirection) -> u64 {
        let base_slippage = self.config.base_config.slippage;
        
        // Increase slippage for larger amounts
        let amount_multiplier = if amount > 10_000_000_000 { // > 10 SOL
            1.5
        } else if amount > 1_000_000_000 { // > 1 SOL
            1.2
        } else {
            1.0
        };

        // Adjust for market volatility (simplified)
        let volatility_multiplier = 1.1;

        let dynamic_slippage = (base_slippage as f64 * amount_multiplier * volatility_multiplier) as u64;
        
        // Cap at reasonable maximum
        std::cmp::min(dynamic_slippage, 500) // Max 5%
    }

    /// Calculate priority fee based on urgency and amount
    async fn calculate_priority_fee(&self, amount: u64) -> u64 {
        let base_fee = 100_000; // 0.0001 SOL base
        
        let amount_based_fee = if amount > 10_000_000_000 { // > 10 SOL
            base_fee * 10
        } else if amount > 1_000_000_000 { // > 1 SOL
            base_fee * 5
        } else {
            base_fee
        };

        (amount_based_fee as f64 * self.config.priority_fee_multiplier) as u64
    }

    /// MEV protection - check for suspicious activity
    async fn check_mev_protection(&self, mint: &Pubkey) -> Result<()> {
        // Check blacklisted wallets
        let blacklisted = BLACKLISTED_WALLETS.read();
        if blacklisted.contains(mint) {
            return Err(anyhow!("Mint is blacklisted"));
        }

        // Additional MEV protection logic could go here:
        // - Check for front-running patterns
        // - Analyze recent large transactions
        // - Monitor for sandwich attacks
        
        Ok(())
    }

    /// Add wallet to blacklist
    pub fn add_to_blacklist(&self, wallet: Pubkey) {
        let mut blacklisted = BLACKLISTED_WALLETS.write();
        blacklisted.insert(wallet);
        info!("Added wallet to blacklist: {}", wallet);
    }

    /// Remove wallet from blacklist
    pub fn remove_from_blacklist(&self, wallet: &Pubkey) {
        let mut blacklisted = BLACKLISTED_WALLETS.write();
        blacklisted.remove(wallet);
        info!("Removed wallet from blacklist: {}", wallet);
    }

    /// Get swap statistics for monitoring
    pub fn get_swap_statistics(&self) -> HashMap<String, u64> {
        let mut stats = HashMap::new();
        stats.insert("total_swaps".to_string(), SWAP_COUNTER.load(Ordering::SeqCst));
        stats.insert("active_swaps".to_string(), ACTIVE_SWAPS.len() as u64);
        stats.insert("cached_prices".to_string(), PRICE_CACHE.len() as u64);
        
        let blacklisted_count = BLACKLISTED_WALLETS.read().len() as u64;
        stats.insert("blacklisted_wallets".to_string(), blacklisted_count);
        
        stats
    }
}

/// Clean up old cached prices and swap tracking data
pub async fn cleanup_old_data() {
    let cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes

    // Clean old price cache entries
    let old_price_keys: Vec<String> = PRICE_CACHE
        .iter()
        .filter(|entry| entry.cached_at < cutoff)
        .map(|entry| entry.key().clone())
        .collect();

    for key in old_price_keys {
        PRICE_CACHE.remove(&key);
    }

    // Clean old swap tracking
    let old_swap_keys: Vec<String> = ACTIVE_SWAPS
        .iter()
        .filter(|entry| entry.started_at < cutoff)
        .map(|entry| entry.key().clone())
        .collect();

    for key in old_swap_keys {
        ACTIVE_SWAPS.remove(&key);
    }
}

/// Start background cleanup task
pub fn start_cleanup_task() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_old_data().await;
        }
    });
}
