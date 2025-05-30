# 🤖 Ethereum MEV Bot

This sophisticated MEV bot automates trading on Ethereum and other EVM-compatible chains (Arbitrum, Base, Optimism) using DEX aggregators (1inch, Paraswap, Kyberswap) for optimal execution and minimal slippage.  Its core feature is a customizable stop-loss strategy to protect against market downturns.

## Features

* **Multi-Chain Support:** Ethereum, Arbitrum, Base, Optimism
* **DEX Aggregation:** 1inch, Paraswap, Kyberswap
* **Customizable Stop-Loss:** Percentage-based or fixed-price triggers
* **Gas Optimization:** Intelligent gas management across networks
* **Secure:** Private keys remain local
* **Notifications:** Email, Telegram, Discord (optional)
* **Performance Analytics:** Detailed trading reports
* **Failover Mechanisms:** Automatic retries and fallback options

## Prerequisites

* Node.js (v16+)
* Ethereum wallet and private key
* Sufficient ETH for gas on each chain
* API keys for notification services (optional)

## Installation

```bash
git clone https://github.com/butter1011/Ethereum-MEV-Bot.git
cd Ethereum-MEV-Bot
npm install
```

## Configuration

Create a `.env` file in the root directory:

```
PRIVATE_KEY=your_private_key_here
INCH_API_URL=your_1inch_api_url
ETH_RPC_URL=https://mainnet.infura.io/v3/your_infura_key
ARBITRUM_RPC_URL=https://arb1.arbitrum.io/rpc
BASE_RPC_URL=https://mainnet.base.org
OPTIMISM_RPC_URL=https://mainnet.optimism.io
MONGODB_URL=your_mongodb_connection_string
```

## Usage

```bash
npm start
```

## Supported DEX Aggregators

| Aggregator | Chains Supported |
|---|---|
| 1inch | Ethereum, Arbitrum, Base, Optimism |
| Paraswap | Ethereum, Arbitrum, Optimism |
| Kyberswap | Ethereum, Arbitrum, Base, Optimism |


## Architecture

1. **Price Monitor:** Tracks token prices.
2. **Strategy Evaluator:** Checks stop-loss conditions.
3. **Execution Engine:** Executes trades via DEX aggregators.
4. **Notification Service:** Sends trade alerts.
5. **Analytics Module:** Generates performance reports.

## Retry Mechanism

The bot uses an exponential backoff retry strategy with jitter for API request failures, retrying up to 5 times.


## Contributing

Fork the repository, create a feature branch, commit changes, push to the branch, and open a pull request.

## License

MIT License. See the [LICENSE](LICENSE) file.

## Disclaimer

This software is for educational purposes only. Use at your own risk.  The creators are not liable for any financial losses.  Test thoroughly with small amounts before using significant capital.

## Acknowledgements

* 1inch API
* Paraswap API
* Kyberswap API
* Ethers.js
* Web3.js

## Contact

* Email: ruizsalvador951011@gmail.com
* Telegram: [@k9h_butter](https://t.me/k9h_butter)
