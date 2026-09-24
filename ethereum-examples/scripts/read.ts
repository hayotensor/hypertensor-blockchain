import { artifacts, network } from "hardhat";
import { contractAddress } from "./helpers.ts";

const connection = await network.create();
try {
  const { ethers } = connection;
  const address = contractAddress();
  const { abi } = await artifacts.readArtifact("SimpleStorage");
  const storage = new ethers.Contract(address, abi, ethers.provider);
  console.log(`Contract: ${address}`);
  console.log(`Value: ${await storage.value()}`);
} finally {
  await connection.close();
}
