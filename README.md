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
git clone https://github.com/butter1011/Eth-TradingBot-Stop-loss.git
cd Eth-TradingBot-Stop-loss
npm install
```

## ⚙️ Configuration

Create a `.env` file in the root directory with the following variables:

```
# Wallet Configuration
PRIVATE_KEY=your_private_key_here

# API Endpoints
INCH_API_URL=your_1inch_api_url

# RPC Endpoints
ETH_RPC_URL=https://mainnet.infura.io/v3/your_infura_key
ARBITRUM_RPC_URL=https://arb1.arbitrum.io/rpc
BASE_RPC_URL=https://mainnet.base.org
OPTIMISM_RPC_URL=https://mainnet.optimism.io

# Database
MONGODB_URL=your_mongodb_connection_string
```

## 🚀 Usage

### Basic Start

```bash
npm start
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

## 🔁 Retry Mechanism

The bot implements an exponential backoff retry strategy for handling API request failures:
- Automatically retries failed requests up to 5 times by default
- Implements exponential delay between retries with random jitter
- Distinguishes between retryable server errors and non-retryable client errors
- Provides detailed logging of retry attempts and failures

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

## 📫 Contact

Feel free to reach out if you have any questions or suggestions:

- Email: [ruizsalvador951011@gmail.com](mailto:ruizsalvador951011@gmail.com)
- Telegram: [@k9h_butter](https://t.me/k9h_butter)
