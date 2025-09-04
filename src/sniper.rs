use crate::{config::SniperConfig, notifier::Notifier, trading_strategy::TradingStrategy, technical_analysis::TechnicalAnalysis, risk_management::RiskManager, pump_fun_monitor::TokenMonitor};
use anyhow::{Result, anyhow};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    system_instruction,
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, debug};

pub struct PumpFunSniper {
    config: SniperConfig,
    notifier: Notifier,
    rpc_client: Arc<RpcClient>,
    wallet: Keypair,
    trading_strategy: Arc<TradingStrategy>,
    risk_manager: Arc<RiskManager>,
    technical_analysis: Arc<TechnicalAnalysis>,
    token_monitor: Arc<TokenMonitor>,
}

impl PumpFunSniper {
    pub fn new(config: SniperConfig, notifier: Notifier) -> Result<Self> {
        config.validate()?;
        
        let rpc_client = Arc::new(RpcClient::new_with_commitment(
            config.rpc_url.clone(),
            CommitmentConfig::confirmed()
        ));
        
        let wallet = Self::load_wallet()?;
        
        let trading_strategy = Arc::new(TradingStrategy::new(Arc::new(config.clone())));
        let risk_manager = Arc::new(RiskManager::new(config.clone(), 100.0));
        let technical_analysis = Arc::new(TechnicalAnalysis::new(
            config.rsi_period as usize,
            20
        ));
        
        let token_monitor = Arc::new(TokenMonitor::new(Arc::new(config.clone())));
        
        Ok(Self {
            config,
            notifier,
            rpc_client,
            wallet,
            trading_strategy,
            risk_manager,
            technical_analysis,
            token_monitor,
        })
    }
    
    fn load_wallet() -> Result<Keypair> {
        if let Ok(private_key_base58) = std::env::var("WALLET_PRIVATE_KEY") {
            let private_key_bytes = bs58::decode(&private_key_base58)
                .into_vec()
                .map_err(|e| anyhow!("Invalid private key format: {}", e))?;
            
            if private_key_bytes.len() != 64 {
                return Err(anyhow!("Invalid private key length: expected 64, got {}", private_key_bytes.len()));
            }
            
            return Ok(Keypair::from_bytes(&private_key_bytes)?);
        }
        
        if let Ok(wallet_content) = std::fs::read_to_string("wallet.json") {
            let wallet_data: serde_json::Value = serde_json::from_str(&wallet_content)
                .map_err(|e| anyhow!("Invalid wallet.json format: {}", e))?;
            
            if let Some(private_key) = wallet_data.get("privateKey").and_then(|v| v.as_str()) {
                let private_key_bytes = bs58::decode(private_key)
                    .into_vec()
                    .map_err(|e| anyhow!("Invalid private key format: {}", e))?;
                
                if private_key_bytes.len() != 64 {
                    return Err(anyhow!("Invalid private key length: expected 64, got {}", private_key_bytes.len()));
                }
                
                return Ok(Keypair::from_bytes(&private_key_bytes)?);
            }
        }
        
        Err(anyhow!("No wallet found. Set WALLET_PRIVATE_KEY environment variable or create wallet.json file"))
    }
    
    pub fn get_wallet_address(&self) -> String {
        self.wallet.pubkey().to_string()
    }
    
    pub async fn start(&self) -> Result<()> {
        info!("Starting Pump.fun token sniper...");
        
        let balance = self.rpc_client.get_balance(&self.wallet.pubkey())?;
        info!("Wallet balance: {} SOL", balance as f64 / 1_000_000_000.0);
        
        if balance < self.config.max_buy_amount as u64 {
            let error_msg = format!("Insufficient balance: {} SOL (need {} SOL)", 
                balance as f64 / 1_000_000_000.0,
                self.config.max_buy_amount as f64 / 1_000_000_000.0);
            
            self.notifier.send_error_alert(&error_msg, "Wallet Balance Check").await?;
            return Err(anyhow!(error_msg));
        }
        
        // Start token monitoring in background
        let monitor = self.token_monitor.clone();
        tokio::spawn(async move {
            if let Err(e) = monitor.start_monitoring().await {
                error!("Token monitoring failed: {}", e);
            }
        });
        
        // Wait a bit for initial token discovery
        sleep(Duration::from_secs(5)).await;
        
        // Send initial monitoring stats
        if let Ok(stats) = self.token_monitor.get_stats().await {
            let _ = self.notifier.send_alert("🔍 **Token Monitor Started**", &stats.to_string()).await;
        }
        
        self.monitor_new_tokens().await?;
        
        Ok(())
    }
    
    async fn monitor_new_tokens(&self) -> Result<()> {
        info!("Starting advanced token monitoring with strategy: {:?}", self.config.buy_strategy);
        
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut position_monitor_interval = tokio::time::interval(Duration::from_secs(5));
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Ok(token_info) = self.check_for_new_token().await {
                        info!("New token detected: {}", token_info.address);
                        
                        if let Ok(should_buy) = self.analyze_and_decide_buy(&token_info).await {
                            if should_buy {
                                if let Ok(tx_hash) = self.snipe_token(&token_info).await {
                                    info!("Successfully sniped token: {} - TX: {}", token_info.address, tx_hash);
                                    
                                    let _ = self.notifier.send_snipe_alert(
                                        &token_info.address,
                                        token_info.amount_sol,
                                        &tx_hash
                                    ).await;
                                } else {
                                    warn!("Failed to snipe token: {}", token_info.address);
                                }
                            } else {
                                debug!("Token {} filtered out by strategy", token_info.address);
                            }
                        }
                    }
                }
                _ = position_monitor_interval.tick() => {
                    self.monitor_positions().await?;
                }
            }

            sleep(Duration::from_millis(100)).await;
        }
    }
    
    async fn check_for_new_token(&self) -> Result<TokenInfo> {
        // Get tokens that match our strategy from the Pump.fun monitor
        let strategy_tokens = self.token_monitor.get_strategy_tokens().await?;
        
        if strategy_tokens.is_empty() {
            return Err(anyhow!("No tokens match current strategy"));
        }
        
        // Get the best token based on our criteria
        let best_token = strategy_tokens.into_iter().next()
            .ok_or_else(|| anyhow!("No suitable tokens found"))?;
        
        // Convert PumpFunToken to TokenInfo
        let token_info = TokenInfo {
            address: best_token.address,
            amount: (self.config.max_buy_amount as f64 / 1_000_000_000.0 * 1_000_000_000.0) as u64,
            amount_sol: self.config.max_buy_amount as f64 / 1_000_000_000.0,
            liquidity_sol: best_token.liquidity_sol,
            holder_count: best_token.holder_count,
            market_cap_sol: best_token.market_cap_sol,
            creator_address: best_token.creator,
            token_age_minutes: (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() - best_token.created_at) / 60,
        };
        
        Ok(token_info)
    }

    async fn analyze_and_decide_buy(&self, token_info: &TokenInfo) -> Result<bool> {
        if !self.config.auto_buy {
            return Ok(false);
        }

        if self.config.buy_delay_ms > 0 {
            sleep(Duration::from_millis(self.config.buy_delay_ms)).await;
        }

        let should_buy = self.trading_strategy.should_buy_token(
            token_info.liquidity_sol,
            token_info.holder_count,
            token_info.market_cap_sol
        ).await?;

        if !should_buy {
            return Ok(false);
        }

        let current_positions = self.trading_strategy.get_open_positions().await;
        let can_open = self.risk_manager.can_open_position(&current_positions, token_info.amount_sol)?;

        if !can_open {
            debug!("Cannot open position due to risk limits");
            return Ok(false);
        }

        if current_positions.len() >= self.config.max_concurrent_buys as usize {
            debug!("Maximum concurrent positions reached");
            return Ok(false);
        }

        Ok(true)
    }

    async fn monitor_positions(&self) -> Result<()> {
        if !self.config.auto_sell {
            return Ok(());
        }

        let tokens_to_sell = self.trading_strategy.check_sell_signals().await?;
        
        for token_address in tokens_to_sell {
            info!("Sell signal triggered for token: {}", token_address);
            
            if let Ok(tx_hash) = self.sell_token(&token_address).await {
                info!("Successfully sold token: {} - TX: {}", token_address, tx_hash);
                
                let _ = self.notifier.send_alert(
                    "💰 **POSITION CLOSED** 💰",
                    &format!("Token: `{}`\nTransaction: `{}`", token_address, tx_hash)
                ).await;
            } else {
                warn!("Failed to sell token: {}", token_address);
            }
        }

        Ok(())
    }

    async fn sell_token(&self, token_address: &str) -> Result<String> {
        info!("Executing sell order for token: {}", token_address);
        
        if let Some(position) = self.trading_strategy.get_position(token_address).await {
            let transaction = self.create_sell_transaction(token_address, &position)?;
            
            let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
            
            info!("Sell transaction sent: {}", signature);
            
            let _ = self.trading_strategy.close_position(token_address).await;
            
            Ok(signature.to_string())
        } else {
            Err(anyhow!("Position not found: {}", token_address))
        }
    }

    fn create_sell_transaction(&self, token_address: &str, position: &crate::trading_strategy::Position) -> Result<Transaction> {
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        
        let instruction = system_instruction::transfer(
            &Pubkey::from_str(token_address)?,
            &self.wallet.pubkey(),
            position.token_amount as u64,
        );
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.wallet.pubkey()),
            &[&self.wallet],
            recent_blockhash,
        );
        
        Ok(transaction)
    }
    
    async fn snipe_token(&self, token_info: &TokenInfo) -> Result<String> {
        info!("Attempting to snipe token: {}", token_info.address);
        
        let transaction = self.create_swap_transaction(token_info)?;
        
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        
        info!("Transaction sent: {}", signature);
        
        self.rpc_client.confirm_transaction(&signature)?;
        
        Ok(signature.to_string())
    }
    
    fn create_swap_transaction(&self, token_info: &TokenInfo) -> Result<Transaction> {
        // In a real implementation, you would:
        // 1. Create the actual swap instruction for Pump.fun
        // 2. Add proper slippage tolerance
        // 3. Set appropriate priority fees
        
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        
        let instruction = system_instruction::transfer(
            &self.wallet.pubkey(),
            &Pubkey::from_str(&token_info.address)?,
            token_info.amount,
        );
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.wallet.pubkey()),
            &[&self.wallet],
            recent_blockhash,
        );
        
        Ok(transaction)
    }
}

#[derive(Debug)]
struct TokenInfo {
    address: String,
    amount: u64,
    amount_sol: f64,
    liquidity_sol: f64,
    holder_count: u32,
    market_cap_sol: f64,
    creator_address: String,
    token_age_minutes: u64,
}

#[allow(dead_code)]
impl TokenInfo {
    fn new(address: String, amount: u64) -> Self {
        Self {
            address,
            amount,
            amount_sol: amount as f64 / 1_000_000_000.0,
            liquidity_sol: 5.0, // Default values for testing
            holder_count: 100,
            market_cap_sol: 10.0,
            creator_address: "default_creator".to_string(),
            token_age_minutes: 30,
        }
    }
}
