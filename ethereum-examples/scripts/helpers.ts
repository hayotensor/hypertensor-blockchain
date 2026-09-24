import assert from "node:assert/strict";
import { isAddress, type ContractTransactionResponse } from "ethers";

export function contractAddress(): string {
  const address = process.env.CONTRACT_ADDRESS ?? "";
  if (!isAddress(address)) {
    throw new Error("Set CONTRACT_ADDRESS to the deployed address printed by npm run demo.");
  }
  return address;
}

export function newValue(fallback: string): bigint {
  const text = process.env.NEW_VALUE ?? fallback;
  if (!/^\d+$/.test(text) || BigInt(text) >= 2n ** 256n) {
    throw new Error("NEW_VALUE must be an unsigned uint256 integer.");
  }
  return BigInt(text);
}

export async function confirm(tx: ContractTransactionResponse | null): Promise<void> {
  assert(tx, "Missing transaction response");
  console.log(`Transaction: ${tx.hash}; waiting for inclusion...`);
  const receipt = await tx.wait(1, 180_000);
  assert(receipt, "Transaction did not return a receipt");
  assert.equal(receipt.status, 1, "Transaction reverted");
  console.log(`Included in block ${receipt.blockNumber}`);
}
