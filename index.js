import "dotenv/config";
import express from "express";
import cors from "cors";
import axios from "axios";
import { ethers, Contract } from "ethers";
import { FusionSDK, PrivateKeyProviderConnector } from "@1inch/fusion-sdk";
import { TradingSdk, SupportedChainId, OrderKind } from "@cowprotocol/cow-sdk";
import Web3 from "web3";
import { Network, Alchemy } from "alchemy-sdk";
import Transaction from "./models/transaction.js";
import connectDB from "./utils/connectDB.js";
const ERC20_ABI = [
  "function balanceOf(address owner) view returns (uint256)",
  "function allowance(address owner, address spender) view returns (uint256)",
  "function approve(address spender, uint256 amount) returns (bool)",
];

const LIMIT_ORDER_CONTRACT = "0x111111125421cA6dc452d289314280a0f8842A65";
const COWSWAP_CONTRACT = "0xC92E8bdf79f0507f65a392b0ab4667716BFE0110";
let UNIQUE_ID = "0x0000000000000000000000000000000000000000";
const TOKENS = {
  1: {
    WETH: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    USDT: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
    USDC: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    DAI: "0x6B175474E89094C44Da98b954EedeAC495271d0F",
  },
  42161: {
    WETH: "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
    USDT: "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9",
    USDC: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
    DAI: "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1",
  },
  10: {
    WETH: "0x4200000000000000000000000000000000000006",
    USDT: "0x94b008aA00579c1307B0EF2c499aD98a8ce58e58",
    USDC: "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85",
    DAI: "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1",
  },
  8453: {
    WETH: "0x4200000000000000000000000000000000000006",
    USDC: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
    DAI: "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb",
  },
};

const app = express();
app.use(cors());
app.use(express.static("public"));
app.use(express.json());
connectDB();

/**
 * Handles the API endpoint for fetching the price of WETH to USDT.
 *
 * @param {Object} req - The HTTP request object.
 * @param {Object} res - The HTTP response object.
 * @returns {Promise<void>} - A Promise that resolves when the response is sent.
 */
app.get("/api/price", async (req, res) => {
  const settings = {
    apiKey: process.env.ALCHEMY_API_KEY,
    network: Network.ETH_MAINNET,
  };
  const alchemy = new Alchemy(settings);
  const symbols = ["ETH"];

  try {
    const prices = await alchemy.prices.getTokenPriceBySymbol(symbols);
    res.json(prices.data[0].prices[0].value);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

app.post("/api/1inch/swap", async (req, res) => {
  let {
    chainId,
    fromToken,
    toToken,
    amount,
    gasPriority,
    slippage = 1,
  } = req.body;

  try {
    const decimals = await getDecimals(chainId, TOKENS[chainId][fromToken]);
    amount = (amount * 10 ** decimals) / 1e18;
    console.log("request body", req.body);

    const rpc_url =
      chainId === 1
        ? process.env.ETH_RPC_URL
        : chainId === 10
        ? process.env.OPTIMISM_RPC_URL
        : chainId === 8453
        ? process.env.BASE_RPC_URL
        : chainId === 42161
        ? process.env.ARBITRUM_RPC_URL
        : process.env.ETH_RPC_URL;

    const provider = new ethers.providers.JsonRpcProvider(rpc_url);
    const wallet = new ethers.Wallet(process.env.PRIVATE_KEY, provider);

    const tokenContract = new Contract(
      TOKENS[chainId][fromToken],
      ERC20_ABI,
      wallet
    );

    const blockchainProvider = new PrivateKeyProviderConnector(
      process.env.PRIVATE_KEY,
      new Web3(rpc_url)
    );

    const sdk = new FusionSDK({
      url: process.env.INCH_API_SWAP_URL,
      network: chainId,
      blockchainProvider,
      authKey: process.env.INCH_API_KEY,
    });

    await new Promise((resolve) => setTimeout(resolve, 2000));

    const currentAllowance = await tokenContract.allowance(
      wallet.address,
      LIMIT_ORDER_CONTRACT
    );

    if (currentAllowance.lt(ethers.BigNumber.from(amount.toString()))) {
      try {
        const approveTx = await tokenContract.approve(
          LIMIT_ORDER_CONTRACT,
          ethers.constants.MaxUint256
        );

        await approveTx.wait(1);
        await tokenContract.allowance(wallet.address, LIMIT_ORDER_CONTRACT);
      } catch (error) {
        console.error("Approval error:", error);
        return res
          .status(500)
          .json({ error: "Failed to approve token: " + error.message });
      }
    } else {
      console.log("Sufficient allowance already exists");
    }

    const orderId = await executeWithRetry(() =>
      sdk.placeOrder({
        fromTokenAddress: TOKENS[chainId][fromToken],
        toTokenAddress: TOKENS[chainId][toToken],
        amount: amount.toString(),
        walletAddress: wallet.address,
        preset: gasPriority,
      })
    );

    console.log("Order ID:", orderId);

    const transaction = await SaveOrder(
      wallet.address,
      {
        chainId,
        fromToken,
        toToken,
        amount,
        slippage,
        decimals,
      },
      "1inch"
    );

    scanWalletAndUpdateTransaction(
      wallet.address,
      transaction,
      chainId,
      "1inch"
    ).catch((error) => console.error("Error scanning wallet:", error));

    res.json({
      success: true,
      orderId: orderId,
      transactionId: transaction._id,
    });
  } catch (error) {
    console.error("Fill order error:", error);

    let errorMessage = error.message || "Unknown error";
    let errorCode = "SWAP_FAILED";

    if (error.response && error.response.data) {
      if (error.response.data.description === "NotEnoughBalanceOrAllowance") {
        errorMessage = "Insufficient balance or allowance for this swap";
        errorCode = "INSUFFICIENT_BALANCE";
      } else if (error.response.data.description) {
        errorMessage = error.response.data.description;
      }
    }

    res.status(400).json({
      success: false,
      error: errorMessage,
      errorCode: errorCode,
    });
  }
});

/**
 * Handles a POST request to the "/api/cowswap/swap" endpoint, which allows users to swap tokens using the CowSwap protocol.
 *
 * The function first retrieves the necessary parameters from the request body, including the chain ID, the "from" token, the "to" token, and the amount to swap. It then calculates the amount of the "from" token to swap based on the token's decimals.
 *
 * Next, the function sets up a connection to the appropriate Ethereum RPC URL based on the chain ID, creates a wallet instance using the provided private key, and initializes a TradingSdk instance.
 *
 * The function then checks the current allowance of the "from" token for the CowSwap contract. If the allowance is insufficient, it attempts to approve the CowSwap contract to spend the necessary amount of the "from" token.
 *
 * Finally, the function creates the necessary parameters for a CowSwap order and submits the order using the TradingSdk. If the order is successful, the function returns a JSON response with a "success" flag set to true.
 *
 * @param {Object} req - The Express.js request object.
 * @param {Object} res - The Express.js response object.
 * @returns {Promise<void>}
 */
app.post("/api/cowswap/swap", async (req, res) => {
  let { chainId, fromToken, toToken, amount, slippage = 1 } = req.body;

  try {
    const decimals = await getDecimals(chainId, TOKENS[chainId][fromToken]);
    amount = (amount * 10 ** decimals) / 1e18;

    const rpc_url =
      chainId === 1
        ? process.env.ETH_RPC_URL
        : chainId === 10
        ? process.env.OPTIMISM_RPC_URL
        : chainId === 8453
        ? process.env.BASE_RPC_URL
        : chainId === 42161
        ? process.env.ARBITRUM_RPC_URL
        : process.env.ETH_RPC_URL;
    const provider = new ethers.providers.JsonRpcProvider(rpc_url);

    const wallet = new ethers.Wallet(process.env.PRIVATE_KEY, provider);
    const sdk = new TradingSdk({
      chainId: chainId,
      signer: wallet,
    });

    await new Promise((resolve) => setTimeout(resolve, 2000));

    const tokenContract = new Contract(
      TOKENS[chainId][fromToken],
      ERC20_ABI,
      wallet
    );

    const currentAllowance = await tokenContract.allowance(
      wallet.address,
      COWSWAP_CONTRACT
    );

    if (currentAllowance.lt(ethers.BigNumber.from(amount.toString()))) {
      try {
        const approveTx = await tokenContract.approve(
          COWSWAP_CONTRACT,
          ethers.constants.MaxUint256
        );
        await approveTx.wait();
      } catch (error) {
        console.error("Approval error:", error);
        return res.status(500).json({ error: error.message });
      }
    }

    const parameters = {
      kind: OrderKind.SELL,
      sellToken: TOKENS[chainId][fromToken],
      sellTokenDecimals: 18,
      buyToken: TOKENS[chainId][toToken],
      buyTokenDecimals: 18,
      amount: amount.toString(),
      slippageBps: slippage * 100,
    };

    const orderId = await executeWithRetry(() => sdk.postSwapOrder(parameters));
    console.log("Order ID:", orderId);

    const transaction = await SaveOrder(
      wallet.address,
      {
        chainId,
        fromToken,
        toToken,
        amount,
        slippage,
        decimals,
      },
      "CowSwap"
    );

    scanWalletAndUpdateTransaction(
      wallet.address,
      transaction,
      chainId,
      "CowSwap"
    ).catch((error) => console.error("Error scanning wallet:", error));

    res.json({
      success: true,
      orderId: orderId,
      transactionId: transaction._id,
    });
  } catch (error) {
    console.error("Fill order error:", error);

    let errorMessage = error.message || "Unknown error";

    if (error.response && error.response.data) {
      if (error.response.data.description) {
        errorMessage = error.response.data.description;
      }
    }

    res.status(400).json({ success: false, error: errorMessage });
  }
});

/**
 * Retrieves the latest transactions and returns them as a JSON response.
 *
 * @param {Object} req - The HTTP request object.
 * @param {Object} res - The HTTP response object.
 * @returns {Promise<void>} - A Promise that resolves when the response is sent.
 */
app.get("/api/transactions/history", async (req, res) => {
  try {
    const transactions = await Transaction.find()
      .sort({ timestamp: -1 })
      .select(
        "timestamp chainId network dex fromToken toToken fromAmount toAmount price txHash status"
      );

    res.json(transactions);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Retrieves the latest transaction and returns it as a JSON response.
 *
 * @param {Object} req - The HTTP request object.
 * @param {Object} res - The HTTP response object.
 * @returns {Promise<void>} - A Promise that resolves when the response is sent.
 */
app.get("/api/transactions/latest", async (req, res) => {
  try {
    const latestTransaction = await Transaction.findOne()
      .sort({ timestamp: -1 })
      .select(
        "timestamp chainId network dex fromToken toToken fromAmount toAmount price txHash status"
      );

    res.json(latestTransaction);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Retrieves the number of decimal places for a given token on a specific blockchain.
 *
 * @param {number} chainId - The ID of the blockchain network.
 * @param {string} tokenAddress - The address of the token.
 * @returns {Promise<number>} - The number of decimal places for the token.
 */
const getDecimals = async (chainId, tokenAddress) => {
  try {
    const rpc_url =
      chainId === 1
        ? process.env.ETH_RPC_URL
        : chainId === 10
        ? process.env.OPTIMISM_RPC_URL
        : chainId === 8453
        ? process.env.BASE_RPC_URL
        : chainId === 42161
        ? process.env.ARBITRUM_RPC_URL
        : process.env.ETH_RPC_URL;

    const provider = new ethers.providers.JsonRpcProvider(rpc_url);
    const tokenContract = new Contract(
      tokenAddress,
      ["function decimals() view returns (uint8)"],
      provider
    );
    const decimals = await tokenContract.decimals();
    return decimals;
  } catch (error) {
    console.error("Error getting decimals:", error);
    return 18;
  }
};

/**
 * Executes a function with retry capability, but quits immediately on 400 errors
 *
 * @param {Function} requestFunction - The function to execute and potentially retry
 * @param {number} maxRetries - Maximum number of retry attempts
 * @param {number} baseDelay - Base delay between retries in ms
 * @returns {Promise<any>} - Result of the function call
 */
const executeWithRetry = async (
  requestFunction,
  maxRetries = 5,
  baseDelay = 3000
) => {
  for (let attempt = 0; attempt < maxRetries; attempt++) {
    try {
      return await requestFunction();
    } catch (error) {
      if (error.response && error.response.status == 400) {
        console.error(
          `Client error (${error.response.status}), not retrying:`,
          error.response?.data || error.message
        );
        throw error;
      }

      const isLastAttempt = attempt === maxRetries - 1;
      if (isLastAttempt) {
        console.error(`Final attempt ${attempt + 1} failed:`, error);
        throw error;
      }

      const jitter = Math.random() * 500;
      const delay = Math.pow(2, attempt) * baseDelay + jitter;

      console.log(
        `Attempt ${attempt + 1} failed, retrying in ${Math.round(delay)}ms`
      );
      await new Promise((resolve) => setTimeout(resolve, delay));
    }
  }
};

/**
 * Tracks an order and saves its details to MongoDB
 *
 * @param {string} orderId - The order ID from 1inch or CowSwap
 * @param {object} orderDetails - Initial order details
 * @param {string} dex - The DEX used ("1inch" or "CowSwap")
 * @returns {Promise<object>} - The saved transaction
 */
async function SaveOrder(userAddresss, orderDetails, dex) {
  try {
    const {
      chainId,
      fromToken,
      toToken,
      amount,
      slippage = 1,
      decimals,
    } = orderDetails;

    let transaction;

    const network =
      {
        1: "ethereum",
        42161: "arbitrum",
        10: "optimism",
        8453: "base",
      }[chainId] || "ethereum";

    const baseTransaction = {
      chainId,
      network,
      dex,
      fromToken,
      toToken,
      slippage,
    };

    transaction = new Transaction({
      ...baseTransaction,
      fromAmount: (amount / 10 ** decimals).toString(),
    });

    await transaction.save();

    return transaction;
  } catch (error) {
    console.error("Error tracking order:", error);
  }
}

/**
 * Scans user wallet for transactions related to an order and updates the database record
 *
 * @param {string} walletAddress - User's wallet address
 * @param {object} transaction - The transaction document from MongoDB
 * @param {number} chainId - Chain ID to scan
 * @param {string} dex - The DEX used ("1inch" or "CowSwap")
 * @returns {Promise<object>} - Updated transaction record
 */
async function scanWalletAndUpdateTransaction(
  walletAddress,
  transaction,
  chainId,
  dex
) {
  console.log(
    `Monitoring wallet ${walletAddress} for transaction on chain ${chainId}...`
  );

  // Set up the appropriate scan API based on chain ID
  const apiUrl =
    chainId === 1
      ? "https://api.etherscan.io/api"
      : chainId === 42161
      ? "https://api.arbiscan.io/api"
      : chainId === 10
      ? "https://api-optimistic.etherscan.io/api"
      : chainId === 8453
      ? "https://api.basescan.org/api"
      : "https://api.etherscan.io/api";

  const apiKey =
    chainId === 1
      ? process.env.ETHERSCAN_API_KEY
      : chainId === 42161
      ? process.env.ARBISCAN_API_KEY
      : chainId === 10
      ? process.env.OPTIMISM_API_KEY
      : chainId === 8453
      ? process.env.BASESCAN_API_KEY
      : process.env.ETHERSCAN_API_KEY;

  let found = false;
  let attempts = 0;
  const maxAttempts = 10;

  while (!found && attempts < maxAttempts) {
    try {
      const response = await axios.get(
        `${apiUrl}?module=account&action=tokentx&address=${walletAddress}&startblock=0&endblock=99999999&page=1&offset=1&sort=desc&apikey=${apiKey}`
      );

      if (response.data.status === "1" && response.data.result.length > 0) {
        const tokenTx = response.data.result[0];
        const timestamp = parseInt(tokenTx.timeStamp) * 1000;

        const tenMinutesAgo = Date.now() - 5 * 60 * 1000;

        if (timestamp > tenMinutesAgo) {
          if (
            tokenTx.to.toLowerCase() === walletAddress.toLowerCase() &&
            transaction.toToken.toLowerCase() ===
              tokenTx.tokenSymbol.toLowerCase()
          ) {
            UNIQUE_ID = Buffer.from(UNIQUE_ID + tokenTx.hash).toString(
              "base64"
            );
            let updatedTransaction;
            console.log("Transaction found!", tokenTx);

            const updateData = {
              txHash: tokenTx.hash,
              status: "completed",
              timestamp: new Date(timestamp),
              uniqueId: UNIQUE_ID,
              toAmount: tokenTx.value / 10 ** tokenTx.tokenDecimal,
            };

            await Transaction.findByIdAndUpdate(transaction._id, updateData, {
              new: true,
            });
            return updatedTransaction;
          }
        }
      }

      attempts++;
      console.log(
        `Attempt ${attempts}/${maxAttempts} - No matching transaction found yet`
      );
      await new Promise((resolve) => setTimeout(resolve, 10000));
    } catch (error) {
      console.error("Error scanning for transactions:", error);
      attempts++;
      await new Promise((resolve) => setTimeout(resolve, 30000));
    }
  }

  if (!found) {
    const updatedTransaction = await Transaction.findByIdAndUpdate(
      transaction._id,
      { status: "pending" },
      { new: true }
    );
    console.log(`No matching transaction found after ${maxAttempts} attempts`);
    return updatedTransaction;
  }
}
const port = process.env.PORT || 5000;
app.listen(port, () => {
  console.log(`Server running at http://localhost:${port}`);
});
