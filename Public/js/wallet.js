const WETH_ADDRESS = "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1";
const CHAIN_IDS = {
  ethereum: 1,
  arbitrum: 42161,
  optimism: 10,
  base: 8453,
};

const ERC20_ABI = [
  {
    constant: true,
    inputs: [
      {
        name: "owner",
        type: "address",
      },
    ],
    name: "balanceOf",
    outputs: [
      {
        name: "",
        type: "uint256",
      },
    ],
    payable: false,
    stateMutability: "view",
    type: "function",
  },
];
const TOKENS = {
  1: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
  42161: "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
  8453: "0x4200000000000000000000000000000000000006",
  10: "0x4200000000000000000000000000000000000006",
};

const NETWORK_PARAMS = {
  arbitrum: {
    chainId: "0xa4b1",
    chainName: "Arbitrum One",
    nativeCurrency: {
      name: "Arbitrum Ether",
      symbol: "ETH",
      decimals: 18,
    },
    rpcUrls: [
      "https://arbitrum-mainnet.infura.io/v3/8540d4064b0d462d94687313fa21080b",
    ],
    blockExplorerUrls: ["https://arbiscan.io/"],
  },
  base: {
    chainId: "0x2105",
    chainName: "Base",
    nativeCurrency: {
      name: "Base Ether",
      symbol: "ETH",
      decimals: 18,
    },
    rpcUrls: [
      "https://base-mainnet.infura.io/v3/8540d4064b0d462d94687313fa21080b",
    ],
    blockExplorerUrls: ["https://basescan.org"],
  },
  optimism: {
    chainId: "0xa",
    chainName: "Optimism",
    nativeCurrency: {
      name: "Optimism Ether",
      symbol: "ETH",
      decimals: 18,
    },
    rpcUrls: [
      "https://optimism-mainnet.infura.io/v3/8540d4064b0d462d94687313fa21080b",
    ],
    blockExplorerUrls: ["https://optimistic.etherscan.io"],
  },
};
/**
 * Connects the user's wallet to the application.
 *
 * This function first checks if the user has MetaMask installed. If not, it displays an error message.
 * If MetaMask is installed, it requests access to the user's Ethereum accounts, retrieves the user's
 * first account and its balance, and updates the UI to display the wallet address and balance.
 * If an error occurs during the connection process, it displays an error message.
 */
async function connectWallet() {
  if (typeof window.ethereum === "undefined") {
    toastr.error(
      "MetaMask is not installed. Please install MetaMask to use this application."
    );
    return;
  }

  try {
    await window.ethereum.request({ method: "eth_requestAccounts" });

    document.getElementById("connectButton").style.display = "none";
    document.getElementById("walletInfo").style.display = "flex";
    const walletAddress = window.ethereum.selectedAddress;
    if (walletAddress) {
      const shortenedAddress = `${walletAddress.slice(
        0,
        4
      )}...${walletAddress.slice(-3)}`;
      document.getElementById("walletAddress").textContent = shortenedAddress;
    } else {
      document.getElementById("walletAddress").textContent =
        "No wallet connected";
    }
    await updateChainBalance();
  } catch (error) {
    toastr.error("Failed to connect wallet: " + error.message);
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
  const selectedChain = chainSelect.value;
  const chainId = CHAIN_IDS[selectedChain];

  try {
    const hexChainId = `0x${parseInt(chainId).toString(16)}`;
    await window.ethereum.request({
      method: "wallet_switchEthereumChain",
      params: [{ chainId: hexChainId }],
    });
  } catch (error) {
    if (error.code === 4902) {
      await window.ethereum.request({
        method: "wallet_addEthereumChain",
        params: [NETWORK_PARAMS[selectedChain]],
      });
    }
  }

  if (window.ethereum.selectedAddress) {
    const wethBalance = await getWETHBalance(selectedChain);
    if (wethBalance === 0) {
      walletBalance.textContent = "0 WETH";
    } else {
      walletBalance.textContent = `${Number(wethBalance).toFixed(4)} WETH`;
    }
  }
}

/**
 * Fetches the WETH balance for the selected chain.
 */
async function getWETHBalance(selectedChain) {
  const wethAddress = TOKENS[CHAIN_IDS[selectedChain]];
  const web3 = new Web3(window.ethereum);
  const tokenContract = new web3.eth.Contract(ERC20_ABI, wethAddress);

  const balance = await tokenContract.methods
    .balanceOf(window.ethereum.selectedAddress)
    .call();
  return web3.utils.fromWei(balance, "ether");
}

document.getElementById("chain").addEventListener("change", updateChainBalance);
window.addEventListener("load", async () => {
  if (
    localStorage.getItem("walletConnected") === "true" &&
    typeof window.ethereum !== "undefined"
  ) {
    connectWallet();
  }
});
