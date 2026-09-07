import { expect } from "chai";
import { ethers } from "ethers";
import { readFileSync } from "fs";
import { resolve } from "path";
import IOverwatchNodeArtifact from "../build/contracts/IOverwatchNode.json";
import StakingArtifact from "../build/contracts/Staking.json";
import SubnetArtifact from "../build/contracts/Subnet.json";
import {
  minimumOutputAfterSlippage,
  minimumSubnetDelegateSharesOut,
  minimumValidatorDelegateSharesOut,
} from "../src/balance-math";

function publicRustSignatures(relativePath: string): string[] {
  const source = readFileSync(resolve(__dirname, relativePath), "utf8");
  return Array.from(
    source.matchAll(/#\[precompile::public\(\s*"([^"]+)"\s*\)\]/g),
    (match) => match[1],
  ).sort();
}

function abiSignatures(abi: any[]): string[] {
  const contractInterface = new ethers.Interface(abi);
  return contractInterface.fragments
    .flatMap((fragment) =>
      ethers.FunctionFragment.isFunction(fragment)
        ? [ethers.FunctionFragment.from(fragment).format("sighash")]
        : [],
    )
    .sort();
}

function networkHelperSource(): string {
  return readFileSync(resolve(__dirname, "../src/network.ts"), "utf8");
}

function directContractCallArguments(method: string): string[] {
  const match = networkHelperSource().match(
    new RegExp(
      `const tx = await contract\\.${method}\\(\\s*([\\s\\S]*?)\\s*\\);`,
    ),
  );
  expect(match, `${method} helper call was not found`).to.not.equal(null);
  return match![1]
    .split(",")
    .map((argument) => argument.trim())
    .filter(Boolean);
}

describe("staking precompile interface", () => {
  const stakingInterface = new ethers.Interface(StakingArtifact.abi);

  const protectedCalls = [
    "addToDelegateStake(uint256,uint256,uint256)",
    "removeDelegateStake(uint256,uint256,uint256)",
    "addValidatorDelegateStake(uint256,uint256,uint256)",
    "removeValidatorDelegateStake(uint256,uint256,uint256)",
    "swapDelegateStake(uint256,uint256,uint256,uint256,uint256,uint256)",
    "swapNodeDelegateStake(uint256,uint256,uint256,uint256,uint256,uint256)",
    "transferFromValidatorToSubnet(uint256,uint256,uint256,uint256,uint256,uint256)",
    "transferFromSubnetToValidator(uint256,uint256,uint256,uint256,uint256,uint256)",
    "updateSwapQueue(uint256,uint256,uint256,uint256,uint256,uint256)",
  ];

  const previews = [
    "previewSubnetDelegateStakeDeposit(uint256,uint256)",
    "previewSubnetDelegateStakeRedeem(uint256,uint256)",
    "previewValidatorDelegateStakeDeposit(uint256,uint256)",
    "previewValidatorDelegateStakeRedeem(uint256,uint256)",
  ];

  const poolViews = [
    "totalSubnetDelegateStakeBalance(uint256)",
    "totalSubnetDelegateStakeShares(uint256)",
    "accountSubnetDelegateStakeShares(address,uint256)",
    "accountSubnetDelegateStakeBalance(address,uint256)",
    "totalValidatorDelegateStakeBalance(uint256)",
    "totalValidatorDelegateStakeShares(uint256)",
    "accountValidatorDelegateStakeShares(address,uint256)",
    "accountValidatorDelegateStakeBalance(address,uint256)",
  ];

  it("keeps every Rust precompile selector in its Solidity ABI", () => {
    expect(abiSignatures(StakingArtifact.abi)).to.deep.equal(
      publicRustSignatures("../../precompiles/src/staking.rs"),
    );
    expect(abiSignatures(SubnetArtifact.abi)).to.deep.equal(
      publicRustSignatures("../../precompiles/src/subnet.rs"),
    );
    expect(abiSignatures(IOverwatchNodeArtifact.abi)).to.deep.equal(
      publicRustSignatures("../../precompiles/src/overwatch_nodes.rs"),
    );
  });

  it("exposes only the slippage- and deadline-protected staking selectors", () => {
    for (const signature of protectedCalls) {
      expect(stakingInterface.getFunction(signature)?.selector).to.equal(
        ethers.id(signature).slice(0, 10),
      );
    }

    expect(
      stakingInterface.getFunction("addToDelegateStake(uint256,uint256)"),
    ).to.equal(null);
    expect(
      stakingInterface.getFunction("removeDelegateStake(uint256,uint256)"),
    ).to.equal(null);
    expect(
      stakingInterface.getFunction(
        "swapDelegateStake(uint256,uint256,uint256)",
      ),
    ).to.equal(null);
    expect(stakingInterface.getFunction("increaseDelegateStake")).to.equal(
      null,
    );
    expect(
      stakingInterface.getFunction("donateValidatorDelegateStake"),
    ).to.equal(null);
  });

  it("exposes deposit and redemption previews for both delegate pool types", () => {
    for (const signature of previews) {
      expect(stakingInterface.getFunction(signature)?.selector).to.equal(
        ethers.id(signature).slice(0, 10),
      );
    }
  });

  it("derives share floors from live pool previews", async () => {
    const calls: unknown[][] = [];
    const previewContract = {
      async previewSubnetDelegateStakeDeposit(...args: unknown[]) {
        calls.push(["subnet", ...args]);
        return BigInt(12_345);
      },
      async previewValidatorDelegateStakeDeposit(...args: unknown[]) {
        calls.push(["validator", ...args]);
        return BigInt(67_890);
      },
    };

    expect(
      await minimumSubnetDelegateSharesOut(
        previewContract,
        "7",
        BigInt(999),
        BigInt(100),
      ),
    ).to.equal(minimumOutputAfterSlippage(BigInt(12_345), BigInt(100)));
    expect(
      await minimumValidatorDelegateSharesOut(
        previewContract,
        "11",
        BigInt(888),
        BigInt(200),
      ),
    ).to.equal(minimumOutputAfterSlippage(BigInt(67_890), BigInt(200)));
    expect(calls).to.deep.equal([
      ["subnet", "7", BigInt(999)],
      ["validator", "11", BigInt(888)],
    ]);
  });

  it("exposes matching balance and share views for both delegate pool types", () => {
    for (const signature of poolViews) {
      expect(stakingInterface.getFunction(signature)?.selector).to.equal(
        ethers.id(signature).slice(0, 10),
      );
    }
  });

  it("rejects accidental native-value transfers to every staking mutation", () => {
    const mutations = stakingInterface.fragments.filter(
      (fragment) =>
        ethers.FunctionFragment.isFunction(fragment) &&
        fragment.stateMutability !== "view",
    );

    expect(mutations.length).to.be.greaterThan(0);
    for (const mutation of mutations) {
      expect(ethers.FunctionFragment.from(mutation).stateMutability).to.equal(
        "nonpayable",
      );
    }
  });

  it("keeps network runtime adapters nonpayable as well as their ABIs", () => {
    for (const relativePath of [
      "../../precompiles/src/staking.rs",
      "../../precompiles/src/subnet.rs",
      "../../precompiles/src/overwatch_nodes.rs",
    ]) {
      expect(
        readFileSync(resolve(__dirname, relativePath), "utf8"),
      ).not.to.include("#[precompile::payable]");
    }
  });

  it("rejects accidental native-value transfers to every subnet mutation", () => {
    const subnetInterface = new ethers.Interface(SubnetArtifact.abi);
    const mutations = subnetInterface.fragments.filter(
      (fragment) =>
        ethers.FunctionFragment.isFunction(fragment) &&
        fragment.stateMutability !== "view",
    );

    expect(mutations.length).to.be.greaterThan(0);
    for (const mutation of mutations) {
      expect(ethers.FunctionFragment.from(mutation).stateMutability).to.equal(
        "nonpayable",
      );
    }
  });

  it("keeps subnet registration argument order aligned with the runtime adapter", () => {
    const subnetInterface = new ethers.Interface(SubnetArtifact.abi);
    const registerSubnet = subnetInterface.getFunction(
      "registerSubnet(uint256,string,string,string,string,uint256,uint256,uint256,(uint256,uint256)[],(string,bytes)[])",
    );
    expect(
      registerSubnet?.inputs.map(({ name, type }) => [name, type]),
    ).to.deep.equal([
      ["maxCost", "uint256"],
      ["name", "string"],
      ["repo", "string"],
      ["description", "string"],
      ["misc", "string"],
      ["minStake", "uint256"],
      ["maxStake", "uint256"],
      ["delegateStakePercentage", "uint256"],
      ["initialValidators", "tuple[]"],
      ["bootnodes", "tuple[]"],
    ]);
    expect(
      registerSubnet?.inputs[8].arrayChildren?.components?.map(
        ({ name, type }) => [name, type],
      ),
    ).to.deep.equal([
      ["validatorId", "uint256"],
      ["count", "uint256"],
    ]);

    const registerSubnetNode = subnetInterface.getFunction(
      "registerSubnetNode(uint256,uint256,address,(string,bytes),(string,bytes),(string,bytes),uint256,string,string,uint256)",
    );
    expect(
      registerSubnetNode?.inputs.map(({ name, type }) => [name, type]),
    ).to.deep.equal([
      ["validatorId", "uint256"],
      ["subnetId", "uint256"],
      ["hotkey", "address"],
      ["peerInfo", "tuple"],
      ["bootnodePeerInfo", "tuple"],
      ["clientPeerInfo", "tuple"],
      ["stakeToBeAdded", "uint256"],
      ["unique", "string"],
      ["nonUnique", "string"],
      ["maxBurnAmount", "uint256"],
    ]);

    expect(directContractCallArguments("registerSubnet")).to.deep.equal([
      "maxCost",
      "name",
      "repo",
      "description",
      "misc",
      "minStake",
      "maxStake",
      "delegateStakePercentage",
      "initialValidators",
      "bootnodes",
    ]);
    expect(directContractCallArguments("registerSubnetNode")).to.deep.equal([
      "validatorId",
      "subnetId",
      "hotkey",
      "peerInfo",
      "bootnodePeerInfo",
      "clientPeerInfo",
      "stakeToBeAdded",
      "unique",
      "nonUnique",
      "maxBurnAmount",
    ]);
  });

  it("keeps the node stake view subnet-first", () => {
    const nodeStake = stakingInterface.getFunction(
      "nodeSubnetStake(uint256,uint256)",
    );
    expect(
      nodeStake?.inputs.map(({ name, type }) => [name, type]),
    ).to.deep.equal([
      ["subnetId", "uint256"],
      ["subnetNodeId", "uint256"],
    ]);
    expect(networkHelperSource()).to.include(
      "contract.nodeSubnetStake(subnetId, subnetNodeId)",
    );
  });

  it("rejects accidental native-value transfers to every Overwatch mutation", () => {
    const overwatchInterface = new ethers.Interface(IOverwatchNodeArtifact.abi);
    const mutations = overwatchInterface.fragments.filter(
      (fragment) =>
        ethers.FunctionFragment.isFunction(fragment) &&
        fragment.stateMutability !== "view",
    );

    expect(mutations.length).to.be.greaterThan(0);
    for (const mutation of mutations) {
      expect(ethers.FunctionFragment.from(mutation).stateMutability).to.equal(
        "nonpayable",
      );
    }
  });

  it("returns the queued destination limit and deadline", () => {
    const queuedSwap = stakingInterface.getFunction(
      "getQueuedSwapCall(uint256)",
    );
    const components = queuedSwap?.outputs[0].components;

    expect(components?.map(({ name, type }) => [name, type])).to.deep.equal([
      ["id", "uint32"],
      ["accountId", "address"],
      ["callType", "uint8"],
      ["toValidatorId", "uint32"],
      ["toSubnetId", "uint32"],
      ["balance", "uint128"],
      ["minSharesOut", "uint128"],
      ["executeBeforeBlock", "uint32"],
      ["queuedAtBlock", "uint32"],
      ["executeAfterBlocks", "uint32"],
    ]);
  });
});
