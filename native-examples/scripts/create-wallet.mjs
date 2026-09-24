import { writeFile } from "node:fs/promises";
import { Keyring } from "@polkadot/keyring";
import { cryptoWaitReady, mnemonicGenerate } from "@polkadot/util-crypto";

await cryptoWaitReady();
const mnemonic = mnemonicGenerate();
const wallet = new Keyring({ type: "sr25519", ss58Format: 42 }).addFromUri(mnemonic);
try {
  await writeFile(new URL("../.env", import.meta.url), [
    "NATIVE_RPC_URL=ws://127.0.0.1:9945",
    `NATIVE_SURI="${mnemonic}"`,
    "CONTRACT_ADDRESS=",
    "READ_ORIGIN=",
    "",
  ].join("\n"), { mode: 0o600, flag: "wx" });
} catch (error) {
  if (error.code === "EEXIST") throw new Error(".env already exists; keeping your existing account unchanged.");
  throw error;
}
console.log(`Native account / funding address: ${wallet.address}`);
console.log("Saved the sr25519 mnemonic to .env (excluded from Git). Keep it private.");
console.log("Fund this native address with TENSOR before running npm run demo.");
