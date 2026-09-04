import { dev } from "@polkadot-api/descriptors";
import { u8aToHex } from "@polkadot/util";
import { blake2AsU8a } from "@polkadot/util-crypto";
import { ApiPromise, WsProvider } from "@polkadot/api";
import { Option } from "@polkadot/types";
import { expect } from "chai";
import { ethers } from "ethers";
import { TypedApi } from "polkadot-api";

import { ETH_LOCAL_URL, SUB_LOCAL_URL } from "../src/config";
import {
  advanceToRevealBlock,
  batchTransferBalanceFromSudoManual,
  commitOverwatchSubnetWeights,
  createAndFinalizeBlock,
  getCurrentRegistrationCost,
  whitelistOverwatchValidatorForDevnet,
  registerOverwatchNode,
  registerSubnet,
  registerValidator,
  removeOverwatchNode,
  revealOverwatchSubnetWeights,
} from "../src/network";
import { getDevnetApi } from "../src/substrate";
import {
  generateRandomEd25519PeerId,
  generateRandomEthersWallet,
  generateRandomMultiaddr,
  generateRandomString,
  OVERWATCH_NODE_CONTRACT_ABI,
  OVERWATCH_NODE_CONTRACT_ADDRESS,
  SUBNET_CONTRACT_ABI,
  SUBNET_CONTRACT_ADDRESS,
} from "../src/utils";

function decodeRevertReason(error: unknown): string | undefined {
  const pending: unknown[] = [error];
  const visited = new Set<object>();

  while (pending.length > 0) {
    const candidate = pending.shift();
    if (typeof candidate === "string") {
      if (candidate.startsWith("0x08c379a0")) {
        try {
          const [reason] = ethers.AbiCoder.defaultAbiCoder().decode(
            ["string"],
            `0x${candidate.slice(10)}`,
          );
          return String(reason);
        } catch {
          // Keep searching nested provider errors for a decodable payload.
        }
      }
      continue;
    }
    if (typeof candidate !== "object" || candidate === null) {
      continue;
    }
    if (visited.has(candidate)) {
      continue;
    }
    visited.add(candidate);

    const errorObject = candidate as Record<string, unknown>;
    if (typeof errorObject.reason === "string") {
      return errorObject.reason;
    }
    for (const value of Object.values(errorObject)) {
      pending.push(value);
    }
  }

  return undefined;
}

async function expectRevertReason(
  call: Promise<unknown>,
  expectedReason: string,
): Promise<void> {
  let error: unknown;
  try {
    await call;
  } catch (caught) {
    error = caught;
  }

  expect(error, `expected call to revert with ${expectedReason}`).not.to.equal(
    undefined,
  );
  expect(decodeRevertReason(error)).to.equal(expectedReason);
}

// Requires a manually-sealed local development chain.
describe("Overwatch validator-hotkey commit and reveal", () => {
  const subnetOwner = generateRandomEthersWallet();
  const validatorColdkey = generateRandomEthersWallet();
  const validatorHotkey = generateRandomEthersWallet();
  const secondValidatorColdkey = generateRandomEthersWallet();
  const secondValidatorHotkey = generateRandomEthersWallet();
  const thirdValidatorColdkey = generateRandomEthersWallet();
  const thirdValidatorHotkey = generateRandomEthersWallet();

  const subnetOwnerContract = new ethers.Contract(
    SUBNET_CONTRACT_ADDRESS,
    SUBNET_CONTRACT_ABI,
    subnetOwner,
  );
  const validatorContract = new ethers.Contract(
    SUBNET_CONTRACT_ADDRESS,
    SUBNET_CONTRACT_ABI,
    validatorColdkey,
  );
  const secondValidatorContract = new ethers.Contract(
    SUBNET_CONTRACT_ADDRESS,
    SUBNET_CONTRACT_ABI,
    secondValidatorColdkey,
  );
  const thirdValidatorContract = new ethers.Contract(
    SUBNET_CONTRACT_ADDRESS,
    SUBNET_CONTRACT_ABI,
    thirdValidatorColdkey,
  );
  const overwatchColdkeyContract = new ethers.Contract(
    OVERWATCH_NODE_CONTRACT_ADDRESS,
    OVERWATCH_NODE_CONTRACT_ABI,
    validatorColdkey,
  );
  const overwatchHotkeyContract = new ethers.Contract(
    OVERWATCH_NODE_CONTRACT_ADDRESS,
    OVERWATCH_NODE_CONTRACT_ABI,
    validatorHotkey,
  );

  let api: ApiPromise;
  let papiApi: TypedApi<typeof dev>;
  let provider: ethers.JsonRpcProvider;
  let validatorId: string;
  let overwatchNodeId: string;
  let subnetId1: string;
  let subnetId2: string;

  before(async () => {
    papiApi = await getDevnetApi();
    api = await ApiPromise.create({ provider: new WsProvider(SUB_LOCAL_URL) });
    provider = new ethers.JsonRpcProvider(ETH_LOCAL_URL);
    await createAndFinalizeBlock(provider);

    const funding = BigInt("10000000000000000000000");
    await batchTransferBalanceFromSudoManual(api, papiApi, provider, [
      { address: subnetOwner.address, balance: funding },
      { address: validatorColdkey.address, balance: funding },
      { address: validatorHotkey.address, balance: funding },
      { address: secondValidatorColdkey.address, balance: funding },
      { address: thirdValidatorColdkey.address, balance: funding },
    ]);

    await registerValidator(
      validatorContract,
      validatorHotkey.address,
      provider,
      true,
    );
    const validatorIdOption = (await api.query.network.coldkeyValidatorId(
      validatorColdkey.address,
    )) as Option<any>;
    expect(validatorIdOption.isSome).to.equal(true);
    validatorId = validatorIdOption.unwrap().toString();

    await registerValidator(
      secondValidatorContract,
      secondValidatorHotkey.address,
      provider,
      true,
    );
    await registerValidator(
      thirdValidatorContract,
      thirdValidatorHotkey.address,
      provider,
      true,
    );
    const secondValidatorIdOption = (await api.query.network.coldkeyValidatorId(
      secondValidatorColdkey.address,
    )) as Option<any>;
    const thirdValidatorIdOption = (await api.query.network.coldkeyValidatorId(
      thirdValidatorColdkey.address,
    )) as Option<any>;
    expect(secondValidatorIdOption.isSome).to.equal(true);
    expect(thirdValidatorIdOption.isSome).to.equal(true);
    const secondValidatorId = secondValidatorIdOption.unwrap();
    const thirdValidatorId = thirdValidatorIdOption.unwrap();

    const minStake = await api.query.network.minSubnetMinStake();
    const maxStake = await api.query.network.networkMaxStakeBalance();
    const delegateStakePercentage =
      await api.query.network.minDelegateStakePercentage();
    const initialValidators = [
      { validatorId: Number(validatorId), count: 1 },
      { validatorId: Number(secondValidatorId.toString()), count: 1 },
      { validatorId: Number(thirdValidatorId.toString()), count: 1 },
    ];

    const registerTestSubnet = async () => {
      const name = generateRandomString(30);
      const peerId = await generateRandomEd25519PeerId();
      const bootnodes = [
        { peerId, multiaddr: await generateRandomMultiaddr(peerId) },
      ];
      const cost = await getCurrentRegistrationCost(subnetOwnerContract, api);
      await registerSubnet(
        subnetOwnerContract,
        cost,
        name,
        generateRandomString(30),
        generateRandomString(30),
        generateRandomString(30),
        minStake.toString(),
        maxStake.toString(),
        delegateStakePercentage.toString(),
        initialValidators,
        bootnodes,
        cost,
        provider,
        true,
      );
      return (await subnetOwnerContract.getSubnetId(name)).toString();
    };

    subnetId1 = await registerTestSubnet();
    subnetId2 = await registerTestSubnet();

    // Whitelist the validator identity directly; no subnet node is created.
    await whitelistOverwatchValidatorForDevnet(api, validatorId, provider);
    const overwatchMinStake = BigInt(
      (await api.query.network.overwatchMinStakeBalance()).toString(),
    );
    await registerOverwatchNode(
      overwatchColdkeyContract,
      overwatchMinStake,
      provider,
      true,
    );
    const [exists, nodeId] =
      await overwatchColdkeyContract.validatorOverwatchNodeId(validatorId);
    expect(exists).to.equal(true);
    overwatchNodeId = nodeId.toString();
  });

  it("commits and reveals as the canonical validator hotkey", async () => {
    const weight = BigInt("1000000000000000000");
    const salt = Array.from(new Uint8Array(Buffer.from("secret-salt")));
    const encodedTuple = api.registry
      .createType("(u128, Vec<u8>)", [weight, salt])
      .toU8a();
    const commitHash = u8aToHex(blake2AsU8a(encodedTuple, 256));
    const commits = [
      { subnetId: Number(subnetId1), weight: commitHash },
      { subnetId: Number(subnetId2), weight: commitHash },
    ];
    const reveals = [
      { subnetId: Number(subnetId1), weight, salt },
      { subnetId: Number(subnetId2), weight, salt },
    ];

    const currentEpoch = (
      await overwatchHotkeyContract.getCurrentOverwatchEpoch()
    ).toString();
    await commitOverwatchSubnetWeights(
      overwatchHotkeyContract,
      overwatchNodeId,
      commits,
      provider,
      true,
    );

    const palletCommits = (
      await api.query.network.overwatchCommits(currentEpoch, overwatchNodeId)
    ).toJSON() as Record<string, string>;
    for (const commit of commits) {
      expect(palletCommits[commit.subnetId.toString()]).to.equal(commit.weight);
      expect(
        await overwatchHotkeyContract.overwatchCommits(
          currentEpoch,
          overwatchNodeId,
          commit.subnetId,
        ),
      ).to.equal(commit.weight);
    }

    await advanceToRevealBlock(api, provider, Number(currentEpoch));
    await revealOverwatchSubnetWeights(
      overwatchHotkeyContract,
      overwatchNodeId,
      reveals,
      provider,
      true,
    );

    const palletReveals = (
      await api.query.network.overwatchReveals(currentEpoch, overwatchNodeId)
    ).toJSON() as Record<string, string | number>;
    for (const reveal of reveals) {
      expect(palletReveals[reveal.subnetId.toString()]?.toString()).to.equal(
        reveal.weight.toString(),
      );
      expect(
        (
          await overwatchHotkeyContract.overwatchReveals(
            currentEpoch,
            reveal.subnetId,
            overwatchNodeId,
          )
        ).toString(),
      ).to.equal(reveal.weight.toString());
    }

    await removeOverwatchNode(
      overwatchColdkeyContract,
      overwatchNodeId,
      provider,
      true,
    );
    expect(
      Object.keys(
        (
          await api.query.network.overwatchCommits(
            currentEpoch,
            overwatchNodeId,
          )
        ).toJSON() as Record<string, string>,
      ),
    ).to.have.length(0);
    expect(
      Object.keys(
        (
          await api.query.network.overwatchReveals(
            currentEpoch,
            overwatchNodeId,
          )
        ).toJSON() as Record<string, string>,
      ),
    ).to.have.length(0);

    await expectRevertReason(
      overwatchHotkeyContract.overwatchCommits(
        currentEpoch,
        overwatchNodeId,
        subnetId1,
      ),
      "Overwatch commit not found",
    );

    await expectRevertReason(
      overwatchHotkeyContract.overwatchReveals(
        currentEpoch,
        subnetId1,
        overwatchNodeId,
      ),
      "Overwatch reveal not found",
    );
  });
});
