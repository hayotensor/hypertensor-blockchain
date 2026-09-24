import { createRequire } from "node:module";
import dotenv from "dotenv";
import hardhatEthers from "@nomicfoundation/hardhat-ethers";
import { configVariable, defineConfig } from "hardhat/config";

dotenv.config({ quiet: true });
const require = createRequire(import.meta.url);

export default defineConfig({
  plugins: [hardhatEthers],
  solidity: {
    version: "0.8.28",
    // Use the pinned npm compiler, including when compiling offline after npm ci.
    path: require.resolve("solc/soljson.js"),
    settings: {
      evmVersion: "cancun",
      optimizer: { enabled: true, runs: 200 },
    },
  },
  networks: {
    talaris: {
      type: "http",
      chainType: "generic",
      url: process.env.ETH_RPC_URL || "http://127.0.0.1:8545",
      chainId: Number(process.env.ETH_CHAIN_ID || "1337"),
      accounts: process.env.PRIVATE_KEY ? [configVariable("PRIVATE_KEY")] : [],
      timeout: 120_000,
      gas: "auto",
      gasPrice: "auto",
    },
  },
});
