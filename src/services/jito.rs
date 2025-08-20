use anyhow::{anyhow, Result};
use dashmap::DashMap;
use jito_json_rpc_client::jsonrpc_client::rpc_client::RpcClient as JitoRpcClient;
use parking_lot::RwLock;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::{
    collections::HashMap,
    env,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock,
    },
    time::{Duration, Instant},
};
use tokio::time::sleep;
use tracing::{debug, info, warn};

// Optimized global constants with lazy initialization
pub static BLOCK_ENGINE_URL: LazyLock<String> = LazyLock::new(|| {
    env::var("JITO_BLOCK_ENGINE_URL")
        .unwrap_or_else(|_| "https://ny.mainnet.block-engine.jito.wtf".to_string())
});

pub static TIP_STREAM_URL: LazyLock<String> = LazyLock::new(|| {
    env::var("JITO_TIP_STREAM_URL")
        .unwrap_or_else(|_| "ws://bundles-api-rest.jito.wtf/api/v1/bundles/tip_stream".to_string())
});

// High-performance caching for tip accounts and values
static TIP_ACCOUNTS: LazyLock<Vec<Pubkey>> = LazyLock::new(|| {
    vec![
        Pubkey::from_str("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5").unwrap(),
        Pubkey::from_str("HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe").unwrap(),
        Pubkey::from_str("Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY").unwrap(),
        Pubkey::from_str("ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49").unwrap(),
        Pubkey::from_str("DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh").unwrap(),
        Pubkey::from_str("ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt").unwrap(),
        Pubkey::from_str("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL").unwrap(),
        Pubkey::from_str("3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT").unwrap(),
    ]
});

// Performance-optimized caching structures
static TIP_CACHE: LazyLock<Arc<RwLock<Option<(f64, Instant)>>>> = 
    LazyLock::new(|| Arc::new(RwLock::new(None)));

static TIP_ACCOUNT_INDEX: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

// Bundle tracking for better success rates
static PENDING_BUNDLES: LazyLock<Arc<DashMap<String, BundleTracker>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

#[derive(Debug, Clone)]
struct BundleTracker {
    bundle_id: String,
    submitted_at: Instant,
    retry_count: u32,
    signatures: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TipResponse {
    #[serde(rename = "25th_percentile")]
    pub percentile_25: f64,
    #[serde(rename = "50th_percentile")]
    pub percentile_50: f64,
    #[serde(rename = "75th_percentile")]
    pub percentile_75: f64,
    #[serde(rename = "95th_percentile")]
    pub percentile_95: f64,
    #[serde(rename = "99th_percentile")]
    pub percentile_99: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BundleStatus {
    pub bundle_id: String,
    pub transactions: Vec<TransactionStatus>,
    pub slot: Option<u64>,
    pub confirmation_status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionStatus {
    pub signature: String,
    pub confirmed: bool,
    pub err: Option<serde_json::Value>,
}

/// Initialize tip accounts for optimal performance
pub async fn init_tip_accounts() -> Result<()> {
    info!("Initializing Jito tip accounts...");
    
    // Pre-warm the tip cache
    let _ = get_tip_value().await;
    
    info!("Jito tip accounts initialized with {} accounts", TIP_ACCOUNTS.len());
    Ok(())
}

/// Get optimized tip account with round-robin selection
pub async fn get_tip_account() -> Result<Pubkey> {
    let index = TIP_ACCOUNT_INDEX.fetch_add(1, Ordering::SeqCst) as usize;
    let tip_account = TIP_ACCOUNTS[index % TIP_ACCOUNTS.len()];
    
    debug!("Selected tip account: {} (index: {})", tip_account, index % TIP_ACCOUNTS.len());
    Ok(tip_account)
}

/// Get tip value with intelligent caching (cache for 30 seconds)
pub async fn get_tip_value() -> Result<f64> {
    const CACHE_DURATION: Duration = Duration::from_secs(30);
    
    // Check cache first
    {
        let cache = TIP_CACHE.read();
        if let Some((cached_tip, cached_at)) = cache.as_ref() {
            if cached_at.elapsed() < CACHE_DURATION {
                debug!("Using cached tip value: {}", cached_tip);
                return Ok(*cached_tip);
            }
        }
    }
    
    // Fetch new tip value
    let tip = fetch_tip_value().await?;
    
    // Update cache
    {
        let mut cache = TIP_CACHE.write();
        *cache = Some((tip, Instant::now()));
    }
    
    info!("Updated tip value: {:.6} SOL", tip);
    Ok(tip)
}

/// Fetch tip value from Jito API with retry logic
async fn fetch_tip_value() -> Result<f64> {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY: Duration = Duration::from_millis(100);
    
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    
    let percentile = env::var("JITO_TIP_PERCENTILE")
        .unwrap_or_else(|_| "50".to_string())
        .parse::<u32>()
        .unwrap_or(50);
    
    for attempt in 1..=MAX_RETRIES {
        match client
            .get(&format!("{}/api/v1/bundles/tip_floor", *BLOCK_ENGINE_URL))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    let tip_response: TipResponse = response.json().await?;
                    
                    let tip = match percentile {
                        25 => tip_response.percentile_25,
                        50 => tip_response.percentile_50,
                        75 => tip_response.percentile_75,
                        95 => tip_response.percentile_95,
                        99 => tip_response.percentile_99,
                        _ => tip_response.percentile_50,
                    };
                    
                    return Ok(tip / 1_000_000_000.0); // Convert lamports to SOL
                }
            }
            Err(e) => {
                warn!("Attempt {} failed: {}", attempt, e);
                if attempt < MAX_RETRIES {
                    sleep(RETRY_DELAY).await;
                }
            }
        }
    }
    
    // Fallback to a reasonable default tip
    warn!("Failed to fetch tip value, using fallback: 0.001 SOL");
    Ok(0.001)
}

/// Enhanced bundle confirmation with better error handling and performance tracking
pub async fn wait_for_bundle_confirmation<F, Fut>(
    get_bundle_statuses: F,
    bundle_id: String,
    poll_interval: Duration,
    timeout: Duration,
) -> Result<Vec<String>>
where
    F: Fn(String) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<Vec<BundleStatus>>> + Send,
{
    let start_time = Instant::now();
    let tracker = BundleTracker {
        bundle_id: bundle_id.clone(),
        submitted_at: start_time,
        retry_count: 0,
        signatures: Vec::new(),
    };
    
    PENDING_BUNDLES.insert(bundle_id.clone(), tracker);
    
    info!("Waiting for bundle confirmation: {}", bundle_id);
    
    while start_time.elapsed() < timeout {
        match get_bundle_statuses(bundle_id.clone()).await {
            Ok(statuses) => {
                for status in statuses {
                    match status.confirmation_status.as_str() {
                        "confirmed" | "finalized" => {
                            let signatures: Vec<String> = status
                                .transactions
                                .into_iter()
                                .filter(|tx| tx.confirmed && tx.err.is_none())
                                .map(|tx| tx.signature)
                                .collect();
                            
                            if !signatures.is_empty() {
                                info!(
                                    "Bundle {} confirmed in {:?} with {} successful transactions",
                                    bundle_id,
                                    start_time.elapsed(),
                                    signatures.len()
                                );
                                
                                // Update tracker before removal
                                if let Some(mut tracker) = PENDING_BUNDLES.get_mut(&bundle_id) {
                                    tracker.signatures = signatures.clone();
                                }
                                
                                PENDING_BUNDLES.remove(&bundle_id);
                                return Ok(signatures);
                            }
                        }
                        "rejected" | "failed" => {
                            warn!("Bundle {} was rejected/failed", bundle_id);
                            PENDING_BUNDLES.remove(&bundle_id);
                            return Err(anyhow!("Bundle was rejected or failed"));
                        }
                        _ => {
                            debug!("Bundle {} status: {}", bundle_id, status.confirmation_status);
                        }
                    }
                }
            }
            Err(e) => {
                debug!("Error checking bundle status: {}", e);
            }
        }
        
        sleep(poll_interval).await;
    }
    
    warn!("Bundle {} confirmation timeout after {:?}", bundle_id, timeout);
    PENDING_BUNDLES.remove(&bundle_id);
    Err(anyhow!("Bundle confirmation timeout"))
}

/// Get statistics about pending bundles for monitoring
pub fn get_bundle_statistics() -> HashMap<String, u32> {
    let mut stats = HashMap::new();
    stats.insert("pending_bundles".to_string(), PENDING_BUNDLES.len() as u32);
    
    let now = Instant::now();
    let old_bundles = PENDING_BUNDLES
        .iter()
        .filter(|entry| now.duration_since(entry.submitted_at) > Duration::from_secs(60))
        .count();
    
    stats.insert("old_bundles".to_string(), old_bundles as u32);
    stats
}

/// Clean up old pending bundles to prevent memory leaks
pub async fn cleanup_old_bundles() {
    let cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes
    
    let old_bundle_ids: Vec<String> = PENDING_BUNDLES
        .iter()
        .filter(|entry| entry.submitted_at < cutoff)
        .map(|entry| entry.bundle_id.clone())
        .collect();
    
    for bundle_id in old_bundle_ids {
        PENDING_BUNDLES.remove(&bundle_id);
        debug!("Cleaned up old bundle: {}", bundle_id);
    }
}

/// Start background task for periodic cleanup
pub fn start_cleanup_task() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_old_bundles().await;
        }
    });
}
