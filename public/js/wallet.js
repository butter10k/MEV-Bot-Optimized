const CHAIN_IDS = {
  ethereum: 1,
  arbitrum: 42161,
  optimism: 10,
  base: 8453,
};

window.currentBalances = {};

function initializeWalletInfo() {
  displayWalletAddress();
  updateChainBalance();

  balanceUpdateInterval = setInterval(updateChainBalance, 5 * 60 * 1000);

  window.addEventListener("beforeunload", clearBalanceInterval);
}

function clearBalanceInterval() {
  if (balanceUpdateInterval) {
    clearInterval(balanceUpdateInterval);
    balanceUpdateInterval = null;
  }
}
/**
 * Updates the wallet balance for the selected Ethereum chain.
 *
 * This function first switches the Ethereum network to the selected chain, or adds the chain if it doesn't exist.
 * It then initializes a new Web3 instance with the updated provider and retrieves the balance of the user's
 * Ethereum address on the selected chain. The balance is then displayed in the UI.
 */
async function updateChainBalance() {
  const chainSelect = document.getElementById("chain");
  const walletBalance = document.getElementById("walletBalance");
  const swapBalance = document.getElementById("swapBalance");
  const selectedChain = chainSelect.value;
  const isUSDMode = window.isUSDMode;
  const stablecoin = document.getElementById("stablecoin");
  const selectedStablecoin = stablecoin ? stablecoin.value : "USDC";

  window.currentBalances = await getAllBalances(selectedChain);

  const ethBalance = parseFloat(currentBalances.eth) || 0;
  walletBalance.textContent =
    ethBalance === 0 ? "0 ETH" : `${Number(ethBalance).toFixed(4)} ETH`;

  if (swapBalance) {
    if (isUSDMode) {
      const stablecoinBalance =
        parseFloat(currentBalances[selectedStablecoin.toLowerCase()]) || 0;
      swapBalance.innerHTML = `<img src="/wallet-two.png" alt="wallet"> ${Number(
        stablecoinBalance
      ).toFixed(2)} ${selectedStablecoin}`;
    } else {
      const wethBalance = parseFloat(currentBalances.weth) || 0;
      swapBalance.innerHTML = `<img src="/wallet-two.png" alt="wallet"> ${Number(
        wethBalance
      ).toFixed(4)} WETH`;
    }
  }
}

async function displayWalletAddress() {
  try {
    const response = await fetch("/api/wallet/address");
    const data = await response.json();
    const address = data.address;

    const formattedAddress = `${address.slice(0, 6)}...${address.slice(-4)}`;

    const walletAddressElement = document.getElementById("walletAddress");
    if (walletAddressElement) {
      walletAddressElement.textContent = formattedAddress;
    }

    const walletInfo = document.getElementById("walletInfo");
    if (walletInfo) {
      walletInfo.style.display = "flex";
    }
  } catch (error) {
    console.error("Error fetching wallet address:", error);
  }
}

/**
 * Fetches all token balances for the selected chain.
 */
async function getAllBalances(selectedChain) {
  try {
    const response = await fetch(
      `/api/wallet/balance?chain=${CHAIN_IDS[selectedChain]}`
    );
    const data = await response.json();
    return data;
  } catch (error) {
    console.error("Error fetching balances:", error);
    return { eth: 0, weth: 0, usdt: 0, usdc: 0, dai: 0 };
  }
}

document.getElementById("chain").addEventListener("change", updateChainBalance);
const stablecoinElement = document.getElementById("stablecoin");
if (stablecoinElement) {
  stablecoinElement.addEventListener("change", updateChainBalance);
}
document.addEventListener("DOMContentLoaded", function () {
  const usdModeCheckbox = document.querySelector("#startUSD .checkbox");
  if (usdModeCheckbox) {
    usdModeCheckbox.addEventListener("change", updateChainBalance);
  }

  initializeWalletInfo();
});
