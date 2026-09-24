import assert from "node:assert/strict";
import { network } from "hardhat";
import { confirm, contractAddress, newValue } from "./helpers.ts";

const connection = await network.create();
try {
  const { ethers } = connection;
  const [signer] = await ethers.getSigners();
  if (!signer) throw new Error("Set a funded Ethereum PRIVATE_KEY in .env.");
  const address = contractAddress();
  const expected = newValue("100");
  const storage = await ethers.getContractAt("SimpleStorage", address, signer);
  console.log(`Contract: ${address}`);
  console.log(`Value before: ${await storage.value()}`);
  await confirm(await storage.setValue(expected));
  const actual = await storage.value();
  assert.equal(actual, expected, "The value read from the chain does not match the update");
  console.log(`Value after: ${actual}`);
  console.log("PASS: read back the expected value.");
} finally {
  await connection.close();
}
