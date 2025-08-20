use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
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

// Global performance metrics
static PERFORMANCE_METRICS: LazyLock<Arc<PerformanceMonitor>> = 
    LazyLock::new(|| Arc::new(PerformanceMonitor::new()));

// High-frequency counters for performance tracking
static TRANSACTION_LATENCY_HISTOGRAM: LazyLock<Arc<DashMap<u64, AtomicU64>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

static SWAP_EXECUTION_METRICS: LazyLock<Arc<DashMap<String, SwapMetrics>>> = 
    LazyLock::new(|| Arc::new(DashMap::new()));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: u64,
    pub total_transactions: u64,
    pub successful_transactions: u64,
    pub failed_transactions: u64,
    pub avg_execution_time_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub throughput_per_second: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub cache_hit_rate: f64,
    pub dex_performance: HashMap<String, DexPerformance>,
    pub network_conditions: NetworkConditions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexPerformance {
    pub success_rate: f64,
    pub avg_execution_time_ms: f64,
    pub total_volume_sol: f64,
    pub avg_price_impact: f64,
    pub transaction_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConditions {
    pub avg_confirmation_time_ms: f64,
    pub current_slot: u64,
    pub estimated_tps: f64,
    pub congestion_level: CongestionLevel,
    pub priority_fee_recommendations: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CongestionLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
struct SwapMetrics {
    execution_time: Duration,
    success: bool,
    dex_used: String,
    amount: u64,
    price_impact: f64,
    timestamp: Instant,
}

pub struct PerformanceMonitor {
    start_time: Instant,
    total_operations: AtomicU64,
    successful_operations: AtomicU64,
    failed_operations: AtomicU64,
    execution_times: Arc<RwLock<Vec<Duration>>>,
    dex_metrics: Arc<DashMap<String, DexMetrics>>,
    system_metrics: Arc<RwLock<SystemMetrics>>,
    monitoring_interval: Duration,
}

#[derive(Debug, Clone)]
struct DexMetrics {
    total_swaps: AtomicU64,
    successful_swaps: AtomicU64,
    total_volume: AtomicU64,
    total_execution_time: AtomicU64,
    price_impact_sum: Arc<RwLock<f64>>,
}

#[derive(Debug, Clone)]
struct SystemMetrics {
    memory_usage_mb: f64,
    cpu_usage_percent: f64,
    network_latency_ms: f64,
    last_updated: Instant,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            total_operations: AtomicU64::new(0),
            successful_operations: AtomicU64::new(0),
            failed_operations: AtomicU64::new(0),
            execution_times: Arc::new(RwLock::new(Vec::with_capacity(10000))),
            dex_metrics: Arc::new(DashMap::new()),
            system_metrics: Arc::new(RwLock::new(SystemMetrics {
                memory_usage_mb: 0.0,
                cpu_usage_percent: 0.0,
                network_latency_ms: 0.0,
                last_updated: Instant::now(),
            })),
            monitoring_interval: Duration::from_secs(10),
        }
    }

    pub async fn record_transaction_start(&self) -> TransactionTimer {
        self.total_operations.fetch_add(1, Ordering::SeqCst);
        TransactionTimer::new()
    }

    pub async fn record_transaction_end(
        &self,
        timer: TransactionTimer,
        success: bool,
        dex: &str,
        amount: u64,
        price_impact: f64,
    ) {
        let execution_time = timer.elapsed();

        if success {
            self.successful_operations.fetch_add(1, Ordering::SeqCst);
        } else {
            self.failed_operations.fetch_add(1, Ordering::SeqCst);
        }

        // Record execution time
        {
            let mut times = self.execution_times.write().await;
            times.push(execution_time);
            
            // Keep only last 10,000 measurements
            if times.len() > 10000 {
                times.remove(0);
            }
        }

        // Update DEX-specific metrics
        let dex_metrics = self.dex_metrics.entry(dex.to_string())
            .or_insert_with(|| DexMetrics {
                total_swaps: AtomicU64::new(0),
                successful_swaps: AtomicU64::new(0),
                total_volume: AtomicU64::new(0),
                total_execution_time: AtomicU64::new(0),
                price_impact_sum: Arc::new(RwLock::new(0.0)),
            });

        dex_metrics.total_swaps.fetch_add(1, Ordering::SeqCst);
        if success {
            dex_metrics.successful_swaps.fetch_add(1, Ordering::SeqCst);
        }
        dex_metrics.total_volume.fetch_add(amount, Ordering::SeqCst);
        dex_metrics.total_execution_time.fetch_add(execution_time.as_millis() as u64, Ordering::SeqCst);
        
        {
            let mut impact_sum = dex_metrics.price_impact_sum.write().await;
            *impact_sum += price_impact;
        }

        // Record in histogram for percentile calculations
        let latency_bucket = (execution_time.as_millis() / 10) * 10; // 10ms buckets
        TRANSACTION_LATENCY_HISTOGRAM
            .entry(latency_bucket as u64)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::SeqCst);

        debug!("Recorded transaction: success={}, dex={}, time={:?}, amount={}, impact={:.2}%",
               success, dex, execution_time, amount, price_impact);
    }

    pub async fn get_performance_snapshot(&self) -> PerformanceSnapshot {
        let total_ops = self.total_operations.load(Ordering::SeqCst);
        let successful_ops = self.successful_operations.load(Ordering::SeqCst);
        let failed_ops = self.failed_operations.load(Ordering::SeqCst);

        let execution_times = self.execution_times.read().await;
        let avg_execution_time = if !execution_times.is_empty() {
            execution_times.iter().map(|d| d.as_millis() as f64).sum::<f64>() / execution_times.len() as f64
        } else {
            0.0
        };

        let (p50, p95, p99) = self.calculate_percentiles(&execution_times).await;

        let runtime_duration = self.start_time.elapsed().as_secs_f64();
        let throughput = if runtime_duration > 0.0 {
            total_ops as f64 / runtime_duration
        } else {
            0.0
        };

        let dex_performance = self.get_dex_performance().await;
        let network_conditions = self.get_network_conditions().await;
        let system_metrics = self.system_metrics.read().await;

        PerformanceSnapshot {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            total_transactions: total_ops,
            successful_transactions: successful_ops,
            failed_transactions: failed_ops,
            avg_execution_time_ms: avg_execution_time,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            throughput_per_second: throughput,
            memory_usage_mb: system_metrics.memory_usage_mb,
            cpu_usage_percent: system_metrics.cpu_usage_percent,
            cache_hit_rate: self.get_cache_hit_rate().await,
            dex_performance,
            network_conditions,
        }
    }

    async fn calculate_percentiles(&self, times: &[Duration]) -> (f64, f64, f64) {
        if times.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        let mut sorted_times: Vec<f64> = times
            .iter()
            .map(|d| d.as_millis() as f64)
            .collect();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let len = sorted_times.len();
        let p50_index = (len as f64 * 0.5) as usize;
        let p95_index = (len as f64 * 0.95) as usize;
        let p99_index = (len as f64 * 0.99) as usize;

        let p50 = sorted_times.get(p50_index).copied().unwrap_or(0.0);
        let p95 = sorted_times.get(p95_index).copied().unwrap_or(0.0);
        let p99 = sorted_times.get(p99_index).copied().unwrap_or(0.0);

        (p50, p95, p99)
    }

    async fn get_dex_performance(&self) -> HashMap<String, DexPerformance> {
        let mut performance = HashMap::new();

        for entry in self.dex_metrics.iter() {
            let dex_name = entry.key().clone();
            let metrics = entry.value();

            let total_swaps = metrics.total_swaps.load(Ordering::SeqCst);
            let successful_swaps = metrics.successful_swaps.load(Ordering::SeqCst);
            let total_volume = metrics.total_volume.load(Ordering::SeqCst);
            let total_execution_time = metrics.total_execution_time.load(Ordering::SeqCst);

            let success_rate = if total_swaps > 0 {
                successful_swaps as f64 / total_swaps as f64
            } else {
                0.0
            };

            let avg_execution_time = if total_swaps > 0 {
                total_execution_time as f64 / total_swaps as f64
            } else {
                0.0
            };

            let avg_price_impact = {
                let impact_sum = metrics.price_impact_sum.read().await;
                if total_swaps > 0 {
                    *impact_sum / total_swaps as f64
                } else {
                    0.0
                }
            };

            performance.insert(dex_name, DexPerformance {
                success_rate,
                avg_execution_time_ms: avg_execution_time,
                total_volume_sol: total_volume as f64 / 1_000_000_000.0, // Convert to SOL
                avg_price_impact,
                transaction_count: total_swaps,
            });
        }

        performance
    }

    async fn get_network_conditions(&self) -> NetworkConditions {
        // This would typically be populated from real network monitoring
        NetworkConditions {
            avg_confirmation_time_ms: 400.0,
            current_slot: 250_000_000,
            estimated_tps: 3000.0,
            congestion_level: CongestionLevel::Medium,
            priority_fee_recommendations: {
                let mut fees = HashMap::new();
                fees.insert("low".to_string(), 1_000);
                fees.insert("medium".to_string(), 5_000);
                fees.insert("high".to_string(), 10_000);
                fees.insert("urgent".to_string(), 20_000);
                fees
            },
        }
    }

    async fn get_cache_hit_rate(&self) -> f64 {
        // This would be fetched from the cache manager
        0.85 // Placeholder 85% hit rate
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting performance monitoring system");

        let monitor = Arc::clone(&PERFORMANCE_METRICS);
        let interval = self.monitoring_interval;

        tokio::spawn(async move {
            let mut monitoring_interval = tokio::time::interval(interval);

            loop {
                monitoring_interval.tick().await;

                // Update system metrics
                monitor.update_system_metrics().await;

                // Log performance summary every minute
                if monitor.start_time.elapsed().as_secs() % 60 == 0 {
                    let snapshot = monitor.get_performance_snapshot().await;
                    info!("Performance Summary: {} total ops, {:.1}% success rate, {:.1}ms avg latency, {:.1} ops/sec",
                          snapshot.total_transactions,
                          (snapshot.successful_transactions as f64 / snapshot.total_transactions.max(1) as f64) * 100.0,
                          snapshot.avg_execution_time_ms,
                          snapshot.throughput_per_second);
                }
            }
        });

        Ok(())
    }

    async fn update_system_metrics(&self) {
        // This would typically use system monitoring libraries
        // For now, we'll use placeholder values
        let mut metrics = self.system_metrics.write().await;
        metrics.memory_usage_mb = 512.0; // Placeholder
        metrics.cpu_usage_percent = 25.0; // Placeholder
        metrics.network_latency_ms = 50.0; // Placeholder
        metrics.last_updated = Instant::now();
    }

    pub fn log_performance_alert(&self, message: &str, severity: AlertSeverity) {
        match severity {
            AlertSeverity::Info => info!("PERF: {}", message),
            AlertSeverity::Warning => warn!("PERF WARNING: {}", message),
            AlertSeverity::Critical => {
                warn!("PERF CRITICAL: {}", message);
                // Could trigger additional alerting systems
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

pub struct TransactionTimer {
    start_time: Instant,
}

impl TransactionTimer {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

// Global performance monitoring functions
pub async fn record_transaction_start() -> TransactionTimer {
    PERFORMANCE_METRICS.record_transaction_start().await
}

pub async fn record_transaction_end(
    timer: TransactionTimer,
    success: bool,
    dex: &str,
    amount: u64,
    price_impact: f64,
) {
    PERFORMANCE_METRICS
        .record_transaction_end(timer, success, dex, amount, price_impact)
        .await;
}

pub async fn get_performance_snapshot() -> PerformanceSnapshot {
    PERFORMANCE_METRICS.get_performance_snapshot().await
}

pub async fn start_performance_monitoring() -> Result<()> {
    PERFORMANCE_METRICS.start_monitoring().await
}

pub fn log_performance_alert(message: &str, severity: AlertSeverity) {
    PERFORMANCE_METRICS.log_performance_alert(message, severity);
}

// Adaptive performance optimization based on current conditions
pub async fn get_optimal_batch_size() -> usize {
    let snapshot = get_performance_snapshot().await;
    
    // Adjust batch size based on current performance
    match snapshot.avg_execution_time_ms {
        t if t < 500.0 => 20,   // Fast execution, larger batches
        t if t < 1000.0 => 15,  // Medium execution, medium batches
        t if t < 2000.0 => 10,  // Slow execution, smaller batches
        _ => 5,                 // Very slow, minimal batches
    }
}

pub async fn get_recommended_priority_fee(urgency: &str) -> u64 {
    let snapshot = get_performance_snapshot().await;
    
    snapshot.network_conditions
        .priority_fee_recommendations
        .get(urgency)
        .copied()
        .unwrap_or(5000) // Default 5000 lamports
}

pub async fn should_throttle_operations() -> bool {
    let snapshot = get_performance_snapshot().await;
    
    // Throttle if success rate is too low or latency too high
    snapshot.successful_transactions as f64 / snapshot.total_transactions.max(1) as f64 < 0.8
        || snapshot.p95_latency_ms > 5000.0 // 5 second P95
        || matches!(snapshot.network_conditions.congestion_level, CongestionLevel::Critical)
}
