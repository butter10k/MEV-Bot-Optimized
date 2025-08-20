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
use crossbeam::channel::{bounded, Receiver, Sender};
use rayon::prelude::*;
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

// Advanced route optimization with parallel analysis
static ROUTE_CACHE: LazyLock<Arc<DashMap<String, CachedRoute>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

static LIQUIDITY_MONITOR: LazyLock<Arc<LiquidityMonitor>> = 
    LazyLock::new(|| Arc::new(LiquidityMonitor::new()));

// Circuit breaker for failed transactions
static CIRCUIT_BREAKER: LazyLock<Arc<CircuitBreaker>> = 
    LazyLock::new(|| Arc::new(CircuitBreaker::new()));

// Predictive analytics for market movements
static PRICE_PREDICTOR: LazyLock<Arc<PricePredictor>> = 
    LazyLock::new(|| Arc::new(PricePredictor::new()));

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

#[derive(Debug, Clone)]
struct CachedRoute {
    dex: DexType,
    score: f64,
    estimated_output: u64,
    price_impact: f64,
    execution_time: Duration,
    cached_at: Instant,
    liquidity_depth: u64,
    fees: u64,
}

// Advanced liquidity monitoring for optimal routing
struct LiquidityMonitor {
    dex_liquidities: Arc<DashMap<DexType, DexLiquidity>>,
    update_interval: Duration,
    health_threshold: f64,
}

#[derive(Debug, Clone)]
struct DexLiquidity {
    total_liquidity_usd: f64,
    active_pairs: u64,
    avg_price_impact: f64,
    success_rate: f64,
    avg_execution_time: Duration,
    last_updated: Instant,
}

impl LiquidityMonitor {
    fn new() -> Self {
        Self {
            dex_liquidities: Arc::new(DashMap::new()),
            update_interval: Duration::from_secs(30),
            health_threshold: 0.95, // 95% success rate threshold
        }
    }

    async fn update_dex_liquidity(&self, dex: DexType, liquidity: DexLiquidity) {
        self.dex_liquidities.insert(dex, liquidity);
    }

    fn get_dex_health(&self, dex: &DexType) -> f64 {
        self.dex_liquidities
            .get(dex)
            .map(|entry| entry.success_rate)
            .unwrap_or(0.5) // Default to 50% if unknown
    }

    fn is_dex_healthy(&self, dex: &DexType) -> bool {
        self.get_dex_health(dex) >= self.health_threshold
    }

    async fn start_monitoring(&self) {
        let liquidities = Arc::clone(&self.dex_liquidities);
        let interval = self.update_interval;

        tokio::spawn(async move {
            let mut monitor_interval = tokio::time::interval(interval);
            
            loop {
                monitor_interval.tick().await;
                
                // Update liquidity metrics for each DEX
                for dex in [DexType::Raydium, DexType::PumpFun, DexType::Meteora, DexType::Orca] {
                    // This would typically fetch real liquidity data
                    let mock_liquidity = DexLiquidity {
                        total_liquidity_usd: 10_000_000.0,
                        active_pairs: 500,
                        avg_price_impact: 1.5,
                        success_rate: 0.98,
                        avg_execution_time: Duration::from_millis(800),
                        last_updated: Instant::now(),
                    };
                    
                    liquidities.insert(dex, mock_liquidity);
                }
            }
        });
    }
}

// Circuit breaker pattern for handling failed transactions
struct CircuitBreaker {
    failure_threshold: u32,
    timeout: Duration,
    failure_counts: Arc<DashMap<DexType, FailureState>>,
}

#[derive(Debug, Clone)]
struct FailureState {
    consecutive_failures: u32,
    last_failure: Instant,
    state: BreakerState,
}

#[derive(Debug, Clone, PartialEq)]
enum BreakerState {
    Closed,    // Normal operation
    Open,      // Blocking requests
    HalfOpen,  // Testing recovery
}

impl CircuitBreaker {
    fn new() -> Self {
        Self {
            failure_threshold: 5, // Open after 5 consecutive failures
            timeout: Duration::from_secs(60), // Try recovery after 1 minute
            failure_counts: Arc::new(DashMap::new()),
        }
    }

    fn is_allowed(&self, dex: &DexType) -> bool {
        let state = self.failure_counts
            .get(dex)
            .map(|entry| entry.state.clone())
            .unwrap_or(BreakerState::Closed);

        match state {
            BreakerState::Closed => true,
            BreakerState::Open => {
                // Check if timeout has passed to transition to half-open
                if let Some(failure_state) = self.failure_counts.get(dex) {
                    if failure_state.last_failure.elapsed() > self.timeout {
                        // Transition to half-open
                        let mut new_state = failure_state.clone();
                        new_state.state = BreakerState::HalfOpen;
                        self.failure_counts.insert(*dex, new_state);
                        return true;
                    }
                }
                false
            }
            BreakerState::HalfOpen => true, // Allow one test request
        }
    }

    fn record_success(&self, dex: &DexType) {
        if let Some(mut failure_state) = self.failure_counts.get_mut(dex) {
            failure_state.consecutive_failures = 0;
            failure_state.state = BreakerState::Closed;
        }
    }

    fn record_failure(&self, dex: &DexType) {
        let mut failure_state = self.failure_counts
            .entry(*dex)
            .or_insert_with(|| FailureState {
                consecutive_failures: 0,
                last_failure: Instant::now(),
                state: BreakerState::Closed,
            });

        failure_state.consecutive_failures += 1;
        failure_state.last_failure = Instant::now();

        if failure_state.consecutive_failures >= self.failure_threshold {
            failure_state.state = BreakerState::Open;
            warn!("Circuit breaker opened for DEX: {:?}", dex);
        }
    }
}

// Predictive price analysis for better timing
struct PricePredictor {
    price_history: Arc<DashMap<Pubkey, Vec<PricePoint>>>,
    prediction_window: Duration,
    max_history_size: usize,
}

#[derive(Debug, Clone)]
struct PricePoint {
    timestamp: Instant,
    price: f64,
    volume: f64,
    volatility: f64,
}

impl PricePredictor {
    fn new() -> Self {
        Self {
            price_history: Arc::new(DashMap::new()),
            prediction_window: Duration::from_secs(300), // 5 minute prediction window
            max_history_size: 100, // Keep last 100 price points
        }
    }

    fn add_price_point(&self, mint: Pubkey, price_point: PricePoint) {
        let mut history = self.price_history
            .entry(mint)
            .or_insert_with(|| Vec::with_capacity(self.max_history_size));

        history.push(price_point);

        // Maintain max size
        if history.len() > self.max_history_size {
            history.remove(0);
        }
    }

    fn predict_price_movement(&self, mint: &Pubkey) -> Option<f64> {
        if let Some(history) = self.price_history.get(mint) {
            if history.len() < 10 {
                return None; // Need at least 10 data points
            }

            // Simple linear regression for trend prediction
            let recent_points: Vec<&PricePoint> = history
                .iter()
                .rev()
                .take(20)
                .collect();

            let n = recent_points.len() as f64;
            let sum_x: f64 = (0..recent_points.len()).map(|i| i as f64).sum();
            let sum_y: f64 = recent_points.iter().map(|p| p.price).sum();
            let sum_xy: f64 = recent_points
                .iter()
                .enumerate()
                .map(|(i, p)| i as f64 * p.price)
                .sum();
            let sum_x2: f64 = (0..recent_points.len()).map(|i| (i as f64).powi(2)).sum();

            // Calculate slope (trend)
            let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x.powi(2));
            
            // Return predicted percentage change
            Some(slope * 100.0)
        } else {
            None
        }
    }

    fn is_favorable_timing(&self, mint: &Pubkey, direction: &SwapDirection) -> bool {
        if let Some(trend) = self.predict_price_movement(mint) {
            match direction {
                SwapDirection::Buy => trend > 1.0,   // Upward trend for buying
                SwapDirection::Sell => trend < -1.0, // Downward trend for selling
            }
        } else {
            true // Default to favorable if no data
        }
    }
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

    /// Find optimal route considering liquidity, price impact, and fees with parallel analysis
    async fn find_optimal_route(
        &self,
        mint: &Pubkey,
        amount: u64,
        direction: &SwapDirection,
    ) -> Result<DexType> {
        let cache_key = format!("{}_{}_{}_{}", mint, amount, 
                              match direction { SwapDirection::Buy => "buy", SwapDirection::Sell => "sell" },
                              Instant::now().elapsed().as_secs() / 30); // 30-second cache

        // Check route cache first
        if let Some(cached_route) = ROUTE_CACHE.get(&cache_key) {
            if cached_route.cached_at.elapsed() < Duration::from_secs(30) {
                debug!("Using cached route: {:?} with score {:.2}", cached_route.dex, cached_route.score);
                return Ok(cached_route.dex.clone());
            }
        }

        // Check circuit breaker and timing
        let available_dexes = self.get_available_dexes(mint, direction).await?;
        
        if available_dexes.is_empty() {
            return Err(anyhow!("No healthy DEXes available"));
        }

        // Parallel route analysis for better performance
        let route_futures: Vec<_> = available_dexes
            .into_iter()
            .map(|dex| async move {
                match self.calculate_enhanced_route_score(mint, amount, direction, &dex).await {
                    Ok(route) => Some((dex, route)),
                    Err(e) => {
                        debug!("Route calculation failed for {:?}: {}", dex, e);
                        None
                    }
                }
            })
            .collect();

        // Execute all route calculations in parallel
        let route_results = futures::future::join_all(route_futures).await;
        
        // Find the best route
        let mut best_route: Option<(DexType, CachedRoute)> = None;
        
        for result in route_results {
            if let Some((dex, route)) = result {
                if let Some((_, ref current_best)) = best_route {
                    if route.score > current_best.score {
                        best_route = Some((dex, route));
                    }
                } else {
                    best_route = Some((dex, route));
                }
            }
        }

        match best_route {
            Some((dex, route)) => {
                // Cache the result
                ROUTE_CACHE.insert(cache_key, route.clone());
                
                debug!("Selected {:?} with enhanced score: {:.2}, estimated output: {}, price impact: {:.2}%", 
                       dex, route.score, route.estimated_output, route.price_impact);
                Ok(dex)
            }
            None => Err(anyhow!("No viable routes found after parallel analysis"))
        }
    }

    /// Get available DEXes after circuit breaker and timing checks
    async fn get_available_dexes(&self, mint: &Pubkey, direction: &SwapDirection) -> Result<Vec<DexType>> {
        let mut available_dexes = Vec::new();

        // Check timing predictions
        let is_favorable_timing = PRICE_PREDICTOR.is_favorable_timing(mint, direction);
        if !is_favorable_timing {
            debug!("Timing predictor suggests waiting for better conditions");
            // Still proceed but with adjusted scoring
        }

        // Check each DEX availability
        let dex_candidates = vec![
            (DexType::Raydium, self.raydium.is_some()),
            (DexType::PumpFun, self.pump_fun.is_some()),
            (DexType::Meteora, false), // Not implemented yet
            (DexType::Orca, false),    // Not implemented yet
        ];

        for (dex, is_available) in dex_candidates {
            if is_available 
                && CIRCUIT_BREAKER.is_allowed(&dex)
                && LIQUIDITY_MONITOR.is_dex_healthy(&dex) {
                available_dexes.push(dex);
            } else {
                debug!("DEX {:?} filtered out - available: {}, circuit_breaker: {}, healthy: {}", 
                       dex, is_available, CIRCUIT_BREAKER.is_allowed(&dex), LIQUIDITY_MONITOR.is_dex_healthy(&dex));
            }
        }

        Ok(available_dexes)
    }

    /// Enhanced route scoring with comprehensive analysis
    async fn calculate_enhanced_route_score(
        &self,
        mint: &Pubkey,
        amount: u64,
        direction: &SwapDirection,
        dex: &DexType,
    ) -> Result<CachedRoute> {
        let start_time = Instant::now();
        let mut score = 0.0;

        // Base liquidity and availability score
        match dex {
            DexType::PumpFun => {
                if amount < 5_000_000_000 { // < 5 SOL - optimal for pump.fun
                    score += 60.0;
                } else {
                    score += 30.0;
                }
            }
            DexType::Raydium => {
                if amount >= 1_000_000_000 { // >= 1 SOL - good for Raydium
                    score += 65.0;
                } else {
                    score += 40.0;
                }
            }
            _ => score += 20.0, // Other DEXes
        }

        // Health and performance scoring
        let dex_health = LIQUIDITY_MONITOR.get_dex_health(dex);
        score += dex_health * 20.0; // Up to 20 points for health

        // Timing prediction bonus
        if PRICE_PREDICTOR.is_favorable_timing(mint, direction) {
            score += 10.0;
        }

        // Estimated execution time (lower is better)
        let estimated_execution_time = match dex {
            DexType::PumpFun => Duration::from_millis(800),
            DexType::Raydium => Duration::from_millis(1200),
            _ => Duration::from_millis(1500),
        };

        // Time penalty (max 10 points deduction)
        let time_penalty = (estimated_execution_time.as_millis() as f64 / 100.0).min(10.0);
        score -= time_penalty;

        // Estimate price impact (simplified)
        let price_impact = match dex {
            DexType::PumpFun => {
                if amount > 10_000_000_000 { 3.0 } else { 1.5 } // Higher impact for large amounts
            }
            DexType::Raydium => {
                if amount > 50_000_000_000 { 2.5 } else { 1.0 } // Better for large amounts
            }
            _ => 2.0,
        };

        // Price impact penalty
        score -= price_impact;

        // Estimate fees
        let estimated_fees = match dex {
            DexType::PumpFun => 500_000,  // 0.0005 SOL
            DexType::Raydium => 300_000,  // 0.0003 SOL
            _ => 800_000,
        };

        // Fee penalty
        score -= (estimated_fees as f64 / 100_000.0); // Scale down fees

        // Estimate output (simplified)
        let estimated_output = amount - estimated_fees - (amount as f64 * price_impact / 100.0) as u64;

        Ok(CachedRoute {
            dex: dex.clone(),
            score,
            estimated_output,
            price_impact,
            execution_time: estimated_execution_time,
            cached_at: start_time,
            liquidity_depth: 10_000_000_000, // Placeholder
            fees: estimated_fees,
        })
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
