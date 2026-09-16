import { dev } from "@polkadot-api/descriptors";
import { ApiPromise, WsProvider } from "@polkadot/api";
import { Option } from "@polkadot/types";
import { expect } from "chai";
import { ethers } from "ethers";
import { TypedApi } from "polkadot-api";

import { ETH_LOCAL_URL, SUB_LOCAL_URL } from "../src/config";
import {
  batchTransferBalanceFromSudoManual,
  createAndFinalizeBlock,
  registerOverwatchNode,
  registerValidator,
  seedOverwatchSignalViewsForDevnet,
  updateOverwatchHotkey,
  whitelistOverwatchValidatorForDevnet,
} from "../src/network";
import { getDevnetApi } from "../src/substrate";
import {
  generateRandomEthersWallet,
  OVERWATCH_NODE_CONTRACT_ABI,
  OVERWATCH_NODE_CONTRACT_ADDRESS,
  SUBNET_CONTRACT_ABI,
  SUBNET_CONTRACT_ADDRESS,
} from "../src/utils";

describe("Overwatch ABI surface", () => {
  const overwatchInterface = new ethers.Interface(OVERWATCH_NODE_CONTRACT_ABI);

  it("exposes effective-signal views and omits deleted eligibility views", () => {
    expect(
      overwatchInterface.hasFunction("effectiveOverwatchSignalMeta"),
    ).to.equal(true);
    expect(
      overwatchInterface.hasFunction("effectiveOverwatchSubnetWeight"),
    ).to.equal(true);

    for (const deletedFunction of [
      "overwatchMinRepScore",
      "overwatchMinAvgAttestationRatio",
      "overwatchMinAge",
    ]) {
      expect(
        overwatchInterface.hasFunction(deletedFunction),
        `${deletedFunction} must not remain in the checked-in ABI`,
      ).to.equal(false);
    }
  });
});

// Requires a manually-sealed local development chain.
describe("Overwatch validator-only views", () => {
  const coldkey = generateRandomEthersWallet();
  const validatorHotkey = generateRandomEthersWallet();
  const overwatchHotkeyOverride = generateRandomEthersWallet();

  const subnetContract = new ethers.Contract(
    SUBNET_CONTRACT_ADDRESS,
    SUBNET_CONTRACT_ABI,
    coldkey,
  );
  const overwatchContract = new ethers.Contract(
    OVERWATCH_NODE_CONTRACT_ADDRESS,
    OVERWATCH_NODE_CONTRACT_ABI,
    coldkey,
  );

  let api: ApiPromise;
  let papiApi: TypedApi<typeof dev>;
  let provider: ethers.JsonRpcProvider;
  let validatorId: string;
  let overwatchNodeId: string;
  let overwatchMinStake: bigint;

  before(async () => {
    papiApi = await getDevnetApi();
    api = await ApiPromise.create({ provider: new WsProvider(SUB_LOCAL_URL) });
    provider = new ethers.JsonRpcProvider(ETH_LOCAL_URL);
    await createAndFinalizeBlock(provider);

    const funding = BigInt("10000000000000000000000");
    await batchTransferBalanceFromSudoManual(api, papiApi, provider, [
      { address: coldkey.address, balance: funding },
      { address: validatorHotkey.address, balance: funding },
    ]);

    await registerValidator(
      subnetContract,
      validatorHotkey.address,
      provider,
      true,
    );
    const validatorIdOption = (await api.query.network.coldkeyValidatorId(
      coldkey.address,
    )) as Option<any>;
    expect(validatorIdOption.isSome).to.equal(true);
    validatorId = validatorIdOption.unwrap().toString();

    await whitelistOverwatchValidatorForDevnet(api, validatorId, provider);
    overwatchMinStake = BigInt(
      (await api.query.network.overwatchMinStakeBalance()).toString(),
    );
    await registerOverwatchNode(
      overwatchContract,
      overwatchMinStake,
      provider,
      true,
    );

    const [exists, nodeId] =
      await overwatchContract.validatorOverwatchNodeId(validatorId);
    expect(exists).to.equal(true);
    overwatchNodeId = nodeId.toString();
  });

  it("matches pallet state and resolves validator hotkey fallback and override", async () => {
    const stakeBalance =
      await api.query.network.overwatchNodeStakeBalance(overwatchNodeId);
    expect(BigInt(stakeBalance.toString())).to.equal(overwatchMinStake);
    expect(
      BigInt(
        (
          await overwatchContract.accountOverwatchStake(overwatchNodeId)
        ).toString(),
      ),
    ).to.equal(overwatchMinStake);

    const totalStake = await api.query.network.totalOverwatchNodeStakeBalance();
    expect(BigInt(totalStake.toString())).to.equal(
      BigInt((await overwatchContract.totalOverwatchStake()).toString()),
    );

    expect((await overwatchContract.maxOverwatchNodes()).toString()).to.equal(
      (await api.query.network.maxOverwatchNodes()).toString(),
    );
    expect((await overwatchContract.totalOverwatchNodes()).toString()).to.equal(
      (await api.query.network.totalOverwatchNodes()).toString(),
    );
    expect(
      (await overwatchContract.totalOverwatchNodeUids()).toString(),
    ).to.equal((await api.query.network.totalOverwatchNodeUids()).toString());
    expect(
      (await overwatchContract.overwatchEpochLengthMultiplier()).toString(),
    ).to.equal(
      (
        await api.query.network.activeOverwatchEpochLengthMultiplier()
      ).toString(),
    );
    expect(
      (await overwatchContract.overwatchEpochStartBlock()).toString(),
    ).to.equal((await api.query.network.overwatchEpochStartBlock()).toString());
    expect(
      (await overwatchContract.overwatchMinStakeBalance()).toString(),
    ).to.equal((await api.query.network.overwatchMinStakeBalance()).toString());

    const latestEffectiveSignal =
      (await api.query.network.latestEffectiveOverwatchSignal()) as Option<any>;
    const latestSignalRevision =
      await api.query.network.latestOverwatchSignalRevision();
    const [signalExists, sourceEpoch, revision, signalValid] =
      await overwatchContract.effectiveOverwatchSignalMeta();
    expect(signalExists).to.equal(latestEffectiveSignal.isSome);
    expect(revision.toString()).to.equal(latestSignalRevision.toString());
    if (latestEffectiveSignal.isSome) {
      const signal = latestEffectiveSignal.unwrap().toJSON() as Record<
        string,
        unknown
      >;
      expect(sourceEpoch.toString()).to.equal(
        String(signal.sourceEpoch ?? signal.source_epoch),
      );
      expect(signalValid).to.equal(signal.valid);
    } else {
      expect(sourceEpoch.toString()).to.equal("0");
      expect(signalValid).to.equal(false);
    }

    const [rawWeightExists, rawWeight, resolvedWeight] =
      await overwatchContract.effectiveOverwatchSubnetWeight(0xffffffff);
    expect(rawWeightExists).to.equal(false);
    expect(rawWeight.toString()).to.equal("0");
    expect(resolvedWeight.toString()).to.equal(
      (await api.query.network.defaultOverwatchSubnetWeight()).toString(),
    );

    const lastFinalized = await api.query.network.lastFinalizedOverwatchEpoch();
    const lastFinalizedJson = lastFinalized.toJSON() as string | number | null;
    const [lastFinalizedExists, lastFinalizedEpoch] =
      await overwatchContract.lastFinalizedOverwatchEpoch();
    expect(lastFinalizedExists).to.equal(lastFinalizedJson !== null);
    if (lastFinalizedJson !== null) {
      expect(lastFinalizedEpoch.toString()).to.equal(
        lastFinalizedJson.toString(),
      );
    }

    const palletNodeId = (await api.query.network.validatorOverwatchNodeId(
      validatorId,
    )) as Option<any>;
    expect(palletNodeId.isSome).to.equal(true);
    expect(palletNodeId.unwrap().toString()).to.equal(overwatchNodeId);
    const [precompileExists, precompileNodeId] =
      await overwatchContract.validatorOverwatchNodeId(validatorId);
    expect(precompileExists).to.equal(true);
    expect(precompileNodeId.toString()).to.equal(overwatchNodeId);

    const [returnedNodeId, fallbackHotkey] =
      await overwatchContract.overwatchNodes(overwatchNodeId);
    expect(returnedNodeId.toString()).to.equal(overwatchNodeId);
    expect(fallbackHotkey.toLowerCase()).to.equal(
      validatorHotkey.address.toLowerCase(),
    );
    expect(
      (
        await overwatchContract.overwatchNodeIdHotkey(overwatchNodeId)
      ).toLowerCase(),
    ).to.equal(validatorHotkey.address.toLowerCase());

    await updateOverwatchHotkey(
      overwatchContract,
      overwatchNodeId,
      overwatchHotkeyOverride.address,
      provider,
      true,
    );
    const [, overriddenHotkey] =
      await overwatchContract.overwatchNodes(overwatchNodeId);
    expect(overriddenHotkey.toLowerCase()).to.equal(
      overwatchHotkeyOverride.address.toLowerCase(),
    );
    expect(
      (
        await overwatchContract.overwatchNodeIdHotkey(overwatchNodeId)
      ).toLowerCase(),
    ).to.equal(overwatchHotkeyOverride.address.toLowerCase());

    await updateOverwatchHotkey(
      overwatchContract,
      overwatchNodeId,
      null,
      provider,
      true,
    );
    const [, restoredFallback] =
      await overwatchContract.overwatchNodes(overwatchNodeId);
    expect(restoredFallback.toLowerCase()).to.equal(
      validatorHotkey.address.toLowerCase(),
    );
    expect(
      (
        await overwatchContract.overwatchNodeIdHotkey(overwatchNodeId)
      ).toLowerCase(),
    ).to.equal(validatorHotkey.address.toLowerCase());

    expect(
      Number((await overwatchContract.getCurrentOverwatchEpoch()).toString()),
    ).to.be.greaterThan(0);
  });

  it("keeps finalized history distinct from the latest effective signal", async () => {
    const currentEpoch = Number(
      (await overwatchContract.getCurrentOverwatchEpoch()).toString(),
    );
    const historicalEpoch = Math.max(0, currentEpoch - 1);
    const subnetId = 0xfffffff0;
    const missingSubnetId = subnetId + 1;
    const historicalSubnetWeight = BigInt("700000000000000000");
    const historicalNodeWeight = BigInt("600000000000000000");
    const effectiveRawWeight = BigInt(0);
    const revision =
      BigInt(
        (await api.query.network.latestOverwatchSignalRevision()).toString(),
      ) + BigInt(1);

    await seedOverwatchSignalViewsForDevnet(api, provider, {
      historicalEpoch,
      subnetId,
      overwatchNodeId,
      historicalSubnetWeight,
      historicalNodeWeight,
      effectiveRawWeight,
      revision,
    });

    expect(
      (
        await overwatchContract.overwatchSubnetWeights(
          historicalEpoch,
          subnetId,
        )
      ).toString(),
    ).to.equal(historicalSubnetWeight.toString());
    expect(
      (
        await overwatchContract.overwatchNodeWeights(
          historicalEpoch,
          overwatchNodeId,
        )
      ).toString(),
    ).to.equal(historicalNodeWeight.toString());

    const [lastFinalizedExists, lastFinalizedEpoch] =
      await overwatchContract.lastFinalizedOverwatchEpoch();
    expect(lastFinalizedExists).to.equal(true);
    expect(lastFinalizedEpoch.toString()).to.equal(historicalEpoch.toString());

    const [signalExists, sourceEpoch, returnedRevision, signalValid] =
      await overwatchContract.effectiveOverwatchSignalMeta();
    expect(signalExists).to.equal(true);
    expect(sourceEpoch.toString()).to.equal(historicalEpoch.toString());
    expect(returnedRevision.toString()).to.equal(revision.toString());
    expect(signalValid).to.equal(true);

    const [rawWeightExists, rawWeight, resolvedWeight] =
      await overwatchContract.effectiveOverwatchSubnetWeight(subnetId);
    expect(rawWeightExists).to.equal(true);
    expect(rawWeight.toString()).to.equal(effectiveRawWeight.toString());
    expect(resolvedWeight.toString()).to.equal(effectiveRawWeight.toString());

    const [missingRawWeightExists, missingRawWeight, missingResolvedWeight] =
      await overwatchContract.effectiveOverwatchSubnetWeight(missingSubnetId);
    expect(missingRawWeightExists).to.equal(false);
    expect(missingRawWeight.toString()).to.equal("0");
    expect(missingResolvedWeight.toString()).to.equal(
      (await api.query.network.defaultOverwatchSubnetWeight()).toString(),
    );
  });
});
