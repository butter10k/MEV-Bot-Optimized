use crate::{config::SniperConfig, notifier::Notifier};
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
use tracing::{info, warn};

pub struct PumpFunSniper {
    config: SniperConfig,
    notifier: Notifier,
    rpc_client: Arc<RpcClient>,
    wallet: Keypair,
}

impl PumpFunSniper {
    pub fn new(config: SniperConfig, notifier: Notifier) -> Result<Self> {
        // Validate configuration
        config.validate()?;
        
        // Initialize RPC client
        let rpc_client = Arc::new(RpcClient::new_with_commitment(
            config.rpc_url.clone(),
            CommitmentConfig::confirmed()
        ));
        
        // Load wallet from environment or file
        let wallet = Self::load_wallet()?;
        
        Ok(Self {
            config,
            notifier,
            rpc_client,
            wallet,
        })
    }
    
    fn load_wallet() -> Result<Keypair> {
        // Try to load from environment variable first
        if let Ok(private_key_base58) = std::env::var("WALLET_PRIVATE_KEY") {
            let private_key_bytes = bs58::decode(&private_key_base58)
                .into_vec()
                .map_err(|e| anyhow!("Invalid private key format: {}", e))?;
            
            if private_key_bytes.len() != 64 {
                return Err(anyhow!("Invalid private key length: expected 64, got {}", private_key_bytes.len()));
            }
            
            return Ok(Keypair::from_bytes(&private_key_bytes)?);
        }
        
        // Try to load from wallet.json file
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
        
        // Verify wallet balance
        let balance = self.rpc_client.get_balance(&self.wallet.pubkey())?;
        info!("Wallet balance: {} SOL", balance as f64 / 1_000_000_000.0);
        
        if balance < self.config.max_buy_amount as u64 {
            let error_msg = format!("Insufficient balance: {} SOL (need {} SOL)", 
                balance as f64 / 1_000_000_000.0,
                self.config.max_buy_amount as f64 / 1_000_000_000.0);
            
            self.notifier.send_error_alert(&error_msg, "Wallet Balance Check").await?;
            return Err(anyhow!(error_msg));
        }
        
        // Start monitoring for new tokens
        self.monitor_new_tokens().await?;
        
        Ok(())
    }
    
    async fn monitor_new_tokens(&self) -> Result<()> {
        info!("Monitoring for new Pump.fun tokens...");
        
        // This is a simplified monitoring loop
        // In a real implementation, you would connect to Pump.fun's WebSocket feed
        // or use their API to monitor for new token launches
        
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        
        loop {
            interval.tick().await;
            
            // Simulate finding a new token (replace with actual monitoring logic)
            if let Ok(token_info) = self.check_for_new_token().await {
                info!("New token detected: {}", token_info.address);
                
                // Attempt to snipe the token
                if let Ok(tx_hash) = self.snipe_token(&token_info).await {
                    info!("Successfully sniped token: {} - TX: {}", token_info.address, tx_hash);
                    
                    // Send success alert
                    let _ = self.notifier.send_snipe_alert(
                        &token_info.address,
                        token_info.amount_sol,
                        &tx_hash
                    ).await;
                } else {
                    warn!("Failed to snipe token: {}", token_info.address);
                }
            }
            
            // Add some delay to prevent excessive API calls
            sleep(Duration::from_millis(100)).await;
        }
    }
    
    async fn check_for_new_token(&self) -> Result<TokenInfo> {
        // This is a placeholder - replace with actual Pump.fun monitoring logic
        // You would typically:
        // 1. Connect to Pump.fun's WebSocket feed
        // 2. Monitor for new token creation events
        // 3. Filter based on your criteria (liquidity, creator, etc.)
        
        // For now, return an error to indicate no new tokens
        Err(anyhow!("No new tokens available"))
    }
    
    async fn snipe_token(&self, token_info: &TokenInfo) -> Result<String> {
        info!("Attempting to snipe token: {}", token_info.address);
        
        // Create the swap transaction
        let transaction = self.create_swap_transaction(token_info)?;
        
        // Send the transaction
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        
        info!("Transaction sent: {}", signature);
        
        // Wait for confirmation
        self.rpc_client.confirm_transaction(&signature)?;
        
        Ok(signature.to_string())
    }
    
    fn create_swap_transaction(&self, token_info: &TokenInfo) -> Result<Transaction> {
        // This is a simplified transaction creation
        // In a real implementation, you would:
        // 1. Create the actual swap instruction for Pump.fun
        // 2. Add proper slippage tolerance
        // 3. Set appropriate priority fees
        
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        
        let instruction = system_instruction::transfer(
            &self.wallet.pubkey(),
            &Pubkey::from_str(&token_info.address)?, // This should be the actual swap instruction
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
}

#[allow(dead_code)]
impl TokenInfo {
    fn new(address: String, amount: u64) -> Self {
        Self {
            address,
            amount,
            amount_sol: amount as f64 / 1_000_000_000.0,
        }
    }
}
