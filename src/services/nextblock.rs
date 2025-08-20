use anyhow::{anyhow, Result};
use dashmap::DashMap;
use parking_lot::RwLock;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    signature::Signature,
    transaction::{Transaction, VersionedTransaction},
};
use std::{
    env,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock,
    },
    time::{Duration, Instant},
};
use tokio::time::sleep;
use tracing::{debug, info, warn};

// Nextblock API configuration
static NEXTBLOCK_API_URL: LazyLock<String> = LazyLock::new(|| {
    env::var("NEXTBLOCK_API_URL")
        .unwrap_or_else(|_| "https://api.nextblock.io".to_string())
});

static NEXTBLOCK_API_KEY: LazyLock<Option<String>> = LazyLock::new(|| {
    env::var("NEXTBLOCK_API_KEY").ok()
});

// Performance tracking
static TRANSACTION_COUNTER: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));
static PENDING_TRANSACTIONS: LazyLock<Arc<DashMap<String, TransactionTracker>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

#[derive(Debug, Clone)]
struct TransactionTracker {
    signature: String,
    submitted_at: Instant,
    retry_count: u32,
    priority_fee: u64,
}

#[derive(Debug, Serialize)]
struct NextblockTransaction {
    transaction: String, // Base64 encoded transaction
    priority_fee_lamports: Option<u64>,
    skip_preflight: bool,
    max_retries: u32,
}

#[derive(Debug, Deserialize)]
struct NextblockResponse {
    signature: String,
    status: String,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TransactionStatus {
    signature: String,
    confirmed: bool,
    slot: Option<u64>,
    confirmations: Option<u32>,
    err: Option<serde_json::Value>,
}

pub struct NextblockClient {
    http_client: HttpClient,
    api_key: Option<String>,
}

impl NextblockClient {
    pub fn new() -> Result<Self> {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("MEV-Bot/1.0")
            .build()?;

        Ok(Self {
            http_client,
            api_key: NEXTBLOCK_API_KEY.clone(),
        })
    }

    /// Submit a transaction with high priority
    pub async fn submit_transaction(
        &self,
        transaction: &Transaction,
        priority_fee: Option<u64>,
    ) -> Result<String> {
        let tx_count = TRANSACTION_COUNTER.fetch_add(1, Ordering::SeqCst);
        
        // Serialize transaction to base64
        let serialized = bincode::serialize(transaction)
            .map_err(|e| anyhow!("Failed to serialize transaction: {}", e))?;
        let encoded_tx = base64::encode(&serialized);

        let request_body = NextblockTransaction {
            transaction: encoded_tx,
            priority_fee_lamports: priority_fee,
            skip_preflight: false,
            max_retries: 3,
        };

        debug!("Submitting transaction #{} to Nextblock", tx_count);

        let mut request = self
            .http_client
            .post(&format!("{}/v1/submit", *NEXTBLOCK_API_URL))
            .json(&request_body);

        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow!("Failed to submit transaction: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Nextblock API error: {}", error_text));
        }

        let nextblock_response: NextblockResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse response: {}", e))?;

        let signature = nextblock_response.signature;
        
        // Track the transaction
        let tracker = TransactionTracker {
            signature: signature.clone(),
            submitted_at: Instant::now(),
            retry_count: 0,
            priority_fee: priority_fee.unwrap_or(0),
        };

        PENDING_TRANSACTIONS.insert(signature.clone(), tracker);

        info!(
            "Transaction submitted to Nextblock: {} (priority fee: {} lamports)",
            signature,
            priority_fee.unwrap_or(0)
        );

        Ok(signature)
    }

    /// Submit a versioned transaction
    pub async fn submit_versioned_transaction(
        &self,
        transaction: &VersionedTransaction,
        priority_fee: Option<u64>,
    ) -> Result<String> {
        // Convert versioned transaction to base64
        let serialized = bincode::serialize(transaction)
            .map_err(|e| anyhow!("Failed to serialize versioned transaction: {}", e))?;
        let encoded_tx = base64::encode(&serialized);

        let request_body = NextblockTransaction {
            transaction: encoded_tx,
            priority_fee_lamports: priority_fee,
            skip_preflight: false,
            max_retries: 3,
        };

        let mut request = self
            .http_client
            .post(&format!("{}/v1/submit", *NEXTBLOCK_API_URL))
            .json(&request_body);

        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow!("Failed to submit versioned transaction: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Nextblock API error: {}", error_text));
        }

        let nextblock_response: NextblockResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse response: {}", e))?;

        Ok(nextblock_response.signature)
    }

    /// Check transaction status with enhanced error handling
    pub async fn get_transaction_status(&self, signature: &str) -> Result<TransactionStatus> {
        let mut request = self
            .http_client
            .get(&format!("{}/v1/status/{}", *NEXTBLOCK_API_URL, signature));

        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow!("Failed to get transaction status: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Nextblock status API error: {}", error_text));
        }

        let status: TransactionStatus = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse status response: {}", e))?;

        Ok(status)
    }

    /// Wait for transaction confirmation with intelligent polling
    pub async fn wait_for_confirmation(
        &self,
        signature: &str,
        timeout: Duration,
    ) -> Result<TransactionStatus> {
        let start_time = Instant::now();
        let mut poll_interval = Duration::from_millis(500); // Start with 500ms
        const MAX_POLL_INTERVAL: Duration = Duration::from_secs(2);

        info!("Waiting for transaction confirmation: {}", signature);

        while start_time.elapsed() < timeout {
            match self.get_transaction_status(signature).await {
                Ok(status) => {
                    if status.confirmed {
                        if status.err.is_none() {
                            info!(
                                "Transaction {} confirmed in {:?} at slot {}",
                                signature,
                                start_time.elapsed(),
                                status.slot.unwrap_or(0)
                            );
                            PENDING_TRANSACTIONS.remove(signature);
                            return Ok(status);
                        } else {
                            warn!("Transaction {} failed: {:?}", signature, status.err);
                            PENDING_TRANSACTIONS.remove(signature);
                            return Err(anyhow!("Transaction failed: {:?}", status.err));
                        }
                    }
                }
                Err(e) => {
                    debug!("Error checking transaction status: {}", e);
                }
            }

            sleep(poll_interval).await;
            
            // Exponential backoff with cap
            poll_interval = std::cmp::min(poll_interval * 2, MAX_POLL_INTERVAL);
        }

        warn!("Transaction {} confirmation timeout after {:?}", signature, timeout);
        PENDING_TRANSACTIONS.remove(signature);
        Err(anyhow!("Transaction confirmation timeout"))
    }

    /// Submit transaction and wait for confirmation in one call
    pub async fn submit_and_confirm(
        &self,
        transaction: &Transaction,
        priority_fee: Option<u64>,
        timeout: Duration,
    ) -> Result<TransactionStatus> {
        let signature = self.submit_transaction(transaction, priority_fee).await?;
        self.wait_for_confirmation(&signature, timeout).await
    }

    /// Batch submit multiple transactions
    pub async fn submit_batch(
        &self,
        transactions: &[Transaction],
        priority_fees: &[Option<u64>],
    ) -> Result<Vec<String>> {
        if transactions.len() != priority_fees.len() {
            return Err(anyhow!("Transactions and priority fees length mismatch"));
        }

        let mut signatures = Vec::new();
        let futures: Vec<_> = transactions
            .iter()
            .zip(priority_fees.iter())
            .map(|(tx, fee)| self.submit_transaction(tx, *fee))
            .collect();

        let results = futures::future::join_all(futures).await;

        for result in results {
            match result {
                Ok(signature) => signatures.push(signature),
                Err(e) => {
                    warn!("Failed to submit transaction in batch: {}", e);
                    return Err(e);
                }
            }
        }

        info!("Submitted batch of {} transactions", signatures.len());
        Ok(signatures)
    }
}

/// Get statistics about pending transactions
pub fn get_transaction_statistics() -> std::collections::HashMap<String, u64> {
    let mut stats = std::collections::HashMap::new();
    stats.insert("total_submitted".to_string(), TRANSACTION_COUNTER.load(Ordering::SeqCst));
    stats.insert("pending_transactions".to_string(), PENDING_TRANSACTIONS.len() as u64);

    let now = Instant::now();
    let old_transactions = PENDING_TRANSACTIONS
        .iter()
        .filter(|entry| now.duration_since(entry.submitted_at) > Duration::from_secs(60))
        .count();

    stats.insert("old_transactions".to_string(), old_transactions as u64);
    stats
}

/// Clean up old pending transactions
pub async fn cleanup_old_transactions() {
    let cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes

    let old_signatures: Vec<String> = PENDING_TRANSACTIONS
        .iter()
        .filter(|entry| entry.submitted_at < cutoff)
        .map(|entry| entry.signature.clone())
        .collect();

    for signature in old_signatures {
        PENDING_TRANSACTIONS.remove(&signature);
        debug!("Cleaned up old transaction: {}", signature);
    }
}

/// Start background cleanup task
pub fn start_cleanup_task() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(120));
        loop {
            interval.tick().await;
            cleanup_old_transactions().await;
        }
    });
}

// Re-export for convenience
pub use base64;
