const CHAINS = {
  ethereum: 1,
  arbitrum: 42161,
  optimism: 10,
  base: 8453,
};

let currentPrice = 0;
let stopLossThreshold = 0;
let isTrading = false;
let initialETHAmount = 0;
let isSimulationMode = false; // Add this flag

async function getETHPrice() {
  try {
    // If simulation mode is enabled, use the simulated price
    if (isSimulationMode) {
      // Just update the UI without fetching
      document.querySelector(
        ".currentPrice span"
      ).textContent = `${currentPrice.toFixed(2)} (Simulated)`;

      return currentPrice;
    }

    // Normal mode - fetch from API
    const response = await fetch("/api/price");
    const responseData = await response.json();

    currentPrice = parseFloat(responseData);
    document.querySelector(
      ".currentPrice span"
    ).textContent = `${currentPrice.toFixed(2)}`;

    return currentPrice;
  } catch (error) {
    console.log("Error fetching ETH price:", error);
    return null;
  }
}

function updateStablecoinOptions() {
  const selectedChain = document.getElementById("chain").value;
  const stablecoinSelect = document.getElementById("stablecoin");
  if (!stablecoinSelect) return;
  stablecoinSelect.innerHTML = "";

  if (selectedChain === "base") {
    stablecoinSelect.add(new Option("USDC", "USDC"));
    stablecoinSelect.add(new Option("DAI", "DAI"));
  } else {
    stablecoinSelect.add(new Option("USDT", "USDT"));
    stablecoinSelect.add(new Option("USDC", "USDC"));
    stablecoinSelect.add(new Option("DAI", "DAI"));
  }
}

// Function to handle price simulation
function setSimulatedPrice(price) {
  currentPrice = parseFloat(price);
  document.querySelector(
    ".currentPrice span"
  ).textContent = `${currentPrice.toFixed(2)} (Simulated)`;

  // Log the price change for testing
  addLogMessage(`Simulated ETH price set to $${currentPrice.toFixed(2)}`);

  return currentPrice;
}

// Add event listeners
document
  .getElementById("chain")
  .addEventListener("change", updateStablecoinOptions);
document
  .getElementById("dexAggregator")
  .addEventListener("change", getETHPrice);
document.addEventListener("DOMContentLoaded", () => {
  getETHPrice();

  // Setup simulation mode controls
  const simulationToggle = document.getElementById("simulationToggle");
  const simulationInputs = document.getElementById("simulationInputs");
  const simulatedPrice = document.getElementById("simulatedPrice");
  const applyPrice = document.getElementById("applyPrice");

  simulationToggle.addEventListener("change", function () {
    isSimulationMode = this.checked;
    simulationInputs.style.display = this.checked ? "block" : "none";

    if (this.checked) {
      addLogMessage("Price simulation mode enabled");

      // If a price is already entered, apply it immediately
      if (simulatedPrice.value) {
        setSimulatedPrice(simulatedPrice.value);
      }
    } else {
      addLogMessage(
        "Price simulation mode disabled, returning to real-time prices"
      );
      getETHPrice(); // Return to real price
    }
  });

  applyPrice.addEventListener("click", function () {
    if (simulatedPrice.value) {
      setSimulatedPrice(simulatedPrice.value);
    }
  });

  // Allow pressing Enter to apply the price
  simulatedPrice.addEventListener("keyup", function (event) {
    if (event.key === "Enter" && this.value) {
      setSimulatedPrice(this.value);
    }
  });
});

// Helper function to add log messages
function addLogMessage(message) {
  const logContainer = document.getElementById("log");
  const noLogsMessage = document.getElementById("noLogsMessage");

  if (noLogsMessage) {
    noLogsMessage.style.display = "none";
  }

  const logItem = document.createElement("p");
  logItem.className = "log-item";

  const timestamp = new Date().toLocaleTimeString();
  logItem.innerHTML = `<span class="timestamp">[${timestamp}]</span> ${message}`;

  logContainer.insertBefore(logItem, logContainer.firstChild);
}

setInterval(getETHPrice, 10000);
