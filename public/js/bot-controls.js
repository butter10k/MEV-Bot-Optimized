// Bot control functions
let bot = new TradingBot();

function startBot() {
  const stopLossPrice = document.getElementById("stopLoss").value;
  const initialAmount = document.getElementById("initialETH").value;
  const stablecoin = document.getElementById("stablecoin").value;
  const slippage = document.getElementById("slippage").value;
  const gasPriority = document.getElementById("gas").value;
  const buffer = document.getElementById("buffer").value;
  const cooldown = document.getElementById("cooldown").value;

  const wasSimulationEnabled = window.isSimulationMode;
  const simulatedPriceValue = document.getElementById("simulatedPrice").value;

  if (!stopLossPrice || !initialAmount) {
    toastr.error("Please fill in all required fields");
    return;
  }

  // Update UI to show bot is running
  document.getElementById("startBot").style.display = "none";
  document.getElementById("stopBot").style.display = "block";
  document.getElementById("statusIndicator").className =
    "status-indicator active";
  document.getElementById("statusText").textContent = "Bot Active";

  addLogMessage("Bot started with stop-loss threshold: $" + stopLossPrice);

  bot.start({
    stopLossPrice,
    initialAmount,
    stablecoin,
    slippage,
    gasPriority,
    buffer,
    cooldown,
    isUSDMode: window.isUSDMode,
  });

  // Restore simulation mode if it was enabled
  if (wasSimulationEnabled) {
    document.getElementById("simulationToggle").checked = true;
    window.isSimulationMode = true;
    document.getElementById("simulationInputs").style.display = "block";

    if (simulatedPriceValue) {
      document.getElementById("simulatedPrice").value = simulatedPriceValue;
      setSimulatedPrice(simulatedPriceValue);
    }
  }
}

function stopBot() {
  bot.stop();

  document.getElementById("startBot").style.display = "block";
  document.getElementById("stopBot").style.display = "none";
  document.getElementById("statusIndicator").className =
    "status-indicator paused";
  document.getElementById("statusText").textContent = "Bot Paused";

  addLogMessage("Bot stopped");
}
