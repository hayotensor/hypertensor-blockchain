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
  whitelistOverwatchValidatorForDevnet,
  registerOverwatchNode,
  registerValidator,
  removeOverwatchNode,
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
describe("Overwatch validator registration lifecycle", () => {
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
  let validatorId: string;
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
    validatorId = validatorIdOption.unwrap().toString();
    await whitelistOverwatchValidatorForDevnet(api, validatorId, provider);
    minStake = BigInt(
      (await api.query.network.overwatchMinStakeBalance()).toString(),
    );
  });

  it("clears approval on owner removal and requires a fresh whitelist vote", async () => {
    await registerOverwatchNode(overwatchContract, minStake, provider, true);

    const [firstExists, firstNodeIdValue] =
      await overwatchContract.validatorOverwatchNodeId(validatorId);
    expect(firstExists).to.equal(true);
    const firstNodeId = firstNodeIdValue.toString();

    const [returnedNodeId, returnedHotkey] =
      await overwatchContract.overwatchNodes(firstNodeId);
    expect(returnedNodeId.toString()).to.equal(firstNodeId);
    expect(returnedHotkey.toLowerCase()).to.equal(
      validatorHotkey.address.toLowerCase(),
    );
    expect(
      (
        await overwatchContract.overwatchNodeIdHotkey(firstNodeId)
      ).toLowerCase(),
    ).to.equal(validatorHotkey.address.toLowerCase());

    let duplicateRejected = false;
    try {
      await overwatchContract.registerOverwatchNode.staticCall(minStake);
    } catch {
      duplicateRejected = true;
    }
    expect(duplicateRejected).to.equal(true);

    await removeOverwatchNode(overwatchContract, firstNodeId, provider, true);

    const palletReverseAfterRemoval =
      (await api.query.network.validatorOverwatchNodeId(
        validatorId,
      )) as Option<any>;
    expect(palletReverseAfterRemoval.isNone).to.equal(true);
    const [existsAfterRemoval, nodeIdAfterRemoval] =
      await overwatchContract.validatorOverwatchNodeId(validatorId);
    expect(existsAfterRemoval).to.equal(false);
    expect(nodeIdAfterRemoval.toString()).to.equal("0");
    const whitelistAfterRemoval =
      (await api.query.network.overwatchValidatorWhitelist(
        validatorId,
      )) as Option<any>;
    expect(whitelistAfterRemoval.isNone).to.equal(true);

    let removedNodeViewRejected = false;
    try {
      await overwatchContract.overwatchNodes(firstNodeId);
    } catch {
      removedNodeViewRejected = true;
    }
    expect(removedNodeViewRejected).to.equal(true);

    let removedHotkeyViewRejected = false;
    try {
      await overwatchContract.overwatchNodeIdHotkey(firstNodeId);
    } catch {
      removedHotkeyViewRejected = true;
    }
    expect(removedHotkeyViewRejected).to.equal(true);

    let unapprovedRegistrationRejected = false;
    try {
      await overwatchContract.registerOverwatchNode.staticCall(minStake);
    } catch {
      unapprovedRegistrationRejected = true;
    }
    expect(unapprovedRegistrationRejected).to.equal(true);

    await whitelistOverwatchValidatorForDevnet(api, validatorId, provider);
    await registerOverwatchNode(overwatchContract, minStake, provider, true);
    const [secondExists, secondNodeIdValue] =
      await overwatchContract.validatorOverwatchNodeId(validatorId);
    expect(secondExists).to.equal(true);
    expect(
      BigInt(secondNodeIdValue.toString()) >
        BigInt(firstNodeIdValue.toString()),
    ).to.equal(true);

    const secondPalletReverse =
      (await api.query.network.validatorOverwatchNodeId(
        validatorId,
      )) as Option<any>;
    expect(secondPalletReverse.isSome).to.equal(true);
    expect(secondPalletReverse.unwrap().toString()).to.equal(
      secondNodeIdValue.toString(),
    );
  });
});
