# 🤖 Ethereum Stop-Loss Trading Bot

<div align="center">
  
  ![Trading Bot](https://img.shields.io/badge/Trading%20Bot-StopLoss-blue?style=for-the-badge)
  ![Chains](https://img.shields.io/badge/Chains-Ethereum%20|%20Arbitrum%20|%20Base%20|%20Optimism-green?style=for-the-badge)
  ![DEX Aggregators](https://img.shields.io/badge/DEX%20Aggregators-1inch%20|%20ParaSwap%20|%20KyberSwap-orange?style=for-the-badge)

  <p>A professional trading bot designed to execute stop-loss strategies across multiple EVM-compatible chains using leading DEX aggregator APIs.</p>
  
</div>

## 📋 Overview

This StopLoss Trading Bot monitors token prices across multiple chains and automatically executes trades when predefined stop-loss conditions are met. By leveraging DEX aggregators (1inch, ParaSwap, and KyberSwap), the bot ensures optimal execution with minimal slippage.

## ✨ Features

- 🌐 **Multi-Chain Support**: Trade on Ethereum, Arbitrum, Base, and Optimism networks
- 🔄 **Multiple DEX Aggregators**: Use 1inch, ParaSwap, and KyberSwap for best execution prices
- 📉 **Customizable Stop-Loss Strategies**: Set percentage-based or fixed-price stop-loss triggers
- ⛽ **Gas Optimization**: Intelligent gas price management across different networks
- 🔒 **Wallet Security**: Private keys never leave your local environment
- 📱 **Notification System**: Receive alerts via email, Telegram, or Discord
- 📊 **Performance Analytics**: Track your trading performance with detailed reports
- 🔁 **Failover Mechanisms**: Automatic retry and fallback options if a DEX aggregator fails

## 🔧 Prerequisites

- Node.js (v16 or higher)
- Ethereum wallet with private key
- Sufficient ETH for gas on each supported chain
- API keys for notification services (optional)

## 📦 Installation

```bash
git clone https://github.com/yourusername/stoploss-trading-bot.git
cd stoploss-trading-bot
npm install
```

## ⚙️ Configuration

Create a `.env` file in the root directory with the following variables:

```
# Wallet Configuration
PRIVATE_KEY=your_private_key_here

# RPC Endpoints
ETHEREUM_RPC=https://mainnet.infura.io/v3/your_infura_key
ARBITRUM_RPC=https://arb1.arbitrum.io/rpc
BASE_RPC=https://mainnet.base.org
OPTIMISM_RPC=https://mainnet.optimism.io

# DEX Aggregator API Keys
ONEINCH_API_KEY=your_1inch_api_key
PARASWAP_API_KEY=your_paraswap_api_key
KYBERSWAP_API_KEY=your_kyberswap_api_key

# Notification Settings (Optional)
TELEGRAM_BOT_TOKEN=your_telegram_bot_token
TELEGRAM_CHAT_ID=your_telegram_chat_id
DISCORD_WEBHOOK_URL=your_discord_webhook_url
EMAIL_SERVICE=gmail
EMAIL_USER=your_email@gmail.com
EMAIL_PASSWORD=your_email_password

# General Settings
LOG_LEVEL=info
```

## 🚀 Usage

### Basic Start

```bash
npm start
```

### With Custom Configuration File

```bash
npm start -- --config=my-config.json
```

### Running in Background

```bash
npm run start:daemon
```

## 📝 Configuration File Format

Create a `config.json` file to define your trading strategies:

```json
{
  "strategies": [
    {
      "name": "ETH-USDC Stop Loss",
      "chain": "ethereum",
      "dexAggregator": "1inch",
      "tokenIn": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
      "tokenOut": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      "stopLossPercentage": 5,
      "amountIn": "1.0",
      "slippage": 0.5
    },
    {
      "name": "ARB-USDT Stop Loss",
      "chain": "arbitrum",
      "dexAggregator": "paraswap",
      "tokenIn": "0x912CE59144191C1204E64559FE8253a0e49E6548",
      "tokenOut": "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9",
      "stopLossPrice": 0.85,
      "amountIn": "100.0",
      "slippage": 0.3
    }
  ],
  "global": {
    "checkInterval": 60,
    "gasMultiplier": 1.2,
    "maxRetries": 3,
    "notificationChannels": ["telegram", "email"]
  }
}
```

## 🔄 Supported DEX Aggregators

<div align="center">

| Aggregator | Chains Supported |
|:----------:|:----------------:|
| 1inch | Ethereum, Arbitrum, Base, Optimism |
| ParaSwap | Ethereum, Arbitrum, Optimism |
| KyberSwap | Ethereum, Arbitrum, Base, Optimism |

</div>

## 🏗️ Architecture

The bot follows a modular architecture:

1. 📊 **Price Monitor**: Continuously checks token prices across chains
2. 🧠 **Strategy Evaluator**: Determines if stop-loss conditions are met
3. ⚡ **Execution Engine**: Interacts with DEX aggregators to execute trades
4. 📲 **Notification Service**: Sends alerts about executed trades
5. 📈 **Analytics Module**: Tracks performance and generates reports

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ⚠️ Disclaimer

This software is for educational purposes only. Use at your own risk. The creators are not responsible for any financial losses incurred through the use of this bot. Always test thoroughly with small amounts before deploying with significant capital.

## 🙏 Acknowledgements

- [1inch API](https://docs.1inch.io/)
- [ParaSwap API](https://developers.paraswap.network/)
- [KyberSwap API](https://docs.kyberswap.com/)
- [Ethers.js](https://docs.ethers.io/)
- [Web3.js](https://web3js.readthedocs.io/)

<div align="center">
  <img src="https://img.shields.io/badge/Made%20with%20%E2%9D%A4%EF%B8%8F%20by-Developers-blue?style=for-the-badge" alt="Made with love">
</div>
