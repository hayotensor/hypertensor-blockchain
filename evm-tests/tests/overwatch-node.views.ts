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
  qualifyOverwatchValidatorForDevnet,
  registerOverwatchNode,
  registerValidator,
  updateOverwatchHotkey,
} from "../src/network";
import { getDevnetApi } from "../src/substrate";
import {
  generateRandomEthersWallet,
  OVERWATCH_NODE_CONTRACT_ABI,
  OVERWATCH_NODE_CONTRACT_ADDRESS,
  SUBNET_CONTRACT_ABI,
  SUBNET_CONTRACT_ADDRESS,
} from "../src/utils";

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

    await qualifyOverwatchValidatorForDevnet(api, validatorId, provider);
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
});
