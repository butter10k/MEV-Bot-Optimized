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
    this.cooldown = 5;
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
    const countdownElement = document.getElementById("countdown");

    while (this.isActive) {
      const currentTime =
        new Date().getTime() - new Date().getTimezoneOffset() * 60000;
      const timeSinceLastTrade = currentTime - this.lastTradeTime;
      let adjustedStopLoss;
      if (this.currentPosition === "WETH") {
        adjustedStopLoss = this.stopLossPrice - this.buffer;
      } else {
        adjustedStopLoss = this.stopLossPrice + this.buffer;
      }

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

        countdownElement.style.display = "none";
      } else {
        const remainingTime = Math.ceil(
          (this.cooldown * 1000 - timeSinceLastTrade) / 1000
        );
        countdownElement.innerText = `Cooldown: ${remainingTime} seconds`;
        countdownElement.style.display = "block";
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

    const txData = {
      chainId: CHAINS[document.getElementById("chain").value],
      fromToken,
      toToken,
      amount: this.currentAmount.toString(),
      slippage: this.slippage,
      gasPriority: this.gasPriority,
    };

    const latestTx = await this.getLatestTransaction();
    if (latestTx) {
      txData.amount = parseFloat(latestTx.toAmount).toString();
    }

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

    if (response.status === 400) {
      toastr.error("Error executing swap: " + response.statusText);
      this.isSwapping = false;
      return false;
    }

    clearTimeout(timeoutId);
    const data = await response.json();
    toastr.success(`Trade executed successfully`);
    this.currentAmount = parseFloat(data.updateTransaction.toAmount);
    addToLog(
      `Trade executed: ${fromToken} → ${toToken} Amount: ${this.currentAmount} at ${currentPrice}`
    );
    this.isSwapping = false;
    return true;
  }
}

let tradingBot = null;

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

  if (!logContainer) {
    return;
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
