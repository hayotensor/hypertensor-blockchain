import { readFile } from "node:fs/promises";
import { Keyring } from "@polkadot/keyring";
import { cryptoWaitReady, decodeAddress } from "@polkadot/util-crypto";
import { config } from "dotenv";
import { Interface, isAddress } from "ethers";
import { Binary, createClient } from "polkadot-api";
import { getTxCreator } from "polkadot-api/tx-creator";
import { getWsProvider } from "polkadot-api/ws";

config({ path: new URL("../.env", import.meta.url), quiet: true });

export async function nativeWallet() {
  const suri = process.env.NATIVE_SURI?.trim();
  if (!suri) throw new Error("Run npm run wallet or set a funded sr25519 NATIVE_SURI in .env.");
  await cryptoWaitReady();
  let pair;
  try {
    pair = new Keyring({ type: "sr25519", ss58Format: 42 }).addFromUri(suri);
  } catch {
    throw new Error("NATIVE_SURI must be a valid sr25519 mnemonic, seed, or secret URI.");
  }
  return {
    address: pair.address,
    signer: getTxCreator(pair.publicKey, "Sr25519", (data) => pair.sign(data)),
  };
}

export async function readOrigin() {
  const address = process.env.READ_ORIGIN?.trim();
  if (!address) return (await nativeWallet()).address;
  decodeAddress(address);
  return address;
}

export function contractAddress() {
  const address = process.env.CONTRACT_ADDRESS ?? "";
  if (!isAddress(address)) {
    throw new Error("Set CONTRACT_ADDRESS to the address printed by npm run demo.");
  }
  return address.toLowerCase();
}

export function newValue(fallback) {
  const value = process.env.NEW_VALUE ?? fallback;
  if (!/^\d+$/.test(value) || BigInt(value) >= 2n ** 256n) {
    throw new Error("NEW_VALUE must be an unsigned uint256 integer.");
  }
  return BigInt(value);
}

export async function artifact() {
  const compiled = JSON.parse(await readFile(new URL("../artifacts/SimpleStorage.json", import.meta.url), "utf8"));
  return { bytecode: Binary.fromHex(compiled.bytecode), abi: new Interface(compiled.abi) };
}

export async function withChain(action) {
  const endpoint = process.env.NATIVE_RPC_URL || "ws://127.0.0.1:9945";
  if (!/^wss?:\/\//.test(endpoint)) throw new Error("NATIVE_RPC_URL must be a ws:// or wss:// native node endpoint.");
  const client = createClient(getWsProvider(endpoint));
  let timer;
  try {
    console.log(`Native RPC: ${endpoint}`);
    return await Promise.race([
      action(client.getUnsafeApi()),
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(
          "Timed out after 180 seconds. Check the native RPC endpoint and that the chain is finalizing blocks. " +
          "A submitted transaction may still execute; check its status before retrying.",
        )), 180_000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
    client.destroy();
  }
}

function describe(value) {
  return JSON.stringify(value, (_, item) => typeof item === "bigint" ? item.toString() : item);
}

export async function confirm(tx, signer) {
  console.log("Submitting native transaction; waiting for finality...");
  const result = await tx.createAndSubmit(signer);
  console.log(`Transaction: ${result.txHash}; finalized in block ${result.block.number}`);
  if (!result.ok) throw new Error(`Native transaction failed: ${describe(result.dispatchError)}`);
  return result;
}

export async function mapping(api, address) {
  const contractAddress = await api.apis.ReviveApi.address(address);
  const owner = await api.query.Revive.OriginalAccount.getValue(contractAddress);
  const registered = owner !== undefined &&
    Buffer.from(decodeAddress(owner)).equals(Buffer.from(decodeAddress(address)));
  return { contractAddress, registered };
}

export async function ensureMapping(api, wallet) {
  const { registered } = await mapping(api, wallet.address);
  if (!registered) {
    console.log("Registering the native account's Revive mapping (once per account)...");
    await confirm(api.tx.Revive.map_account(), wallet.signer);
  }
}

function successfulExecution(dryRun, instantiation = false) {
  if (!dryRun.result.success) throw new Error(`Revive dry run failed: ${describe(dryRun.result.value)}`);
  const result = instantiation ? dryRun.result.value.result : dryRun.result.value;
  if ((result.flags & 1) !== 0) throw new Error(`Contract reverted: ${Binary.toHex(result.data)}`);
  return result;
}

function limits(dryRun) {
  // Use peak storage usage, including temporary deposits, rather than the net change.
  const deposit = dryRun.max_storage_deposit.type === "Charge" ? dryRun.max_storage_deposit.value : 0n;
  const headroom = (value) => (value * 120n + 99n) / 100n;
  return {
    weight_limit: {
      ref_time: headroom(dryRun.weight_required.ref_time),
      proof_size: headroom(dryRun.weight_required.proof_size),
    },
    storage_deposit_limit: headroom(deposit),
  };
}

export async function deploy(api, wallet, bytecode, abi) {
  // Revive expects EVM constructor arguments appended to the creation bytecode.
  const code = Binary.fromHex(Binary.toHex(bytecode) + abi.encodeDeploy([0n]).slice(2));
  const data = Binary.fromHex("0x");
  const dryRun = await api.apis.ReviveApi.instantiate(
    wallet.address, 0n, undefined, undefined, { type: "Upload", value: code }, data, undefined,
  );
  successfulExecution(dryRun, true);
  const result = await confirm(api.tx.Revive.instantiate_with_code({
    value: 0n, ...limits(dryRun), code, data, salt: undefined,
  }), wallet.signer);
  // The finalized event supplies the actual address after native nonce handling.
  const event = result.events.find(({ type, value }) => type === "Revive" && value.type === "Instantiated");
  if (!event) throw new Error("Deployment succeeded but no Revive.Instantiated event was found.");
  return event.value.value.contract;
}

export async function readValue(api, origin, address, abi) {
  const dryRun = await api.apis.ReviveApi.call(
    origin, address, 0n, undefined, undefined, Binary.fromHex(abi.encodeFunctionData("value")),
  );
  const result = successfulExecution(dryRun);
  return abi.decodeFunctionResult("value", Binary.toHex(result.data))[0];
}

export async function setValue(api, wallet, address, abi, value) {
  const data = Binary.fromHex(abi.encodeFunctionData("setValue", [value]));
  const dryRun = await api.apis.ReviveApi.call(wallet.address, address, 0n, undefined, undefined, data);
  successfulExecution(dryRun);
  await confirm(api.tx.Revive.call({
    dest: address, value: 0n, ...limits(dryRun), data,
  }), wallet.signer);
}
