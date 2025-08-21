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

// Global AI sentiment system
static SENTIMENT_ANALYZER: LazyLock<Arc<SentimentAnalyzer>> = 
    LazyLock::new(|| Arc::new(SentimentAnalyzer::new()));

static MARKET_PREDICTOR: LazyLock<Arc<MarketPredictor>> = 
    LazyLock::new(|| Arc::new(MarketPredictor::new()));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSentiment {
    pub token_mint: Pubkey,
    pub overall_sentiment: SentimentScore,
    pub confidence: f64,
    pub bullish_signals: Vec<BullishSignal>,
    pub bearish_signals: Vec<BearishSignal>,
    pub social_sentiment: SocialSentiment,
    pub technical_sentiment: TechnicalSentiment,
    pub on_chain_sentiment: OnChainSentiment,
    pub prediction_horizon: Duration,
    pub last_updated: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SentimentScore {
    VeryBearish,  // -100 to -60
    Bearish,      // -60 to -20
    Neutral,      // -20 to +20
    Bullish,      // +20 to +60
    VeryBullish,  // +60 to +100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BullishSignal {
    pub signal_type: SignalType,
    pub strength: f64,
    pub description: String,
    pub detected_at: Instant,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BearishSignal {
    pub signal_type: SignalType,
    pub strength: f64,
    pub description: String,
    pub detected_at: Instant,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalType {
    // Social signals
    TwitterMention,
    RedditDiscussion,
    TelegramActivity,
    InfluencerEndorsement,
    
    // Technical signals
    VolumeSpike,
    PriceBreakout,
    LiquidityIncrease,
    WhaleActivity,
    
    // On-chain signals
    LargeTransfers,
    TokenBurns,
    NewHolders,
    DexListings,
    
    // Market structure
    ArbitrageActivity,
    CrossChainBridging,
    StakingActivity,
    GovernanceVoting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialSentiment {
    pub twitter_sentiment: f64,
    pub reddit_sentiment: f64,
    pub telegram_sentiment: f64,
    pub mention_volume: u64,
    pub engagement_rate: f64,
    pub influencer_mentions: u32,
    pub sentiment_velocity: f64, // Rate of change
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalSentiment {
    pub price_momentum: f64,
    pub volume_trend: f64,
    pub volatility_index: f64,
    pub support_resistance_strength: f64,
    pub trend_strength: TrendStrength,
    pub rsi_level: f64,
    pub moving_average_position: MovingAveragePosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendStrength {
    Weak,
    Moderate,
    Strong,
    VeryStrong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MovingAveragePosition {
    BelowAll,
    BetweenShortMed,
    BetweenMedLong,
    AboveAll,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainSentiment {
    pub holder_count_change: f64,
    pub whale_accumulation: f64,
    pub dex_trading_activity: f64,
    pub liquidity_changes: f64,
    pub smart_money_flow: f64,
    pub token_age_distribution: f64,
    pub supply_on_exchanges: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPrediction {
    pub token_mint: Pubkey,
    pub price_prediction_1h: PricePrediction,
    pub price_prediction_4h: PricePrediction,
    pub price_prediction_24h: PricePrediction,
    pub probability_distributions: Vec<PriceProbability>,
    pub key_factors: Vec<PredictionFactor>,
    pub confidence_score: f64,
    pub model_version: String,
    pub predicted_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePrediction {
    pub predicted_price: f64,
    pub price_change_percent: f64,
    pub confidence_interval: (f64, f64), // Lower and upper bounds
    pub probability_up: f64,
    pub probability_down: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceProbability {
    pub price_range: (f64, f64),
    pub probability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionFactor {
    pub factor_name: String,
    pub impact_weight: f64,
    pub current_value: f64,
    pub historical_correlation: f64,
}

pub struct SentimentAnalyzer {
    token_sentiments: Arc<DashMap<Pubkey, MarketSentiment>>,
    social_data_sources: Vec<SocialDataSource>,
    technical_analyzer: Arc<TechnicalAnalyzer>,
    on_chain_analyzer: Arc<OnChainAnalyzer>,
    sentiment_history: Arc<DashMap<Pubkey, Vec<SentimentPoint>>>,
    update_interval: Duration,
    analysis_counter: AtomicU64,
}

#[derive(Debug, Clone)]
struct SentimentPoint {
    timestamp: Instant,
    sentiment_score: f64,
    confidence: f64,
    volume: f64,
}

#[derive(Debug, Clone)]
struct SocialDataSource {
    platform: SocialPlatform,
    endpoint: String,
    api_key: Option<String>,
    rate_limit: Duration,
    enabled: bool,
}

#[derive(Debug, Clone)]
enum SocialPlatform {
    Twitter,
    Reddit,
    Telegram,
    Discord,
    YouTube,
}

impl SentimentAnalyzer {
    pub fn new() -> Self {
        Self {
            token_sentiments: Arc::new(DashMap::new()),
            social_data_sources: Self::initialize_social_sources(),
            technical_analyzer: Arc::new(TechnicalAnalyzer::new()),
            on_chain_analyzer: Arc::new(OnChainAnalyzer::new()),
            sentiment_history: Arc::new(DashMap::new()),
            update_interval: Duration::from_secs(60), // 1 minute updates
            analysis_counter: AtomicU64::new(0),
        }
    }

    fn initialize_social_sources() -> Vec<SocialDataSource> {
        vec![
            SocialDataSource {
                platform: SocialPlatform::Twitter,
                endpoint: "https://api.twitter.com/2/tweets/search/recent".to_string(),
                api_key: std::env::var("TWITTER_BEARER_TOKEN").ok(),
                rate_limit: Duration::from_secs(1),
                enabled: true,
            },
            SocialDataSource {
                platform: SocialPlatform::Reddit,
                endpoint: "https://www.reddit.com/r/solana/search.json".to_string(),
                api_key: None,
                rate_limit: Duration::from_secs(2),
                enabled: true,
            },
        ]
    }

    pub async fn analyze_token_sentiment(&self, token_mint: Pubkey) -> Result<MarketSentiment> {
        let analysis_id = self.analysis_counter.fetch_add(1, Ordering::SeqCst);
        debug!("Starting sentiment analysis {} for token: {}", analysis_id, token_mint);

        // Gather data from multiple sources in parallel
        let (social_sentiment, technical_sentiment, on_chain_sentiment) = tokio::join!(
            self.analyze_social_sentiment(token_mint),
            self.technical_analyzer.analyze_technical_sentiment(token_mint),
            self.on_chain_analyzer.analyze_on_chain_sentiment(token_mint)
        );

        let social_sentiment = social_sentiment?;
        let technical_sentiment = technical_sentiment?;
        let on_chain_sentiment = on_chain_sentiment?;

        // Combine signals
        let (bullish_signals, bearish_signals) = self.extract_signals(&social_sentiment, &technical_sentiment, &on_chain_sentiment).await;

        // Calculate overall sentiment
        let overall_sentiment = self.calculate_overall_sentiment(&bullish_signals, &bearish_signals).await;

        // Calculate confidence
        let confidence = self.calculate_confidence(&social_sentiment, &technical_sentiment, &on_chain_sentiment).await;

        let sentiment = MarketSentiment {
            token_mint,
            overall_sentiment,
            confidence,
            bullish_signals,
            bearish_signals,
            social_sentiment,
            technical_sentiment,
            on_chain_sentiment,
            prediction_horizon: Duration::from_hours(24),
            last_updated: Instant::now(),
        };

        // Store sentiment
        self.token_sentiments.insert(token_mint, sentiment.clone());

        // Update sentiment history
        self.update_sentiment_history(token_mint, &sentiment).await;

        info!("Completed sentiment analysis for {}: {:?} (confidence: {:.2})", 
              token_mint, sentiment.overall_sentiment, sentiment.confidence);

        Ok(sentiment)
    }

    async fn analyze_social_sentiment(&self, token_mint: Pubkey) -> Result<SocialSentiment> {
        let mut twitter_sentiment = 0.0;
        let mut reddit_sentiment = 0.0;
        let mut telegram_sentiment = 0.0;
        let mut mention_volume = 0;
        let mut engagement_rate = 0.0;
        let mut influencer_mentions = 0;

        // Search for token mentions across platforms
        for source in &self.social_data_sources {
            if !source.enabled {
                continue;
            }

            match source.platform {
                SocialPlatform::Twitter => {
                    if let Ok(data) = self.fetch_twitter_sentiment(token_mint, source).await {
                        twitter_sentiment = data.sentiment_score;
                        mention_volume += data.mention_count;
                        engagement_rate += data.engagement_rate;
                        influencer_mentions += data.influencer_mentions;
                    }
                }
                SocialPlatform::Reddit => {
                    if let Ok(data) = self.fetch_reddit_sentiment(token_mint, source).await {
                        reddit_sentiment = data.sentiment_score;
                        mention_volume += data.mention_count;
                    }
                }
                _ => {
                    // Other platforms would be implemented similarly
                }
            }
        }

        // Calculate sentiment velocity (rate of change)
        let sentiment_velocity = self.calculate_sentiment_velocity(token_mint).await;

        Ok(SocialSentiment {
            twitter_sentiment,
            reddit_sentiment,
            telegram_sentiment,
            mention_volume,
            engagement_rate: engagement_rate / self.social_data_sources.len() as f64,
            influencer_mentions,
            sentiment_velocity,
        })
    }

    async fn fetch_twitter_sentiment(&self, token_mint: Pubkey, source: &SocialDataSource) -> Result<SocialSentimentData> {
        // Simplified Twitter sentiment analysis
        // In reality, this would use the Twitter API and NLP models
        
        // Mock data for demonstration
        Ok(SocialSentimentData {
            sentiment_score: 0.3, // Slightly bullish
            mention_count: 45,
            engagement_rate: 0.12,
            influencer_mentions: 2,
        })
    }

    async fn fetch_reddit_sentiment(&self, token_mint: Pubkey, source: &SocialDataSource) -> Result<SocialSentimentData> {
        // Simplified Reddit sentiment analysis
        Ok(SocialSentimentData {
            sentiment_score: 0.1, // Slightly bullish
            mention_count: 23,
            engagement_rate: 0.08,
            influencer_mentions: 0,
        })
    }

    async fn calculate_sentiment_velocity(&self, token_mint: Pubkey) -> f64 {
        if let Some(history) = self.sentiment_history.get(&token_mint) {
            if history.len() >= 2 {
                let recent = &history[history.len() - 1];
                let previous = &history[history.len() - 2];
                
                let time_diff = recent.timestamp.duration_since(previous.timestamp).as_secs_f64();
                let sentiment_diff = recent.sentiment_score - previous.sentiment_score;
                
                return sentiment_diff / time_diff; // Sentiment change per second
            }
        }
        0.0
    }

    async fn extract_signals(
        &self,
        social: &SocialSentiment,
        technical: &TechnicalSentiment,
        on_chain: &OnChainSentiment,
    ) -> (Vec<BullishSignal>, Vec<BearishSignal>) {
        let mut bullish_signals = Vec::new();
        let mut bearish_signals = Vec::new();

        // Social signals
        if social.twitter_sentiment > 0.5 {
            bullish_signals.push(BullishSignal {
                signal_type: SignalType::TwitterMention,
                strength: social.twitter_sentiment,
                description: "Strong positive Twitter sentiment".to_string(),
                detected_at: Instant::now(),
                confidence: 0.7,
            });
        } else if social.twitter_sentiment < -0.5 {
            bearish_signals.push(BearishSignal {
                signal_type: SignalType::TwitterMention,
                strength: -social.twitter_sentiment,
                description: "Strong negative Twitter sentiment".to_string(),
                detected_at: Instant::now(),
                confidence: 0.7,
            });
        }

        // Technical signals
        if technical.volume_trend > 2.0 {
            bullish_signals.push(BullishSignal {
                signal_type: SignalType::VolumeSpike,
                strength: technical.volume_trend / 2.0,
                description: "Significant volume increase detected".to_string(),
                detected_at: Instant::now(),
                confidence: 0.8,
            });
        }

        if technical.price_momentum > 0.1 {
            bullish_signals.push(BullishSignal {
                signal_type: SignalType::PriceBreakout,
                strength: technical.price_momentum * 10.0,
                description: "Positive price momentum".to_string(),
                detected_at: Instant::now(),
                confidence: 0.6,
            });
        } else if technical.price_momentum < -0.1 {
            bearish_signals.push(BearishSignal {
                signal_type: SignalType::PriceBreakout,
                strength: -technical.price_momentum * 10.0,
                description: "Negative price momentum".to_string(),
                detected_at: Instant::now(),
                confidence: 0.6,
            });
        }

        // On-chain signals
        if on_chain.whale_accumulation > 0.3 {
            bullish_signals.push(BullishSignal {
                signal_type: SignalType::WhaleActivity,
                strength: on_chain.whale_accumulation,
                description: "Whale accumulation detected".to_string(),
                detected_at: Instant::now(),
                confidence: 0.9,
            });
        }

        if on_chain.holder_count_change > 0.2 {
            bullish_signals.push(BullishSignal {
                signal_type: SignalType::NewHolders,
                strength: on_chain.holder_count_change,
                description: "Increasing holder count".to_string(),
                detected_at: Instant::now(),
                confidence: 0.8,
            });
        }

        (bullish_signals, bearish_signals)
    }

    async fn calculate_overall_sentiment(&self, bullish: &[BullishSignal], bearish: &[BearishSignal]) -> SentimentScore {
        let bullish_score: f64 = bullish.iter()
            .map(|s| s.strength * s.confidence)
            .sum();
        
        let bearish_score: f64 = bearish.iter()
            .map(|s| s.strength * s.confidence)
            .sum();

        let net_sentiment = (bullish_score - bearish_score) * 100.0;

        match net_sentiment {
            s if s > 60.0 => SentimentScore::VeryBullish,
            s if s > 20.0 => SentimentScore::Bullish,
            s if s > -20.0 => SentimentScore::Neutral,
            s if s > -60.0 => SentimentScore::Bearish,
            _ => SentimentScore::VeryBearish,
        }
    }

    async fn calculate_confidence(&self, social: &SocialSentiment, technical: &TechnicalSentiment, on_chain: &OnChainSentiment) -> f64 {
        let mut confidence = 0.0;
        let mut weight_sum = 0.0;

        // Social confidence (weight: 0.3)
        if social.mention_volume > 10 {
            confidence += 0.3 * (social.mention_volume as f64 / 100.0).min(1.0);
            weight_sum += 0.3;
        }

        // Technical confidence (weight: 0.4)
        let technical_confidence = (technical.volume_trend.abs() + technical.price_momentum.abs()) / 2.0;
        confidence += 0.4 * technical_confidence.min(1.0);
        weight_sum += 0.4;

        // On-chain confidence (weight: 0.3)
        let on_chain_confidence = (on_chain.whale_accumulation.abs() + on_chain.smart_money_flow.abs()) / 2.0;
        confidence += 0.3 * on_chain_confidence.min(1.0);
        weight_sum += 0.3;

        if weight_sum > 0.0 {
            confidence / weight_sum
        } else {
            0.5 // Default neutral confidence
        }
    }

    async fn update_sentiment_history(&self, token_mint: Pubkey, sentiment: &MarketSentiment) {
        let sentiment_score = match sentiment.overall_sentiment {
            SentimentScore::VeryBearish => -80.0,
            SentimentScore::Bearish => -40.0,
            SentimentScore::Neutral => 0.0,
            SentimentScore::Bullish => 40.0,
            SentimentScore::VeryBullish => 80.0,
        };

        let sentiment_point = SentimentPoint {
            timestamp: Instant::now(),
            sentiment_score,
            confidence: sentiment.confidence,
            volume: sentiment.social_sentiment.mention_volume as f64,
        };

        let mut history = self.sentiment_history.entry(token_mint)
            .or_insert_with(|| Vec::with_capacity(100));

        history.push(sentiment_point);

        // Keep only last 100 points
        if history.len() > 100 {
            history.remove(0);
        }
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting AI sentiment analysis system");

        let analyzer = Arc::clone(&SENTIMENT_ANALYZER);
        let interval = self.update_interval;

        tokio::spawn(async move {
            let mut update_interval = tokio::time::interval(interval);
            
            loop {
                update_interval.tick().await;
                
                // Update sentiment for all monitored tokens
                analyzer.update_all_sentiments().await;
            }
        });

        Ok(())
    }

    async fn update_all_sentiments(&self) {
        // Get list of tokens to monitor (this would come from configuration)
        let monitored_tokens = self.get_monitored_tokens().await;
        
        for token in monitored_tokens {
            if let Err(e) = self.analyze_token_sentiment(token).await {
                warn!("Failed to analyze sentiment for token {}: {}", token, e);
            }
        }
    }

    async fn get_monitored_tokens(&self) -> Vec<Pubkey> {
        // This would typically come from a configuration or active trading list
        vec![]
    }
}

// Technical analysis component
pub struct TechnicalAnalyzer {
    price_data: Arc<DashMap<Pubkey, Vec<PricePoint>>>,
    indicators: Arc<DashMap<Pubkey, TechnicalIndicators>>,
}

#[derive(Debug, Clone)]
struct PricePoint {
    timestamp: Instant,
    price: f64,
    volume: f64,
}

#[derive(Debug, Clone)]
struct TechnicalIndicators {
    rsi: f64,
    moving_average_20: f64,
    moving_average_50: f64,
    bollinger_upper: f64,
    bollinger_lower: f64,
    volume_sma: f64,
}

impl TechnicalAnalyzer {
    pub fn new() -> Self {
        Self {
            price_data: Arc::new(DashMap::new()),
            indicators: Arc::new(DashMap::new()),
        }
    }

    pub async fn analyze_technical_sentiment(&self, token_mint: Pubkey) -> Result<TechnicalSentiment> {
        // Simplified technical analysis
        // In reality, this would use comprehensive technical indicators
        
        Ok(TechnicalSentiment {
            price_momentum: 0.05, // 5% momentum
            volume_trend: 1.2,    // 20% above average
            volatility_index: 0.3, // 30% volatility
            support_resistance_strength: 0.7,
            trend_strength: TrendStrength::Moderate,
            rsi_level: 65.0,
            moving_average_position: MovingAveragePosition::AboveAll,
        })
    }
}

// On-chain analysis component
pub struct OnChainAnalyzer {
    holder_data: Arc<DashMap<Pubkey, HolderMetrics>>,
    transaction_data: Arc<DashMap<Pubkey, TransactionMetrics>>,
}

#[derive(Debug, Clone)]
struct HolderMetrics {
    total_holders: u64,
    whale_holders: u64,
    holder_distribution: Vec<(u64, f64)>, // (balance_range, percentage)
    recent_holder_change: f64,
}

#[derive(Debug, Clone)]
struct TransactionMetrics {
    daily_volume: f64,
    unique_traders: u64,
    large_transactions: u64,
    average_transaction_size: f64,
}

impl OnChainAnalyzer {
    pub fn new() -> Self {
        Self {
            holder_data: Arc::new(DashMap::new()),
            transaction_data: Arc::new(DashMap::new()),
        }
    }

    pub async fn analyze_on_chain_sentiment(&self, token_mint: Pubkey) -> Result<OnChainSentiment> {
        // Simplified on-chain analysis
        Ok(OnChainSentiment {
            holder_count_change: 0.15,      // 15% increase
            whale_accumulation: 0.25,       // 25% whale accumulation
            dex_trading_activity: 1.8,      // 80% above average
            liquidity_changes: 0.1,         // 10% liquidity increase
            smart_money_flow: 0.3,          // Smart money flowing in
            token_age_distribution: 0.6,    // Good age distribution
            supply_on_exchanges: 0.4,       // 40% on exchanges
        })
    }
}

// Market prediction component
pub struct MarketPredictor {
    prediction_models: Vec<PredictionModel>,
    historical_accuracy: Arc<DashMap<String, f64>>,
}

#[derive(Debug, Clone)]
struct PredictionModel {
    model_name: String,
    model_type: ModelType,
    accuracy_score: f64,
    enabled: bool,
}

#[derive(Debug, Clone)]
enum ModelType {
    LinearRegression,
    RandomForest,
    NeuralNetwork,
    LSTM,
    Ensemble,
}

impl MarketPredictor {
    pub fn new() -> Self {
        Self {
            prediction_models: vec![
                PredictionModel {
                    model_name: "SentimentLSTM".to_string(),
                    model_type: ModelType::LSTM,
                    accuracy_score: 0.72,
                    enabled: true,
                },
                PredictionModel {
                    model_name: "TechnicalRF".to_string(),
                    model_type: ModelType::RandomForest,
                    accuracy_score: 0.68,
                    enabled: true,
                },
            ],
            historical_accuracy: Arc::new(DashMap::new()),
        }
    }

    pub async fn predict_price_movement(&self, token_mint: Pubkey, sentiment: &MarketSentiment) -> Result<MarketPrediction> {
        // Simplified prediction logic
        let current_price = 1.0; // Would fetch actual current price
        
        let sentiment_factor = match sentiment.overall_sentiment {
            SentimentScore::VeryBullish => 1.1,
            SentimentScore::Bullish => 1.05,
            SentimentScore::Neutral => 1.0,
            SentimentScore::Bearish => 0.95,
            SentimentScore::VeryBearish => 0.9,
        };

        let prediction_1h = PricePrediction {
            predicted_price: current_price * sentiment_factor,
            price_change_percent: (sentiment_factor - 1.0) * 100.0,
            confidence_interval: (current_price * 0.98, current_price * 1.02),
            probability_up: if sentiment_factor > 1.0 { 0.7 } else { 0.3 },
            probability_down: if sentiment_factor < 1.0 { 0.7 } else { 0.3 },
        };

        Ok(MarketPrediction {
            token_mint,
            price_prediction_1h: prediction_1h.clone(),
            price_prediction_4h: prediction_1h.clone(),
            price_prediction_24h: prediction_1h,
            probability_distributions: vec![],
            key_factors: vec![],
            confidence_score: sentiment.confidence,
            model_version: "v1.0".to_string(),
            predicted_at: Instant::now(),
        })
    }
}

#[derive(Debug, Clone)]
struct SocialSentimentData {
    sentiment_score: f64,
    mention_count: u64,
    engagement_rate: f64,
    influencer_mentions: u32,
}

// Global AI functions
pub async fn analyze_market_sentiment(token_mint: Pubkey) -> Result<MarketSentiment> {
    SENTIMENT_ANALYZER.analyze_token_sentiment(token_mint).await
}

pub async fn predict_price_movement(token_mint: Pubkey) -> Result<MarketPrediction> {
    let sentiment = analyze_market_sentiment(token_mint).await?;
    MARKET_PREDICTOR.predict_price_movement(token_mint, &sentiment).await
}

pub async fn start_ai_system() -> Result<()> {
    SENTIMENT_ANALYZER.start_monitoring().await?;
    info!("AI sentiment analysis and prediction system started");
    Ok(())
}

pub async fn get_current_sentiment(token_mint: Pubkey) -> Option<MarketSentiment> {
    SENTIMENT_ANALYZER.token_sentiments.get(&token_mint).map(|entry| entry.clone())
}
