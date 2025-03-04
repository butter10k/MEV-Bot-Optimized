toastr.options = {
  closeButton: true,
  progressBar: false,
  positionClass: "toast-top-right",
  timeOut: "5000",
};

class TradingBot {
  constructor() {
    this.isActive = false;
    this.lastTradeTime = 0;
    this.currentPosition = "";
    this.stopLossPrice = 0;
    this.slippage = 1;
    this.initialAmount = 0;
    this.currentAmount = 0;
    this.selectedStablecoin = "";
    this.buffer = 0.5;
    this.cooldown = 30;
    this.gasPriority = "normal";
    this.isUSDMode = false;
  }

  async start(config) {
    console.log("Starting bot with config:", config);
    this.isActive = true;
    this.stopLossPrice = parseFloat(config.stopLossPrice);
    this.slippage =
      config.slippage === "Auto" ? 0.5 : parseFloat(config.slippage);
    this.initialAmount = parseFloat(config.initialAmount);
    this.selectedStablecoin = config.stablecoin;
    this.buffer = config.buffer ? parseFloat(config.buffer) : 0.5;
    this.cooldown = config.cooldown ? parseInt(config.cooldown) : 5;
    this.gasPriority = config.gasPriority || "normal";
    this.isUSDMode = config.isUSDMode || false;
    this.currentPosition = config.isUSDMode ? "STABLE" : "WETH";

    const latestTx = await this.getLatestTransaction();
    if (latestTx) {
      this.currentAmount = parseFloat(latestTx.toAmount);

      console.log("Latest transaction:", latestTx);
      console.log("Current amount:", this.currentAmount);
      if (latestTx.toToken === this.selectedStablecoin) {
        this.currentPosition = "STABLE";
      } else if (latestTx.toToken === "WETH") {
        this.currentPosition = "WETH";
      } else {
        this.currentPosition = this.isUSDMode ? "STABLE" : "WETH";
      }
    } else {
      this.currentAmount = this.initialAmount;
      this.currentPosition = this.isUSDMode ? "STABLE" : "WETH";
    }

    await this.startMonitoring();
  }
  async stop() {
    this.isActive = false;
  }

  async getLatestTransaction() {
    try {
      const response = await fetch("/api/transactions/latest");
      const data = await response.json();
      return data;
    } catch (error) {
      console.error("Error fetching latest transaction:", error);
      return null;
    }
  }

  async startMonitoring() {
    console.log("Starting monitoring...");
    const swapService = document.getElementById("dexAggregator").value;

    while (this.isActive) {
      const currentTime =
        new Date().getTime() - new Date().getTimezoneOffset() * 60000;
      console.log("Current time:", currentTime);
      const timeSinceLastTrade = currentTime - this.lastTradeTime;
      const adjustedStopLoss = this.stopLossPrice - this.buffer;
      console.log("Time since last trade:", timeSinceLastTrade);
      console.log("Last trade time:", this.lastTradeTime);
      console.log("Current position:", this.currentPosition);

      if (timeSinceLastTrade >= this.cooldown * 1000) {
        console.log("Checking for trade opportunities...");
        if (
          currentPrice < adjustedStopLoss &&
          this.currentPosition === "WETH"
        ) {
          await this.executeSwap("WETH", this.selectedStablecoin, swapService);
          this.currentPosition = "STABLE";
          this.lastTradeTime = currentTime;
        } else if (
          currentPrice > this.stopLossPrice &&
          this.currentPosition === "STABLE"
        ) {
          await this.executeSwap(this.selectedStablecoin, "WETH", swapService);
          this.currentPosition = "WETH";
          this.lastTradeTime = currentTime;
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }

  async executeSwap(fromToken, toToken, swapService) {
    if (this.isSwapping) {
      console.warn("Swap already in progress. Skipping new swap request.");
      return false;
    }

    this.isSwapping = true;
    console.log(`Executing swap: ${fromToken} → ${toToken}`);

    try {
      const txData = {
        chainId: CHAINS[document.getElementById("chain").value],
        fromToken,
        toToken,
        amount: (this.currentAmount || this.initialAmount).toString(),
        slippage: this.slippage,
        gasPriority: this.gasPriority,
      };

      let apiUrl;
      if (swapService === "1inch") {
        apiUrl = "/api/1inch/swap";
      } else if (swapService === "cowswap") {
        apiUrl = "/api/cowswap/swap";
      } else {
        throw new Error("Invalid swap service selected");
      }

      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 1000000);

      const response = await fetch(apiUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(txData),
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      const data = await response.json();

      if (response.ok && data.success) {
        toastr.success(`Trade executed successfully`);
        addToLog(
          `Trade executed: ${fromToken} → ${toToken} Amount: ${this.currentAmount} at ${currentPrice}`
        );
        return true;
      } else {
        throw new Error(data.error || "Failed to execute swap");
      }
    } catch (error) {
      console.error("Swap execution failed");
      toastr.error("Insufficient funds or network error");
      return false;
    } finally {
      this.isSwapping = false;
    }
  }
}

let tradingBot = null;

/**
 * Starts the trading bot with the provided configuration.
 * The bot will automatically execute trades based on the specified parameters.
 *
 * @param {Object} config - The configuration object for the trading bot.
 * @param {string} config.stopLossPrice - The price at which the bot should execute a stop-loss trade.
 * @param {string} config.initialAmount - The initial amount of Ethereum to be used for trading.
 * @param {string} config.stablecoin - The stablecoin to be used for trading.
 * @param {string} [config.slippage] - The maximum allowed slippage for trades, defaulting to 1%.
 * @param {string} [config.gasPriority] - The gas priority to be used for trades, defaulting to "normal".
 * @param {string} [config.cooldown] - The cooldown period in seconds between trades, defaulting to 30 seconds.
 * @param {string} [config.buffer] - The buffer amount to be used for trades, defaulting to 0.5.
 * @param {boolean} [config.isUSDMode] - Whether the bot should operate in USD mode, defaulting to false.
 */
function startBot() {
  const config = {
    stopLossPrice: document.getElementById("stopLoss").value,
    initialAmount: document.getElementById("initialETH").value,
    stablecoin: document.getElementById("stablecoin").value,
    slippage: document.getElementById("slippage").value || "1",
    gasPriority: document.getElementById("gas").value || "normal",
    cooldown: document.getElementById("cooldown").value || "30",
    buffer: document.getElementById("buffer").value || "0.5",
    isUSDMode: window.isUSDMode || false,
  };

  if (!config.stopLossPrice || !config.initialAmount) {
    toastr.warning("Please enter stop-loss price and initial amount");
    return;
  }

  tradingBot = new TradingBot();
  tradingBot.start(config);

  toastr.success("Bot started successfully");
  const statusIndicator = document.getElementById("statusIndicator");
  statusIndicator.classList.remove("paused");
  statusIndicator.classList.add("running");
  statusIndicator.classList.add("active");
  document.getElementById("statusText").textContent = "Bot Running";

  document.querySelectorAll("input, select").forEach((element) => {
    element.setAttribute("disabled", "true");
  });

  document.getElementById("startBot").style.display = "none";
  document.getElementById("stopBot").style.display = "inline-block";

  addToLog(
    `Bot started - Stop loss: ${config.stopLossPrice}, Amount: ${config.initialAmount} ETH, Stablecoin: ${config.stablecoin}, Slippage: ${config.slippage}`
  );
}

/**
 * Stops the trading bot and resets the UI.
 * The bot will no longer execute any trades.
 */
function stopBot() {
  toastr.warning("Bot stopped");
  const statusIndicator = document.getElementById("statusIndicator");
  statusIndicator.classList.remove("running");
  statusIndicator.classList.remove("active");
  statusIndicator.classList.add("paused");
  document.getElementById("statusText").textContent = "Bot Paused";

  document.querySelectorAll("input, select").forEach((element) => {
    element.removeAttribute("disabled");
  });

  const stopButton = document.getElementById("stopBot");
  const startButton = document.getElementById("startBot");
  stopButton.style.display = "none";
  startButton.style.display = "inline-block";

  const now = new Date();
  const formattedTime = now.toUTCString().replace("GMT", "UTC");
  addToLog(`Bot stopped by user at ${formattedTime}`);

  tradingBot.stop();
}

/**
 * Adds a new log entry to the log container. The log entry includes the current time and the provided message.
 * If there are no other log entries, it will display a "No logs available" message.
 *
 * @param {string} message - The message to be added to the log.
 */
function addToLog(message) {
  const now = new Date();
  const formattedTime = now.toUTCString().replace("GMT", "UTC");
  const logEntry = `[${formattedTime}] ${message}`;

  const newLog = document.createElement("p");
  newLog.classList.add("log-item");
  newLog.innerHTML = logEntry;

  const logContainer = document.getElementById("log");
  logContainer.insertBefore(newLog, logContainer.firstChild);

  saveLogToLocalStorage(logEntry);

  const noLogsMessage = document.getElementById("noLogsMessage");
  if (logContainer.children.length > 1) {
    noLogsMessage.style.display = "none";
  } else {
    noLogsMessage.style.display = "block";
  }
}

/**
 * Saves a log entry to the browser's localStorage.
 *
 * @param {string} logEntry - The log entry to be saved.
 */
function saveLogToLocalStorage(logEntry) {
  let logs = JSON.parse(localStorage.getItem("logs")) || [];
  logs.push(logEntry);
  localStorage.setItem("logs", JSON.stringify(logs));
}

/**
 * Loads the trading logs from localStorage and displays them in the log container.
 * If there are no logs available, it adds a message to the log container.
 */
async function loadLogs() {
  const logs = JSON.parse(localStorage.getItem("logs")) || [];
  const logContainer = document.getElementById("log");

  // Check if the log container exists before trying to use it
  if (!logContainer) {
    // console.error("Log container element with ID 'log' not found in the document");
    return; // Exit the function if the container doesn't exist
  }

  logs.reverse();

  if (logs.length > 0) {
    logs.forEach((log) => {
      const logItem = document.createElement("p");
      logItem.classList.add("log-item");
      logItem.innerHTML = log;
      logContainer.appendChild(logItem);
    });
  }
}

function checkSimulatedPrice() {
  if (window.isSimulationMode && window.simulatedPrice !== undefined) {
    return window.simulatedPrice;
  }
  return null;
}
document.addEventListener("DOMContentLoaded", () => {
  loadLogs();
});
