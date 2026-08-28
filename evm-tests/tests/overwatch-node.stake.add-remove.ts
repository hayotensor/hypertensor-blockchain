import { dev } from "@polkadot-api/descriptors";
import { ApiPromise, WsProvider } from "@polkadot/api";
import { Option } from "@polkadot/types";
import { expect } from "chai";
import { ethers } from "ethers";
import { TypedApi } from "polkadot-api";

import { ETH_LOCAL_URL, SUB_LOCAL_URL } from "../src/config";
import {
  addToOverwatchStake,
  batchTransferBalanceFromSudoManual,
  createAndFinalizeBlock,
  qualifyOverwatchValidatorForDevnet,
  registerOverwatchNode,
  registerValidator,
  removeOverwatchStake,
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
describe("Overwatch node-ID stake calls", () => {
  const coldkey = generateRandomEthersWallet();
  const validatorHotkey = generateRandomEthersWallet();

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
  let overwatchNodeId: string;
  let minStake: bigint;

  before(async () => {
    papiApi = await getDevnetApi();
    api = await ApiPromise.create({ provider: new WsProvider(SUB_LOCAL_URL) });
    provider = new ethers.JsonRpcProvider(ETH_LOCAL_URL);
    await createAndFinalizeBlock(provider);

    await batchTransferBalanceFromSudoManual(api, papiApi, provider, [
      {
        address: coldkey.address,
        balance: BigInt("10000000000000000000000"),
      },
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
    const validatorId = validatorIdOption.unwrap().toString();
    await qualifyOverwatchValidatorForDevnet(api, validatorId, provider);

    minStake = BigInt(
      (await api.query.network.overwatchMinStakeBalance()).toString(),
    );
    await registerOverwatchNode(overwatchContract, minStake, provider, true);
    const [exists, nodeId] =
      await overwatchContract.validatorOverwatchNodeId(validatorId);
    expect(exists).to.equal(true);
    overwatchNodeId = nodeId.toString();
  });

  it("adds and removes stake using the active Overwatch node ID", async () => {
    const addedStake = BigInt("10000000000000000000");
    await addToOverwatchStake(
      overwatchContract,
      overwatchNodeId,
      addedStake,
      provider,
      true,
    );

    const afterAdd = BigInt(
      (
        await api.query.network.overwatchNodeStakeBalance(overwatchNodeId)
      ).toString(),
    );
    expect(afterAdd).to.equal(minStake + addedStake);
    expect(
      BigInt(
        (
          await overwatchContract.accountOverwatchStake(overwatchNodeId)
        ).toString(),
      ),
    ).to.equal(afterAdd);

    const removedStake = BigInt("1000000000000000000");
    await removeOverwatchStake(
      overwatchContract,
      overwatchNodeId,
      removedStake,
      provider,
      true,
    );

    const afterRemove = BigInt(
      (
        await api.query.network.overwatchNodeStakeBalance(overwatchNodeId)
      ).toString(),
    );
    expect(afterRemove).to.equal(afterAdd - removedStake);
    expect(
      BigInt(
        (
          await overwatchContract.accountOverwatchStake(overwatchNodeId)
        ).toString(),
      ),
    ).to.equal(afterRemove);
  });
});
