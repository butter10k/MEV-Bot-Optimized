const WETH_ADDRESS = "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1";
const CHAIN_IDS = {
  ethereum: 1,
  arbitrum: 42161,
  optimism: 10,
  base: 8453,
};
const TOKENS = {
  1: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
  42161: "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
  8453: "0x4200000000000000000000000000000000000006",
  10: "0x4200000000000000000000000000000000000006",
};

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
  const selectedChain = chainSelect.value;

  const wethBalance = await getWETHBalance(selectedChain);
  if (wethBalance === 0) {
    walletBalance.textContent = "0 WETH";
  } else {
    walletBalance.textContent = `${Number(wethBalance).toFixed(4)} WETH`;
  }
}

/**
 * Fetches the WETH balance for the selected chain.
 */
/**
 * Fetches the WETH balance for the selected chain.
 */
async function getWETHBalance(selectedChain) {
  const wethAddress = TOKENS[CHAIN_IDS[selectedChain]];

  try {
    const response = await fetch(
      `/api/wallet/balance?chain=${CHAIN_IDS[selectedChain]}&token=${wethAddress}`
    );
    const data = await response.json();
    const balance = data.balance || 0;
    return balance;
  } catch (error) {
    console.error("Error fetching WETH balance:", error);
    return 0;
  }
}

document.getElementById("chain").addEventListener("change", updateChainBalance);
document.addEventListener("DOMContentLoaded", updateChainBalance);
