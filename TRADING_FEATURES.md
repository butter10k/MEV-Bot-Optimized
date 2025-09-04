# Advanced Trading Features & Strategies

## Overview
The MEV Bot has been enhanced with comprehensive trading strategies, risk management, and technical analysis capabilities.

## 🎯 Buy Strategies

### 1. **Immediate Buy**
- Buys tokens immediately when detected
- Fastest execution for high-priority tokens
- Best for tokens with high potential

### 2. **Volume Spike Buy**
- Waits for volume to spike above threshold
- Configurable volume multiplier (default: 3x)
- Helps avoid low-liquidity traps

### 3. **Price Breakout Buy**
- Buys when price breaks above resistance
- Requires 20%+ price increase
- Good for momentum trading

### 4. **Liquidity Addition Buy**
- Buys when liquidity is added to pool
- Requires 1.5x minimum liquidity
- Safer entry with better liquidity

### 5. **Holder Growth Buy**
- Buys when holder count increases rapidly
- Requires 2x minimum holder count
- Indicates growing community interest

## 💰 Sell Strategies

### 1. **Take Profit**
- Sells at configured profit percentage
- Default: 50% profit
- Configurable from 1% to 1000%

### 2. **Stop Loss**
- Sells at configured loss percentage
- Default: 20% loss
- Maximum: 50% loss

### 3. **Trailing Stop**
- Dynamic stop loss that follows price up
- Configurable trailing percentage
- Locks in profits while limiting losses

### 4. **Time-Based Sell**
- Sells after specified time period
- Default: 24 hours
- Good for short-term trading

### 5. **Volume Drop Sell**
- Sells when volume decreases significantly
- Helps exit before major dumps
- Advanced volume analysis required

### 6. **Holder Decline Sell**
- Sells when holder count decreases
- Indicates declining interest
- Advanced holder tracking required

## 🛡️ Risk Management

### Portfolio Risk Limits
- **Max Portfolio Risk**: Maximum percentage of portfolio at risk (default: 20%)
- **Max Single Position**: Maximum percentage for single position (default: 5%)
- **Risk/Reward Ratio**: Minimum risk/reward ratio (default: 2.0)

### Position Sizing
- Automatic position size calculation based on risk
- Dynamic stop loss adjustment
- Volatility-based position sizing

### Concurrent Position Limits
- Maximum concurrent open positions (default: 5)
- Prevents over-exposure to single market

## 🔍 Token Filtering

### Liquidity Requirements
- **Min Liquidity**: 0.5 SOL (default)
- **Max Liquidity**: 100 SOL (default)
- Helps avoid both low and extremely high liquidity tokens

### Holder Requirements
- **Min Holders**: 10 (default)
- **Max Holders**: 10,000 (default)
- Balances community growth with early entry

### Market Cap Limits
- **Min Market Cap**: 1 SOL (default)
- **Max Market Cap**: 1,000 SOL (default)
- Focuses on small to medium cap opportunities

### Creator Filtering
- **Blacklisted Creators**: Comma-separated list
- **Whitelisted Creators**: Comma-separated list
- Helps avoid known scammers

### Token Age Requirements
- **Min Age**: 1 minute (default)
- **Max Age**: 72 hours (default)
- Balances freshness with stability

## 📊 Technical Analysis

### RSI (Relative Strength Index)
- **Period**: 14 (default)
- **Oversold Threshold**: 30 (default)
- **Overbought Threshold**: 70 (default)
- Helps identify entry/exit points

### Volume Analysis
- **Volume SMA**: Simple Moving Average of volume
- **Volume Spike Detection**: Configurable multiplier
- **Volume Change Tracking**: 24-hour volume changes

### Price Analysis
- **Price SMA**: Simple Moving Average of price
- **Price Trend Detection**: Uptrend/Downtrend/Sideways
- **Volatility Calculation**: Standard deviation of prices

## ⚙️ Configuration

### Environment Variables
All features are configurable via environment variables:

```bash
# Basic Settings
RPC_URL=https://api.mainnet-beta.solana.com
# Note: BOT_TOKEN and CHAT_ID are now hardcoded in the config file
# Update src/config.rs with your actual values

# Trading Strategy
BUY_STRATEGY=VolumeSpike
SELL_STRATEGY=TrailingStop
AUTO_BUY=true
AUTO_SELL=true

# Risk Management
MAX_PORTFOLIO_RISK=15.0
MAX_SINGLE_POSITION=3.0
STOP_LOSS_PERCENTAGE=15.0
TAKE_PROFIT_PERCENTAGE=75.0

# Token Filtering
MIN_LIQUIDITY_SOL=1.0
MAX_LIQUIDITY_SOL=50.0
MIN_HOLDER_COUNT=25
MAX_HOLDER_COUNT=5000

# Technical Analysis
ENABLE_TECHNICAL_ANALYSIS=true
RSI_PERIOD=21
RSI_OVERSOLD=25.0
RSI_OVERBOUGHT=75.0
VOLUME_MULTIPLIER=2.5
```

## 🚀 Usage Examples

### Conservative Strategy
```bash
BUY_STRATEGY=LiquidityAddition
SELL_STRATEGY=TakeProfit
MAX_PORTFOLIO_RISK=10.0
MAX_SINGLE_POSITION=2.0
MIN_LIQUIDITY_SOL=2.0
MIN_HOLDER_COUNT=50
```

### Aggressive Strategy
```bash
BUY_STRATEGY=Immediate
SELL_STRATEGY=TrailingStop
MAX_PORTFOLIO_RISK=30.0
MAX_SINGLE_POSITION=8.0
MIN_LIQUIDITY_SOL=0.3
MIN_HOLDER_COUNT=5
```

### Momentum Strategy
```bash
BUY_STRATEGY=PriceBreakout
SELL_STRATEGY=VolumeDrop
MAX_PORTFOLIO_RISK=25.0
MAX_SINGLE_POSITION=6.0
ENABLE_TECHNICAL_ANALYSIS=true
RSI_OVERSOLD=20.0
```

## 📈 Performance Monitoring

### Position Tracking
- Real-time P&L calculation
- Entry/exit price tracking
- Position duration monitoring

### Risk Metrics
- Portfolio risk percentage
- Position concentration analysis
- Risk-adjusted returns

### Strategy Performance
- Buy/sell signal accuracy
- Strategy-specific metrics
- Performance comparison

## 🔧 Advanced Features

### Dynamic Stop Loss
- Adjusts based on volatility
- Trailing stop loss
- Custom stop loss algorithms

### Position Scaling
- Incremental position building
- DCA (Dollar Cost Averaging) support
- Position size optimization

### Market Analysis
- Token correlation analysis
- Market sentiment tracking
- News impact assessment

## ⚠️ Important Notes

1. **Test First**: Always test strategies on testnet first
2. **Risk Management**: Never risk more than you can afford to lose
3. **Monitoring**: Regularly review and adjust strategy parameters
4. **Backtesting**: Use historical data to validate strategies
5. **Diversification**: Don't put all eggs in one basket

## 🆘 Support

For questions or issues with the trading features:
1. Check the configuration parameters
2. Review the logs for error messages
3. Verify your RPC endpoint is working
4. Ensure sufficient wallet balance
5. Check network conditions
