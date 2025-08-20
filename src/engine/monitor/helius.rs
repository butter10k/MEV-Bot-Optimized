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
use tokio::sync::{mpsc, Semaphore};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use tracing::{debug, error, info, warn};

use crate::{
    common::{logger::Logger, utils::AppState},
    engine::swap::{OptimizedSwapConfig, SwapDirection, SwapEngine},
};

// Performance tracking
static MESSAGE_COUNTER: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static RELEVANT_TX_COUNTER: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

// Enhanced connection management with pooling
static CONNECTION_RETRY_COUNT: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static ACTIVE_CONNECTIONS: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));
static CONNECTION_POOL_SIZE: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(4));

// Connection pool for better throughput
static CONNECTION_POOL: LazyLock<Arc<ConnectionPool>> = LazyLock::new(|| {
    Arc::new(ConnectionPool::new(4))
});

// Adaptive backpressure control
static BACKPRESSURE_SEMAPHORE: LazyLock<Arc<Semaphore>> = LazyLock::new(|| {
    Arc::new(Semaphore::new(1000)) // Allow 1000 concurrent operations
});

// Enhanced transaction tracking with performance optimization
static PROCESSED_TRANSACTIONS: LazyLock<Arc<DashMap<String, ProcessedTx>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

// Token monitoring cache for faster lookups
static MONITORED_TOKENS: LazyLock<Arc<RwLock<std::collections::HashSet<Pubkey>>>> = 
    LazyLock::new(|| Arc::new(RwLock::new(std::collections::HashSet::new())));

#[derive(Debug, Clone)]
struct ProcessedTx {
    signature: String,
    processed_at: Instant,
    transaction_type: TransactionType,
    amount_sol: f64,
    token_mint: Option<Pubkey>,
}

#[derive(Debug, Clone, PartialEq)]
enum TransactionType {
    Buy,
    Sell,
    LiquidityAdd,
    LiquidityRemove,
    Other,
}

// High-performance connection pool for WebSocket multiplexing
struct ConnectionPool {
    max_size: usize,
    connections: Arc<RwLock<Vec<Arc<PooledConnection>>>>,
    connection_factory: Box<dyn Fn() -> PooledConnection + Send + Sync>,
    health_check_interval: Duration,
}

#[derive(Debug)]
struct PooledConnection {
    id: usize,
    stream: Arc<RwLock<Option<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    last_used: Arc<RwLock<Instant>>,
    is_healthy: Arc<AtomicBool>,
    message_count: Arc<AtomicU64>,
    error_count: Arc<AtomicU64>,
}

impl ConnectionPool {
    fn new(max_size: usize) -> Self {
        Self {
            max_size,
            connections: Arc::new(RwLock::new(Vec::new())),
            connection_factory: Box::new(|| PooledConnection::new(0)),
            health_check_interval: Duration::from_secs(30),
        }
    }

    async fn get_connection(&self) -> Result<Arc<PooledConnection>> {
        let connections = self.connections.read();
        
        // Find a healthy, available connection
        for conn in connections.iter() {
            if conn.is_healthy.load(Ordering::SeqCst) {
                *conn.last_used.write() = Instant::now();
                return Ok(Arc::clone(conn));
            }
        }
        
        drop(connections);
        
        // Create new connection if pool not full
        let mut connections = self.connections.write();
        if connections.len() < self.max_size {
            let new_conn = Arc::new(PooledConnection::new(connections.len()));
            connections.push(Arc::clone(&new_conn));
            return Ok(new_conn);
        }
        
        Err(anyhow!("Connection pool exhausted"))
    }

    async fn return_connection(&self, conn: Arc<PooledConnection>) {
        // Connection is automatically returned when dropped
        // Health check will clean up unhealthy connections
    }

    async fn start_health_check(&self) {
        let connections = Arc::clone(&self.connections);
        let interval = self.health_check_interval;
        
        tokio::spawn(async move {
            let mut health_interval = tokio::time::interval(interval);
            
            loop {
                health_interval.tick().await;
                
                let mut connections = connections.write();
                connections.retain(|conn| {
                    let is_healthy = conn.is_healthy.load(Ordering::SeqCst);
                    let last_used = *conn.last_used.read();
                    let is_stale = last_used.elapsed() > Duration::from_secs(300); // 5 minutes
                    
                    if !is_healthy || is_stale {
                        debug!("Removing unhealthy/stale connection {}", conn.id);
                        false
                    } else {
                        true
                    }
                });
            }
        });
    }
}

impl PooledConnection {
    fn new(id: usize) -> Self {
        Self {
            id,
            stream: Arc::new(RwLock::new(None)),
            last_used: Arc::new(RwLock::new(Instant::now())),
            is_healthy: Arc::new(AtomicBool::new(true)),
            message_count: Arc::new(AtomicU64::new(0)),
            error_count: Arc::new(AtomicU64::new(0)),
        }
    }

    async fn connect(&self, url: &str) -> Result<()> {
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                *self.stream.write() = Some(ws_stream);
                self.is_healthy.store(true, Ordering::SeqCst);
                debug!("Connection {} established successfully", self.id);
                Ok(())
            }
            Err(e) => {
                self.is_healthy.store(false, Ordering::SeqCst);
                self.error_count.fetch_add(1, Ordering::SeqCst);
                Err(anyhow!("Failed to connect: {}", e))
            }
        }
    }

    async fn send_message(&self, message: Message) -> Result<()> {
        let mut stream_guard = self.stream.write();
        if let Some(ref mut stream) = *stream_guard {
            match stream.send(message).await {
                Ok(_) => {
                    self.message_count.fetch_add(1, Ordering::SeqCst);
                    *self.last_used.write() = Instant::now();
                    Ok(())
                }
                Err(e) => {
                    self.is_healthy.store(false, Ordering::SeqCst);
                    self.error_count.fetch_add(1, Ordering::SeqCst);
                    Err(anyhow!("Failed to send message: {}", e))
                }
            }
        } else {
            Err(anyhow!("Connection not established"))
        }
    }

    async fn receive_message(&self) -> Result<Option<Message>> {
        let mut stream_guard = self.stream.write();
        if let Some(ref mut stream) = *stream_guard {
            match timeout(Duration::from_secs(1), stream.next()).await {
                Ok(Some(msg_result)) => {
                    match msg_result {
                        Ok(message) => {
                            self.message_count.fetch_add(1, Ordering::SeqCst);
                            *self.last_used.write() = Instant::now();
                            Ok(Some(message))
                        }
                        Err(e) => {
                            self.is_healthy.store(false, Ordering::SeqCst);
                            self.error_count.fetch_add(1, Ordering::SeqCst);
                            Err(anyhow!("WebSocket error: {}", e))
                        }
                    }
                }
                Ok(None) => {
                    self.is_healthy.store(false, Ordering::SeqCst);
                    Ok(None) // Stream ended
                }
                Err(_) => {
                    // Timeout, but connection might still be healthy
                    Ok(None)
                }
            }
        } else {
            Err(anyhow!("Connection not established"))
        }
    }
}

#[derive(Debug, Deserialize)]
struct HeliusWebhookPayload {
    #[serde(rename = "type")]
    transaction_type: String,
    description: String,
    source: String,
    fee: u64,
    #[serde(rename = "feePayer")]
    fee_payer: String,
    signature: String,
    slot: u64,
    timestamp: u64,
    #[serde(rename = "tokenTransfers")]
    token_transfers: Vec<TokenTransfer>,
    #[serde(rename = "nativeTransfers")]
    native_transfers: Vec<NativeTransfer>,
    #[serde(rename = "accountData")]
    account_data: Vec<AccountData>,
    #[serde(rename = "transactionError")]
    transaction_error: Option<serde_json::Value>,
    instructions: Vec<Instruction>,
}

#[derive(Debug, Deserialize)]
struct TokenTransfer {
    #[serde(rename = "fromTokenAccount")]
    from_token_account: String,
    #[serde(rename = "toTokenAccount")]
    to_token_account: String,
    #[serde(rename = "fromUserAccount")]
    from_user_account: String,
    #[serde(rename = "toUserAccount")]
    to_user_account: String,
    #[serde(rename = "tokenAmount")]
    token_amount: f64,
    mint: String,
    #[serde(rename = "tokenStandard")]
    token_standard: String,
}

#[derive(Debug, Deserialize)]
struct NativeTransfer {
    #[serde(rename = "fromUserAccount")]
    from_user_account: String,
    #[serde(rename = "toUserAccount")]
    to_user_account: String,
    amount: u64,
}

#[derive(Debug, Deserialize)]
struct AccountData {
    account: String,
    #[serde(rename = "nativeBalanceChange")]
    native_balance_change: i64,
    #[serde(rename = "tokenBalanceChanges")]
    token_balance_changes: Vec<TokenBalanceChange>,
}

#[derive(Debug, Deserialize)]
struct TokenBalanceChange {
    #[serde(rename = "userAccount")]
    user_account: String,
    #[serde(rename = "tokenAccount")]
    token_account: String,
    #[serde(rename = "rawTokenAmount")]
    raw_token_amount: RawTokenAmount,
    mint: String,
}

#[derive(Debug, Deserialize)]
struct RawTokenAmount {
    #[serde(rename = "tokenAmount")]
    token_amount: String,
    decimals: u8,
}

#[derive(Debug, Deserialize)]
struct Instruction {
    accounts: Vec<String>,
    data: String,
    #[serde(rename = "programId")]
    program_id: String,
    #[serde(rename = "innerInstructions")]
    inner_instructions: Vec<InnerInstruction>,
}

#[derive(Debug, Deserialize)]
struct InnerInstruction {
    accounts: Vec<String>,
    data: String,
    #[serde(rename = "programId")]
    program_id: String,
}

#[derive(Debug, Serialize)]
struct SubscriptionRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: serde_json::Value,
}

pub struct HeliusMonitor {
    rpc_client: Arc<RpcClient>,
    websocket_url: String,
    api_key: String,
    swap_engine: Option<Arc<SwapEngine>>,
    logger: Logger,
    connection_timeout: Duration,
    reconnect_delay: Duration,
}

impl HeliusMonitor {
    pub fn new(app_state: &AppState, websocket_url: String) -> Result<Self> {
        let api_key = env::var("HELIUS_API_KEY")
            .map_err(|_| anyhow!("HELIUS_API_KEY environment variable not set"))?;

        Ok(Self {
            rpc_client: app_state.rpc_nonblocking_client.clone(),
            websocket_url,
            api_key,
            swap_engine: None,
            logger: Logger::new("[HELIUS MONITOR] => ".to_string()),
            connection_timeout: Duration::from_secs(30),
            reconnect_delay: Duration::from_secs(5),
        })
    }

    pub fn with_swap_engine(mut self, swap_engine: Arc<SwapEngine>) -> Self {
        self.swap_engine = Some(swap_engine);
        self
    }

    /// Start monitoring with enhanced error handling and reconnection
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting Helius monitoring...");
        
        loop {
            match self.connect_and_monitor().await {
                Ok(_) => {
                    info!("Helius monitoring session ended normally");
                    break;
                }
                Err(e) => {
                    let retry_count = CONNECTION_RETRY_COUNT.fetch_add(1, Ordering::SeqCst);
                    warn!(
                        "Helius connection failed (attempt {}): {}. Retrying in {:?}...",
                        retry_count + 1, e, self.reconnect_delay
                    );
                    
                    sleep(self.reconnect_delay).await;
                    
                    // Exponential backoff with cap
                    if retry_count < 10 {
                        continue;
                    } else {
                        error!("Max reconnection attempts reached");
                        return Err(anyhow!("Failed to establish stable connection after 10 attempts"));
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn connect_and_monitor(&self) -> Result<()> {
        // Establish WebSocket connection
        let ws_stream = timeout(
            self.connection_timeout,
            connect_async(&self.websocket_url)
        ).await??;

        info!("Connected to Helius WebSocket");
        let (ws_stream, _) = ws_stream;
        let (mut write, mut read) = ws_stream.split();

        // Subscribe to relevant transaction types
        let subscription_requests = self.create_subscription_requests();
        
        for request in subscription_requests {
            let message = Message::Text(serde_json::to_string(&request)?);
            write.send(message).await?;
            debug!("Sent subscription request: {}", request.method);
        }

        // Process incoming messages
        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    MESSAGE_COUNTER.fetch_add(1, Ordering::SeqCst);
                    
                    if let Err(e) = self.process_message(&text).await {
                        warn!("Error processing message: {}", e);
                    }
                }
                Ok(Message::Ping(data)) => {
                    write.send(Message::Pong(data)).await?;
                }
                Ok(Message::Close(_)) => {
                    warn!("WebSocket connection closed by server");
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn create_subscription_requests(&self) -> Vec<SubscriptionRequest> {
        vec![
            // Subscribe to program logs for Raydium
            SubscriptionRequest {
                jsonrpc: "2.0".to_string(),
                id: 1,
                method: "programSubscribe".to_string(),
                params: serde_json::json!([
                    "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8", // Raydium AMM V4
                    {
                        "encoding": "base64",
                        "commitment": "processed"
                    }
                ]),
            },
            // Subscribe to program logs for Pump.fun
            SubscriptionRequest {
                jsonrpc: "2.0".to_string(),
                id: 2,
                method: "programSubscribe".to_string(),
                params: serde_json::json!([
                    "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P", // Pump.fun
                    {
                        "encoding": "base64",
                        "commitment": "processed"
                    }
                ]),
            },
            // Subscribe to account changes for high-activity wallets
            SubscriptionRequest {
                jsonrpc: "2.0".to_string(),
                id: 3,
                method: "accountSubscribe".to_string(),
                params: serde_json::json!([
                    "So11111111111111111111111111111111111111112", // Wrapped SOL
                    {
                        "encoding": "base64",
                        "commitment": "processed"
                    }
                ]),
            },
        ]
    }

    async fn process_message(&self, message: &str) -> Result<()> {
        // Parse the message as JSON
        let parsed: serde_json::Value = serde_json::from_str(message)?;
        
        // Check if this is a transaction notification
        if let Some(params) = parsed.get("params") {
            if let Some(result) = params.get("result") {
                if let Some(value) = result.get("value") {
                    return self.process_transaction_data(value).await;
                }
            }
        }

        // Handle subscription confirmations
        if let Some(result) = parsed.get("result") {
            if result.is_number() {
                debug!("Subscription confirmed with ID: {}", result);
            }
        }

        Ok(())
    }

    async fn process_transaction_data(&self, data: &serde_json::Value) -> Result<()> {
        // Extract signature for deduplication
        let signature = data.get("signature")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown");

        // Skip if already processed
        if PROCESSED_TRANSACTIONS.contains_key(signature) {
            return Ok(());
        }

        // Analyze transaction for trading opportunities
        let analysis = self.analyze_transaction(data).await?;
        
        if let Some((tx_type, amount_sol, token_mint)) = analysis {
            RELEVANT_TX_COUNTER.fetch_add(1, Ordering::SeqCst);
            
            // Store processed transaction
            PROCESSED_TRANSACTIONS.insert(
                signature.to_string(),
                ProcessedTx {
                    signature: signature.to_string(),
                    processed_at: Instant::now(),
                    transaction_type: tx_type.clone(),
                    amount_sol,
                    token_mint,
                }
            );

            // Execute trading strategy if applicable
            if let Some(swap_engine) = &self.swap_engine {
                self.execute_trading_strategy(
                    &swap_engine,
                    &tx_type,
                    amount_sol,
                    token_mint,
                    signature
                ).await?;
            }
        }

        Ok(())
    }

    async fn analyze_transaction(&self, data: &serde_json::Value) -> Result<Option<(TransactionType, f64, Option<Pubkey>)>> {
        // Extract instruction data
        let instructions = data.get("transaction")
            .and_then(|tx| tx.get("message"))
            .and_then(|msg| msg.get("instructions"))
            .and_then(|inst| inst.as_array())
            .unwrap_or(&vec![]);

        // Look for DEX-related instructions
        for instruction in instructions {
            if let Some(program_id) = instruction.get("programId").and_then(|p| p.as_str()) {
                match program_id {
                    "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8" => {
                        // Raydium AMM V4
                        return self.analyze_raydium_instruction(instruction).await;
                    }
                    "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P" => {
                        // Pump.fun
                        return self.analyze_pumpfun_instruction(instruction).await;
                    }
                    _ => continue,
                }
            }
        }

        Ok(None)
    }

    async fn analyze_raydium_instruction(&self, instruction: &serde_json::Value) -> Result<Option<(TransactionType, f64, Option<Pubkey>)>> {
        // Simplified Raydium instruction analysis
        // In a real implementation, you would decode the instruction data
        
        // For now, return a placeholder analysis
        Ok(Some((TransactionType::Buy, 1.0, None)))
    }

    async fn analyze_pumpfun_instruction(&self, instruction: &serde_json::Value) -> Result<Option<(TransactionType, f64, Option<Pubkey>)>> {
        // Simplified Pump.fun instruction analysis
        // In a real implementation, you would decode the instruction data
        
        // For now, return a placeholder analysis
        Ok(Some((TransactionType::Buy, 0.5, None)))
    }

    async fn execute_trading_strategy(
        &self,
        swap_engine: &SwapEngine,
        tx_type: &TransactionType,
        amount_sol: f64,
        token_mint: Option<Pubkey>,
        signature: &str,
    ) -> Result<()> {
        // Implement trading strategy based on detected transaction
        match tx_type {
            TransactionType::Buy if amount_sol > 1.0 => {
                // Follow large buy orders
                if let Some(mint) = token_mint {
                    info!(
                        "Detected large buy order: {:.2} SOL for token {}. Following trade...",
                        amount_sol, mint
                    );
                    
                    // Execute follow trade with smaller amount
                    let follow_amount = (amount_sol * 0.1 * 1_000_000_000.0) as u64; // 10% of detected amount
                    
                    if let Err(e) = swap_engine.execute_swap(
                        &mint.to_string(),
                        follow_amount,
                        SwapDirection::Buy
                    ).await {
                        warn!("Failed to execute follow trade: {}", e);
                    }
                }
            }
            TransactionType::Sell if amount_sol > 0.3 => {
                // React to large sell orders
                if let Some(mint) = token_mint {
                    info!(
                        "Detected large sell order: {:.2} SOL for token {}. Consider exit...",
                        amount_sol, mint
                    );
                    
                    // Implement exit strategy if we hold this token
                    // This would require position tracking
                }
            }
            _ => {
                debug!("Transaction type {:?} with amount {:.2} SOL - no action", tx_type, amount_sol);
            }
        }

        Ok(())
    }

    /// Add token to monitoring list
    pub fn add_monitored_token(&self, token: Pubkey) {
        let mut monitored = MONITORED_TOKENS.write();
        monitored.insert(token);
        info!("Added token to monitoring: {}", token);
    }

    /// Remove token from monitoring list
    pub fn remove_monitored_token(&self, token: &Pubkey) {
        let mut monitored = MONITORED_TOKENS.write();
        monitored.remove(token);
        info!("Removed token from monitoring: {}", token);
    }

    /// Get monitoring statistics
    pub fn get_statistics(&self) -> std::collections::HashMap<String, u64> {
        let mut stats = std::collections::HashMap::new();
        stats.insert("total_messages".to_string(), MESSAGE_COUNTER.load(Ordering::SeqCst));
        stats.insert("relevant_transactions".to_string(), RELEVANT_TX_COUNTER.load(Ordering::SeqCst));
        stats.insert("connection_retries".to_string(), CONNECTION_RETRY_COUNT.load(Ordering::SeqCst));
        stats.insert("processed_transactions".to_string(), PROCESSED_TRANSACTIONS.len() as u64);
        stats.insert("monitored_tokens".to_string(), MONITORED_TOKENS.read().len() as u64);
        stats
    }
}

/// Monitor function for pump.fun transactions
pub async fn pumpfun_monitor(
    rpc_wss: &str,
    state: AppState,
    slippage: u64,
    use_jito: bool,
) {
    let logger = Logger::new("[PUMP.FUN MONITOR] => ".to_string());
    logger.log("Starting Pump.fun monitoring...".to_string());

    // Initialize monitoring with enhanced configuration
    let monitor = match HeliusMonitor::new(&state, rpc_wss.to_string()) {
        Ok(monitor) => monitor,
        Err(e) => {
            logger.log(format!("Failed to initialize Helius monitor: {}", e));
            return;
        }
    };

    // Start monitoring
    if let Err(e) = monitor.start_monitoring().await {
        logger.log(format!("Monitoring failed: {}", e));
    }
}

/// Monitor function for Raydium transactions
pub async fn raydium_monitor(
    rpc_wss: &str,
    state: AppState,
    slippage: u64,
    use_jito: bool,
) {
    let logger = Logger::new("[RAYDIUM MONITOR] => ".to_string());
    logger.log("Starting Raydium monitoring...".to_string());

    // Initialize monitoring
    let monitor = match HeliusMonitor::new(&state, rpc_wss.to_string()) {
        Ok(monitor) => monitor,
        Err(e) => {
            logger.log(format!("Failed to initialize Helius monitor: {}", e));
            return;
        }
    };

    // Start monitoring
    if let Err(e) = monitor.start_monitoring().await {
        logger.log(format!("Monitoring failed: {}", e));
    }
}

/// Clean up old processed transactions
pub async fn cleanup_old_transactions() {
    let cutoff = Instant::now() - Duration::from_secs(600); // 10 minutes

    let old_signatures: Vec<String> = PROCESSED_TRANSACTIONS
        .iter()
        .filter(|entry| entry.processed_at < cutoff)
        .map(|entry| entry.key().clone())
        .collect();

    for signature in old_signatures {
        PROCESSED_TRANSACTIONS.remove(&signature);
    }
    
    if !old_signatures.is_empty() {
        debug!("Cleaned up {} old transaction records", old_signatures.len());
    }
}

/// Start background cleanup task
pub fn start_cleanup_task() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
        loop {
            interval.tick().await;
            cleanup_old_transactions().await;
        }
    });
}
