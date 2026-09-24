import assert from "node:assert/strict";
import { artifact, contractAddress, ensureMapping, nativeWallet, newValue, readValue, setValue, withChain } from "./helpers.mjs";

const address = contractAddress();
const expected = newValue("100");
const wallet = await nativeWallet();
const { abi } = await artifact();
await withChain(async (api) => {
  await ensureMapping(api, wallet);
  console.log(`Contract: ${address}`);
  console.log(`Value before: ${await readValue(api, wallet.address, address, abi)}`);
  await setValue(api, wallet, address, abi, expected);
  const actual = await readValue(api, wallet.address, address, abi);
  assert.equal(actual, expected, "The value read from the chain does not match the update");
  console.log(`Value after: ${actual}`);
  console.log("PASS: read back the expected value.");
});
