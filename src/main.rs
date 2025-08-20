use dotenv::dotenv;
use raydium_pump_snipe_bot::{
    common::{
        logger::Logger,
        performance::{start_performance_monitoring, log_performance_alert, AlertSeverity},
        cache::initialize_cache_system,
        utils::{
            create_nonblocking_rpc_client, create_rpc_client, import_env_var, import_wallet,
            AppState, SwapConfig,
        },
    },
    engine::{
        monitor::{
            helius::{pumpfun_monitor, raydium_monitor, start_cleanup_task as start_helius_cleanup},
            yellowstone::{start_yellowstone_cleanup_task, YellowstoneMonitor},
        },
        swap::{OptimizedSwapConfig, SwapEngine, SwapDirection, SwapInType, start_cleanup_task as start_swap_cleanup},
    },
    services::{
        jito::{self, start_cleanup_task as start_jito_cleanup},
        nextblock::{start_cleanup_task as start_nextblock_cleanup, NextblockClient},
    },
    dex::{pump_fun::Pump, raydium::Raydium},
};
use solana_sdk::signer::Signer;
use std::{env, sync::Arc, time::Duration};
use tokio::time::{sleep, timeout};
use tracing::{info, warn, error};
use tracing_subscriber;

// Global memory allocator optimization
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() -> anyhow::Result<()> {
    // Initialize optimized logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    let logger = Logger::new("[INIT] => ".to_string());
    info!("🚀 Starting Optimized MEV Bot v0.2.0");

    // Load environment variables
    dotenv().ok();
    
    // Enhanced configuration loading with validation
    let config = load_and_validate_config(&logger).await?;
    
    // Initialize high-performance caching system
    initialize_cache_system().await?;
    info!("High-performance caching system initialized");
    
    // Start performance monitoring
    start_performance_monitoring().await?;
    info!("Performance monitoring system started");
    
    // Initialize connection pools and clients
    let app_state = initialize_connection_pools(&logger).await?;
    
    // Initialize enhanced services
    let services = initialize_services(&logger, &config).await?;
    
    // Initialize DEX integrations with optimization
    let dex_engines = initialize_dex_engines(&app_state, &logger).await?;
    
    // Create optimized swap engine
    let swap_engine = create_optimized_swap_engine(dex_engines, &config).await?;
    
    // Start background cleanup tasks
    start_background_tasks(&logger).await;
    
    // Start multi-threaded monitoring with parallel processing
    start_parallel_monitoring(&app_state, &config, swap_engine, &logger).await?;
    
    Ok(())
}

#[derive(Clone)]
struct BotConfig {
    rpc_wss: String,
    slippage: u64,
    use_jito: bool,
    monitoring_mode: MonitoringMode,
    max_concurrent_trades: usize,
    performance_mode: bool,
}

#[derive(Clone, PartialEq)]
enum MonitoringMode {
    PumpFunOnly,
    RaydiumOnly,
    Both,
    YellowstoneOnly,
    Hybrid, // Best performance mode
}

async fn load_and_validate_config(logger: &Logger) -> anyhow::Result<BotConfig> {
    logger.log("Loading and validating configuration...".to_string());
    
    let rpc_wss = import_env_var("RPC_WSS");
    let slippage = import_env_var("SLIPPAGE").parse::<u64>().unwrap_or(50); // Default 0.5%
    let use_jito = env::var("USE_JITO").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true);
    
    let monitoring_mode = match env::var("MONITORING_MODE").unwrap_or_else(|_| "hybrid".to_string()).to_lowercase().as_str() {
        "pumpfun" => MonitoringMode::PumpFunOnly,
        "raydium" => MonitoringMode::RaydiumOnly,
        "both" => MonitoringMode::Both,
        "yellowstone" => MonitoringMode::YellowstoneOnly,
        "hybrid" | _ => MonitoringMode::Hybrid,
    };
    
    let max_concurrent_trades = env::var("MAX_CONCURRENT_TRADES")
        .unwrap_or_else(|_| "5".to_string())
        .parse()
        .unwrap_or(5);
    
    let performance_mode = env::var("PERFORMANCE_MODE")
        .unwrap_or_else(|_| "true".to_string())
        .parse()
        .unwrap_or(true);
    
    // Validate configuration
    if slippage > 1000 { // > 10%
        warn!("High slippage tolerance configured: {}bps", slippage);
    }
    
    if max_concurrent_trades > 20 {
        warn!("High concurrent trade limit: {}", max_concurrent_trades);
    }
    
    let config = BotConfig {
        rpc_wss,
        slippage,
        use_jito,
        monitoring_mode,
        max_concurrent_trades,
        performance_mode,
    };
    
    info!("Configuration loaded successfully: monitoring_mode={:?}, slippage={}bps", 
          config.monitoring_mode, config.slippage);
    
    Ok(config)
}

async fn initialize_connection_pools(logger: &Logger) -> anyhow::Result<AppState> {
    logger.log("Initializing optimized connection pools...".to_string());
    
    // Create connection pools with retry logic
    let rpc_client = timeout(
        Duration::from_secs(30),
        create_rpc_client()
    ).await??;
    
    let rpc_nonblocking_client = timeout(
        Duration::from_secs(30),
        create_nonblocking_rpc_client()
    ).await??;
    
    let wallet = import_wallet()?;
    
    let state = AppState {
        rpc_client,
        rpc_nonblocking_client,
        wallet: wallet.clone(),
    };
    
    info!("Connection pools initialized for wallet: {}", wallet.pubkey());
    Ok(state)
}

async fn initialize_services(logger: &Logger, config: &BotConfig) -> anyhow::Result<ServiceManager> {
    logger.log("Initializing enhanced services...".to_string());
    
    let mut services = ServiceManager::new();
    
    // Initialize Jito if enabled
    if config.use_jito {
        if let Err(e) = timeout(Duration::from_secs(60), jito::init_tip_accounts()).await? {
            warn!("Jito initialization failed: {}. Continuing without Jito.", e);
        } else {
            info!("Jito bundle service initialized successfully");
            services.jito_enabled = true;
        }
    }
    
    // Initialize Nextblock as fallback
    match NextblockClient::new() {
        Ok(client) => {
            services.nextblock_client = Some(Arc::new(client));
            info!("Nextblock fast confirmation service initialized");
        }
        Err(e) => {
            warn!("Nextblock initialization failed: {}", e);
        }
    }
    
    Ok(services)
}

#[derive(Clone)]
struct ServiceManager {
    jito_enabled: bool,
    nextblock_client: Option<Arc<NextblockClient>>,
}

impl ServiceManager {
    fn new() -> Self {
        Self {
            jito_enabled: false,
            nextblock_client: None,
        }
    }
}

async fn initialize_dex_engines(app_state: &AppState, logger: &Logger) -> anyhow::Result<DexEngines> {
    logger.log("Initializing DEX engines...".to_string());
    
    // Initialize Pump.fun
    let pump_fun = Pump::new(
        app_state.rpc_nonblocking_client.clone(),
        app_state.rpc_client.clone(),
        app_state.wallet.clone(),
    );
    
    // Initialize Raydium (placeholder - would need actual implementation)
    let raydium = None; // Raydium::new(...) when implemented
    
    info!("DEX engines initialized: Pump.fun=✓, Raydium=pending");
    
    Ok(DexEngines {
        pump_fun: Some(pump_fun),
        raydium,
    })
}

struct DexEngines {
    pump_fun: Option<Pump>,
    raydium: Option<Raydium>,
}

async fn create_optimized_swap_engine(
    dex_engines: DexEngines,
    config: &BotConfig,
) -> anyhow::Result<Arc<SwapEngine>> {
    let optimized_config = OptimizedSwapConfig {
        base_config: SwapConfig {
            swap_direction: SwapDirection::Buy,
            amount: 0, // Will be set per trade
            slippage: config.slippage,
            use_jito: config.use_jito,
            swap_in_type: SwapInType::ExactIn,
        },
        max_price_impact: if config.performance_mode { 5.0 } else { 3.0 },
        dynamic_slippage: config.performance_mode,
        mev_protection: true,
        route_optimization: config.performance_mode,
        priority_fee_multiplier: if config.performance_mode { 2.0 } else { 1.5 },
        min_liquidity_threshold: 5_000_000_000, // 5 SOL minimum
    };
    
    let swap_engine = SwapEngine::new(
        dex_engines.raydium,
        dex_engines.pump_fun,
        optimized_config,
    );
    
    info!("Optimized swap engine created with performance_mode={}", config.performance_mode);
    Ok(Arc::new(swap_engine))
}

async fn start_background_tasks(logger: &Logger) {
    logger.log("Starting background cleanup tasks...".to_string());
    
    // Start all cleanup tasks
    start_helius_cleanup();
    start_yellowstone_cleanup_task();
    start_jito_cleanup();
    start_nextblock_cleanup();
    start_swap_cleanup();
    
    // Start monitoring statistics task
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            log_performance_statistics().await;
        }
    });
    
    info!("Background tasks started successfully");
}

async fn start_parallel_monitoring(
    app_state: &AppState,
    config: &BotConfig,
    swap_engine: Arc<SwapEngine>,
    logger: &Logger,
) -> anyhow::Result<()> {
    logger.log("Starting parallel monitoring systems...".to_string());
    
    match config.monitoring_mode {
        MonitoringMode::PumpFunOnly => {
            info!("Starting Pump.fun monitoring only");
            pumpfun_monitor(&config.rpc_wss, app_state.clone(), config.slippage, config.use_jito).await;
        }
        MonitoringMode::RaydiumOnly => {
            info!("Starting Raydium monitoring only");
            raydium_monitor(&config.rpc_wss, app_state.clone(), config.slippage, config.use_jito).await;
        }
        MonitoringMode::Both => {
            info!("Starting both Pump.fun and Raydium monitoring");
            let state1 = app_state.clone();
            let state2 = app_state.clone();
            let rpc_wss1 = config.rpc_wss.clone();
            let rpc_wss2 = config.rpc_wss.clone();
            
            tokio::join!(
                async { pumpfun_monitor(&rpc_wss1, state1, config.slippage, config.use_jito).await },
                async { raydium_monitor(&rpc_wss2, state2, config.slippage, config.use_jito).await }
            );
        }
        MonitoringMode::YellowstoneOnly => {
            info!("Starting Yellowstone gRPC monitoring only");
            let yellowstone_url = import_env_var("YELLOWSTONE_RPC_WSS");
            let monitor = YellowstoneMonitor::new(app_state, yellowstone_url)?
                .with_swap_engine(swap_engine);
            monitor.start_monitoring().await?;
        }
        MonitoringMode::Hybrid => {
            info!("Starting hybrid monitoring (optimal performance)");
            
            let state1 = app_state.clone();
            let state2 = app_state.clone();
            let state3 = app_state.clone();
            let swap_engine1 = swap_engine.clone();
            let rpc_wss1 = config.rpc_wss.clone();
            let rpc_wss2 = config.rpc_wss.clone();
            let yellowstone_url = import_env_var("YELLOWSTONE_RPC_WSS");
            
            // Run all monitoring systems in parallel for maximum coverage
            tokio::select! {
                _ = async { pumpfun_monitor(&rpc_wss1, state1, config.slippage, config.use_jito).await } => {},
                _ = async { raydium_monitor(&rpc_wss2, state2, config.slippage, config.use_jito).await } => {},
                _ = async {
                    if let Ok(monitor) = YellowstoneMonitor::new(&state3, yellowstone_url) {
                        let _ = monitor.with_swap_engine(swap_engine1).start_monitoring().await;
                    }
                } => {},
            }
        }
    }
    
    Ok(())
}

async fn log_performance_statistics() {
    // This would collect and log performance metrics from all components
    info!("=== Performance Statistics ===");
    // Add actual statistics logging here
}
