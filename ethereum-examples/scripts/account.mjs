import dotenv from "dotenv";
import { JsonRpcProvider, Wallet, formatEther } from "ethers";
import { printWalletAddresses } from "./wallet-info.mjs";

dotenv.config({ quiet: true });
if (!/^0x[0-9a-fA-F]{64}$/.test(process.env.PRIVATE_KEY ?? "")) {
  throw new Error("Set an Ethereum PRIVATE_KEY in .env, or run npm run wallet to create a new .env.");
}
const wallet = new Wallet(process.env.PRIVATE_KEY);
printWalletAddresses(wallet);

const provider = new JsonRpcProvider(process.env.ETH_RPC_URL || "http://127.0.0.1:8545");
try {
  console.log(`Spendable balance: ${formatEther(await provider.getBalance(wallet.address))} TENSOR`);
} finally {
  provider.destroy();
}
