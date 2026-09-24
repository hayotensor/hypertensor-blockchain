import { readFile, writeFile } from "node:fs/promises";
import { Wallet } from "ethers";
import { printWalletAddresses } from "./wallet-info.mjs";

const wallet = Wallet.createRandom();
const example = await readFile(new URL("../.env.example", import.meta.url), "utf8");

try {
  await writeFile(
    new URL("../.env", import.meta.url),
    example.replace(/^PRIVATE_KEY=.*$/m, `PRIVATE_KEY=${wallet.privateKey}`),
    { flag: "wx", mode: 0o600 },
  );
} catch (error) {
  if (error.code !== "EEXIST") throw error;
  console.error("A .env file already exists; it was left unchanged. Use npm run account to inspect that wallet.");
  process.exit(1);
}

console.log("Created a new Ethereum wallet in the gitignored .env file.");
printWalletAddresses(wallet);
console.log("Fund the native funding address with TENSOR, then run npm run demo.");
