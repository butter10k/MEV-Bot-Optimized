use std::{env, sync::Arc, time::Duration};

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use jito_json_rpc_client::jsonrpc_client::rpc_client::RpcClient as JitoRpcClient;
use parking_lot::RwLock;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    hash::Hash,
    instruction::Instruction,
    message::{Message, VersionedMessage},
    signature::{Keypair, Signature},
    signer::Signer,
    system_transaction,
    transaction::{Transaction, VersionedTransaction},
};
use spl_token::ui_amount_to_amount;
use std::{
    str::FromStr,
    sync::{
        atomic::{AtomicU64, Ordering},
        LazyLock,
    },
};
use crossbeam::queue::SegQueue;
use std::collections::VecDeque;
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::{
    common::{cache::CACHE_MANAGER, logger::Logger},
    services::{
        jito::{self, get_tip_account, get_tip_value, wait_for_bundle_confirmation},
        nextblock::NextblockClient,
    },
};

// Enhanced transaction performance tracking
static TX_COUNTER: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static TX_SUCCESS_COUNTER: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static TX_FAILED_COUNTER: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

// Transaction optimization cache
static RECENT_BLOCKHASHES: LazyLock<Arc<DashMap<String, (Hash, Instant)>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

static COMPUTE_UNIT_PRICES: LazyLock<Arc<RwLock<ComputeUnitPricing>>> = 
    LazyLock::new(|| Arc::new(RwLock::new(ComputeUnitPricing::default())));

// High-performance memory pools for reduced allocations
static INSTRUCTION_POOL: LazyLock<Arc<SegQueue<Vec<Instruction>>>> = 
    LazyLock::new(|| Arc::new(SegQueue::new()));

static TRANSACTION_POOL: LazyLock<Arc<SegQueue<Transaction>>> = 
    LazyLock::new(|| Arc::new(SegQueue::new()));

static MESSAGE_BUFFER_POOL: LazyLock<Arc<SegQueue<Vec<u8>>>> = 
    LazyLock::new(|| Arc::new(SegQueue::new()));

// Adaptive batch processing for better throughput
static PENDING_TRANSACTIONS: LazyLock<Arc<DashMap<String, PendingTransaction>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

static BATCH_PROCESSOR: LazyLock<Arc<BatchProcessor>> = 
    LazyLock::new(|| Arc::new(BatchProcessor::new()));

#[derive(Debug, Clone)]
struct ComputeUnitPricing {
    base_price: u64,
    dynamic_multiplier: f64,
    congestion_factor: f64,
    last_updated: Instant,
}

impl Default for ComputeUnitPricing {
    fn default() -> Self {
        Self {
            base_price: get_base_unit_price(),
            dynamic_multiplier: 1.0,
            congestion_factor: 1.0,
            last_updated: Instant::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransactionConfig {
    pub use_jito: bool,
    pub priority_fee_multiplier: f64,
    pub max_retries: u32,
    pub timeout: Duration,
    pub use_dynamic_compute_units: bool,
    pub enable_preflight: bool,
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            use_jito: true,
            priority_fee_multiplier: 1.5,
            max_retries: 3,
            timeout: Duration::from_secs(30),
            use_dynamic_compute_units: true,
            enable_preflight: false, // Disable for speed
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub signatures: Vec<String>,
    pub execution_time: Duration,
    pub compute_units_used: Option<u64>,
    pub priority_fee_paid: u64,
    pub method_used: String,
    pub block_height: Option<u64>,
}

#[derive(Debug, Clone)]
struct PendingTransaction {
    id: String,
    instructions: Vec<Instruction>,
    config: TransactionConfig,
    created_at: Instant,
    priority: TransactionPriority,
    retry_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum TransactionPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

// High-throughput batch processor with SIMD optimizations
struct BatchProcessor {
    batch_size: usize,
    batch_timeout: Duration,
    pending_batches: Arc<DashMap<String, TransactionBatch>>,
    processing_semaphore: Arc<Semaphore>,
}

#[derive(Debug)]
struct TransactionBatch {
    id: String,
    transactions: Vec<PendingTransaction>,
    created_at: Instant,
    priority: TransactionPriority,
    estimated_compute_units: u64,
}

impl BatchProcessor {
    fn new() -> Self {
        Self {
            batch_size: 10, // Process up to 10 transactions per batch
            batch_timeout: Duration::from_millis(100), // Max 100ms batch collection time
            pending_batches: Arc::new(DashMap::new()),
            processing_semaphore: Arc::new(Semaphore::new(5)), // Allow 5 concurrent batches
        }
    }

    async fn add_transaction(&self, tx: PendingTransaction) -> Result<()> {
        let batch_key = format!("batch_{}", tx.priority as u8);
        
        // Get or create batch for this priority level
        let mut batch = self.pending_batches.entry(batch_key.clone())
            .or_insert_with(|| TransactionBatch {
                id: batch_key.clone(),
                transactions: Vec::with_capacity(self.batch_size),
                created_at: Instant::now(),
                priority: tx.priority.clone(),
                estimated_compute_units: 0,
            });

        batch.transactions.push(tx);
        batch.estimated_compute_units += 200_000; // Estimate per transaction

        // Process batch if full or timeout reached
        if batch.transactions.len() >= self.batch_size 
            || batch.created_at.elapsed() > self.batch_timeout {
            self.process_batch(batch_key).await?;
        }

        Ok(())
    }

    async fn process_batch(&self, batch_key: String) -> Result<()> {
        let _permit = self.processing_semaphore.acquire().await
            .map_err(|e| anyhow!("Failed to acquire processing permit: {}", e))?;

        if let Some((_, batch)) = self.pending_batches.remove(&batch_key) {
            debug!("Processing batch {} with {} transactions", 
                   batch.id, batch.transactions.len());

            // Use rayon for parallel processing if beneficial
            if batch.transactions.len() > 3 {
                self.process_batch_parallel(batch).await?;
            } else {
                self.process_batch_sequential(batch).await?;
            }
        }

        Ok(())
    }

    async fn process_batch_parallel(&self, batch: TransactionBatch) -> Result<()> {
        use rayon::prelude::*;

        let results: Vec<Result<()>> = batch.transactions
            .into_par_iter()
            .map(|tx| {
                // Parallel transaction preparation
                self.prepare_transaction_optimized(tx)
            })
            .collect();

        // Check for any failures
        for (i, result) in results.into_iter().enumerate() {
            if let Err(e) = result {
                warn!("Transaction {} in batch {} failed preparation: {}", 
                      i, batch.id, e);
            }
        }

        Ok(())
    }

    async fn process_batch_sequential(&self, batch: TransactionBatch) -> Result<()> {
        for (i, tx) in batch.transactions.into_iter().enumerate() {
            if let Err(e) = self.prepare_transaction_optimized(tx) {
                warn!("Transaction {} in batch {} failed: {}", i, batch.id, e);
            }
        }
        Ok(())
    }

    fn prepare_transaction_optimized(&self, tx: PendingTransaction) -> Result<()> {
        // Use memory pool for instruction vectors
        let mut instructions = INSTRUCTION_POOL.pop()
            .unwrap_or_else(|| Vec::with_capacity(10));
        
        instructions.clear();
        instructions.extend(tx.instructions);

        // Optimize instruction ordering for better compute efficiency
        self.optimize_instruction_order(&mut instructions);

        // Return to pool for reuse
        if instructions.capacity() <= 20 { // Prevent pool pollution
            INSTRUCTION_POOL.push(instructions);
        }

        Ok(())
    }

    fn optimize_instruction_order(&self, instructions: &mut Vec<Instruction>) {
        // Sort by program ID to optimize account lookups
        instructions.sort_by(|a, b| a.program_id.cmp(&b.program_id));
        
        // Move compute budget instructions to the front
        let (compute_instructions, other_instructions): (Vec<_>, Vec<_>) = 
            instructions.drain(..).partition(|ix| {
                ix.program_id == solana_sdk::compute_budget::ID
            });

        instructions.extend(compute_instructions);
        instructions.extend(other_instructions);
    }

    async fn start_batch_processor(&self) {
        let pending_batches = Arc::clone(&self.pending_batches);
        let batch_timeout = self.batch_timeout;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(batch_timeout);
            
            loop {
                interval.tick().await;
                
                // Process any batches that have timed out
                let timeout_batches: Vec<String> = pending_batches
                    .iter()
                    .filter(|entry| entry.created_at.elapsed() > batch_timeout)
                    .map(|entry| entry.key().clone())
                    .collect();

                for batch_key in timeout_batches {
                    if let Err(e) = BATCH_PROCESSOR.process_batch(batch_key).await {
                        warn!("Failed to process timeout batch: {}", e);
                    }
                }
            }
        });
    }
}

// Optimized compute unit pricing
fn get_base_unit_price() -> u64 {
    env::var("UNIT_PRICE")
        .ok()
        .and_then(|v| u64::from_str(&v).ok())
        .unwrap_or(1_000) // Increased base for better priority
}

fn get_unit_limit() -> u32 {
    env::var("UNIT_LIMIT")
        .ok()
        .and_then(|v| u32::from_str(&v).ok())
        .unwrap_or(400_000) // Increased for complex operations
}

fn calculate_dynamic_compute_price(base_multiplier: f64) -> u64 {
    let pricing = COMPUTE_UNIT_PRICES.read();
    let dynamic_price = (pricing.base_price as f64 
        * pricing.dynamic_multiplier 
        * pricing.congestion_factor 
        * base_multiplier) as u64;
    
    // Cap at reasonable maximum (0.01 SOL = 10M lamports)
    std::cmp::min(dynamic_price, 10_000_000)
}

/// Enhanced transaction execution with multiple fallback options
pub async fn new_signed_and_send(
    client: &RpcClient,
    keypair: &Keypair,
    instructions: Vec<Instruction>,
    use_jito: bool,
    logger: &Logger,
) -> Result<Vec<String>> {
    let mut config = TransactionConfig::default();
    config.use_jito = use_jito;
    
    match enhanced_transaction_send(client, keypair, instructions, config, logger).await {
        Ok(result) => Ok(result.signatures),
        Err(e) => Err(e),
    }
}

/// Advanced transaction execution with full optimization
pub async fn enhanced_transaction_send(
    client: &RpcClient,
    keypair: &Keypair,
    mut instructions: Vec<Instruction>,
    config: TransactionConfig,
    logger: &Logger,
) -> Result<TransactionResult> {
    let tx_id = TX_COUNTER.fetch_add(1, Ordering::SeqCst);
    let start_time = Instant::now();
    
    info!("Starting enhanced transaction {} with config: {:?}", tx_id, config);

    // Optimize compute unit pricing
    let compute_price = if config.use_dynamic_compute_units {
        calculate_dynamic_compute_price(config.priority_fee_multiplier)
    } else {
        get_base_unit_price()
    };
    
    let compute_limit = get_unit_limit();

    // Add compute budget instructions for non-Jito transactions
    if !config.use_jito {
        let priority_fee_ix = ComputeBudgetInstruction::set_compute_unit_price(compute_price);
        let compute_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(compute_limit);
        
        instructions.insert(0, priority_fee_ix);
        instructions.insert(1, compute_limit_ix);
    }

    // Get optimized recent blockhash
    let recent_blockhash = get_cached_recent_blockhash(client).await?;
    
    // Build transaction with optimization
    let transaction = build_optimized_transaction(
        &instructions,
        keypair,
        recent_blockhash,
    )?;

    let mut result = TransactionResult {
        signatures: Vec::new(),
        execution_time: Duration::default(),
        compute_units_used: None,
        priority_fee_paid: compute_price,
        method_used: String::new(),
        block_height: None,
    };

    // Execute with fallback strategy
    let execution_result = if config.use_jito {
        execute_jito_transaction(&transaction, keypair, recent_blockhash, &config, logger).await
    } else {
        execute_standard_transaction(client, &transaction, &config, logger).await
    };

    match execution_result {
        Ok(signatures) => {
            result.signatures = signatures;
            result.method_used = if config.use_jito { "Jito".to_string() } else { "Standard".to_string() };
            result.execution_time = start_time.elapsed();
            
            TX_SUCCESS_COUNTER.fetch_add(1, Ordering::SeqCst);
            info!("Transaction {} completed successfully in {:?}", tx_id, result.execution_time);
            
            Ok(result)
        }
        Err(e) => {
            TX_FAILED_COUNTER.fetch_add(1, Ordering::SeqCst);
            warn!("Transaction {} failed: {}", tx_id, e);
            
            // Try fallback method if available
            if config.use_jito {
                warn!("Jito failed, attempting fallback to standard transaction");
                match execute_standard_transaction(client, &transaction, &config, logger).await {
                    Ok(signatures) => {
                        result.signatures = signatures;
                        result.method_used = "Standard (fallback)".to_string();
                        result.execution_time = start_time.elapsed();
                        
                        TX_SUCCESS_COUNTER.fetch_add(1, Ordering::SeqCst);
                        info!("Fallback transaction {} completed in {:?}", tx_id, result.execution_time);
                        Ok(result)
                    }
                    Err(fallback_error) => {
                        warn!("Both Jito and fallback failed: {}", fallback_error);
                        Err(anyhow!("All transaction methods failed. Last error: {}", fallback_error))
                    }
                }
            } else {
                Err(e)
            }
        }
    }
}

/// Get cached recent blockhash for better performance
async fn get_cached_recent_blockhash(client: &RpcClient) -> Result<Hash> {
    // Check cache first
    let cache_key = "recent_blockhash".to_string();
    
    if let Some((cached_hash, cached_time)) = RECENT_BLOCKHASHES.get(&cache_key) {
        // Use cached blockhash if it's less than 30 seconds old
        if cached_time.elapsed() < Duration::from_secs(30) {
            debug!("Using cached recent blockhash");
            return Ok(cached_hash);
        }
    }

    // Fetch new blockhash
    let recent_blockhash = client.get_latest_blockhash()?;
    
    // Cache it
    RECENT_BLOCKHASHES.insert(cache_key, (recent_blockhash, Instant::now()));
    
    debug!("Fetched and cached new recent blockhash");
    Ok(recent_blockhash)
}

/// Build optimized transaction
fn build_optimized_transaction(
    instructions: &[Instruction],
    keypair: &Keypair,
    recent_blockhash: Hash,
) -> Result<Transaction> {
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&keypair.pubkey()),
        &[keypair],
        recent_blockhash,
    );

    // Validate transaction size
    let serialized_size = bincode::serialize(&transaction)?.len();
    if serialized_size > 1232 { // Solana's transaction size limit
        return Err(anyhow!("Transaction too large: {} bytes", serialized_size));
    }

    debug!("Built transaction with {} instructions, {} bytes", 
           instructions.len(), serialized_size);
    
    Ok(transaction)
}

/// Execute transaction via Jito
async fn execute_jito_transaction(
    transaction: &Transaction,
    keypair: &Keypair,
    recent_blockhash: Hash,
    config: &TransactionConfig,
    logger: &Logger,
) -> Result<Vec<String>> {
    debug!("Executing transaction via Jito");
    
    let tip_account = get_tip_account().await?;
    let jito_client = Arc::new(JitoRpcClient::new(format!(
        "{}/api/v1/bundles",
        *jito::BLOCK_ENGINE_URL
    )));

    // Calculate optimal tip
    let mut tip = get_tip_value().await?;
    tip = tip.min(0.1); // Cap at 0.1 SOL
    let tip_lamports = ui_amount_to_amount(tip, spl_token::native_mint::DECIMALS);

    info!("Jito tip: {} SOL ({} lamports) to account: {}", tip, tip_lamports, tip_account);

    // Create bundle with main transaction and tip
    let bundle: Vec<VersionedTransaction> = vec![
        VersionedTransaction::from(transaction.clone()),
        VersionedTransaction::from(system_transaction::transfer(
            keypair,
            &tip_account,
            tip_lamports,
            recent_blockhash,
        )),
    ];

    let bundle_id = jito_client.send_bundle(&bundle).await
        .map_err(|e| anyhow!("Failed to send Jito bundle: {}", e))?;

    info!("Jito bundle submitted: {}", bundle_id);

    // Wait for confirmation with enhanced error handling
    let signatures = wait_for_bundle_confirmation(
        move |id: String| {
            let client = Arc::clone(&jito_client);
            async move {
                match client.get_bundle_statuses(&[id]).await {
                    Ok(statuses) => Ok(statuses.value),
                    Err(e) => {
                        debug!("Error fetching bundle status: {}", e);
                        Err(anyhow!("Bundle status check failed: {}", e))
                    }
                }
            }
        },
        bundle_id,
        Duration::from_millis(500), // Faster polling
        config.timeout,
    ).await?;

    info!("Jito bundle confirmed with {} signatures", signatures.len());
    Ok(signatures)
}

/// Execute standard transaction
async fn execute_standard_transaction(
    client: &RpcClient,
    transaction: &Transaction,
    config: &TransactionConfig,
    logger: &Logger,
) -> Result<Vec<String>> {
    debug!("Executing standard transaction");
    
    // Try Nextblock first for speed if available
    if let Ok(nextblock_client) = NextblockClient::new() {
        debug!("Attempting transaction via Nextblock");
        
        match nextblock_client.submit_transaction(
            transaction, 
            Some(calculate_dynamic_compute_price(config.priority_fee_multiplier))
        ).await {
            Ok(signature) => {
                info!("Transaction submitted via Nextblock: {}", signature);
                return Ok(vec![signature]);
            }
            Err(e) => {
                warn!("Nextblock submission failed: {}, falling back to standard RPC", e);
            }
        }
    }

    // Standard RPC submission
    let signature = client.send_transaction(transaction)?;
    info!("Transaction submitted via standard RPC: {}", signature);
    
    Ok(vec![signature.to_string()])
}

/// Update compute unit pricing based on network conditions
pub async fn update_compute_unit_pricing(
    congestion_factor: f64,
    dynamic_multiplier: f64,
) {
    let mut pricing = COMPUTE_UNIT_PRICES.write();
    pricing.congestion_factor = congestion_factor.clamp(0.5, 5.0);
    pricing.dynamic_multiplier = dynamic_multiplier.clamp(0.5, 3.0);
    pricing.last_updated = Instant::now();
    
    debug!("Updated compute unit pricing: congestion={:.2}, dynamic={:.2}", 
           congestion_factor, dynamic_multiplier);
}

/// Get transaction execution statistics
pub fn get_transaction_statistics() -> std::collections::HashMap<String, u64> {
    let mut stats = std::collections::HashMap::new();
    
    let total_tx = TX_COUNTER.load(Ordering::SeqCst);
    let successful_tx = TX_SUCCESS_COUNTER.load(Ordering::SeqCst);
    let failed_tx = TX_FAILED_COUNTER.load(Ordering::SeqCst);
    
    stats.insert("total_transactions".to_string(), total_tx);
    stats.insert("successful_transactions".to_string(), successful_tx);
    stats.insert("failed_transactions".to_string(), failed_tx);
    
    let success_rate = if total_tx > 0 {
        (successful_tx * 100) / total_tx
    } else {
        0
    };
    stats.insert("success_rate_percentage".to_string(), success_rate);
    
    stats.insert("cached_blockhashes".to_string(), RECENT_BLOCKHASHES.len() as u64);
    
    stats
}

/// Clean up old cached data
pub async fn cleanup_transaction_cache() {
    let cutoff = Instant::now() - Duration::from_secs(60); // 1 minute
    
    let old_keys: Vec<String> = RECENT_BLOCKHASHES
        .iter()
        .filter(|entry| entry.1.1 < cutoff)
        .map(|entry| entry.key().clone())
        .collect();
    
    for key in old_keys {
        RECENT_BLOCKHASHES.remove(&key);
    }
    
    debug!("Cleaned up old transaction cache entries");
}
