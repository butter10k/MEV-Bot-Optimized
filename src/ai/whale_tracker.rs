use anyhow::{anyhow, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock,
    },
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

// Global whale tracking system
static WHALE_TRACKER: LazyLock<Arc<WhaleTracker>> = 
    LazyLock::new(|| Arc::new(WhaleTracker::new()));

static COPY_TRADING_ENGINE: LazyLock<Arc<CopyTradingEngine>> = 
    LazyLock::new(|| Arc::new(CopyTradingEngine::new()));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhaleWallet {
    pub address: Pubkey,
    pub label: Option<String>,
    pub total_portfolio_value: f64,
    pub sol_balance: f64,
    pub token_holdings: Vec<TokenHolding>,
    pub whale_tier: WhaleTier,
    pub activity_score: f64,
    pub success_rate: f64,
    pub avg_trade_size: f64,
    pub first_seen: Instant,
    pub last_activity: Instant,
    pub tags: Vec<WhaleTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenHolding {
    pub token_mint: Pubkey,
    pub balance: f64,
    pub value_usd: f64,
    pub percentage_of_portfolio: f64,
    pub acquisition_price: Option<f64>,
    pub unrealized_pnl: Option<f64>,
    pub holding_period: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WhaleTier {
    Mega,     // >$10M portfolio
    Large,    // $1M - $10M
    Medium,   // $100K - $1M
    Small,    // $10K - $100K
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WhaleTag {
    DeFiTrader,
    NFTCollector,
    YieldFarmer,
    Arbitrageur,
    LongTermHolder,
    DayTrader,
    MEVBot,
    Market_Maker,
    Influencer,
    Institution,
    Validator,
    Developer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhaleActivity {
    pub whale_address: Pubkey,
    pub activity_type: ActivityType,
    pub token_mint: Pubkey,
    pub amount: f64,
    pub value_usd: f64,
    pub transaction_signature: String,
    pub timestamp: Instant,
    pub dex_used: Option<String>,
    pub price_impact: Option<f64>,
    pub confidence_score: f64,
    pub follow_signal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityType {
    LargeBuy,
    LargeSell,
    NewPosition,
    PositionIncrease,
    PositionDecrease,
    PositionExit,
    LiquidityAdd,
    LiquidityRemove,
    Stake,
    Unstake,
    Arbitrage,
    MEVExtraction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTradingSignal {
    pub whale_address: Pubkey,
    pub signal_type: SignalType,
    pub token_mint: Pubkey,
    pub suggested_action: TradingAction,
    pub confidence: f64,
    pub urgency: SignalUrgency,
    pub risk_level: RiskLevel,
    pub suggested_amount: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub signal_strength: f64,
    pub historical_accuracy: f64,
    pub generated_at: Instant,
    pub expires_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalType {
    Entry,
    Exit,
    ScaleIn,
    ScaleOut,
    HodlSignal,
    AvoidToken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradingAction {
    Buy,
    Sell,
    Hold,
    Avoid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalUrgency {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Conservative,
    Moderate,
    Aggressive,
    Speculative,
}

pub struct WhaleTracker {
    tracked_whales: Arc<DashMap<Pubkey, WhaleWallet>>,
    whale_activities: Arc<DashMap<String, WhaleActivity>>,
    activity_history: Arc<DashMap<Pubkey, VecDeque<WhaleActivity>>>,
    portfolio_tracker: Arc<PortfolioTracker>,
    pattern_analyzer: Arc<PatternAnalyzer>,
    minimum_whale_threshold: f64,
    activity_window: Duration,
    tracking_counter: AtomicU64,
}

impl WhaleTracker {
    pub fn new() -> Self {
        Self {
            tracked_whales: Arc::new(DashMap::new()),
            whale_activities: Arc::new(DashMap::new()),
            activity_history: Arc::new(DashMap::new()),
            portfolio_tracker: Arc::new(PortfolioTracker::new()),
            pattern_analyzer: Arc::new(PatternAnalyzer::new()),
            minimum_whale_threshold: 10000.0, // $10K minimum
            activity_window: Duration::from_hours(24), // 24 hour activity window
            tracking_counter: AtomicU64::new(0),
        }
    }

    pub async fn add_whale_wallet(&self, address: Pubkey, label: Option<String>) -> Result<()> {
        // Analyze wallet to determine if it qualifies as a whale
        let portfolio_value = self.portfolio_tracker.calculate_portfolio_value(address).await?;
        
        if portfolio_value < self.minimum_whale_threshold {
            return Err(anyhow!("Wallet does not meet whale threshold: ${:.2}", portfolio_value));
        }

        let whale_tier = self.determine_whale_tier(portfolio_value);
        let token_holdings = self.portfolio_tracker.get_token_holdings(address).await?;
        let activity_score = self.calculate_activity_score(address).await;
        let success_rate = self.calculate_success_rate(address).await;
        let avg_trade_size = self.calculate_avg_trade_size(address).await;
        let tags = self.analyze_whale_behavior(address).await;

        let whale = WhaleWallet {
            address,
            label,
            total_portfolio_value: portfolio_value,
            sol_balance: self.portfolio_tracker.get_sol_balance(address).await.unwrap_or(0.0),
            token_holdings,
            whale_tier,
            activity_score,
            success_rate,
            avg_trade_size,
            first_seen: Instant::now(),
            last_activity: Instant::now(),
            tags,
        };

        self.tracked_whales.insert(address, whale);
        info!("Added whale wallet: {} (${:.2} portfolio)", address, portfolio_value);

        Ok(())
    }

    async fn determine_whale_tier(&self, portfolio_value: f64) -> WhaleTier {
        match portfolio_value {
            v if v >= 10_000_000.0 => WhaleTier::Mega,
            v if v >= 1_000_000.0 => WhaleTier::Large,
            v if v >= 100_000.0 => WhaleTier::Medium,
            _ => WhaleTier::Small,
        }
    }

    async fn calculate_activity_score(&self, address: Pubkey) -> f64 {
        // Analyze transaction frequency, volume, and diversity
        // This would examine recent transaction history
        0.75 // Placeholder score
    }

    async fn calculate_success_rate(&self, address: Pubkey) -> f64 {
        // Analyze historical trade profitability
        0.68 // Placeholder 68% success rate
    }

    async fn calculate_avg_trade_size(&self, address: Pubkey) -> f64 {
        // Calculate average transaction size
        50000.0 // Placeholder $50K average
    }

    async fn analyze_whale_behavior(&self, address: Pubkey) -> Vec<WhaleTag> {
        // Analyze transaction patterns to determine whale type
        vec![WhaleTag::DeFiTrader, WhaleTag::LongTermHolder] // Placeholder tags
    }

    pub async fn track_transaction(&self, 
        whale_address: Pubkey,
        token_mint: Pubkey,
        amount: f64,
        value_usd: f64,
        transaction_signature: String,
        activity_type: ActivityType,
    ) -> Result<WhaleActivity> {
        
        // Check if this wallet is being tracked
        if !self.tracked_whales.contains_key(&whale_address) {
            // Auto-add if transaction is large enough
            if value_usd >= self.minimum_whale_threshold / 10.0 { // 1/10th threshold for auto-add
                let _ = self.add_whale_wallet(whale_address, None).await;
            } else {
                return Err(anyhow!("Wallet not tracked and transaction too small"));
            }
        }

        // Determine if this should be a follow signal
        let follow_signal = self.should_generate_signal(&whale_address, &activity_type, value_usd).await;

        // Calculate confidence score
        let confidence_score = self.calculate_activity_confidence(&whale_address, &activity_type, value_usd).await;

        let activity = WhaleActivity {
            whale_address,
            activity_type: activity_type.clone(),
            token_mint,
            amount,
            value_usd,
            transaction_signature: transaction_signature.clone(),
            timestamp: Instant::now(),
            dex_used: Some("Unknown".to_string()), // Would be determined from tx analysis
            price_impact: None, // Would be calculated
            confidence_score,
            follow_signal,
        };

        // Store activity
        self.whale_activities.insert(transaction_signature.clone(), activity.clone());

        // Update activity history
        let mut history = self.activity_history.entry(whale_address)
            .or_insert_with(|| VecDeque::with_capacity(1000));
        history.push_back(activity.clone());
        
        // Keep only recent activities
        if history.len() > 1000 {
            history.pop_front();
        }

        // Update whale's last activity
        if let Some(mut whale) = self.tracked_whales.get_mut(&whale_address) {
            whale.last_activity = Instant::now();
        }

        // Generate copy trading signal if appropriate
        if follow_signal {
            if let Err(e) = COPY_TRADING_ENGINE.generate_signal(&activity).await {
                warn!("Failed to generate copy trading signal: {}", e);
            }
        }

        info!("Tracked whale activity: {:?} {} on {} (${:.2})", 
              activity_type, whale_address, token_mint, value_usd);

        Ok(activity)
    }

    async fn should_generate_signal(&self, whale_address: &Pubkey, activity_type: &ActivityType, value_usd: f64) -> bool {
        // Check whale success rate and activity type
        if let Some(whale) = self.tracked_whales.get(whale_address) {
            let min_value_threshold = match whale.whale_tier {
                WhaleTier::Mega => 100_000.0,    // $100K minimum for mega whales
                WhaleTier::Large => 50_000.0,    // $50K for large whales
                WhaleTier::Medium => 10_000.0,   // $10K for medium whales
                WhaleTier::Small => 5_000.0,     // $5K for small whales
            };

            // Only signal for significant activities from successful whales
            whale.success_rate > 0.6 && value_usd >= min_value_threshold
        } else {
            false
        }
    }

    async fn calculate_activity_confidence(&self, whale_address: &Pubkey, activity_type: &ActivityType, value_usd: f64) -> f64 {
        let mut confidence = 0.5; // Base confidence

        if let Some(whale) = self.tracked_whales.get(whale_address) {
            // Factor in whale's historical success rate
            confidence += whale.success_rate * 0.3;

            // Factor in activity size relative to whale's typical trades
            let size_factor = (value_usd / whale.avg_trade_size).min(2.0) * 0.1;
            confidence += size_factor;

            // Factor in whale tier
            let tier_bonus = match whale.whale_tier {
                WhaleTier::Mega => 0.2,
                WhaleTier::Large => 0.15,
                WhaleTier::Medium => 0.1,
                WhaleTier::Small => 0.05,
            };
            confidence += tier_bonus;
        }

        confidence.clamp(0.0, 1.0)
    }

    pub async fn get_whale_activities(&self, whale_address: Option<Pubkey>, limit: usize) -> Vec<WhaleActivity> {
        let mut activities = Vec::new();

        match whale_address {
            Some(address) => {
                // Get activities for specific whale
                if let Some(history) = self.activity_history.get(&address) {
                    activities.extend(history.iter().rev().take(limit).cloned());
                }
            }
            None => {
                // Get recent activities from all whales
                for entry in self.whale_activities.iter() {
                    activities.push(entry.value().clone());
                }
                
                // Sort by timestamp and take most recent
                activities.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                activities.truncate(limit);
            }
        }

        activities
    }

    pub async fn discover_new_whales(&self) -> Result<Vec<Pubkey>> {
        // Scan for large transactions to discover new whales
        // This would typically integrate with transaction monitoring
        let mut discovered = Vec::new();

        // Placeholder logic - in reality this would scan mempool/recent blocks
        debug!("Scanning for new whale wallets...");

        Ok(discovered)
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting whale tracking system");

        let tracker = Arc::clone(&WHALE_TRACKER);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Update whale portfolios
                tracker.update_whale_portfolios().await;
                
                // Discover new whales
                if let Ok(new_whales) = tracker.discover_new_whales().await {
                    for whale_address in new_whales {
                        let _ = tracker.add_whale_wallet(whale_address, None).await;
                    }
                }
                
                // Clean up old activities
                tracker.cleanup_old_activities().await;
            }
        });

        Ok(())
    }

    async fn update_whale_portfolios(&self) {
        for mut whale_entry in self.tracked_whales.iter_mut() {
            let address = *whale_entry.key();
            let whale = whale_entry.value_mut();
            
            // Update portfolio value
            if let Ok(new_value) = self.portfolio_tracker.calculate_portfolio_value(address).await {
                whale.total_portfolio_value = new_value;
                whale.whale_tier = self.determine_whale_tier(new_value).await;
            }
            
            // Update token holdings
            if let Ok(holdings) = self.portfolio_tracker.get_token_holdings(address).await {
                whale.token_holdings = holdings;
            }
        }
    }

    async fn cleanup_old_activities(&self) {
        let cutoff = Instant::now() - self.activity_window;
        
        // Clean up main activities map
        let old_activities: Vec<String> = self.whale_activities
            .iter()
            .filter(|entry| entry.timestamp < cutoff)
            .map(|entry| entry.key().clone())
            .collect();

        for activity_id in old_activities {
            self.whale_activities.remove(&activity_id);
        }

        // Clean up activity history
        for mut history_entry in self.activity_history.iter_mut() {
            let history = history_entry.value_mut();
            while let Some(activity) = history.front() {
                if activity.timestamp < cutoff {
                    history.pop_front();
                } else {
                    break;
                }
            }
        }
    }
}

// Portfolio tracking component
pub struct PortfolioTracker {
    cached_portfolios: Arc<DashMap<Pubkey, CachedPortfolio>>,
    cache_ttl: Duration,
}

#[derive(Debug, Clone)]
struct CachedPortfolio {
    total_value: f64,
    sol_balance: f64,
    token_holdings: Vec<TokenHolding>,
    last_updated: Instant,
}

impl PortfolioTracker {
    pub fn new() -> Self {
        Self {
            cached_portfolios: Arc::new(DashMap::new()),
            cache_ttl: Duration::from_secs(300), // 5 minute cache
        }
    }

    pub async fn calculate_portfolio_value(&self, address: Pubkey) -> Result<f64> {
        // Check cache first
        if let Some(cached) = self.cached_portfolios.get(&address) {
            if cached.last_updated.elapsed() < self.cache_ttl {
                return Ok(cached.total_value);
            }
        }

        // Calculate portfolio value (simplified)
        let sol_balance = self.get_sol_balance(address).await.unwrap_or(0.0);
        let sol_value = sol_balance * 100.0; // Assume SOL = $100

        let token_holdings = self.get_token_holdings(address).await?;
        let token_value: f64 = token_holdings.iter().map(|h| h.value_usd).sum();

        let total_value = sol_value + token_value;

        // Cache the result
        self.cached_portfolios.insert(address, CachedPortfolio {
            total_value,
            sol_balance,
            token_holdings,
            last_updated: Instant::now(),
        });

        Ok(total_value)
    }

    pub async fn get_sol_balance(&self, address: Pubkey) -> Result<f64> {
        // This would query the Solana RPC for actual balance
        Ok(50.0) // Placeholder 50 SOL
    }

    pub async fn get_token_holdings(&self, address: Pubkey) -> Result<Vec<TokenHolding>> {
        // This would query token accounts for the address
        Ok(vec![]) // Placeholder empty holdings
    }
}

// Pattern analysis component
pub struct PatternAnalyzer {
    trading_patterns: Arc<DashMap<Pubkey, TradingPattern>>,
}

#[derive(Debug, Clone)]
struct TradingPattern {
    wallet: Pubkey,
    typical_trade_size: f64,
    preferred_tokens: Vec<Pubkey>,
    trading_frequency: f64,
    risk_tolerance: f64,
    strategy_type: StrategyType,
    success_metrics: SuccessMetrics,
}

#[derive(Debug, Clone)]
enum StrategyType {
    Momentum,
    ValueInvesting,
    Arbitrage,
    ScalpTrading,
    SwingTrading,
    LongTermHodl,
}

#[derive(Debug, Clone)]
struct SuccessMetrics {
    win_rate: f64,
    avg_return: f64,
    max_drawdown: f64,
    sharpe_ratio: f64,
}

impl PatternAnalyzer {
    pub fn new() -> Self {
        Self {
            trading_patterns: Arc::new(DashMap::new()),
        }
    }

    pub async fn analyze_whale_pattern(&self, whale_address: Pubkey, activities: &[WhaleActivity]) -> TradingPattern {
        // Analyze trading patterns (simplified)
        TradingPattern {
            wallet: whale_address,
            typical_trade_size: 25000.0, // $25K average
            preferred_tokens: vec![],
            trading_frequency: 0.5, // 0.5 trades per day
            risk_tolerance: 0.7, // 70% risk tolerance
            strategy_type: StrategyType::Momentum,
            success_metrics: SuccessMetrics {
                win_rate: 0.65,
                avg_return: 0.15, // 15% average return
                max_drawdown: 0.25, // 25% max drawdown
                sharpe_ratio: 1.2,
            },
        }
    }
}

// Copy trading engine
pub struct CopyTradingEngine {
    active_signals: Arc<DashMap<String, CopyTradingSignal>>,
    signal_history: Arc<DashMap<Pubkey, Vec<CopyTradingSignal>>>,
    performance_tracker: Arc<DashMap<Pubkey, CopyTradingPerformance>>,
}

#[derive(Debug, Clone)]
struct CopyTradingPerformance {
    whale_address: Pubkey,
    signals_generated: u64,
    successful_signals: u64,
    average_return: f64,
    total_return: f64,
    max_drawdown: f64,
    last_updated: Instant,
}

impl CopyTradingEngine {
    pub fn new() -> Self {
        Self {
            active_signals: Arc::new(DashMap::new()),
            signal_history: Arc::new(DashMap::new()),
            performance_tracker: Arc::new(DashMap::new()),
        }
    }

    pub async fn generate_signal(&self, activity: &WhaleActivity) -> Result<CopyTradingSignal> {
        let signal_type = self.determine_signal_type(activity);
        let suggested_action = self.determine_trading_action(activity);
        let confidence = activity.confidence_score;
        let urgency = self.determine_urgency(activity);
        let risk_level = self.assess_risk_level(activity);
        
        // Calculate position sizing
        let suggested_amount = self.calculate_position_size(activity).await;
        
        // Set stop loss and take profit
        let (stop_loss, take_profit) = self.calculate_risk_management(activity).await;
        
        let signal = CopyTradingSignal {
            whale_address: activity.whale_address,
            signal_type,
            token_mint: activity.token_mint,
            suggested_action,
            confidence,
            urgency,
            risk_level,
            suggested_amount,
            stop_loss,
            take_profit,
            signal_strength: confidence * 100.0,
            historical_accuracy: self.get_whale_accuracy(activity.whale_address).await,
            generated_at: Instant::now(),
            expires_at: Instant::now() + Duration::from_minutes(30), // 30 minute expiry
        };

        // Store signal
        let signal_id = format!("{}_{}", activity.whale_address, activity.transaction_signature);
        self.active_signals.insert(signal_id, signal.clone());

        // Add to history
        let mut history = self.signal_history.entry(activity.whale_address)
            .or_insert_with(|| Vec::with_capacity(100));
        history.push(signal.clone());

        info!("Generated copy trading signal: {:?} {} (confidence: {:.2})", 
              signal.suggested_action, signal.token_mint, signal.confidence);

        Ok(signal)
    }

    fn determine_signal_type(&self, activity: &WhaleActivity) -> SignalType {
        match activity.activity_type {
            ActivityType::LargeBuy | ActivityType::NewPosition | ActivityType::PositionIncrease => SignalType::Entry,
            ActivityType::LargeSell | ActivityType::PositionExit | ActivityType::PositionDecrease => SignalType::Exit,
            _ => SignalType::HodlSignal,
        }
    }

    fn determine_trading_action(&self, activity: &WhaleActivity) -> TradingAction {
        match activity.activity_type {
            ActivityType::LargeBuy | ActivityType::NewPosition | ActivityType::PositionIncrease => TradingAction::Buy,
            ActivityType::LargeSell | ActivityType::PositionExit | ActivityType::PositionDecrease => TradingAction::Sell,
            _ => TradingAction::Hold,
        }
    }

    fn determine_urgency(&self, activity: &WhaleActivity) -> SignalUrgency {
        if activity.value_usd > 500_000.0 {
            SignalUrgency::Critical
        } else if activity.value_usd > 100_000.0 {
            SignalUrgency::High
        } else if activity.value_usd > 25_000.0 {
            SignalUrgency::Medium
        } else {
            SignalUrgency::Low
        }
    }

    fn assess_risk_level(&self, activity: &WhaleActivity) -> RiskLevel {
        // Risk assessment based on various factors
        match activity.confidence_score {
            c if c > 0.8 => RiskLevel::Conservative,
            c if c > 0.6 => RiskLevel::Moderate,
            c if c > 0.4 => RiskLevel::Aggressive,
            _ => RiskLevel::Speculative,
        }
    }

    async fn calculate_position_size(&self, activity: &WhaleActivity) -> f64 {
        // Position sizing based on whale tier and activity size
        let base_amount = match activity.value_usd {
            v if v > 1_000_000.0 => 10_000.0,   // $10K for mega trades
            v if v > 100_000.0 => 5_000.0,      // $5K for large trades
            v if v > 25_000.0 => 2_500.0,       // $2.5K for medium trades
            _ => 1_000.0,                       // $1K for smaller trades
        };

        // Adjust based on confidence
        base_amount * activity.confidence_score
    }

    async fn calculate_risk_management(&self, activity: &WhaleActivity) -> (Option<f64>, Option<f64>) {
        // Simple risk management - would be more sophisticated in practice
        let current_price = 1.0; // Would fetch actual price
        
        let stop_loss = Some(current_price * 0.95); // 5% stop loss
        let take_profit = Some(current_price * 1.20); // 20% take profit

        (stop_loss, take_profit)
    }

    async fn get_whale_accuracy(&self, whale_address: Pubkey) -> f64 {
        if let Some(performance) = self.performance_tracker.get(&whale_address) {
            if performance.signals_generated > 0 {
                performance.successful_signals as f64 / performance.signals_generated as f64
            } else {
                0.5 // Default 50% if no history
            }
        } else {
            0.5
        }
    }

    pub async fn get_active_signals(&self) -> Vec<CopyTradingSignal> {
        let now = Instant::now();
        
        // Filter out expired signals
        let expired_signals: Vec<String> = self.active_signals
            .iter()
            .filter(|entry| entry.expires_at < now)
            .map(|entry| entry.key().clone())
            .collect();

        for signal_id in expired_signals {
            self.active_signals.remove(&signal_id);
        }

        // Return active signals sorted by signal strength
        let mut signals: Vec<CopyTradingSignal> = self.active_signals
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        signals.sort_by(|a, b| b.signal_strength.partial_cmp(&a.signal_strength).unwrap_or(std::cmp::Ordering::Equal));
        signals
    }
}

// Global whale tracking functions
pub async fn start_whale_tracking() -> Result<()> {
    WHALE_TRACKER.start_monitoring().await
}

pub async fn track_whale_wallet(address: Pubkey, label: Option<String>) -> Result<()> {
    WHALE_TRACKER.add_whale_wallet(address, label).await
}

pub async fn get_whale_activities(whale_address: Option<Pubkey>, limit: usize) -> Vec<WhaleActivity> {
    WHALE_TRACKER.get_whale_activities(whale_address, limit).await
}

pub async fn get_copy_trading_signals() -> Vec<CopyTradingSignal> {
    COPY_TRADING_ENGINE.get_active_signals().await
}

pub async fn track_whale_transaction(
    whale_address: Pubkey,
    token_mint: Pubkey,
    amount: f64,
    value_usd: f64,
    transaction_signature: String,
    activity_type: ActivityType,
) -> Result<WhaleActivity> {
    WHALE_TRACKER.track_transaction(
        whale_address,
        token_mint,
        amount,
        value_usd,
        transaction_signature,
        activity_type,
    ).await
}
