import { artifact, contractAddress, readOrigin, readValue, withChain } from "./helpers.mjs";

const address = contractAddress();
const origin = await readOrigin();
const { abi } = await artifact();
await withChain(async (api) => {
  console.log(`Contract: ${address}`);
  console.log(`Value: ${await readValue(api, origin, address, abi)}`);
});
