use anyhow::{Result, anyhow};
use crate::trading_strategy::Position;
use crate::config::SniperConfig;

pub struct RiskManager {
    config: SniperConfig,
    total_portfolio_value: f64,
    #[allow(dead_code)]
    max_risk_per_trade: f64,
}

impl RiskManager {
    pub fn new(config: SniperConfig, total_portfolio_value: f64) -> Self {
        let max_risk_per_trade = total_portfolio_value * (config.max_single_position / 100.0);
        
        Self {
            config,
            total_portfolio_value,
            max_risk_per_trade,
        }
    }

    #[allow(dead_code)]
    pub fn calculate_position_size(&self, token_price: f64, stop_loss_price: f64) -> Result<f64> {
        let risk_per_share = (token_price - stop_loss_price).abs();
        
        if risk_per_share == 0.0 {
            return Err(anyhow!("Invalid stop loss price"));
        }

        let max_risk_amount = self.max_risk_per_trade;
        let position_size = max_risk_amount / risk_per_share;
        
        let max_position_sol = self.config.max_buy_amount as f64 / 1_000_000_000.0;
        let position_size_sol = position_size * token_price;
        
        if position_size_sol > max_position_sol {
            return Ok(max_position_sol);
        }
        
        Ok(position_size_sol)
    }

    pub fn can_open_position(&self, current_positions: &[Position], new_position_value: f64) -> Result<bool> {
        let total_positions_value: f64 = current_positions.iter().map(|p| p.amount_sol).sum();
        let new_total = total_positions_value + new_position_value;
        
        let max_allowed = self.total_portfolio_value * (self.config.max_portfolio_risk / 100.0);
        
        Ok(new_total <= max_allowed)
    }

    #[allow(dead_code)]
    pub fn get_portfolio_risk_percentage(&self, current_positions: &[Position]) -> f64 {
        let total_positions_value: f64 = current_positions.iter().map(|p| p.amount_sol).sum();
        (total_positions_value / self.total_portfolio_value) * 100.0
    }

    #[allow(dead_code)]
    pub fn should_close_for_risk(&self, position: &Position, current_price: f64) -> bool {
        if current_price <= position.stop_loss_price {
            return true;
        }
        
        if current_price >= position.take_profit_price {
            return true;
        }
        
        let max_loss_percentage = self.config.stop_loss_percentage;
        let current_loss = ((current_price - position.entry_price) / position.entry_price) * 100.0;
        
        if current_loss <= -max_loss_percentage {
            return true;
        }
        
        false
    }

    #[allow(dead_code)]
    pub fn calculate_dynamic_stop_loss(&self, entry_price: f64, _current_price: f64, volatility: f64) -> f64 {
        let base_stop_loss = entry_price * (1.0 - self.config.stop_loss_percentage / 100.0);

        let volatility_adjustment = volatility * 0.1;
        let adjusted_stop_loss = base_stop_loss - volatility_adjustment;
        
        let min_stop_loss = entry_price * 0.5; 
        
        adjusted_stop_loss.max(min_stop_loss)
    }

    #[allow(dead_code)]
    pub fn calculate_trailing_stop(&self, entry_price: f64, current_price: f64, highest_price: f64) -> f64 {
        if current_price <= entry_price {
            return entry_price * (1.0 - self.config.stop_loss_percentage / 100.0);
        }
        
        let trailing_percentage = self.config.trailing_stop_percentage / 100.0;
        highest_price * (1.0 - trailing_percentage)
    }

    #[allow(dead_code)]
    pub fn validate_risk_parameters(&self) -> Result<()> {
        if self.config.max_portfolio_risk > 100.0 {
            return Err(anyhow!("Max portfolio risk cannot exceed 100%"));
        }
        
        if self.config.max_single_position > 100.0 {
            return Err(anyhow!("Max single position cannot exceed 100%"));
        }
        
        if self.config.stop_loss_percentage > 50.0 {
            return Err(anyhow!("Stop loss percentage too high (max 50%)"));
        }
        
        if self.config.take_profit_percentage < 1.0 {
            return Err(anyhow!("Take profit percentage too low (min 1%)"));
        }
        
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_risk_metrics(&self, current_positions: &[Position]) -> RiskMetrics {
        let portfolio_risk = self.get_portfolio_risk_percentage(current_positions);
        let open_positions = current_positions.iter().filter(|p| p.status == crate::trading_strategy::PositionStatus::Open).count();
        let total_positions_value: f64 = current_positions.iter().map(|p| p.amount_sol).sum();
        
        let avg_position_size = if open_positions > 0 {
            total_positions_value / open_positions as f64
        } else {
            0.0
        };
        
        RiskMetrics {
            portfolio_risk_percentage: portfolio_risk,
            open_positions_count: open_positions,
            total_positions_value,
            average_position_size: avg_position_size,
            max_allowed_risk: self.total_portfolio_value * (self.config.max_portfolio_risk / 100.0),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RiskMetrics {
    pub portfolio_risk_percentage: f64,
    pub open_positions_count: usize,
    pub total_positions_value: f64,
    pub average_position_size: f64,
    pub max_allowed_risk: f64,
}

impl RiskMetrics {
    pub fn is_high_risk(&self) -> bool {
        self.portfolio_risk_percentage > 80.0
    }
    
    pub fn is_medium_risk(&self) -> bool {
        self.portfolio_risk_percentage > 50.0 && self.portfolio_risk_percentage <= 80.0
    }
    
    #[allow(dead_code)]
    pub fn is_low_risk(&self) -> bool {
        self.portfolio_risk_percentage <= 50.0
    }
}
