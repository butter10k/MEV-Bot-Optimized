use std::collections::VecDeque;
use anyhow::Result;

pub struct TechnicalAnalysis {
    #[allow(dead_code)]
    rsi_period: usize,
    #[allow(dead_code)]
    volume_period: usize,
    #[allow(dead_code)]
    price_history: VecDeque<PricePoint>,
    #[allow(dead_code)]
    volume_history: VecDeque<VolumePoint>,
}

#[derive(Debug, Clone)]
pub struct PricePoint {
    #[allow(dead_code)]
    pub timestamp: std::time::Instant,
    #[allow(dead_code)]
    pub price: f64,
}

#[derive(Debug, Clone)]
pub struct VolumePoint {
    #[allow(dead_code)]
    pub timestamp: std::time::Instant,
    #[allow(dead_code)]
    pub volume: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TechnicalIndicators {
    pub rsi: f64,
    pub volume_sma: f64,
    pub price_sma: f64,
    pub price_change_24h: f64,
    pub volume_change_24h: f64,
    pub volatility: f64,
}

impl TechnicalAnalysis {
    pub fn new(rsi_period: usize, volume_period: usize) -> Self {
        Self {
            rsi_period,
            volume_period,
            price_history: VecDeque::new(),
            volume_history: VecDeque::new(),
        }
    }

    #[allow(dead_code)]
    pub fn add_price_point(&mut self, price: f64) {
        let now = std::time::Instant::now();
        self.price_history.push_back(PricePoint { timestamp: now, price });
        
        if self.price_history.len() > self.rsi_period * 2 {
            self.price_history.pop_front();
        }
    }

    #[allow(dead_code)]
    pub fn add_volume_point(&mut self, volume: f64) {
        let now = std::time::Instant::now();
        self.volume_history.push_back(VolumePoint { timestamp: now, volume });
        
        if self.volume_history.len() > self.volume_period * 2 {
            self.volume_history.pop_front();
        }
    }

    #[allow(dead_code)]
    pub fn calculate_indicators(&self) -> Result<TechnicalIndicators> {
        let rsi = self.calculate_rsi()?;
        let volume_sma = self.calculate_volume_sma()?;
        let price_sma = self.calculate_price_sma()?;
        let price_change_24h = self.calculate_price_change_24h()?;
        let volume_change_24h = self.calculate_volume_change_24h()?;
        let volatility = self.calculate_volatility()?;

        Ok(TechnicalIndicators {
            rsi,
            volume_sma,
            price_sma,
            price_change_24h,
            volume_change_24h,
            volatility,
        })
    }

    #[allow(dead_code)]
    fn calculate_rsi(&self) -> Result<f64> {
        if self.price_history.len() < self.rsi_period + 1 {
            return Ok(50.0);
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in 1..=self.rsi_period {
            let current_price = self.price_history[self.price_history.len() - i].price;
            let previous_price = self.price_history[self.price_history.len() - i - 1].price;
            
            let change = current_price - previous_price;
            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let avg_gain = gains / self.rsi_period as f64;
        let avg_loss = losses / self.rsi_period as f64;

        if avg_loss == 0.0 {
            return Ok(100.0);
        }

        let rs = avg_gain / avg_loss;
        let rsi = 100.0 - (100.0 / (1.0 + rs));

        Ok(rsi)
    }

    #[allow(dead_code)]
    fn calculate_volume_sma(&self) -> Result<f64> {
        if self.volume_history.is_empty() {
            return Ok(0.0);
        }

        let sum: f64 = self.volume_history
            .iter()
            .take(self.volume_period.min(self.volume_history.len()))
            .map(|v| v.volume)
            .sum();

        Ok(sum / self.volume_period.min(self.volume_history.len()) as f64)
    }

    #[allow(dead_code)]
    fn calculate_price_sma(&self) -> Result<f64> {
        if self.price_history.is_empty() {
            return Ok(0.0);
        }

        let sum: f64 = self.price_history
            .iter()
            .take(self.rsi_period.min(self.price_history.len()))
            .map(|p| p.price)
            .sum();

        Ok(sum / self.rsi_period.min(self.price_history.len()) as f64)
    }

    #[allow(dead_code)]
    fn calculate_price_change_24h(&self) -> Result<f64> {
        if self.price_history.len() < 2 {
            return Ok(0.0);
        }

        let current_price = self.price_history.back().unwrap().price;
        let oldest_price = self.price_history.front().unwrap().price;
        
        Ok(((current_price - oldest_price) / oldest_price) * 100.0)
    }

    #[allow(dead_code)]
    fn calculate_volume_change_24h(&self) -> Result<f64> {
        if self.volume_history.len() < 2 {
            return Ok(0.0);
        }

        let current_volume = self.volume_history.back().unwrap().volume;
        let oldest_volume = self.volume_history.front().unwrap().volume;
        
        if oldest_volume == 0.0 {
            return Ok(0.0);
        }
        
        Ok(((current_volume - oldest_volume) / oldest_volume) * 100.0)
    }

    #[allow(dead_code)]
    fn calculate_volatility(&self) -> Result<f64> {
        if self.price_history.len() < 2 {
            return Ok(0.0);
        }

        let prices: Vec<f64> = self.price_history.iter().map(|p| p.price).collect();
        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        
        let variance: f64 = prices.iter()
            .map(|p| (p - mean).powi(2))
            .sum::<f64>() / prices.len() as f64;
        
        Ok(variance.sqrt())
    }

    #[allow(dead_code)]
    pub fn is_oversold(&self, threshold: f64) -> Result<bool> {
        let rsi = self.calculate_rsi()?;
        Ok(rsi < threshold)
    }

    #[allow(dead_code)]
    pub fn is_overbought(&self, threshold: f64) -> Result<bool> {
        let rsi = self.calculate_rsi()?;
        Ok(rsi > threshold)
    }

    #[allow(dead_code)]
    pub fn has_volume_spike(&self, multiplier: f64) -> Result<bool> {
        let current_volume = self.volume_history.back().map(|v| v.volume).unwrap_or(0.0);
        let avg_volume = self.calculate_volume_sma()?;
        
        if avg_volume == 0.0 {
            return Ok(false);
        }
        
        Ok(current_volume > avg_volume * multiplier)
    }

    #[allow(dead_code)]
    pub fn get_price_trend(&self) -> Result<PriceTrend> {
        if self.price_history.len() < 3 {
            return Ok(PriceTrend::Neutral);
        }

        let recent_prices: Vec<f64> = self.price_history
            .iter()
            .rev()
            .take(3)
            .map(|p| p.price)
            .collect();

        if recent_prices[0] > recent_prices[1] && recent_prices[1] > recent_prices[2] {
            Ok(PriceTrend::Uptrend)
        } else if recent_prices[0] < recent_prices[1] && recent_prices[1] < recent_prices[2] {
            Ok(PriceTrend::Downtrend)
        } else {
            Ok(PriceTrend::Sideways)
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum PriceTrend {
    Uptrend,
    Downtrend,
    Sideways,
    Neutral,
}

impl PriceTrend {
    #[allow(dead_code)]
    pub fn to_string(&self) -> &'static str {
        match self {
            PriceTrend::Uptrend => "Uptrend",
            PriceTrend::Downtrend => "Downtrend",
            PriceTrend::Sideways => "Sideways",
            PriceTrend::Neutral => "Neutral",
        }
    }
}
