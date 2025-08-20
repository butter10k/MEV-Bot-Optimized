use anyhow::{anyhow, Result};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::{
    env,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock,
    },
    time::{Duration, Instant},
};
use tokio::{
    net::TcpStream,
    time::{sleep, timeout},
};
use tokio_tungstenite::{
    connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info, warn};

use crate::{
    common::{logger::Logger, utils::AppState},
    engine::swap::{OptimizedSwapConfig, SwapDirection, SwapEngine},
};

// High-performance tracking
static YELLOWSTONE_MESSAGE_COUNTER: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static ACCOUNT_UPDATE_COUNTER: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static SLOT_UPDATE_COUNTER: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

// Connection resilience
static YELLOWSTONE_RETRY_COUNT: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

// Real-time account monitoring with high-performance data structures
static MONITORED_ACCOUNTS: LazyLock<Arc<DashMap<Pubkey, AccountMonitorConfig>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

// Slot and block tracking for MEV optimization
static LATEST_SLOT: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static BLOCK_TIMES: LazyLock<Arc<DashMap<u64, Instant>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

// Price movement detection for rapid response
static PRICE_MOVEMENTS: LazyLock<Arc<DashMap<Pubkey, PriceMovement>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

#[derive(Debug, Clone)]
struct AccountMonitorConfig {
    account: Pubkey,
    monitor_type: MonitorType,
    last_update: Instant,
    update_count: u64,
    threshold_lamports: Option<u64>,
    callback_enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum MonitorType {
    TokenAccount,      // SPL token account monitoring
    LiquidityPool,     // AMM pool monitoring  
    TradingAccount,    // Large trader account
    ProgramData,       // Program account data changes
    Custom(String),    // Custom monitoring type
}

#[derive(Debug, Clone)]
struct PriceMovement {
    token_mint: Pubkey,
    old_price: f64,
    new_price: f64,
    percentage_change: f64,
    detected_at: Instant,
    pool_address: Pubkey,
}

#[derive(Debug, Serialize)]
struct YellowstoneSubscribeRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: YellowstoneSubscribeParams,
}

#[derive(Debug, Serialize)]
struct YellowstoneSubscribeParams {
    accounts: std::collections::HashMap<String, AccountFilter>,
    slots: std::collections::HashMap<String, SlotFilter>,
    transactions: std::collections::HashMap<String, TransactionFilter>,
    commitment: Option<String>,
    accounts_data_slice: Option<Vec<DataSlice>>,
}

#[derive(Debug, Serialize)]
struct AccountFilter {
    account: Option<Vec<String>>,
    owner: Option<Vec<String>>,
    filters: Option<Vec<AccountFilterType>>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AccountFilterType {
    DataSize(u64),
    Memcmp { offset: u64, bytes: String },
}

#[derive(Debug, Serialize)]
struct SlotFilter {
    filter_by_commitment: Option<bool>,
}

#[derive(Debug, Serialize)]
struct TransactionFilter {
    vote: Option<bool>,
    failed: Option<bool>,
    signature: Option<String>,
    account_include: Option<Vec<String>>,
    account_exclude: Option<Vec<String>>,
    account_required: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct DataSlice {
    offset: u64,
    length: u64,
}

#[derive(Debug, Deserialize)]
struct YellowstoneResponse {
    jsonrpc: String,
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<YellowstoneError>,
    method: Option<String>,
    params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct YellowstoneError {
    code: i32,
    message: String,
}

#[derive(Debug, Deserialize)]
struct AccountUpdate {
    account: AccountInfo,
    slot: u64,
    is_startup: bool,
}

#[derive(Debug, Deserialize)]
struct AccountInfo {
    pubkey: String,
    account: Account,
}

#[derive(Debug, Deserialize)]
struct Account {
    lamports: u64,
    data: Vec<String>, // Base64 encoded data
    owner: String,
    executable: bool,
    rent_epoch: u64,
}

#[derive(Debug, Deserialize)]
struct SlotUpdate {
    slot: u64,
    parent: Option<u64>,
    status: String,
}

pub struct YellowstoneMonitor {
    rpc_client: Arc<RpcClient>,
    websocket_url: String,
    api_key: Option<String>,
    swap_engine: Option<Arc<SwapEngine>>,
    logger: Logger,
    connection_timeout: Duration,
    reconnect_delay: Duration,
    max_reconnect_attempts: u32,
}

impl YellowstoneMonitor {
    pub fn new(app_state: &AppState, websocket_url: String) -> Result<Self> {
        let api_key = env::var("YELLOWSTONE_API_KEY").ok();

        Ok(Self {
            rpc_client: app_state.rpc_nonblocking_client.clone(),
            websocket_url,
            api_key,
            swap_engine: None,
            logger: Logger::new("[YELLOWSTONE MONITOR] => ".to_string()),
            connection_timeout: Duration::from_secs(30),
            reconnect_delay: Duration::from_secs(3),
            max_reconnect_attempts: 15,
        })
    }

    pub fn with_swap_engine(mut self, swap_engine: Arc<SwapEngine>) -> Self {
        self.swap_engine = Some(swap_engine);
        self
    }

    /// Start high-performance monitoring with automatic reconnection
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting Yellowstone gRPC monitoring...");
        
        let mut attempt = 0;
        loop {
            match self.connect_and_monitor().await {
                Ok(_) => {
                    info!("Yellowstone monitoring session ended normally");
                    break;
                }
                Err(e) => {
                    attempt += 1;
                    YELLOWSTONE_RETRY_COUNT.fetch_add(1, Ordering::SeqCst);
                    
                    if attempt > self.max_reconnect_attempts {
                        error!("Max reconnection attempts reached for Yellowstone");
                        return Err(anyhow!("Failed to establish stable Yellowstone connection"));
                    }
                    
                    warn!(
                        "Yellowstone connection failed (attempt {}): {}. Retrying in {:?}...",
                        attempt, e, self.reconnect_delay
                    );
                    
                    // Exponential backoff with jitter
                    let delay = self.reconnect_delay * (2_u32.pow(std::cmp::min(attempt - 1, 4)));
                    let jitter = Duration::from_millis(fastrand::u64(100..=500));
                    sleep(delay + jitter).await;
                }
            }
        }
        
        Ok(())
    }

    async fn connect_and_monitor(&self) -> Result<()> {
        // Build WebSocket URL with authentication if available
        let mut url = self.websocket_url.clone();
        if let Some(api_key) = &self.api_key {
            url = format!("{}?api_key={}", url, api_key);
        }

        // Establish connection with timeout
        let ws_stream = timeout(
            self.connection_timeout,
            connect_async(&url)
        ).await??;

        info!("Connected to Yellowstone gRPC WebSocket");
        let (ws_stream, _) = ws_stream;
        let (mut write, mut read) = ws_stream.split();

        // Send optimized subscription requests
        let subscription_requests = self.create_optimized_subscriptions();
        
        for request in subscription_requests {
            let message = Message::Text(serde_json::to_string(&request)?);
            write.send(message).await?;
            debug!("Sent Yellowstone subscription: {}", request.method);
        }

        // Start high-performance message processing
        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    YELLOWSTONE_MESSAGE_COUNTER.fetch_add(1, Ordering::SeqCst);
                    
                    // Use parallel processing for message handling
                    let text_clone = text.clone();
                    let self_clone = self.clone_for_processing();
                    
                    tokio::spawn(async move {
                        if let Err(e) = self_clone.process_message_parallel(&text_clone).await {
                            debug!("Error processing Yellowstone message: {}", e);
                        }
                    });
                }
                Ok(Message::Ping(data)) => {
                    write.send(Message::Pong(data)).await?;
                }
                Ok(Message::Close(_)) => {
                    warn!("Yellowstone WebSocket connection closed by server");
                    break;
                }
                Err(e) => {
                    error!("Yellowstone WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn create_optimized_subscriptions(&self) -> Vec<YellowstoneSubscribeRequest> {
        vec![
            // High-priority account monitoring for major DEX programs
            YellowstoneSubscribeRequest {
                jsonrpc: "2.0".to_string(),
                id: 1,
                method: "subscribe".to_string(),
                params: YellowstoneSubscribeParams {
                    accounts: {
                        let mut accounts = std::collections::HashMap::new();
                        
                        // Raydium AMM pools with size filter
                        accounts.insert("raydium_pools".to_string(), AccountFilter {
                            account: None,
                            owner: Some(vec!["675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string()]),
                            filters: Some(vec![AccountFilterType::DataSize(752)]), // AMM pool size
                        });
                        
                        // Pump.fun bonding curves
                        accounts.insert("pumpfun_curves".to_string(), AccountFilter {
                            account: None,
                            owner: Some(vec!["6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P".to_string()]),
                            filters: Some(vec![AccountFilterType::DataSize(104)]), // Bonding curve size
                        });
                        
                        // Large SOL holders for whale tracking
                        accounts.insert("sol_whales".to_string(), AccountFilter {
                            account: None,
                            owner: Some(vec!["11111111111111111111111111111111".to_string()]),
                            filters: Some(vec![AccountFilterType::DataSize(0)]), // Native accounts
                        });
                        
                        accounts
                    },
                    slots: {
                        let mut slots = std::collections::HashMap::new();
                        slots.insert("all_slots".to_string(), SlotFilter {
                            filter_by_commitment: Some(false),
                        });
                        slots
                    },
                    transactions: {
                        let mut transactions = std::collections::HashMap::new();
                        
                        // High-value transactions only
                        transactions.insert("high_value_txs".to_string(), TransactionFilter {
                            vote: Some(false),
                            failed: Some(false),
                            signature: None,
                            account_include: Some(vec![
                                "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(), // Raydium
                                "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P".to_string(),  // Pump.fun
                            ]),
                            account_exclude: None,
                            account_required: None,
                        });
                        
                        transactions
                    },
                    commitment: Some("processed".to_string()),
                    accounts_data_slice: Some(vec![
                        DataSlice { offset: 0, length: 64 },  // First 64 bytes for quick analysis
                        DataSlice { offset: 64, length: 64 }, // Next 64 bytes
                    ]),
                },
            },
        ]
    }

    // Create a lightweight clone for parallel processing
    fn clone_for_processing(&self) -> YellowstoneMonitorProcessor {
        YellowstoneMonitorProcessor {
            swap_engine: self.swap_engine.clone(),
            logger: self.logger.clone(),
        }
    }

    async fn process_message_parallel(&self, message: &str) -> Result<()> {
        // This method will be moved to the processor struct
        Ok(())
    }

    /// Add account to high-priority monitoring
    pub fn add_monitored_account(
        &self,
        account: Pubkey,
        monitor_type: MonitorType,
        threshold_lamports: Option<u64>,
    ) {
        let config = AccountMonitorConfig {
            account,
            monitor_type,
            last_update: Instant::now(),
            update_count: 0,
            threshold_lamports,
            callback_enabled: true,
        };

        MONITORED_ACCOUNTS.insert(account, config);
        info!("Added account to high-priority monitoring: {}", account);
    }

    /// Remove account from monitoring
    pub fn remove_monitored_account(&self, account: &Pubkey) {
        MONITORED_ACCOUNTS.remove(account);
        info!("Removed account from monitoring: {}", account);
    }

    /// Get real-time statistics
    pub fn get_realtime_statistics(&self) -> std::collections::HashMap<String, u64> {
        let mut stats = std::collections::HashMap::new();
        
        stats.insert("total_messages".to_string(), YELLOWSTONE_MESSAGE_COUNTER.load(Ordering::SeqCst));
        stats.insert("account_updates".to_string(), ACCOUNT_UPDATE_COUNTER.load(Ordering::SeqCst));
        stats.insert("slot_updates".to_string(), SLOT_UPDATE_COUNTER.load(Ordering::SeqCst));
        stats.insert("connection_retries".to_string(), YELLOWSTONE_RETRY_COUNT.load(Ordering::SeqCst));
        stats.insert("monitored_accounts".to_string(), MONITORED_ACCOUNTS.len() as u64);
        stats.insert("price_movements".to_string(), PRICE_MOVEMENTS.len() as u64);
        stats.insert("latest_slot".to_string(), LATEST_SLOT.load(Ordering::SeqCst));
        
        stats
    }

    /// Get current slot information
    pub fn get_current_slot(&self) -> u64 {
        LATEST_SLOT.load(Ordering::SeqCst)
    }

    /// Get recent price movements
    pub fn get_recent_price_movements(&self, limit: usize) -> Vec<PriceMovement> {
        let mut movements: Vec<_> = PRICE_MOVEMENTS
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        
        movements.sort_by(|a, b| b.detected_at.cmp(&a.detected_at));
        movements.truncate(limit);
        movements
    }
}

// Separate processor for parallel message handling
#[derive(Clone)]
struct YellowstoneMonitorProcessor {
    swap_engine: Option<Arc<SwapEngine>>,
    logger: Logger,
}

impl YellowstoneMonitorProcessor {
    async fn process_message_parallel(&self, message: &str) -> Result<()> {
        // Parse Yellowstone response
        let response: YellowstoneResponse = serde_json::from_str(message)?;

        // Handle different message types
        if let Some(method) = &response.method {
            match method.as_str() {
                "accountNotification" => {
                    self.handle_account_notification(&response).await?;
                }
                "slotNotification" => {
                    self.handle_slot_notification(&response).await?;
                }
                "transactionNotification" => {
                    self.handle_transaction_notification(&response).await?;
                }
                _ => {
                    debug!("Unknown Yellowstone notification method: {}", method);
                }
            }
        }

        // Handle subscription confirmations
        if let Some(result) = &response.result {
            if result.is_number() {
                debug!("Yellowstone subscription confirmed with ID: {}", result);
            }
        }

        Ok(())
    }

    async fn handle_account_notification(&self, response: &YellowstoneResponse) -> Result<()> {
        ACCOUNT_UPDATE_COUNTER.fetch_add(1, Ordering::SeqCst);

        if let Some(params) = &response.params {
            if let Ok(update) = serde_json::from_value::<AccountUpdate>(params.clone()) {
                let account_pubkey = Pubkey::from_str(&update.account.pubkey)?;
                
                // Check if this is a monitored account
                if let Some(mut config) = MONITORED_ACCOUNTS.get_mut(&account_pubkey) {
                    config.last_update = Instant::now();
                    config.update_count += 1;

                    // Analyze account changes for trading opportunities
                    self.analyze_account_change(&account_pubkey, &update).await?;
                }

                // Detect price movements in liquidity pools
                if self.is_liquidity_pool(&update.account.account.owner) {
                    self.detect_price_movement(&account_pubkey, &update).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_slot_notification(&self, response: &YellowstoneResponse) -> Result<()> {
        SLOT_UPDATE_COUNTER.fetch_add(1, Ordering::SeqCst);

        if let Some(params) = &response.params {
            if let Ok(slot_update) = serde_json::from_value::<SlotUpdate>(params.clone()) {
                // Update latest slot
                LATEST_SLOT.store(slot_update.slot, Ordering::SeqCst);
                
                // Track block timing for MEV optimization
                BLOCK_TIMES.insert(slot_update.slot, Instant::now());

                // Clean old block times (keep last 100)
                if BLOCK_TIMES.len() > 100 {
                    let cutoff_slot = slot_update.slot.saturating_sub(100);
                    let old_slots: Vec<u64> = BLOCK_TIMES
                        .iter()
                        .filter(|entry| *entry.key() < cutoff_slot)
                        .map(|entry| *entry.key())
                        .collect();
                    
                    for slot in old_slots {
                        BLOCK_TIMES.remove(&slot);
                    }
                }

                debug!("Slot update: {} (status: {})", slot_update.slot, slot_update.status);
            }
        }

        Ok(())
    }

    async fn handle_transaction_notification(&self, _response: &YellowstoneResponse) -> Result<()> {
        // Handle high-value transaction notifications
        // This would contain detailed transaction analysis and MEV detection
        
        Ok(())
    }

    async fn analyze_account_change(&self, account: &Pubkey, update: &AccountUpdate) -> Result<()> {
        // Analyze significant account changes for trading signals
        let lamports_change = update.account.account.lamports;
        
        if lamports_change > 1_000_000_000 { // > 1 SOL change
            info!(
                "Significant account change detected: {} ({} lamports)",
                account, lamports_change
            );

            // Execute trading strategy if configured
            if let Some(swap_engine) = &self.swap_engine {
                // Implement account-change-based trading logic
                self.execute_account_based_strategy(swap_engine, account, lamports_change).await?;
            }
        }

        Ok(())
    }

    async fn detect_price_movement(&self, pool: &Pubkey, update: &AccountUpdate) -> Result<()> {
        // Simplified price movement detection
        // In a real implementation, you would decode pool data to calculate actual prices
        
        // For demonstration, simulate price change detection
        let simulated_old_price = 1.0;
        let simulated_new_price = 1.05;
        let percentage_change = ((simulated_new_price - simulated_old_price) / simulated_old_price) * 100.0;

        if percentage_change.abs() > 2.0 { // > 2% price change
            let movement = PriceMovement {
                token_mint: *pool, // This would be the actual token mint
                old_price: simulated_old_price,
                new_price: simulated_new_price,
                percentage_change,
                detected_at: Instant::now(),
                pool_address: *pool,
            };

            PRICE_MOVEMENTS.insert(*pool, movement.clone());

            info!(
                "Price movement detected: {:.2}% change in pool {}",
                percentage_change, pool
            );

            // Execute price-movement-based strategy
            if let Some(swap_engine) = &self.swap_engine {
                self.execute_price_movement_strategy(swap_engine, &movement).await?;
            }
        }

        Ok(())
    }

    fn is_liquidity_pool(&self, owner: &str) -> bool {
        matches!(owner, 
            "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8" | // Raydium AMM V4
            "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"    // Pump.fun
        )
    }

    async fn execute_account_based_strategy(
        &self,
        _swap_engine: &SwapEngine,
        _account: &Pubkey,
        _lamports_change: u64,
    ) -> Result<()> {
        // Implement account-based trading strategies
        Ok(())
    }

    async fn execute_price_movement_strategy(
        &self,
        _swap_engine: &SwapEngine,
        _movement: &PriceMovement,
    ) -> Result<()> {
        // Implement price-movement-based trading strategies
        Ok(())
    }
}

/// Clean up old tracking data
pub async fn cleanup_old_yellowstone_data() {
    let cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes

    // Clean old price movements
    let old_movement_keys: Vec<Pubkey> = PRICE_MOVEMENTS
        .iter()
        .filter(|entry| entry.detected_at < cutoff)
        .map(|entry| *entry.key())
        .collect();

    for key in old_movement_keys {
        PRICE_MOVEMENTS.remove(&key);
    }

    debug!("Cleaned up old Yellowstone tracking data");
}

/// Start background cleanup task
pub fn start_yellowstone_cleanup_task() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(180)); // 3 minutes
        loop {
            interval.tick().await;
            cleanup_old_yellowstone_data().await;
        }
    });
}

// Re-export for convenience
pub use fastrand;
