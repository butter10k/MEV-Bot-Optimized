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
use tracing::{debug, info, warn, error};

// Global risk management system
static RISK_MANAGER: LazyLock<Arc<RiskManager>> = 
    LazyLock::new(|| Arc::new(RiskManager::new()));

static POSITION_MANAGER: LazyLock<Arc<PositionManager>> = 
    LazyLock::new(|| Arc::new(PositionManager::new()));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_position_size_sol: f64,
    pub max_portfolio_allocation_percent: f64,
    pub max_daily_loss_sol: f64,
    pub max_drawdown_percent: f64,
    pub max_concurrent_positions: u32,
    pub max_exposure_per_token_percent: f64,
    pub min_liquidity_threshold_sol: f64,
    pub max_slippage_tolerance_percent: f64,
    pub max_gas_fee_per_trade_sol: f64,
    pub cooldown_period_after_loss: Duration,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_size_sol: 10.0,              // Max 10 SOL per position
            max_portfolio_allocation_percent: 20.0,    // Max 20% of portfolio per trade
            max_daily_loss_sol: 5.0,                  // Max 5 SOL loss per day
            max_drawdown_percent: 25.0,               // Max 25% drawdown
            max_concurrent_positions: 5,               // Max 5 open positions
            max_exposure_per_token_percent: 30.0,     // Max 30% exposure to single token
            min_liquidity_threshold_sol: 100.0,       // Min 100 SOL liquidity required
            max_slippage_tolerance_percent: 5.0,      // Max 5% slippage
            max_gas_fee_per_trade_sol: 0.01,         // Max 0.01 SOL gas per trade
            cooldown_period_after_loss: Duration::from_minutes(30), // 30 min cooldown
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub token_mint: Pubkey,
    pub entry_price: f64,
    pub current_price: f64,
    pub size_sol: f64,
    pub size_tokens: f64,
    pub entry_time: Instant,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub position_type: PositionType,
    pub risk_level: RiskLevel,
    pub max_loss: f64,
    pub trailing_stop: Option<TrailingStop>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionType {
    Long,
    Short,
    Arbitrage,
    Hedge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Conservative,
    Moderate,
    Aggressive,
    Speculative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailingStop {
    pub trail_amount: f64,
    pub current_stop: f64,
    pub highest_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub portfolio_value: f64,
    pub total_exposure: f64,
    pub daily_pnl: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub max_drawdown: f64,
    pub current_drawdown: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub sharpe_ratio: f64,
    pub var_95: f64,      // Value at Risk 95%
    pub expected_shortfall: f64,
    pub exposure_by_token: HashMap<Pubkey, f64>,
    pub risk_score: f64,
    pub margin_usage: f64,
    pub leverage_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub affected_position: Option<String>,
    pub recommended_action: RecommendedAction,
    pub timestamp: Instant,
    pub auto_executed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    MaxLossExceeded,
    DrawdownLimit,
    ConcentrationRisk,
    LiquidityRisk,
    SlippageAlert,
    VolatilitySpike,
    MarginCall,
    PositionSizeLimit,
    GasFeeAlert,
    CooldownViolation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendedAction {
    ReducePosition,
    ClosePosition,
    AddStopLoss,
    IncreaseStopLoss,
    PauseTrading,
    DiversifyHoldings,
    ReduceLeverage,
    WaitForLiquidity,
    ReduceGasPrice,
    None,
}

pub struct RiskManager {
    risk_limits: Arc<RwLock<RiskLimits>>,
    active_alerts: Arc<DashMap<String, RiskAlert>>,
    daily_metrics: Arc<DashMap<String, DailyRiskMetrics>>, // Date -> Metrics
    portfolio_history: Arc<RwLock<Vec<PortfolioSnapshot>>>,
    risk_events: Arc<DashMap<String, RiskEvent>>,
    circuit_breakers: Arc<DashMap<String, CircuitBreaker>>,
    last_loss_time: Arc<RwLock<Option<Instant>>>,
    monitoring_enabled: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
struct DailyRiskMetrics {
    date: String,
    total_trades: u32,
    winning_trades: u32,
    total_pnl: f64,
    max_drawdown: f64,
    largest_loss: f64,
    largest_win: f64,
    gas_fees_paid: f64,
}

#[derive(Debug, Clone)]
struct PortfolioSnapshot {
    timestamp: Instant,
    total_value: f64,
    cash_balance: f64,
    positions_value: f64,
    unrealized_pnl: f64,
    leverage: f64,
}

#[derive(Debug, Clone)]
struct RiskEvent {
    event_id: String,
    event_type: AlertType,
    timestamp: Instant,
    details: String,
    resolved: bool,
    impact_score: f64,
}

#[derive(Debug, Clone)]
struct CircuitBreaker {
    breaker_type: String,
    trigger_threshold: f64,
    current_value: f64,
    cooldown_period: Duration,
    last_triggered: Option<Instant>,
    is_active: bool,
}

impl RiskManager {
    pub fn new() -> Self {
        Self {
            risk_limits: Arc::new(RwLock::new(RiskLimits::default())),
            active_alerts: Arc::new(DashMap::new()),
            daily_metrics: Arc::new(DashMap::new()),
            portfolio_history: Arc::new(RwLock::new(Vec::new())),
            risk_events: Arc::new(DashMap::new()),
            circuit_breakers: Arc::new(DashMap::new()),
            last_loss_time: Arc::new(RwLock::new(None)),
            monitoring_enabled: Arc::new(RwLock::new(true)),
        }
    }

    pub async fn evaluate_trade_risk(&self, 
        token_mint: Pubkey,
        trade_size_sol: f64,
        expected_slippage: f64,
        gas_fee: f64,
        position_type: PositionType,
    ) -> Result<TradeRiskAssessment> {
        
        let monitoring_enabled = *self.monitoring_enabled.read().await;
        if !monitoring_enabled {
            return Ok(TradeRiskAssessment::approved_with_warning("Risk monitoring disabled"));
        }

        let risk_limits = self.risk_limits.read().await;
        let current_metrics = self.calculate_current_risk_metrics().await?;
        
        if trade_size_sol > risk_limits.max_position_size_sol {
            return Ok(TradeRiskAssessment::rejected(
                format!("Trade size ({:.2} SOL) exceeds maximum allowed ({:.2} SOL)", 
                       trade_size_sol, risk_limits.max_position_size_sol)
            ));
        }

        let allocation_percent = (trade_size_sol / current_metrics.portfolio_value) * 100.0;
        if allocation_percent > risk_limits.max_portfolio_allocation_percent {
            return Ok(TradeRiskAssessment::rejected(
                format!("Trade allocation ({:.1}%) exceeds maximum allowed ({:.1}%)", 
                       allocation_percent, risk_limits.max_portfolio_allocation_percent)
            ));
        }

        if current_metrics.daily_pnl < -risk_limits.max_daily_loss_sol {
            return Ok(TradeRiskAssessment::rejected(
                format!("Daily loss limit reached ({:.2} SOL)", risk_limits.max_daily_loss_sol)
            ));
        }

        let active_positions = POSITION_MANAGER.get_active_position_count().await;
        if active_positions >= risk_limits.max_concurrent_positions {
            return Ok(TradeRiskAssessment::rejected(
                format!("Maximum concurrent positions ({}) reached", risk_limits.max_concurrent_positions)
            ));
        }

        let token_exposure = current_metrics.exposure_by_token.get(&token_mint).unwrap_or(&0.0);
        let new_exposure = (*token_exposure + trade_size_sol) / current_metrics.portfolio_value * 100.0;
        if new_exposure > risk_limits.max_exposure_per_token_percent {
            return Ok(TradeRiskAssessment::rejected(
                format!("Token exposure would exceed limit ({:.1}% > {:.1}%)", 
                       new_exposure, risk_limits.max_exposure_per_token_percent)
            ));
        }

        if expected_slippage > risk_limits.max_slippage_tolerance_percent {
            return Ok(TradeRiskAssessment::rejected(
                format!("Expected slippage ({:.2}%) exceeds tolerance ({:.2}%)", 
                       expected_slippage, risk_limits.max_slippage_tolerance_percent)
            ));
        }

        if gas_fee > risk_limits.max_gas_fee_per_trade_sol {
            return Ok(TradeRiskAssessment::rejected(
                format!("Gas fee ({:.4} SOL) exceeds maximum ({:.4} SOL)", 
                       gas_fee, risk_limits.max_gas_fee_per_trade_sol)
            ));
        }

        if let Some(last_loss) = *self.last_loss_time.read().await {
            if last_loss.elapsed() < risk_limits.cooldown_period_after_loss {
                return Ok(TradeRiskAssessment::rejected(
                    format!("Cooldown period active after loss ({}s remaining)", 
                           (risk_limits.cooldown_period_after_loss - last_loss.elapsed()).as_secs())
                ));
            }
        }

        if self.check_circuit_breakers().await {
            return Ok(TradeRiskAssessment::rejected("Circuit breaker active".to_string()));
        }

        let risk_score = self.calculate_trade_risk_score(
            trade_size_sol, 
            allocation_percent, 
            expected_slippage, 
            &current_metrics
        ).await;

        if risk_score < 0.3 {
            Ok(TradeRiskAssessment::approved())
        } else if risk_score < 0.7 {
            Ok(TradeRiskAssessment::approved_with_warning(
                format!("Moderate risk trade (score: {:.2})", risk_score)
            ))
        } else {
            Ok(TradeRiskAssessment::rejected(
                format!("High risk trade (score: {:.2})", risk_score)
            ))
        }
    }

    async fn calculate_trade_risk_score(&self, 
        trade_size: f64, 
        allocation_percent: f64, 
        slippage: f64, 
        metrics: &RiskMetrics
    ) -> f64 {
        let mut risk_score = 0.0;

        risk_score += (trade_size / 50.0).min(0.3);

        risk_score += (allocation_percent / 50.0).min(0.2); 

        risk_score += (slippage / 10.0).min(0.2); 

        risk_score += (metrics.current_drawdown / 25.0).min(0.2);

        risk_score += (metrics.var_95 / 100.0).min(0.1);
        risk_score.min(1.0)
    }

    async fn check_circuit_breakers(&self) -> bool {
        for breaker in self.circuit_breakers.iter() {
            if breaker.is_active {
                if let Some(last_triggered) = breaker.last_triggered {
                    if last_triggered.elapsed() < breaker.cooldown_period {
                        return true; 
                    }
                } else {
                    return true; 
                }
            }
        }
        false
    }

    async fn calculate_current_risk_metrics(&self) -> Result<RiskMetrics> {
        let positions = POSITION_MANAGER.get_all_positions().await;
        let portfolio_value = self.calculate_portfolio_value(&positions).await;
        
        let total_exposure: f64 = positions.iter().map(|p| p.size_sol).sum();
        let unrealized_pnl: f64 = positions.iter().map(|p| p.unrealized_pnl).sum();
        
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let daily_pnl = self.daily_metrics.get(&today)
            .map(|metrics| metrics.total_pnl)
            .unwrap_or(0.0);

        let mut exposure_by_token = HashMap::new();
        for position in &positions {
            *exposure_by_token.entry(position.token_mint).or_insert(0.0) += position.size_sol;
        }

        Ok(RiskMetrics {
            portfolio_value,
            total_exposure,
            daily_pnl,
            unrealized_pnl,
            realized_pnl: 0.0, 
            max_drawdown: 15.0,
            current_drawdown: 5.0,
            win_rate: 0.65, 
            profit_factor: 1.4, 
            sharpe_ratio: 1.2, 
            var_95: 8.5,
            expected_shortfall: 12.0, 
            exposure_by_token,
            risk_score: 0.3, 
            margin_usage: 0.0,
            leverage_ratio: 1.0,
        })
    }

    async fn calculate_portfolio_value(&self, positions: &[Position]) -> f64 {
        let position_value: f64 = positions.iter().map(|p| p.size_sol).sum();
        let cash_balance = 100.0;
        
        cash_balance + position_value
    }

    pub async fn monitor_positions(&self) -> Result<Vec<RiskAlert>> {
        let mut alerts = Vec::new();
        let positions = POSITION_MANAGER.get_all_positions().await;
        let risk_limits = self.risk_limits.read().await;

        for position in positions {
            if let Some(stop_loss) = position.stop_loss {
                if position.current_price <= stop_loss {
                    alerts.push(self.create_alert(
                        AlertType::MaxLossExceeded,
                        AlertSeverity::Critical,
                        format!("Stop loss triggered for position {}", position.id),
                        Some(position.id.clone()),
                        RecommendedAction::ClosePosition,
                    ).await);
                }
            }

            if position.unrealized_pnl < -position.max_loss {
                alerts.push(self.create_alert(
                    AlertType::MaxLossExceeded,
                    AlertSeverity::Warning,
                    format!("Position {} exceeding max loss", position.id),
                    Some(position.id.clone()),
                    RecommendedAction::ReducePosition,
                ).await);
            }

            if let Some(mut trailing_stop) = position.trailing_stop.clone() {
                if position.current_price > trailing_stop.highest_price {
                    trailing_stop.highest_price = position.current_price;
                    trailing_stop.current_stop = position.current_price - trailing_stop.trail_amount;
                    
                    POSITION_MANAGER.update_trailing_stop(&position.id, trailing_stop).await?;
                }
            }
        }

        let metrics = self.calculate_current_risk_metrics().await?;
        
        if metrics.current_drawdown > risk_limits.max_drawdown_percent {
            alerts.push(self.create_alert(
                AlertType::DrawdownLimit,
                AlertSeverity::Critical,
                format!("Portfolio drawdown ({:.1}%) exceeds limit ({:.1}%)", 
                       metrics.current_drawdown, risk_limits.max_drawdown_percent),
                None,
                RecommendedAction::PauseTrading,
            ).await);
        }

        for alert in &alerts {
            let alert_id = format!("{}_{}", 
                                 chrono::Utc::now().timestamp_millis(), 
                                 rand::random::<u32>());
            self.active_alerts.insert(alert_id, alert.clone());
        }

        Ok(alerts)
    }

    async fn create_alert(&self,
        alert_type: AlertType,
        severity: AlertSeverity,
        message: String,
        position_id: Option<String>,
        action: RecommendedAction,
    ) -> RiskAlert {
        RiskAlert {
            alert_type,
            severity,
            message,
            affected_position: position_id,
            recommended_action: action,
            timestamp: Instant::now(),
            auto_executed: false,
        }
    }

    pub async fn update_risk_limits(&self, new_limits: RiskLimits) {
        *self.risk_limits.write().await = new_limits;
        info!("Risk limits updated");
    }

    pub async fn get_risk_metrics(&self) -> Result<RiskMetrics> {
        self.calculate_current_risk_metrics().await
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting risk management system");

        let risk_manager = Arc::clone(&RISK_MANAGER);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                if let Ok(alerts) = risk_manager.monitor_positions().await {
                    for alert in alerts {
                        match alert.severity {
                            AlertSeverity::Critical | AlertSeverity::Emergency => {
                                error!("RISK ALERT: {}", alert.message);
                            }
                            AlertSeverity::Warning => {
                                warn!("Risk warning: {}", alert.message);
                            }
                            AlertSeverity::Info => {
                                info!("Risk info: {}", alert.message);
                            }
                        }
                    }
                }
                
                risk_manager.take_portfolio_snapshot().await;
            }
        });

        Ok(())
    }

    async fn take_portfolio_snapshot(&self) {
        let positions = POSITION_MANAGER.get_all_positions().await;
        let portfolio_value = self.calculate_portfolio_value(&positions).await;
        let unrealized_pnl: f64 = positions.iter().map(|p| p.unrealized_pnl).sum();
        
        let snapshot = PortfolioSnapshot {
            timestamp: Instant::now(),
            total_value: portfolio_value,
            cash_balance: 100.0, 
            positions_value: portfolio_value - 100.0,
            unrealized_pnl,
            leverage: 1.0,
        };

        let mut history = self.portfolio_history.write().await;
        history.push(snapshot);

        if history.len() > 1000 {
            history.remove(0);
        }
    }
}

pub struct PositionManager {
    active_positions: Arc<DashMap<String, Position>>,
    closed_positions: Arc<DashMap<String, Position>>,
    position_counter: AtomicU64,
}

impl PositionManager {
    pub fn new() -> Self {
        Self {
            active_positions: Arc::new(DashMap::new()),
            closed_positions: Arc::new(DashMap::new()),
            position_counter: AtomicU64::new(0),
        }
    }

    pub async fn open_position(&self, 
        token_mint: Pubkey,
        entry_price: f64,
        size_sol: f64,
        size_tokens: f64,
        position_type: PositionType,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    ) -> Result<String> {
        
        let position_id = format!("pos_{}", self.position_counter.fetch_add(1, Ordering::SeqCst));
        
        let position = Position {
            id: position_id.clone(),
            token_mint,
            entry_price,
            current_price: entry_price,
            size_sol,
            size_tokens,
            entry_time: Instant::now(),
            stop_loss,
            take_profit,
            unrealized_pnl: 0.0,
            realized_pnl: 0.0,
            position_type,
            risk_level: RiskLevel::Moderate,
            max_loss: size_sol * 0.1,
            trailing_stop: None,
        };

        self.active_positions.insert(position_id.clone(), position);
        info!("Opened position: {} for {} ({:.2} SOL)", position_id, token_mint, size_sol);

        Ok(position_id)
    }

    pub async fn close_position(&self, position_id: &str, exit_price: f64) -> Result<Position> {
        if let Some((_, mut position)) = self.active_positions.remove(position_id) {
            let price_change = (exit_price - position.entry_price) / position.entry_price;
            position.realized_pnl = position.size_sol * price_change;
            position.current_price = exit_price;

            self.closed_positions.insert(position_id.to_string(), position.clone());

            info!("Closed position: {} with PnL: {:.4} SOL", position_id, position.realized_pnl);
            Ok(position)
        } else {
            Err(anyhow!("Position not found: {}", position_id))
        }
    }

    pub async fn update_position_price(&self, position_id: &str, current_price: f64) -> Result<()> {
        if let Some(mut position) = self.active_positions.get_mut(position_id) {
            position.current_price = current_price;
            
            let price_change = (current_price - position.entry_price) / position.entry_price;
            position.unrealized_pnl = position.size_sol * price_change;

            Ok(())
        } else {
            Err(anyhow!("Position not found: {}", position_id))
        }
    }

    pub async fn update_trailing_stop(&self, position_id: &str, trailing_stop: TrailingStop) -> Result<()> {
        if let Some(mut position) = self.active_positions.get_mut(position_id) {
            position.trailing_stop = Some(trailing_stop);
            Ok(())
        } else {
            Err(anyhow!("Position not found: {}", position_id))
        }
    }

    pub async fn get_all_positions(&self) -> Vec<Position> {
        self.active_positions.iter().map(|entry| entry.value().clone()).collect()
    }

    pub async fn get_active_position_count(&self) -> u32 {
        self.active_positions.len() as u32
    }

    pub async fn get_position(&self, position_id: &str) -> Option<Position> {
        self.active_positions.get(position_id).map(|entry| entry.value().clone())
    }
}

#[derive(Debug, Clone)]
pub struct TradeRiskAssessment {
    pub approved: bool,
    pub risk_score: f64,
    pub warnings: Vec<String>,
    pub rejection_reason: Option<String>,
    pub recommended_size: Option<f64>,
}

impl TradeRiskAssessment {
    pub fn approved() -> Self {
        Self {
            approved: true,
            risk_score: 0.2,
            warnings: vec![],
            rejection_reason: None,
            recommended_size: None,
        }
    }

    pub fn approved_with_warning(warning: String) -> Self {
        Self {
            approved: true,
            risk_score: 0.5,
            warnings: vec![warning],
            rejection_reason: None,
            recommended_size: None,
        }
    }

    pub fn rejected(reason: String) -> Self {
        Self {
            approved: false,
            risk_score: 1.0,
            warnings: vec![],
            rejection_reason: Some(reason),
            recommended_size: None,
        }
    }
}

pub async fn evaluate_trade_risk(
    token_mint: Pubkey,
    trade_size_sol: f64,
    expected_slippage: f64,
    gas_fee: f64,
    position_type: PositionType,
) -> Result<TradeRiskAssessment> {
    RISK_MANAGER.evaluate_trade_risk(token_mint, trade_size_sol, expected_slippage, gas_fee, position_type).await
}

pub async fn open_position(
    token_mint: Pubkey,
    entry_price: f64,
    size_sol: f64,
    size_tokens: f64,
    position_type: PositionType,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
) -> Result<String> {
    POSITION_MANAGER.open_position(token_mint, entry_price, size_sol, size_tokens, position_type, stop_loss, take_profit).await
}

pub async fn close_position(position_id: &str, exit_price: f64) -> Result<Position> {
    POSITION_MANAGER.close_position(position_id, exit_price).await
}

pub async fn get_risk_metrics() -> Result<RiskMetrics> {
    RISK_MANAGER.get_risk_metrics().await
}

pub async fn start_risk_management() -> Result<()> {
    RISK_MANAGER.start_monitoring().await
}

pub async fn update_risk_limits(new_limits: RiskLimits) {
    RISK_MANAGER.update_risk_limits(new_limits).await
}
