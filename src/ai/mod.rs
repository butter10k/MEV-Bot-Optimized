pub mod sentiment_analyzer;
pub mod whale_tracker;

pub use sentiment_analyzer::{
    analyze_market_sentiment, predict_price_movement, start_ai_system, 
    get_current_sentiment, MarketSentiment, MarketPrediction, SentimentScore
};

pub use whale_tracker::{
    start_whale_tracking, track_whale_wallet, get_whale_activities,
    WhaleActivity, WhaleWallet, get_copy_trading_signals
};
