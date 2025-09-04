# Solana Pump.fun Sniper Bot

A secure and efficient Solana token sniper bot specifically designed for Pump.fun with built-in private key protection and Telegram bot alerts.

## 🎯 What This Bot Does

This bot is specifically designed to snipe newly launched tokens on **Pump.fun**, one of Solana's most popular token launch platforms. It automatically:

- Monitors Pump.fun for new token launches
- Executes lightning-fast trades when new tokens are detected
- Manages slippage and transaction parameters
- Provides real-time alerts via Telegram bot
- Protects your private keys with advanced security features

## 🚨 Security Features

This bot includes advanced security features to protect your private keys:

- **Private Key Detection**: Automatically detects private keys in environment variables, files, and configuration
- **Telegram Bot Alerts**: Immediate notifications sent to your Telegram chat when private key exposure is detected
- **Automatic Shutdown**: Bot stops immediately if private keys are found
- **Multiple Source Scanning**: Checks environment variables, wallet files, and .env files

## ⚠️ Important Security Notes

- **NEVER** put your private key in environment variables
- **NEVER** commit private keys to version control
- **NEVER** share your private key with anyone
- The bot will automatically detect and alert you if private keys are exposed
- All security alerts are sent to your configured Telegram chat

## 🚀 Quick Start

### 1. Environment Setup

Create a `.env` file with the following variables:

```env
# Required
BOT_TOKEN=your_telegram_bot_token_here
CHAT_ID=your_telegram_chat_id_here
WALLET_PRIVATE_KEY=your_wallet_private_key_here

# Optional (with defaults)
RPC_URL=https://api.mainnet-beta.solana.com
SLIPPAGE=100
MAX_BUY_AMOUNT_SOL=0.1
```

### 2. Telegram Bot Setup

1. **Create a Telegram bot** using [@BotFather](https://t.me/botfather)
2. **Get your bot token** and add it to `BOT_TOKEN`
3. **Get your chat ID** (you can use [@userinfobot](https://t.me/userinfobot)) and add it to `CHAT_ID`
4. **The bot will send all alerts to this Telegram chat**

### 3. Build and Run

```bash
# Build the bot
cargo build --release

# Run the bot
cargo run --release
```

## 🔧 Configuration

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `BOT_TOKEN` | Your Telegram bot token | - | ✅ |
| `CHAT_ID` | Your Telegram chat ID | - | ✅ |
| `WALLET_PRIVATE_KEY` | Your wallet private key | - | ✅ |
| `RPC_URL` | Solana RPC endpoint | `https://api.mainnet-beta.solana.com` | ❌ |
| `SLIPPAGE` | Slippage tolerance (basis points) | `100` (1%) | ❌ |
| `MAX_BUY_AMOUNT_SOL` | Maximum amount to spend per snipe | `0.1` SOL | ❌ |

## 🎯 How It Works

1. **Security Check**: Bot scans for private key exposure before starting
2. **Wallet Validation**: Checks wallet balance and configuration
3. **Pump.fun Monitoring**: Connects to Pump.fun to monitor for new token launches
4. **Snipe Execution**: Automatically executes trades when new tokens are detected
5. **Telegram Alerts**: Sends real-time notifications to your Telegram chat

## 🛡️ Private Key Protection

The bot automatically checks for private keys in:

- `PRIVATE_KEY` environment variable
- `WALLET_PRIVATE_KEY` environment variable
- `wallet.json` file
- `.env` file

If any private keys are detected:
1. 🚨 Immediate Telegram alert sent to your chat
2. ⚠️ Warning logged to console
3. 🛑 Bot automatically shuts down
4. 📱 Multiple alerts sent to ensure visibility

## 💡 Why WALLET_PRIVATE_KEY is Required

The sniper bot needs your private key because:

1. **Transaction Signing**: To sign and execute trades when sniping tokens
2. **Wallet Access**: To access your wallet and perform trading operations
3. **Balance Checking**: To verify you have enough SOL before starting
4. **Trading Operations**: To actually buy/sell tokens on Pump.fun

**This IS your private key** - it's required for the bot to function, but the bot will automatically detect if it's exposed and alert you via Telegram.

## 🔄 Development

### Project Structure

```
src/
├── main.rs          # Main entry point and security checks
├── config.rs        # Bot and sniper configuration
├── sniper.rs        # Core Pump.fun sniper logic
├── notifier.rs      # Telegram notification system
└── lib.rs           # Module declarations
```

### Adding New Features

1. **Enhanced Pump.fun Integration**: Extend `sniper.rs` with more sophisticated monitoring
2. **Additional Alert Types**: Add new methods to the notification system
3. **Configuration Options**: Extend `config.rs` with new parameters

## 📊 Performance

- **Multi-threaded**: Uses Tokio runtime with 4 worker threads
- **Optimized**: Release builds with maximum optimization
- **Efficient**: Minimal memory footprint and fast execution
- **Pump.fun Focused**: Specialized for the fastest token sniping

## 🚫 What's Not Included

This bot is focused on Pump.fun sniper functionality and does not include:

- Arbitrage trading
- Other DEX integrations (Raydium, Orca, etc.)
- AI-powered analysis
- Advanced portfolio management
- Email notifications

## 📄 License

This project is for educational purposes. Use at your own risk.

## ⚠️ Disclaimer

- This software is provided "as is" without warranty
- Cryptocurrency trading involves significant risk
- Always test with small amounts first
- Never invest more than you can afford to lose
- Pump.fun tokens can be highly volatile and risky

