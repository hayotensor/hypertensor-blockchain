import assert from "node:assert/strict";
import { network } from "hardhat";
import { confirm, newValue } from "./helpers.ts";

const connection = await network.create();
try {
  const { ethers } = connection;
  const [deployer] = await ethers.getSigners();
  if (!deployer) throw new Error("Run npm run wallet or set a funded Ethereum PRIVATE_KEY in .env.");
  const balance = await ethers.provider.getBalance(deployer.address);
  console.log(`Deployer: ${deployer.address}`);
  console.log(`Spendable balance: ${ethers.formatEther(balance)} TENSOR`);
  if (balance === 0n) throw new Error("Fund the deployer first. npm run account prints its native funding address.");

  const expected = newValue("42");
  if (expected === 0n) throw new Error("Choose a nonzero NEW_VALUE so the demo changes the initial value.");
  console.log("Deploying SimpleStorage with value 0...");
  const storage = await ethers.deployContract("SimpleStorage", [0n], deployer);
  // Wait before reading or sending the next transaction.
  await confirm(storage.deploymentTransaction());
  const address = await storage.getAddress();
  console.log(`Contract address: ${address}`);

  const before = await storage.value();
  assert.equal(before, 0n, "Unexpected initial value");
  console.log(`Value before: ${before}`);

  console.log(`Setting value to ${expected}...`);
  await confirm(await storage.setValue(expected));
  const after = await storage.value();
  assert.equal(after, expected, "The value read from the chain does not match the update");
  console.log(`Value after: ${after}`);
  console.log("PASS: deployed, updated, and read back the expected value.");
  console.log(`For later calls, put CONTRACT_ADDRESS=${address} in .env.`);
} finally {
  await connection.close();
}
