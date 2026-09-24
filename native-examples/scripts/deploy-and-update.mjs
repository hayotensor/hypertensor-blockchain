import assert from "node:assert/strict";
import { artifact, deploy, ensureMapping, nativeWallet, newValue, readValue, setValue, withChain } from "./helpers.mjs";

const expected = newValue("42");
if (expected === 0n) throw new Error("Choose a nonzero NEW_VALUE so the demo changes the initial value.");
const wallet = await nativeWallet();
const { bytecode, abi } = await artifact();
await withChain(async (api) => {
  console.log(`Deployer: ${wallet.address}`);
  await ensureMapping(api, wallet);
  console.log("Deploying SimpleStorage with value 0...");
  const address = await deploy(api, wallet, bytecode, abi);
  console.log(`Contract address: ${address}`);
  const before = await readValue(api, wallet.address, address, abi);
  assert.equal(before, 0n, "Unexpected initial value");
  console.log(`Value before: ${before}`);
  console.log(`Setting value to ${expected}...`);
  await setValue(api, wallet, address, abi, expected);
  const after = await readValue(api, wallet.address, address, abi);
  assert.equal(after, expected, "The value read from the chain does not match the update");
  console.log(`Value after: ${after}`);
  console.log("PASS: deployed, updated, and read back the expected value.");
  console.log(`For later calls, put CONTRACT_ADDRESS=${address} in .env.`);
});
