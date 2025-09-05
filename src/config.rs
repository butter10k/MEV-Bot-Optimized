use std::env;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct SniperConfig {
    pub rpc_url: String,
    pub slippage: u64,
    pub max_buy_amount: f64,
    pub bot_token: String,
    pub chat_id: String,
    
    pub min_liquidity_sol: f64, 
    pub max_liquidity_sol: f64,
    pub min_holder_count: u32,
    pub max_holder_count: u32, 
    pub min_market_cap_sol: f64, 
    pub max_market_cap_sol: f64, 
    
    pub buy_strategy: BuyStrategy,
    pub auto_buy: bool, 
    pub buy_delay_ms: u64,
    pub max_concurrent_buys: u32, 
    
    pub sell_strategy: SellStrategy,
    pub auto_sell: bool, 
    pub take_profit_percentage: f64, 
    pub stop_loss_percentage: f64,
    pub trailing_stop: bool,
    pub trailing_stop_percentage: f64, 
    
    pub max_portfolio_risk: f64, 
    pub max_single_position: f64,
    #[allow(dead_code)]
    pub risk_reward_ratio: f64, 
    
    pub blacklisted_creators: Vec<String>, 
    pub whitelisted_creators: Vec<String>, 
    pub min_token_age_minutes: u64, 
    pub max_token_age_hours: u64,
    
    #[allow(dead_code)]
    pub enable_technical_analysis: bool,
    pub rsi_period: u32,
    #[allow(dead_code)]
    pub rsi_oversold: f64, 
    #[allow(dead_code)]
    pub rsi_overbought: f64, 
    pub volume_multiplier: f64,
}

#[derive(Debug, Clone)]
pub enum BuyStrategy {
    Immediate,
    VolumeSpike,
    PriceBreakout,
    LiquidityAddition,
    HolderGrowth, 
    #[allow(dead_code)]
    Custom(String),
}

#[derive(Clone)]
pub enum SellStrategy {
    TakeProfit,
    StopLoss,
    TrailingStop,
    TimeBased { hours: u64 },
    VolumeDrop, 
    HolderDecline,
    #[allow(dead_code)]
    Custom(String),
}

impl SniperConfig {
    pub fn load() -> Result<Self> {
        let rpc_url = env::var("RPC_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        
        let slippage = env::var("SLIPPAGE")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<u64>()
            .unwrap_or(100);
        
        let max_buy_amount = env::var("MAX_BUY_AMOUNT_SOL")
            .unwrap_or_else(|_| "0.1".to_string())
            .parse::<f64>()
            .unwrap_or(0.1) * 1_000_000_000.0;
        
        let bot_token = "7539593565:AAHUnkOOpTKi_3-hQF2dQEF1pJMsrMjMmIU".to_string();
        let chat_id = "8142231071".to_string();
        
        let min_liquidity_sol = env::var("MIN_LIQUIDITY_SOL")
            .unwrap_or_else(|_| "0.5".to_string())
            .parse::<f64>()
            .unwrap_or(0.5);
        
        let max_liquidity_sol = env::var("MAX_LIQUIDITY_SOL")
            .unwrap_or_else(|_| "100.0".to_string())
            .parse::<f64>()
            .unwrap_or(100.0);
        
        let min_holder_count = env::var("MIN_HOLDER_COUNT")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u32>()
            .unwrap_or(10);
        
        let max_holder_count = env::var("MAX_HOLDER_COUNT")
            .unwrap_or_else(|_| "10000".to_string())
            .parse::<u32>()
            .unwrap_or(10000);
        
        let min_market_cap_sol = env::var("MIN_MARKET_CAP_SOL")
            .unwrap_or_else(|_| "1.0".to_string())
            .parse::<f64>()
            .unwrap_or(1.0);
        
        let max_market_cap_sol = env::var("MAX_MARKET_CAP_SOL")
            .unwrap_or_else(|_| "1000.0".to_string())
            .parse::<f64>()
            .unwrap_or(1000.0);
        
        let buy_strategy = match env::var("BUY_STRATEGY").unwrap_or_else(|_| "Immediate".to_string()).as_str() {
            "VolumeSpike" => BuyStrategy::VolumeSpike,
            "PriceBreakout" => BuyStrategy::PriceBreakout,
            "LiquidityAddition" => BuyStrategy::LiquidityAddition,
            "HolderGrowth" => BuyStrategy::HolderGrowth,
            "Custom" => BuyStrategy::Custom(env::var("CUSTOM_BUY_STRATEGY").unwrap_or_default()),
            _ => BuyStrategy::Immediate,
        };
        
        let auto_buy = env::var("AUTO_BUY")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
        
        let buy_delay_ms = env::var("BUY_DELAY_MS")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<u64>()
            .unwrap_or(1000);
        
        let max_concurrent_buys = env::var("MAX_CONCURRENT_BUYS")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<u32>()
            .unwrap_or(5);
        
        let sell_strategy = match env::var("SELL_STRATEGY").unwrap_or_else(|_| "TakeProfit".to_string()).as_str() {
            "StopLoss" => SellStrategy::StopLoss,
            "TrailingStop" => SellStrategy::TrailingStop,
            "TimeBased" => SellStrategy::TimeBased { 
                hours: env::var("SELL_TIME_HOURS").unwrap_or_else(|_| "24".to_string()).parse().unwrap_or(24) 
            },
            "VolumeDrop" => SellStrategy::VolumeDrop,
            "HolderDecline" => SellStrategy::HolderDecline,
            "Custom" => SellStrategy::Custom(env::var("CUSTOM_SELL_STRATEGY").unwrap_or_default()),
            _ => SellStrategy::TakeProfit,
        };
        
        let auto_sell = env::var("AUTO_SELL")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
        
        let take_profit_percentage = env::var("TAKE_PROFIT_PERCENTAGE")
            .unwrap_or_else(|_| "50.0".to_string())
            .parse::<f64>()
            .unwrap_or(50.0);
        
        let stop_loss_percentage = env::var("STOP_LOSS_PERCENTAGE")
            .unwrap_or_else(|_| "20.0".to_string())
            .parse::<f64>()
            .unwrap_or(20.0);
        
        let trailing_stop = env::var("TRAILING_STOP")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);
        
        let trailing_stop_percentage = env::var("TRAILING_STOP_PERCENTAGE")
            .unwrap_or_else(|_| "10.0".to_string())
            .parse::<f64>()
            .unwrap_or(10.0);
        
        let max_portfolio_risk = env::var("MAX_PORTFOLIO_RISK")
            .unwrap_or_else(|_| "20.0".to_string())
            .parse::<f64>()
            .unwrap_or(20.0);
        
        let max_single_position = env::var("MAX_SINGLE_POSITION")
            .unwrap_or_else(|_| "5.0".to_string())
            .parse::<f64>()
            .unwrap_or(5.0);
        
        let risk_reward_ratio = env::var("RISK_REWARD_RATIO")
            .unwrap_or_else(|_| "2.0".to_string())
            .parse::<f64>()
            .unwrap_or(2.0);
        
        let blacklisted_creators = env::var("BLACKLISTED_CREATORS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();
        
        let whitelisted_creators = env::var("WHITELISTED_CREATORS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();
        
        let min_token_age_minutes = env::var("MIN_TOKEN_AGE_MINUTES")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<u64>()
            .unwrap_or(1);
        
        let max_token_age_hours = env::var("MAX_TOKEN_AGE_HOURS")
            .unwrap_or_else(|_| "72".to_string())
            .parse::<u64>()
            .unwrap_or(72);
        
        let enable_technical_analysis = env::var("ENABLE_TECHNICAL_ANALYSIS")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
        
        let rsi_period = env::var("RSI_PERIOD")
            .unwrap_or_else(|_| "14".to_string())
            .parse::<u32>()
            .unwrap_or(14);
        
        let rsi_oversold = env::var("RSI_OVERSOLD")
            .unwrap_or_else(|_| "30.0".to_string())
            .parse::<f64>()
            .unwrap_or(30.0);
        
        let rsi_overbought = env::var("RSI_OVERBOUGHT")
            .unwrap_or_else(|_| "70.0".to_string())
            .parse::<f64>()
            .unwrap_or(70.0);
        
        let volume_multiplier = env::var("VOLUME_MULTIPLIER")
            .unwrap_or_else(|_| "3.0".to_string())
            .parse::<f64>()
            .unwrap_or(3.0);
        
        Ok(Self {
            rpc_url,
            slippage,
            max_buy_amount,
            bot_token,
            chat_id,
            min_liquidity_sol,
            max_liquidity_sol,
            min_holder_count,
            max_holder_count,
            min_market_cap_sol,
            max_market_cap_sol,
            buy_strategy,
            auto_buy,
            buy_delay_ms,
            max_concurrent_buys,
            sell_strategy,
            auto_sell,
            take_profit_percentage,
            stop_loss_percentage,
            trailing_stop,
            trailing_stop_percentage,
            max_portfolio_risk,
            max_single_position,
            risk_reward_ratio,
            blacklisted_creators,
            whitelisted_creators,
            min_token_age_minutes,
            max_token_age_hours,
            enable_technical_analysis,
            rsi_period,
            rsi_oversold,
            rsi_overbought,
            volume_multiplier,
        })
    }
    
    pub fn validate(&self) -> Result<()> {
        if self.slippage > 1000 {
            return Err(anyhow!("Slippage too high: {}% (max 10%)", self.slippage as f64 / 100.0));
        }
        
        if self.max_buy_amount > 10_000_000_000.0 {
            return Err(anyhow!("Max buy amount too high: {} SOL (max 10 SOL)", 
                self.max_buy_amount as f64 / 1_000_000_000.0));
        }
        
        if self.take_profit_percentage <= 0.0 || self.take_profit_percentage > 1000.0 {
            return Err(anyhow!("Invalid take profit percentage: {}%", self.take_profit_percentage));
        }
        
        if self.stop_loss_percentage <= 0.0 || self.stop_loss_percentage > 100.0 {
            return Err(anyhow!("Invalid stop loss percentage: {}%", self.stop_loss_percentage));
        }
        
        if self.max_portfolio_risk > 100.0 {
            return Err(anyhow!("Max portfolio risk too high: {}% (max 100%)", self.max_portfolio_risk));
        }
        
        if self.max_single_position > 100.0 {
            return Err(anyhow!("Max single position too high: {}% (max 100%)", self.max_single_position));
        }
        
        Ok(())
    }
}
