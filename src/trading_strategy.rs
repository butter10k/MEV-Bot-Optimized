use crate::config::SniperConfig;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info};

#[derive(Debug, Clone)]
pub struct Position {
    pub token_address: String,
    pub entry_price: f64,
    pub entry_time: std::time::Instant,
    pub amount_sol: f64,
    pub token_amount: f64,
    pub current_price: f64,
    pub pnl_percentage: f64,
    pub stop_loss_price: f64,
    pub take_profit_price: f64,
    pub status: PositionStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PositionStatus {
    Open,
    Closed,
    PendingClose,
}

pub struct TradingStrategy {
    config: Arc<SniperConfig>,
    positions: Arc<RwLock<HashMap<String, Position>>>,
}

impl TradingStrategy {
    pub fn new(config: Arc<SniperConfig>) -> Self {
        Self {
            config,
            positions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn should_buy_token(&self, liquidity: f64, holders: u32, market_cap: f64) -> Result<bool> {
        if liquidity < self.config.min_liquidity_sol || liquidity > self.config.max_liquidity_sol {
            return Ok(false);
        }
        
        if holders < self.config.min_holder_count || holders > self.config.max_holder_count {
            return Ok(false);
        }
        
        if market_cap < self.config.min_market_cap_sol || market_cap > self.config.max_market_cap_sol {
            return Ok(false);
        }
        
        Ok(true)
    }

    pub async fn open_position(&self, token_address: String, amount_sol: f64, entry_price: f64) -> Result<()> {
        let mut positions = self.positions.write().await;
        
        let open_count = positions.values().filter(|p| p.status == PositionStatus::Open).count();
        if open_count >= self.config.max_concurrent_buys as usize {
            return Err(anyhow!("Maximum concurrent positions reached"));
        }

        let position = Position {
            token_address: token_address.clone(),
            entry_price,
            entry_time: std::time::Instant::now(),
            amount_sol,
            token_amount: amount_sol / entry_price,
            current_price: entry_price,
            pnl_percentage: 0.0,
            stop_loss_price: entry_price * (1.0 - self.config.stop_loss_percentage / 100.0),
            take_profit_price: entry_price * (1.0 + self.config.take_profit_percentage / 100.0),
            status: PositionStatus::Open,
        };

        positions.insert(token_address, position);
        info!("Opened position with {} SOL", amount_sol);
        
        Ok(())
    }

    pub async fn check_sell_signals(&self) -> Result<Vec<String>> {
        let mut positions = self.positions.write().await;
        let mut tokens_to_sell = Vec::new();

        for (token_address, position) in positions.iter_mut() {
            if position.status != PositionStatus::Open {
                continue;
            }

            if self.should_sell_position(position).await? {
                position.status = PositionStatus::PendingClose;
                tokens_to_sell.push(token_address.clone());
            }
        }

        Ok(tokens_to_sell)
    }

    async fn should_sell_position(&self, position: &Position) -> Result<bool> {
        match &self.config.sell_strategy {
            crate::config::SellStrategy::TakeProfit => {
                Ok(position.current_price >= position.take_profit_price)
            }
            crate::config::SellStrategy::StopLoss => {
                Ok(position.current_price <= position.stop_loss_price)
            }
            crate::config::SellStrategy::TrailingStop => {
                if self.config.trailing_stop {
                    let trailing_stop = position.current_price * (1.0 - self.config.trailing_stop_percentage / 100.0);
                    Ok(position.entry_price > trailing_stop)
                } else {
                    Ok(false)
                }
            }
            crate::config::SellStrategy::TimeBased { hours } => {
                let elapsed = position.entry_time.elapsed();
                Ok(elapsed.as_secs() >= hours * 3600)
            }
            _ => Ok(false),
        }
    }

    pub async fn update_position_price(&self, token_address: &str, new_price: f64) -> Result<()> {
        let mut positions = self.positions.write().await;
        
        if let Some(position) = positions.get_mut(token_address) {
            position.current_price = new_price;
            position.pnl_percentage = ((new_price - position.entry_price) / position.entry_price) * 100.0;
        }

        Ok(())
    }

    pub async fn get_open_positions(&self) -> Vec<Position> {
        let positions = self.positions.read().await;
        positions
            .values()
            .filter(|p| p.status == PositionStatus::Open)
            .cloned()
            .collect()
    }
}
