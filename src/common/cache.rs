use anyhow::Result;
use dashmap::DashMap;
use moka::future::{Cache, CacheBuilder};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock,
    },
    time::{Duration, Instant},
};
use tracing::{debug, info};

// Cache performance metrics
static CACHE_HITS: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static CACHE_MISSES: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static CACHE_OPERATIONS: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

// High-performance cached data structures
static PRICE_CACHE: LazyLock<Cache<String, CachedPrice>> = LazyLock::new(|| {
    CacheBuilder::new(1000) // Max 1000 price entries
        .time_to_live(Duration::from_secs(30)) // 30 second TTL
        .time_to_idle(Duration::from_secs(10)) // 10 second idle timeout
        .build()
});

static TOKEN_METADATA_CACHE: LazyLock<Cache<Pubkey, CachedTokenMetadata>> = LazyLock::new(|| {
    CacheBuilder::new(5000) // Max 5000 token entries
        .time_to_live(Duration::from_secs(300)) // 5 minute TTL
        .time_to_idle(Duration::from_secs(60))  // 1 minute idle timeout
        .build()
});

static ACCOUNT_BALANCE_CACHE: LazyLock<Cache<Pubkey, CachedBalance>> = LazyLock::new(|| {
    CacheBuilder::new(500) // Max 500 account entries
        .time_to_live(Duration::from_secs(10)) // 10 second TTL for balances
        .time_to_idle(Duration::from_secs(5))   // 5 second idle timeout
        .build()
});

static POOL_STATE_CACHE: LazyLock<Cache<Pubkey, CachedPoolState>> = LazyLock::new(|| {
    CacheBuilder::new(200) // Max 200 pool entries
        .time_to_live(Duration::from_secs(60)) // 1 minute TTL
        .time_to_idle(Duration::from_secs(30))  // 30 second idle timeout
        .build()
});

// Real-time tracking caches with manual eviction
static RECENT_TRANSACTIONS: LazyLock<Arc<DashMap<String, CachedTransaction>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

static WALLET_ACTIVITY: LazyLock<Arc<DashMap<Pubkey, WalletActivity>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPrice {
    pub token_mint: Pubkey,
    pub price_usd: f64,
    pub price_sol: f64,
    pub volume_24h: f64,
    pub price_change_24h: f64,
    pub liquidity_usd: f64,
    pub market_cap: Option<f64>,
    pub last_updated: Instant,
    pub source: PriceSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PriceSource {
    Raydium,
    PumpFun,
    Jupiter,
    Coingecko,
    Birdeye,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTokenMetadata {
    pub mint: Pubkey,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub supply: u64,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub telegram: Option<String>,
    pub is_verified: bool,
    pub is_mutable: bool,
    pub last_updated: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedBalance {
    pub account: Pubkey,
    pub balance_lamports: u64,
    pub balance_sol: f64,
    pub token_balances: Vec<TokenBalance>,
    pub last_updated: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub mint: Pubkey,
    pub amount: u64,
    pub decimals: u8,
    pub ui_amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPoolState {
    pub pool_address: Pubkey,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_reserve: u64,
    pub token_b_reserve: u64,
    pub current_price: f64,
    pub liquidity_usd: f64,
    pub volume_24h: f64,
    pub fee_rate: f64,
    pub pool_type: PoolType,
    pub last_updated: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoolType {
    RaydiumAmm,
    RaydiumClmm,
    PumpFunBonding,
    Meteora,
    Orca,
}

#[derive(Debug, Clone)]
pub struct CachedTransaction {
    pub signature: String,
    pub block_time: u64,
    pub processed_at: Instant,
    pub transaction_type: TransactionType,
    pub amount_sol: f64,
    pub token_mint: Option<Pubkey>,
    pub from_account: Option<Pubkey>,
    pub to_account: Option<Pubkey>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransactionType {
    Swap,
    Transfer,
    LiquidityAdd,
    LiquidityRemove,
    TokenMint,
    TokenBurn,
    Other,
}

#[derive(Debug, Clone)]
pub struct WalletActivity {
    pub wallet: Pubkey,
    pub total_volume_24h: f64,
    pub transaction_count_24h: u64,
    pub avg_transaction_size: f64,
    pub last_transaction: Instant,
    pub risk_score: f64,
    pub is_whale: bool,
    pub is_blacklisted: bool,
}

pub struct CacheManager;

impl CacheManager {
    /// Get price data with intelligent caching
    pub async fn get_price(&self, token_mint: &Pubkey) -> Option<CachedPrice> {
        CACHE_OPERATIONS.fetch_add(1, Ordering::SeqCst);
        
        let cache_key = token_mint.to_string();
        
        if let Some(cached_price) = PRICE_CACHE.get(&cache_key).await {
            CACHE_HITS.fetch_add(1, Ordering::SeqCst);
            debug!("Cache hit for price: {}", token_mint);
            Some(cached_price)
        } else {
            CACHE_MISSES.fetch_add(1, Ordering::SeqCst);
            debug!("Cache miss for price: {}", token_mint);
            None
        }
    }

    /// Cache price data with automatic eviction
    pub async fn cache_price(&self, price_data: CachedPrice) {
        let cache_key = price_data.token_mint.to_string();
        PRICE_CACHE.insert(cache_key, price_data).await;
        debug!("Cached price for token: {}", price_data.token_mint);
    }

    /// Get token metadata with caching
    pub async fn get_token_metadata(&self, mint: &Pubkey) -> Option<CachedTokenMetadata> {
        CACHE_OPERATIONS.fetch_add(1, Ordering::SeqCst);
        
        if let Some(metadata) = TOKEN_METADATA_CACHE.get(mint).await {
            CACHE_HITS.fetch_add(1, Ordering::SeqCst);
            debug!("Cache hit for token metadata: {}", mint);
            Some(metadata)
        } else {
            CACHE_MISSES.fetch_add(1, Ordering::SeqCst);
            debug!("Cache miss for token metadata: {}", mint);
            None
        }
    }

    /// Cache token metadata
    pub async fn cache_token_metadata(&self, metadata: CachedTokenMetadata) {
        TOKEN_METADATA_CACHE.insert(metadata.mint, metadata.clone()).await;
        debug!("Cached token metadata for: {}", metadata.mint);
    }

    /// Get account balance with caching
    pub async fn get_account_balance(&self, account: &Pubkey) -> Option<CachedBalance> {
        CACHE_OPERATIONS.fetch_add(1, Ordering::SeqCst);
        
        if let Some(balance) = ACCOUNT_BALANCE_CACHE.get(account).await {
            CACHE_HITS.fetch_add(1, Ordering::SeqCst);
            debug!("Cache hit for account balance: {}", account);
            Some(balance)
        } else {
            CACHE_MISSES.fetch_add(1, Ordering::SeqCst);
            debug!("Cache miss for account balance: {}", account);
            None
        }
    }

    /// Cache account balance
    pub async fn cache_account_balance(&self, balance: CachedBalance) {
        ACCOUNT_BALANCE_CACHE.insert(balance.account, balance.clone()).await;
        debug!("Cached account balance for: {}", balance.account);
    }

    /// Get pool state with caching
    pub async fn get_pool_state(&self, pool: &Pubkey) -> Option<CachedPoolState> {
        CACHE_OPERATIONS.fetch_add(1, Ordering::SeqCst);
        
        if let Some(pool_state) = POOL_STATE_CACHE.get(pool).await {
            CACHE_HITS.fetch_add(1, Ordering::SeqCst);
            debug!("Cache hit for pool state: {}", pool);
            Some(pool_state)
        } else {
            CACHE_MISSES.fetch_add(1, Ordering::SeqCst);
            debug!("Cache miss for pool state: {}", pool);
            None
        }
    }

    /// Cache pool state
    pub async fn cache_pool_state(&self, pool_state: CachedPoolState) {
        POOL_STATE_CACHE.insert(pool_state.pool_address, pool_state.clone()).await;
        debug!("Cached pool state for: {}", pool_state.pool_address);
    }

    /// Track recent transaction (manual eviction)
    pub fn track_transaction(&self, transaction: CachedTransaction) {
        RECENT_TRANSACTIONS.insert(transaction.signature.clone(), transaction.clone());
        debug!("Tracked transaction: {}", transaction.signature);
        
        // Manual cleanup if too many entries
        if RECENT_TRANSACTIONS.len() > 10000 {
            self.cleanup_old_transactions();
        }
    }

    /// Check if transaction was recently processed
    pub fn is_transaction_processed(&self, signature: &str) -> bool {
        RECENT_TRANSACTIONS.contains_key(signature)
    }

    /// Update wallet activity tracking
    pub fn update_wallet_activity(&self, wallet: Pubkey, activity: WalletActivity) {
        WALLET_ACTIVITY.insert(wallet, activity);
        debug!("Updated wallet activity for: {}", wallet);
        
        // Manual cleanup if too many entries
        if WALLET_ACTIVITY.len() > 5000 {
            self.cleanup_old_wallet_activity();
        }
    }

    /// Get wallet activity data
    pub fn get_wallet_activity(&self, wallet: &Pubkey) -> Option<WalletActivity> {
        WALLET_ACTIVITY.get(wallet).map(|entry| entry.value().clone())
    }

    /// Check if wallet is flagged as high-risk
    pub fn is_high_risk_wallet(&self, wallet: &Pubkey) -> bool {
        if let Some(activity) = self.get_wallet_activity(wallet) {
            activity.risk_score > 0.8 || activity.is_blacklisted
        } else {
            false
        }
    }

    /// Manual cleanup of old transactions
    fn cleanup_old_transactions(&self) {
        let cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes
        
        let old_signatures: Vec<String> = RECENT_TRANSACTIONS
            .iter()
            .filter(|entry| entry.processed_at < cutoff)
            .map(|entry| entry.key().clone())
            .collect();
        
        for signature in old_signatures {
            RECENT_TRANSACTIONS.remove(&signature);
        }
        
        debug!("Cleaned up old transaction records");
    }

    /// Manual cleanup of old wallet activity
    fn cleanup_old_wallet_activity(&self) {
        let cutoff = Instant::now() - Duration::from_hours(24); // 24 hours
        
        let old_wallets: Vec<Pubkey> = WALLET_ACTIVITY
            .iter()
            .filter(|entry| entry.last_transaction < cutoff)
            .map(|entry| *entry.key())
            .collect();
        
        for wallet in old_wallets {
            WALLET_ACTIVITY.remove(&wallet);
        }
        
        debug!("Cleaned up old wallet activity records");
    }

    /// Get comprehensive cache statistics
    pub async fn get_cache_statistics(&self) -> HashMap<String, u64> {
        let mut stats = HashMap::new();
        
        // Performance metrics
        stats.insert("cache_hits".to_string(), CACHE_HITS.load(Ordering::SeqCst));
        stats.insert("cache_misses".to_string(), CACHE_MISSES.load(Ordering::SeqCst));
        stats.insert("cache_operations".to_string(), CACHE_OPERATIONS.load(Ordering::SeqCst));
        
        // Cache sizes
        stats.insert("price_cache_size".to_string(), PRICE_CACHE.entry_count());
        stats.insert("metadata_cache_size".to_string(), TOKEN_METADATA_CACHE.entry_count());
        stats.insert("balance_cache_size".to_string(), ACCOUNT_BALANCE_CACHE.entry_count());
        stats.insert("pool_cache_size".to_string(), POOL_STATE_CACHE.entry_count());
        stats.insert("transaction_cache_size".to_string(), RECENT_TRANSACTIONS.len() as u64);
        stats.insert("wallet_activity_size".to_string(), WALLET_ACTIVITY.len() as u64);
        
        // Hit rate calculation
        let total_ops = CACHE_OPERATIONS.load(Ordering::SeqCst);
        let hit_rate = if total_ops > 0 {
            (CACHE_HITS.load(Ordering::SeqCst) * 100) / total_ops
        } else {
            0
        };
        stats.insert("hit_rate_percentage".to_string(), hit_rate);
        
        stats
    }

    /// Force cache cleanup (can be called manually)
    pub async fn force_cleanup(&self) {
        self.cleanup_old_transactions();
        self.cleanup_old_wallet_activity();
        
        // Invalidate expired entries in Moka caches
        PRICE_CACHE.run_pending_tasks().await;
        TOKEN_METADATA_CACHE.run_pending_tasks().await;
        ACCOUNT_BALANCE_CACHE.run_pending_tasks().await;
        POOL_STATE_CACHE.run_pending_tasks().await;
        
        info!("Forced cache cleanup completed");
    }

    /// Preload commonly used data for better performance
    pub async fn preload_common_data(&self) -> Result<()> {
        info!("Starting cache preloading...");
        
        // Preload common token metadata (SOL, USDC, etc.)
        let common_tokens = vec![
            "So11111111111111111111111111111111111111112", // Wrapped SOL
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", // USDC
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB", // USDT
        ];
        
        for token_str in common_tokens {
            if let Ok(mint) = token_str.parse() {
                // This would typically fetch from an API and cache
                let metadata = CachedTokenMetadata {
                    mint,
                    name: "Preloaded Token".to_string(),
                    symbol: "TKN".to_string(),
                    decimals: 9,
                    supply: 0,
                    description: None,
                    image_url: None,
                    website: None,
                    twitter: None,
                    telegram: None,
                    is_verified: true,
                    is_mutable: false,
                    last_updated: Instant::now(),
                };
                
                self.cache_token_metadata(metadata).await;
            }
        }
        
        info!("Cache preloading completed");
        Ok(())
    }
}

/// Global cache manager instance
pub static CACHE_MANAGER: LazyLock<CacheManager> = LazyLock::new(|| CacheManager);

/// Background task to maintain cache health
pub async fn start_cache_maintenance_task() {
    info!("Starting cache maintenance task");
    
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(120)); // 2 minutes
        
        loop {
            interval.tick().await;
            
            // Run cache maintenance
            CACHE_MANAGER.force_cleanup().await;
            
            // Log cache statistics periodically
            let stats = CACHE_MANAGER.get_cache_statistics().await;
            debug!("Cache stats: hit_rate={}%, operations={}", 
                  stats.get("hit_rate_percentage").unwrap_or(&0),
                  stats.get("cache_operations").unwrap_or(&0));
        }
    });
}

/// Initialize caching system with preloading
pub async fn initialize_cache_system() -> Result<()> {
    info!("Initializing high-performance caching system");
    
    // Preload common data
    CACHE_MANAGER.preload_common_data().await?;
    
    // Start maintenance task
    start_cache_maintenance_task().await;
    
    info!("Caching system initialized successfully");
    Ok(())
}
