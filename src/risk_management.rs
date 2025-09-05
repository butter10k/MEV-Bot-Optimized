use anyhow::{Result, anyhow};
use crate::trading_strategy::Position;
use crate::config::SniperConfig;

pub struct RiskManager {
    config: SniperConfig,
    total_portfolio_value: f64,
}

impl RiskManager {
    pub fn new(config: SniperConfig, total_portfolio_value: f64) -> Self {
        
        Self {
            config,
            total_portfolio_value,
        }
    }

    pub fn can_open_position(&self, current_positions: &[Position], new_position_value: f64) -> Result<bool> {
        let total_positions_value: f64 = current_positions.iter().map(|p| p.amount_sol).sum();
        let new_total = total_positions_value + new_position_value;
        
        let max_allowed = self.total_portfolio_value * (self.config.max_portfolio_risk / 100.0);
        
        Ok(new_total <= max_allowed)
    }

    pub fn get_portfolio_risk_percentage(&self, current_positions: &[Position]) -> f64 {
        let total_positions_value: f64 = current_positions.iter().map(|p| p.amount_sol).sum();
        (total_positions_value / self.total_portfolio_value) * 100.0
    }

    pub fn calculate_dynamic_stop_loss(&self, entry_price: f64, _current_price: f64, volatility: f64) -> f64 {
        let base_stop_loss = entry_price * (1.0 - self.config.stop_loss_percentage / 100.0);

        let volatility_adjustment = volatility * 0.1;
        let adjusted_stop_loss = base_stop_loss - volatility_adjustment;
        
        let min_stop_loss = entry_price * 0.5; 
        
        adjusted_stop_loss.max(min_stop_loss)
    }

    pub fn calculate_trailing_stop(&self, entry_price: f64, current_price: f64, highest_price: f64) -> f64 {
        if current_price <= entry_price {
            return entry_price * (1.0 - self.config.stop_loss_percentage / 100.0);
        }
        
        let trailing_percentage = self.config.trailing_stop_percentage / 100.0;
        highest_price * (1.0 - trailing_percentage)
    }

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

impl RiskMetrics {
    pub fn is_low_risk(&self) -> bool {
        self.portfolio_risk_percentage <= 50.0
    }
}
