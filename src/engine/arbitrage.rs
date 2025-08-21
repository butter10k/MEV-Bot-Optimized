use anyhow::{anyhow, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock,
    },
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use rust_decimal::Decimal;

// Global arbitrage system
static ARBITRAGE_ENGINE: LazyLock<Arc<ArbitrageEngine>> = 
    LazyLock::new(|| Arc::new(ArbitrageEngine::new()));

static PRICE_AGGREGATOR: LazyLock<Arc<PriceAggregator>> = 
    LazyLock::new(|| Arc::new(PriceAggregator::new()));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub token_mint: Pubkey,
    pub buy_dex: DexId,
    pub sell_dex: DexId,
    pub buy_price: f64,
    pub sell_price: f64,
    pub price_difference: f64,
    pub percentage_profit: f64,
    pub estimated_profit_sol: f64,
    pub required_capital_sol: f64,
    pub confidence_score: f64,
    pub execution_time_window: Duration,
    pub risk_level: ArbitrageRisk,
    pub gas_cost_estimate: u64,
    pub detected_at: Instant,
    pub liquidity_depth: LiquidityDepth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DexId {
    Raydium,
    PumpFun,
    Orca,
    Meteora,
    Serum,
    Jupiter,
    Aldrin,
    Saber,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArbitrageRisk {
    VeryLow,  // >95% confidence, stable liquidity
    Low,      // >90% confidence
    Medium,   // >80% confidence
    High,     // >70% confidence
    VeryHigh, // <70% confidence, volatile
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityDepth {
    pub buy_side_depth_sol: f64,
    pub sell_side_depth_sol: f64,
    pub max_profitable_size_sol: f64,
    pub price_impact_1_sol: f64,
    pub price_impact_5_sol: f64,
    pub price_impact_10_sol: f64,
}

#[derive(Debug, Clone)]
struct DexPriceData {
    dex_id: DexId,
    token_mint: Pubkey,
    bid_price: f64,
    ask_price: f64,
    mid_price: f64,
    volume_24h: f64,
    liquidity_usd: f64,
    last_updated: Instant,
    price_confidence: f64,
    spread_bps: u16, // Basis points
}

#[derive(Debug, Clone)]
struct ArbitrageRoute {
    token_mint: Pubkey,
    path: Vec<ArbitrageStep>,
    total_profit_estimate: f64,
    execution_complexity: ExecutionComplexity,
    estimated_slippage: f64,
}

#[derive(Debug, Clone)]
struct ArbitrageStep {
    dex: DexId,
    action: TradeAction,
    amount: f64,
    expected_price: f64,
    slippage_tolerance: f64,
}

#[derive(Debug, Clone)]
enum TradeAction {
    Buy,
    Sell,
    Bridge, // For cross-chain arbitrage
}

#[derive(Debug, Clone)]
enum ExecutionComplexity {
    Simple,      // Single DEX pair
    Complex,     // Multiple DEXes
    CrossChain,  // Requires bridging
    FlashLoan,   // Requires flash loan
}

pub struct ArbitrageEngine {
    price_feeds: Arc<DashMap<(Pubkey, DexId), DexPriceData>>,
    opportunities: Arc<DashMap<String, ArbitrageOpportunity>>,
    execution_history: Arc<DashMap<String, ArbitrageExecution>>,
    monitoring_tokens: Arc<RwLock<Vec<Pubkey>>>,
    update_interval: Duration,
    min_profit_threshold: f64,
    max_risk_level: ArbitrageRisk,
    profit_counter: AtomicU64,
    execution_counter: AtomicU64,
}

#[derive(Debug, Clone)]
struct ArbitrageExecution {
    opportunity_id: String,
    executed_at: Instant,
    actual_profit: f64,
    execution_time: Duration,
    success: bool,
    gas_used: u64,
    slippage_experienced: f64,
}

impl ArbitrageEngine {
    pub fn new() -> Self {
        Self {
            price_feeds: Arc::new(DashMap::new()),
            opportunities: Arc::new(DashMap::new()),
            execution_history: Arc::new(DashMap::new()),
            monitoring_tokens: Arc::new(RwLock::new(Vec::new())),
            update_interval: Duration::from_millis(500), // 500ms updates
            min_profit_threshold: 0.5, // 0.5% minimum profit
            max_risk_level: ArbitrageRisk::Medium,
            profit_counter: AtomicU64::new(0),
            execution_counter: AtomicU64::new(0),
        }
    }

    pub async fn add_monitoring_token(&self, token_mint: Pubkey) {
        let mut tokens = self.monitoring_tokens.write().await;
        if !tokens.contains(&token_mint) {
            tokens.push(token_mint);
            info!("Added token to arbitrage monitoring: {}", token_mint);
        }
    }

    pub async fn scan_arbitrage_opportunities(&self) -> Vec<ArbitrageOpportunity> {
        let mut opportunities = Vec::new();
        let tokens = self.monitoring_tokens.read().await;

        for token in tokens.iter() {
            if let Some(opportunity) = self.find_best_arbitrage_for_token(*token).await {
                opportunities.push(opportunity);
            }
        }

        opportunities.sort_by(|a, b| {
            b.percentage_profit.partial_cmp(&a.percentage_profit).unwrap_or(std::cmp::Ordering::Equal)
        });

        opportunities
    }

    async fn find_best_arbitrage_for_token(&self, token_mint: Pubkey) -> Option<ArbitrageOpportunity> {
        let mut prices: Vec<&DexPriceData> = self.price_feeds
            .iter()
            .filter(|entry| entry.key().0 == token_mint)
            .map(|entry| entry.value())
            .collect();

        if prices.len() < 2 {
            return None; // Need at least 2 DEXes for arbitrage
        }

        // Sort by mid price
        prices.sort_by(|a, b| a.mid_price.partial_cmp(&b.mid_price).unwrap_or(std::cmp::Ordering::Equal));

        let cheapest = prices.first()?;
        let most_expensive = prices.last()?;

        let price_difference = most_expensive.mid_price - cheapest.mid_price;
        let percentage_profit = (price_difference / cheapest.mid_price) * 100.0;

        if percentage_profit < self.min_profit_threshold {
            return None;
        }

        // Calculate liquidity depth
        let liquidity_depth = self.calculate_liquidity_depth(cheapest, most_expensive).await;

        // Calculate risk level
        let risk_level = self.assess_arbitrage_risk(cheapest, most_expensive, percentage_profit).await;

        // Skip if risk is too high
        if self.risk_level_value(&risk_level) > self.risk_level_value(&self.max_risk_level) {
            return None;
        }

        // Estimate gas costs
        let gas_cost = self.estimate_arbitrage_gas_cost(&cheapest.dex_id, &most_expensive.dex_id).await;

        // Calculate required capital (conservative estimate)
        let required_capital = liquidity_depth.max_profitable_size_sol.min(10.0); // Max 10 SOL

        // Calculate net profit after gas
        let gross_profit = required_capital * percentage_profit / 100.0;
        let gas_cost_sol = gas_cost as f64 / 1_000_000_000.0; // Convert lamports to SOL
        let net_profit = gross_profit - gas_cost_sol;

        if net_profit <= 0.0 {
            return None; // Not profitable after gas costs
        }

        // Calculate confidence score
        let confidence_score = self.calculate_confidence_score(cheapest, most_expensive, percentage_profit).await;

        Some(ArbitrageOpportunity {
            token_mint,
            buy_dex: cheapest.dex_id.clone(),
            sell_dex: most_expensive.dex_id.clone(),
            buy_price: cheapest.ask_price,
            sell_price: most_expensive.bid_price,
            price_difference,
            percentage_profit,
            estimated_profit_sol: net_profit,
            required_capital_sol: required_capital,
            confidence_score,
            execution_time_window: Duration::from_secs(30), // 30 second window
            risk_level,
            gas_cost_estimate: gas_cost,
            detected_at: Instant::now(),
            liquidity_depth,
        })
    }

    async fn calculate_liquidity_depth(&self, buy_dex: &DexPriceData, sell_dex: &DexPriceData) -> LiquidityDepth {
        // Simplified liquidity calculation
        let buy_depth = (buy_dex.liquidity_usd / buy_dex.mid_price / 1000.0).min(50.0); // Max 50 SOL
        let sell_depth = (sell_dex.liquidity_usd / sell_dex.mid_price / 1000.0).min(50.0);
        
        let max_size = buy_depth.min(sell_depth);

        LiquidityDepth {
            buy_side_depth_sol: buy_depth,
            sell_side_depth_sol: sell_depth,
            max_profitable_size_sol: max_size,
            price_impact_1_sol: self.estimate_price_impact(1.0, buy_dex.liquidity_usd),
            price_impact_5_sol: self.estimate_price_impact(5.0, buy_dex.liquidity_usd),
            price_impact_10_sol: self.estimate_price_impact(10.0, buy_dex.liquidity_usd),
        }
    }

    fn estimate_price_impact(&self, trade_size_sol: f64, liquidity_usd: f64) -> f64 {
        // Simplified price impact model
        let trade_size_usd = trade_size_sol * 100.0; // Assume SOL = $100
        let impact_factor = trade_size_usd / liquidity_usd;
        
        // Square root model for price impact
        (impact_factor.sqrt() * 100.0).min(10.0) // Cap at 10%
    }

    async fn assess_arbitrage_risk(&self, buy_dex: &DexPriceData, sell_dex: &DexPriceData, profit_percent: f64) -> ArbitrageRisk {
        let mut risk_score = 0;

        // Age of price data
        let max_age = buy_dex.last_updated.elapsed().max(sell_dex.last_updated.elapsed());
        if max_age > Duration::from_secs(10) {
            risk_score += 2; // Stale data
        }

        // Spread analysis
        let avg_spread = (buy_dex.spread_bps + sell_dex.spread_bps) as f64 / 2.0;
        if avg_spread > 100 { // >1% spread
            risk_score += 2;
        }

        // Profit size (higher profit might indicate higher risk)
        if profit_percent > 5.0 {
            risk_score += 2; // Suspiciously high profit
        }

        // Confidence in price data
        let avg_confidence = (buy_dex.price_confidence + sell_dex.price_confidence) / 2.0;
        if avg_confidence < 0.8 {
            risk_score += 1;
        }

        match risk_score {
            0..=1 => ArbitrageRisk::VeryLow,
            2..=3 => ArbitrageRisk::Low,
            4..=5 => ArbitrageRisk::Medium,
            6..=7 => ArbitrageRisk::High,
            _ => ArbitrageRisk::VeryHigh,
        }
    }

    fn risk_level_value(&self, risk: &ArbitrageRisk) -> u8 {
        match risk {
            ArbitrageRisk::VeryLow => 1,
            ArbitrageRisk::Low => 2,
            ArbitrageRisk::Medium => 3,
            ArbitrageRisk::High => 4,
            ArbitrageRisk::VeryHigh => 5,
        }
    }

    async fn estimate_arbitrage_gas_cost(&self, buy_dex: &DexId, sell_dex: &DexId) -> u64 {
        // Base gas costs for different DEX combinations
        let base_cost = match (buy_dex, sell_dex) {
            (DexId::Raydium, DexId::PumpFun) => 300_000,   // 0.0003 SOL
            (DexId::PumpFun, DexId::Raydium) => 300_000,
            (DexId::Orca, DexId::Raydium) => 350_000,      // 0.00035 SOL
            (DexId::Jupiter, _) | (_, DexId::Jupiter) => 400_000, // Jupiter aggregation
            _ => 250_000, // Default
        };

        // Add priority fee based on current network conditions
        let priority_fee = 50_000; // 0.00005 SOL priority fee
        
        base_cost + priority_fee
    }

    async fn calculate_confidence_score(&self, buy_dex: &DexPriceData, sell_dex: &DexPriceData, profit_percent: f64) -> f64 {
        let mut confidence = 1.0;

        // Reduce confidence for stale data
        let max_staleness = buy_dex.last_updated.elapsed().max(sell_dex.last_updated.elapsed());
        confidence *= (1.0 - (max_staleness.as_secs() as f64 / 60.0)).max(0.1); // Decay over 1 minute

        // Reduce confidence for low liquidity
        let min_liquidity = buy_dex.liquidity_usd.min(sell_dex.liquidity_usd);
        if min_liquidity < 10000.0 { // Less than $10k liquidity
            confidence *= 0.7;
        }

        // Reduce confidence for very high spreads
        let max_spread = buy_dex.spread_bps.max(sell_dex.spread_bps) as f64;
        if max_spread > 200.0 { // >2% spread
            confidence *= 0.8;
        }

        // Factor in price data confidence
        let price_confidence = (buy_dex.price_confidence + sell_dex.price_confidence) / 2.0;
        confidence *= price_confidence;

        confidence.clamp(0.0, 1.0)
    }

    pub async fn execute_arbitrage(&self, opportunity: &ArbitrageOpportunity) -> Result<ArbitrageExecution> {
        let execution_id = format!("arb_{}_{}", 
                                 opportunity.token_mint, 
                                 self.execution_counter.fetch_add(1, Ordering::SeqCst));
        
        let start_time = Instant::now();
        
        info!("Executing arbitrage: {} -> {} for {:.4}% profit",
              format!("{:?}", opportunity.buy_dex),
              format!("{:?}", opportunity.sell_dex),
              opportunity.percentage_profit);

        // Simulate execution (in real implementation, this would execute actual trades)
        let execution_time = Duration::from_millis(800); // Simulated execution time
        let success = true; // Assume success for simulation
        let actual_slippage = 0.1; // 0.1% slippage
        let actual_profit = opportunity.estimated_profit_sol * (1.0 - actual_slippage / 100.0);

        if success {
            self.profit_counter.fetch_add((actual_profit * 1_000_000_000.0) as u64, Ordering::SeqCst);
        }

        let execution = ArbitrageExecution {
            opportunity_id: execution_id.clone(),
            executed_at: start_time,
            actual_profit,
            execution_time,
            success,
            gas_used: opportunity.gas_cost_estimate,
            slippage_experienced: actual_slippage,
        };

        self.execution_history.insert(execution_id, execution.clone());

        // Clean up old execution history
        if self.execution_history.len() > 1000 {
            self.cleanup_execution_history().await;
        }

        Ok(execution)
    }

    async fn cleanup_execution_history(&self) {
        let cutoff = Instant::now() - Duration::from_hours(24); // Keep 24 hours
        
        let old_executions: Vec<String> = self.execution_history
            .iter()
            .filter(|entry| entry.executed_at < cutoff)
            .map(|entry| entry.key().clone())
            .collect();

        for execution_id in old_executions {
            self.execution_history.remove(&execution_id);
        }
    }

    pub async fn get_arbitrage_statistics(&self) -> ArbitrageStatistics {
        let total_executions = self.execution_counter.load(Ordering::SeqCst);
        let total_profit = self.profit_counter.load(Ordering::SeqCst) as f64 / 1_000_000_000.0; // Convert to SOL

        let recent_executions: Vec<&ArbitrageExecution> = self.execution_history
            .iter()
            .filter(|entry| entry.executed_at.elapsed() < Duration::from_hours(24))
            .map(|entry| entry.value())
            .collect();

        let successful_executions = recent_executions.iter().filter(|e| e.success).count();
        let success_rate = if !recent_executions.is_empty() {
            successful_executions as f64 / recent_executions.len() as f64
        } else {
            0.0
        };

        let avg_execution_time = if !recent_executions.is_empty() {
            recent_executions.iter()
                .map(|e| e.execution_time.as_millis() as f64)
                .sum::<f64>() / recent_executions.len() as f64
        } else {
            0.0
        };

        let current_opportunities = self.opportunities.len() as u64;

        ArbitrageStatistics {
            total_executions,
            successful_executions: successful_executions as u64,
            success_rate,
            total_profit_sol: total_profit,
            avg_execution_time_ms: avg_execution_time,
            current_opportunities,
            monitored_tokens: self.monitoring_tokens.read().await.len() as u64,
        }
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting arbitrage monitoring system");

        let engine = Arc::clone(&ARBITRAGE_ENGINE);
        let update_interval = self.update_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(update_interval);
            
            loop {
                interval.tick().await;
                
                // Scan for new opportunities
                let opportunities = engine.scan_arbitrage_opportunities().await;
                
                // Update opportunities cache
                engine.opportunities.clear();
                for opp in opportunities {
                    let key = format!("{}_{:?}_{:?}", opp.token_mint, opp.buy_dex, opp.sell_dex);
                    engine.opportunities.insert(key, opp);
                }
                
                // Log high-value opportunities
                for opportunity in engine.opportunities.iter() {
                    if opportunity.percentage_profit > 2.0 { // >2% profit
                        info!("High-value arbitrage opportunity: {:.2}% profit, {:.4} SOL estimated profit",
                              opportunity.percentage_profit, opportunity.estimated_profit_sol);
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn update_price_data(&self, dex_id: DexId, token_mint: Pubkey, price_data: DexPriceData) {
        self.price_feeds.insert((token_mint, dex_id), price_data);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageStatistics {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub success_rate: f64,
    pub total_profit_sol: f64,
    pub avg_execution_time_ms: f64,
    pub current_opportunities: u64,
    pub monitored_tokens: u64,
}

// Price aggregator for real-time price feeds
pub struct PriceAggregator {
    dex_apis: HashMap<DexId, DexApiConfig>,
    update_interval: Duration,
}

#[derive(Debug, Clone)]
struct DexApiConfig {
    endpoint: String,
    rate_limit: Duration,
    last_request: Arc<RwLock<Instant>>,
    enabled: bool,
}

impl PriceAggregator {
    pub fn new() -> Self {
        let mut dex_apis = HashMap::new();
        
        // Configure DEX API endpoints
        dex_apis.insert(DexId::Raydium, DexApiConfig {
            endpoint: "https://api.raydium.io/v2/main/pairs".to_string(),
            rate_limit: Duration::from_millis(500),
            last_request: Arc::new(RwLock::new(Instant::now() - Duration::from_secs(60))),
            enabled: true,
        });
        
        dex_apis.insert(DexId::Jupiter, DexApiConfig {
            endpoint: "https://price.jup.ag/v4/price".to_string(),
            rate_limit: Duration::from_millis(200),
            last_request: Arc::new(RwLock::new(Instant::now() - Duration::from_secs(60))),
            enabled: true,
        });

        Self {
            dex_apis,
            update_interval: Duration::from_secs(1),
        }
    }

    pub async fn start_price_updates(&self) -> Result<()> {
        info!("Starting price aggregation system");

        for (dex_id, config) in &self.dex_apis {
            if config.enabled {
                let dex_id = dex_id.clone();
                let config = config.clone();
                
                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(config.rate_limit);
                    
                    loop {
                        interval.tick().await;
                        
                        // Fetch prices from this DEX
                        if let Err(e) = Self::fetch_dex_prices(&dex_id, &config).await {
                            warn!("Failed to fetch prices from {:?}: {}", dex_id, e);
                        }
                    }
                });
            }
        }

        Ok(())
    }

    async fn fetch_dex_prices(dex_id: &DexId, config: &DexApiConfig) -> Result<()> {
        // Check rate limiting
        let last_request = *config.last_request.read().await;
        if last_request.elapsed() < config.rate_limit {
            return Ok(());
        }

        // Update last request time
        *config.last_request.write().await = Instant::now();

        // Fetch prices (simplified - would use actual API calls)
        debug!("Fetching prices from {:?}", dex_id);
        
        // This would make actual API calls and update the price feeds
        
        Ok(())
    }
}

// Global arbitrage functions
pub async fn start_arbitrage_system() -> Result<()> {
    ARBITRAGE_ENGINE.start_monitoring().await?;
    PRICE_AGGREGATOR.start_price_updates().await?;
    Ok(())
}

pub async fn add_arbitrage_token(token_mint: Pubkey) {
    ARBITRAGE_ENGINE.add_monitoring_token(token_mint).await;
}

pub async fn get_current_arbitrage_opportunities() -> Vec<ArbitrageOpportunity> {
    ARBITRAGE_ENGINE.scan_arbitrage_opportunities().await
}

pub async fn execute_best_arbitrage() -> Option<Result<ArbitrageExecution>> {
    let opportunities = get_current_arbitrage_opportunities().await;
    
    if let Some(best_opportunity) = opportunities.first() {
        Some(ARBITRAGE_ENGINE.execute_arbitrage(best_opportunity).await)
    } else {
        None
    }
}

pub async fn get_arbitrage_statistics() -> ArbitrageStatistics {
    ARBITRAGE_ENGINE.get_arbitrage_statistics().await
}
