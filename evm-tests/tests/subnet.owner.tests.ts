import * as assert from "assert";
import { getDevnetApi } from "../src/substrate"
import { dev } from "@polkadot-api/descriptors"
import { PolkadotSigner, TypedApi } from "polkadot-api";
import { ethers } from "ethers"
import { generateRandomEd25519PeerId, generateRandomEthersWallet, generateRandomMultiaddr, generateRandomString, getPublicClient, STAKING_CONTRACT_ABI, STAKING_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, SUBNET_CONTRACT_ADDRESS, waitForBlocks } from "../src/utils"
import { Option } from '@polkadot/types';
import {
    activateSubnet,
    addToDelegateStake,
    batchTransferBalanceFromSudo,
    getCurrentRegistrationCost,
    getConsensusEligibleFromSubnetEpoch,
    getMinSubnetDelegateStakeBalance,
    getPauseStartedGlobalEpoch,
    getPauseStartedSubnetEpoch,
    getSlotIndex,
    getSubnetAtSlot,
    ownerAddOrUpdateInitialValidators,
    ownerDeactivateSubnet,
    ownerPauseSubnet,
    ownerRemoveInitialValidators,
    ownerUnpauseSubnet,
    ownerUpdateChurnLimit,
    ownerUpdateDelegateStakePercentage,
    ownerUpdateDescription,
    ownerUpdateIdleClassificationEpochs,
    ownerUpdateIncludedClassificationEpochs,
    ownerUpdateMaxRegisteredNodes,
    ownerUpdateMisc,
    ownerUpdateName,
    ownerUpdateRegistrationQueueEpochs,
    ownerUpdateRepo,
    transferSubnetOwnership,
    acceptSubnetOwnership,
    ownerAddBootnodeAccess,
    ownerUpdateTargetNodeRegistrationsPerEpoch,
    ownerUpdateNodeBurnRateAlpha,
    ownerUpdateQueueImmunityEpochs,
    updateBootnodes,
    registerSubnet,
    registerSubnetNode,
    registerValidator,
    transferBalanceFromSudo,
    ownerUpdateMinMaxStake
} from "../src/network"
import { ETH_LOCAL_URL, SUB_LOCAL_URL } from "../src/config";
import { PublicClient } from "viem";
import { ApiPromise, WsProvider } from "@polkadot/api";
import { expect } from "chai";
import { minimumSubnetDelegateSharesOut } from "../src/balance-math";

// npm test -- -g "Test subnet register activate-0xuhnrfvok"
describe("Test subnet owner-0xuhnrfvok", () => {
    // init eth part
    const wallet1 = generateRandomEthersWallet();
    const wallet2 = generateRandomEthersWallet();
    const wallet3 = generateRandomEthersWallet();
    const wallet4 = generateRandomEthersWallet();
    const wallet5 = generateRandomEthersWallet();
    const wallet6 = generateRandomEthersWallet();
    const wallet7 = generateRandomEthersWallet();
    const wallet8 = generateRandomEthersWallet();

    const ALL_WALLETS = new Map([
        [wallet1, wallet2],
        [wallet3, wallet4],
        [wallet5, wallet6],
        [wallet7, wallet8],
    ]);

    let publicClient: PublicClient;
    let papiApi: TypedApi<typeof dev>
    let api: ApiPromise

    const sudoTransferAmount = BigInt(1000e18)

    // sudo account alice as signer
    let alice: PolkadotSigner;
    before(async () => {
        publicClient = await getPublicClient(ETH_LOCAL_URL)
        // init variables got from await and async
        papiApi = await getDevnetApi()

        const provider = new WsProvider(SUB_LOCAL_URL);

        api = await ApiPromise.create({ provider });

        await transferBalanceFromSudo(
            api,
            papiApi,
            SUB_LOCAL_URL,
            wallet1.address,
            sudoTransferAmount,
        )
    })

    // Status: passing
    // npm test -- -g "testing subnet owner functions-0xARAD3gb3"
    it("testing subnet owner functions-0xARAD3gb3", async () => {
        const subnetContract = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet1);
        expect(subnetContract.interface.hasFunction("getPrevPauseEpoch")).to.be.false;
        expect(subnetContract.interface.hasFunction("getConsensusStartSubnetEpoch")).to.be.false;
        expect(subnetContract.interface.hasFunction("getConsensusEligibleFromSubnetEpoch")).to.be.true;
        expect(subnetContract.interface.hasFunction("getPauseStartedGlobalEpoch")).to.be.true;
        expect(subnetContract.interface.hasFunction("getPauseStartedSubnetEpoch")).to.be.true;
        expect(subnetContract.interface.hasFunction("getSlotAssignment")).to.be.false;
        expect(subnetContract.interface.hasFunction("getSubnetAtSlot")).to.be.true;

        // Designated general-epoch slots are never assigned to subnets.
        await assert.rejects(() => getSubnetAtSlot(subnetContract, "0"));

        const nonexistentSubnetId = BigInt("4294967295").toString();
        await assert.rejects(() => getConsensusEligibleFromSubnetEpoch(subnetContract, nonexistentSubnetId));
        await assert.rejects(() => getPauseStartedGlobalEpoch(subnetContract, nonexistentSubnetId));
        await assert.rejects(() => getPauseStartedSubnetEpoch(subnetContract, nonexistentSubnetId));

        const cost = await getCurrentRegistrationCost(subnetContract, api)
        const subnetName = generateRandomString(30)
        const repo = generateRandomString(30)
        const description = generateRandomString(30)
        const misc = generateRandomString(30)
        const minStake = await api.query.network.minSubnetMinStake();
        const maxStake = await api.query.network.networkMaxStakeBalance();
        const delegateStakePercentage = await api.query.network.minDelegateStakePercentage();

        let BOOTNODES: { peerId: string; multiaddr: Uint8Array }[] = [
            {
                peerId: (await generateRandomEd25519PeerId()),
                multiaddr: await generateRandomMultiaddr((await generateRandomEd25519PeerId()))
            }
        ]

        const validatorIds = new Map<string, string>();
        const coldkeys = Array.from(ALL_WALLETS.keys());
        await batchTransferBalanceFromSudo(
            api,
            papiApi,
            coldkeys.map(wallet => ({
                address: wallet.address,
                balance: sudoTransferAmount,
            })),
        );
        for (const [coldkey, hotkey] of ALL_WALLETS.entries()) {
            const validatorContract = new ethers.Contract(
                SUBNET_CONTRACT_ADDRESS,
                SUBNET_CONTRACT_ABI,
                coldkey,
            );
            await registerValidator(validatorContract, hotkey.address);
            const validatorIdOption = await api.query.network.coldkeyValidatorId(coldkey.address) as Option<any>;
            expect(validatorIdOption.isSome).to.equal(true);
            validatorIds.set(coldkey.address, validatorIdOption.unwrap().toString());
        }
        const initialValidators = [...validatorIds.values()].map(validatorId => ({
            validatorId,
            count: 1,
        }));

        await registerSubnet(
            subnetContract,
            cost,
            subnetName,
            repo,
            description,
            misc,
            minStake.toString(),
            maxStake.toString(),
            delegateStakePercentage.toString(),
            initialValidators,
            BOOTNODES,
        )

        const palletSubnetId = await api.query.network.subnetName(subnetName);
        expect(palletSubnetId != undefined);

        const subnetId = await subnetContract.getSubnetId(subnetName);
        expect(BigInt(subnetId)).to.not.equal(BigInt(0))
        await assert.rejects(() => getConsensusEligibleFromSubnetEpoch(subnetContract, subnetId));
        await assert.rejects(() => getPauseStartedGlobalEpoch(subnetContract, subnetId));
        await assert.rejects(() => getPauseStartedSubnetEpoch(subnetContract, subnetId));

        const assignedSlot = (await api.query.network.subnetSlot(subnetId)).toString();
        expect(await getSlotIndex(subnetContract, subnetId)).to.equal(BigInt(assignedSlot));
        expect(await getSubnetAtSlot(subnetContract, assignedSlot)).to.equal(BigInt(subnetId));

        const minStakeAmount = (await api.query.network.minSubnetMinStake()).toString();
        const recipients = coldkeys.map(wallet => ({
            address: wallet.address,
            balance: BigInt(minStakeAmount + BigInt(500))
        }));

        await batchTransferBalanceFromSudo(
            api,
            papiApi,
            recipients
        )
        // Get enough nodes registered to meet min nodes requirement
        await Promise.all([...ALL_WALLETS.entries()].map(async ([coldkey, hotkey]) => {
            const accountSubnetContract = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, coldkey);

            let peer1 = await generateRandomEd25519PeerId()
            let peer_info_1 = {
                peerId: peer1,
                multiaddr: await generateRandomMultiaddr(peer1)
            }
            let peer_info_2 = {
                peerId: "",
                multiaddr: new Uint8Array()
            }
            let peer_info_3 = {
                peerId: "",
                multiaddr: new Uint8Array()
            }

            const unique = generateRandomString(5)
            const nonUnique = generateRandomString(5)

            await registerSubnetNode(
                accountSubnetContract,
                validatorIds.get(coldkey.address)!,
                subnetId,
                hotkey.address,
                peer_info_1,
                peer_info_2,
                peer_info_3,
                BigInt(minStakeAmount),
                unique,
                nonUnique,
                "1000000000000000000"
            );
        }));

        //
        // Subnet owner functions
        //

        const newSubnetName = generateRandomString(30)
        const newRepo = generateRandomString(30)
        const newDescription = generateRandomString(30)
        const newMisc = generateRandomString(30)
        const newChurnLimit = (BigInt((await api.query.network.maxChurnLimit()).toString()) - BigInt(1)).toString();
        const newMinStake = (BigInt((await api.query.network.minSubnetMinStake()).toString()) + BigInt(1)).toString();
        const newMaxStake = (BigInt((await api.query.network.networkMaxStakeBalance()).toString()) - BigInt(1)).toString();
        const newDelegateStakePercentage = (BigInt((await api.query.network.minDelegateStakePercentage()).toString()) + BigInt(1)).toString();
        const currentSubnetNodeQueueEpochs = (await api.query.network.subnetNodeQueueEpochs(subnetId)).toString();
        const newSubnetNodeQueueEpochs = (BigInt((await api.query.network.minQueueEpochs()).toString()) + BigInt(1)).toString();
        const newIdleClassificationEpochs = (BigInt((await api.query.network.minIdleClassificationEpochs()).toString()) + BigInt(1)).toString();
        const newIncludedClassificationEpochs = (BigInt((await api.query.network.minIncludedClassificationEpochs()).toString()) + BigInt(1)).toString();
        const newMaxRegisteredNodes = (BigInt((await api.query.network.minMaxRegisteredNodes()).toString()) + BigInt(1)).toString();
        const newTargetNodeRegistrationsPerEpoch = (BigInt(newMaxRegisteredNodes) - BigInt(1)).toString();
        const newNodeBurnRateAlpha = (BigInt((await api.query.network.nodeBurnRateAlpha(subnetId)).toString()) - BigInt(1)).toString();
        const currentQueueImmunityEpochs = (await api.query.network.queueImmunityEpochs(subnetId)).toString();
        const newQueueImmunityEpochs = (BigInt(currentQueueImmunityEpochs) - BigInt(1)).toString();

        const wallet9 = generateRandomEthersWallet();
        const wallet10 = generateRandomEthersWallet();

        await ownerUpdateName(subnetContract, subnetId, newSubnetName)
        let subnetData = await api.query.network.subnetsData(subnetId)
        expect(subnetData != undefined);
        let subnetDataOpt = subnetData as Option<any>;
        expect(subnetDataOpt.isSome);
        if (subnetDataOpt.isSome) {
            const subnetData = subnetDataOpt.unwrap();
            const human = subnetData.toHuman();
            const subnetNameStored = human.name;
            expect(subnetNameStored).to.equal(newSubnetName)
        }

        await ownerUpdateRepo(subnetContract, subnetId, newRepo)
        subnetData = await api.query.network.subnetsData(subnetId)
        expect(subnetData != undefined);
        subnetDataOpt = subnetData as Option<any>;
        expect(subnetDataOpt.isSome);
        if (subnetDataOpt.isSome) {
            const subnetData = subnetDataOpt.unwrap();
            const human = subnetData.toHuman();
            const repoStored = human.repo;
            expect(repoStored).to.equal(newRepo)
        }

        await ownerUpdateDescription(subnetContract, subnetId, newDescription)
        subnetData = await api.query.network.subnetsData(subnetId)
        expect(subnetData != undefined);
        subnetDataOpt = subnetData as Option<any>;
        expect(subnetDataOpt.isSome);
        if (subnetDataOpt.isSome) {
            const subnetData = subnetDataOpt.unwrap();
            const human = subnetData.toHuman();
            const descriptionStored = human.description;
            expect(descriptionStored).to.equal(newDescription)
        }

        await ownerUpdateMisc(subnetContract, subnetId, newMisc)
        subnetData = await api.query.network.subnetsData(subnetId)
        expect(subnetData != undefined);
        subnetDataOpt = subnetData as Option<any>;
        expect(subnetDataOpt.isSome);
        if (subnetDataOpt.isSome) {
            const subnetData = subnetDataOpt.unwrap();
            const human = subnetData.toHuman();
            const miscStored = human.misc;
            expect(miscStored).to.equal(newMisc)
        }

        await ownerUpdateChurnLimit(subnetContract, subnetId, newChurnLimit)
        expect((await api.query.network.churnLimit(subnetId)).toString()).to.be.equal(newChurnLimit)

        await ownerUpdateRegistrationQueueEpochs(subnetContract, subnetId, newSubnetNodeQueueEpochs)
        expect((await api.query.network.subnetNodeQueueEpochs(subnetId)).toString()).to.be.equal(currentSubnetNodeQueueEpochs)
        const pendingSubnetNodeQueueEpochs = await api.query.network.pendingSubnetNodeQueueEpochs(subnetId) as Option<any>
        expect(pendingSubnetNodeQueueEpochs.isSome).to.be.true
        expect(pendingSubnetNodeQueueEpochs.unwrap().value.toString()).to.be.equal(newSubnetNodeQueueEpochs)

        await ownerUpdateIdleClassificationEpochs(subnetContract, subnetId, newIdleClassificationEpochs)
        expect((await api.query.network.idleClassificationEpochs(subnetId)).toString()).to.be.equal(newIdleClassificationEpochs)

        await ownerUpdateIncludedClassificationEpochs(subnetContract, subnetId, newIncludedClassificationEpochs)
        expect((await api.query.network.includedClassificationEpochs(subnetId)).toString()).to.be.equal(newIncludedClassificationEpochs)

        await batchTransferBalanceFromSudo(
            api,
            papiApi,
            [wallet9, wallet10].map(wallet => ({
                address: wallet.address,
                balance: sudoTransferAmount,
            })),
        );
        const addedValidatorIds: string[] = [];
        for (const wallet of [wallet9, wallet10]) {
            const validatorContract = new ethers.Contract(
                SUBNET_CONTRACT_ADDRESS,
                SUBNET_CONTRACT_ABI,
                wallet,
            );
            await registerValidator(validatorContract, generateRandomEthersWallet().address);
            const validatorIdOption = await api.query.network.coldkeyValidatorId(wallet.address) as Option<any>;
            expect(validatorIdOption.isSome).to.equal(true);
            addedValidatorIds.push(validatorIdOption.unwrap().toString());
        }
        const addValidators = [
            { validatorId: addedValidatorIds[0], count: 5 },
            { validatorId: addedValidatorIds[1], count: 3 },
        ];

        await ownerAddOrUpdateInitialValidators(subnetContract, subnetId, addValidators)
        let currentValidators = await api.query.network.nodeRegistrationInitialValidatorIds(subnetId)
        expect(currentValidators != undefined);
        let currentValidatorsOpt = currentValidators as Option<any>;
        expect(currentValidatorsOpt.isSome);
        if (currentValidatorsOpt.isSome) {
            const validatorsMap = currentValidatorsOpt.unwrap();

            // Convert to a plain object for easier comparison
            const validatorsObject = validatorsMap.toJSON();

            for (const entry of addValidators) {
                expect(validatorsObject[entry.validatorId]).to.equal(entry.count);
            }

            initialValidators.forEach(entry => {
                expect(validatorsObject[entry.validatorId]).to.equal(entry.count);
            });
        }

        const removeValidators = [addedValidatorIds[1]]

        await ownerRemoveInitialValidators(subnetContract, subnetId, removeValidators)
        currentValidators = await api.query.network.nodeRegistrationInitialValidatorIds(subnetId)
        expect(currentValidators != undefined);
        currentValidatorsOpt = currentValidators as Option<any>;
        expect(currentValidatorsOpt.isSome);
        if (currentValidatorsOpt.isSome) {
            const validatorsMap = currentValidatorsOpt.unwrap();
            const validatorsJson = validatorsMap.toJSON();

            expect(validatorsJson[addedValidatorIds[1]]).to.equal(undefined);
        }


        await ownerUpdateMinMaxStake(subnetContract, subnetId, newMinStake, newMaxStake)
        expect((await api.query.network.subnetMinStakeBalance(subnetId)).toString()).to.be.equal(newMinStake)
        expect((await api.query.network.subnetMaxStakeBalance(subnetId)).toString()).to.be.equal(newMaxStake)

        const lastSubnetDelegateStakeRewardsUpdate = Number((await api.query.network.lastSubnetDelegateStakeRewardsUpdate(subnetId)).toString());

        // Updating requires to be greater than min update period
        const subnetDelegateStakeRewardsUpdatePeriod = Number((await api.query.network.subnetDelegateStakeRewardsUpdatePeriod()).toString());

        await waitForBlocks(api, subnetDelegateStakeRewardsUpdatePeriod + 2);
        await ownerUpdateDelegateStakePercentage(subnetContract, subnetId, newDelegateStakePercentage)
        expect((await api.query.network.subnetDelegateStakeRewardsPercentage(subnetId)).toString()).to.be.equal(newDelegateStakePercentage)


        await ownerUpdateMaxRegisteredNodes(subnetContract, subnetId, newMaxRegisteredNodes)
        expect((await api.query.network.maxRegisteredNodes(subnetId)).toString()).to.be.equal(newMaxRegisteredNodes)


        await ownerUpdateTargetNodeRegistrationsPerEpoch(subnetContract, subnetId, newTargetNodeRegistrationsPerEpoch)
        expect((await api.query.network.targetNodeRegistrationsPerEpoch(subnetId)).toString()).to.be.equal(newTargetNodeRegistrationsPerEpoch)


        await ownerUpdateNodeBurnRateAlpha(subnetContract, subnetId, newNodeBurnRateAlpha)
        expect((await api.query.network.nodeBurnRateAlpha(subnetId)).toString()).to.be.equal(newNodeBurnRateAlpha)


        await ownerUpdateQueueImmunityEpochs(subnetContract, subnetId, newQueueImmunityEpochs)
        expect((await api.query.network.queueImmunityEpochs(subnetId)).toString()).to.be.equal(currentQueueImmunityEpochs)
        const pendingQueueImmunityEpochs = await api.query.network.pendingQueueImmunityEpochs(subnetId) as Option<any>
        expect(pendingQueueImmunityEpochs.isSome).to.be.true
        expect(pendingQueueImmunityEpochs.unwrap().value.toString()).to.be.equal(newQueueImmunityEpochs)


        const addBootnodes = [
            {
                peerId: (await generateRandomEd25519PeerId()),
                multiaddr: await generateRandomMultiaddr()
            }
        ]
        const removeBootnodes = [BOOTNODES[0].peerId];

        await updateBootnodes(
            subnetContract,
            subnetId,
            addBootnodes,
            removeBootnodes
        )
        const newBootnodes = await api.query.network.subnetBootnodes(subnetId)
        const bootnodesJson = newBootnodes.toJSON() as Record<string, string>;
        const storedPeerIds = new Set(
            Object.keys(bootnodesJson).map(peerId =>
                peerId.startsWith("0x") ? ethers.toUtf8String(peerId) : peerId
            ),
        );
        expect(storedPeerIds.has(BOOTNODES[0].peerId)).to.equal(false);
        addBootnodes.forEach(bootnode => {
            expect(storedPeerIds.has(bootnode.peerId)).to.equal(true);
        });

        const newAccessWallet = generateRandomEthersWallet();
        await ownerAddBootnodeAccess(
            subnetContract,
            subnetId,
            newAccessWallet.address,
        )
        const newAccess = await api.query.network.subnetBootnodeAccess(subnetId)
        const newAccessJson = newAccess.toJSON() as string[];
        const accessSet = new Set(newAccessJson.map(addr => addr.toLowerCase()));
        expect(accessSet.has(newAccessWallet.address.toLowerCase())).to.equal(true);


        // ================
        // Activate subnet before calling pause, unpause, and deactivate (required to pause and unpause)
        // ================
        // get enough delegate stake to meet min delegate stake requirement
        let minDelegateStake = await getMinSubnetDelegateStakeBalance(
            subnetContract,
            subnetId.toString()
        );

        if (Number(minDelegateStake) < 1000) {
            minDelegateStake = BigInt(1e18);
        }

        await transferBalanceFromSudo(
            api,
            papiApi,
            SUB_LOCAL_URL,
            wallet1.address,
            BigInt(minDelegateStake + BigInt(1000000)),
        )

        const delegateStakerBalance = (await papiApi.query.System.Account.getValue(wallet1.address)).data.free
        expect(Number(delegateStakerBalance)).to.be.greaterThanOrEqual(Number(minDelegateStake));

        // Delegate stake
        const stakingContract = new ethers.Contract(STAKING_CONTRACT_ADDRESS, STAKING_CONTRACT_ABI, wallet1);
        await addToDelegateStake(
            stakingContract,
            subnetId,
            minDelegateStake,
            await minimumSubnetDelegateSharesOut(
                stakingContract,
                subnetId,
                minDelegateStake,
                BigInt(100),
            )
        );

        await activateSubnet(
            subnetContract,
            subnetId,
        )

        subnetData = await api.query.network.subnetsData(subnetId)

        expect(subnetData != undefined);

        subnetDataOpt = subnetData as Option<any>;
        expect(subnetDataOpt.isSome);

        if (subnetDataOpt.isSome) {
            const subnetData = subnetDataOpt.unwrap();
            const human = subnetData.toHuman();
            expect(human.state).to.equal("Active")
        }

        const consensusEligibleFromSubnetEpoch = await getConsensusEligibleFromSubnetEpoch(
            subnetContract,
            subnetId,
        );
        await assert.rejects(() => getPauseStartedGlobalEpoch(subnetContract, subnetId));
        await assert.rejects(() => getPauseStartedSubnetEpoch(subnetContract, subnetId));

        // Pause cooldown is local to this subnet. Wait until the assigned subnet slot has
        // completed the configured number of rounds after activation.
        const subnetPauseCooldownEpochs = Number(
            (await api.query.network.subnetPauseCooldownEpochs()).toString(),
        );
        const subnetSlot = Number(assignedSlot);
        const epochLength = Number(api.consts.network.epochLength.toString());
        const firstPausableBlock = subnetSlot
            + (Number(consensusEligibleFromSubnetEpoch) + subnetPauseCooldownEpochs) * epochLength;
        const currentBlock = Number((await api.query.system.number()).toString());
        if (currentBlock < firstPausableBlock) {
            await waitForBlocks(api, firstPausableBlock - currentBlock);
        }

        await ownerPauseSubnet(subnetContract, subnetId)
        subnetData = await api.query.network.subnetsData(subnetId)
        expect(subnetData != undefined);
        subnetDataOpt = subnetData as Option<any>;
        expect(subnetDataOpt.isSome);
        if (subnetDataOpt.isSome) {
            const subnetData = subnetDataOpt.unwrap();
            const human = subnetData.toHuman();
            expect(human.state).to.equal("Paused")

            const pause = subnetData.pause.unwrap();
            expect(await getPauseStartedGlobalEpoch(subnetContract, subnetId))
                .to.equal(BigInt(pause.startedGlobalEpoch.toString()));
            expect(await getPauseStartedSubnetEpoch(subnetContract, subnetId))
                .to.equal(BigInt(pause.startedSubnetEpoch.toString()));
        }
        await assert.rejects(() => getConsensusEligibleFromSubnetEpoch(subnetContract, subnetId));

        await ownerUnpauseSubnet(subnetContract, subnetId)
        subnetData = await api.query.network.subnetsData(subnetId)
        expect(subnetData != undefined);
        subnetDataOpt = subnetData as Option<any>;
        expect(subnetDataOpt.isSome);
        if (subnetDataOpt.isSome) {
            const subnetData = subnetDataOpt.unwrap();
            const human = subnetData.toHuman();
            expect(human.state).to.equal("Active")

            expect(await getConsensusEligibleFromSubnetEpoch(subnetContract, subnetId))
                .to.equal(BigInt(subnetData.consensusEligibleFromSubnetEpoch.unwrap().toString()));
        }
        await assert.rejects(() => getPauseStartedGlobalEpoch(subnetContract, subnetId));
        await assert.rejects(() => getPauseStartedSubnetEpoch(subnetContract, subnetId));

        const newOwner = generateRandomEthersWallet();
        await transferSubnetOwnership(subnetContract, subnetId, newOwner.address)

        await transferBalanceFromSudo(
            api,
            papiApi,
            SUB_LOCAL_URL,
            newOwner.address,
            BigInt(1e18),
        )

        const newOwnerSubnetContract = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, newOwner);
        await acceptSubnetOwnership(newOwnerSubnetContract, subnetId)
        const subnetOwner = (await api.query.network.subnetOwner(subnetId)).toString()
        expect(subnetOwner).to.equal(newOwner.address)

        await ownerDeactivateSubnet(newOwnerSubnetContract, subnetId)
        subnetData = await api.query.network.subnetsData(subnetId)
        expect(subnetData == undefined);
        await assert.rejects(() => getSlotIndex(newOwnerSubnetContract, subnetId));
        await assert.rejects(() => getSubnetAtSlot(newOwnerSubnetContract, assignedSlot));

        console.log("✅ Subnet owner functions testing complete")
    })
});
