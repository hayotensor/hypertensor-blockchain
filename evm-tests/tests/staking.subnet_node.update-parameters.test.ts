import * as assert from "assert";
import { getDevnetApi } from "../src/substrate"
import { dev } from "@polkadot-api/descriptors"
import { PolkadotSigner, TypedApi } from "polkadot-api";
import { ethers } from "ethers"
import { generateRandomEd25519PeerId, generateRandomEthersWallet, generateRandomMultiaddr, generateRandomString, getPublicClient, STAKING_CONTRACT_ABI, STAKING_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, SUBNET_CONTRACT_ADDRESS } from "../src/utils"
import {
    batchTransferBalanceFromSudo,
    getCurrentRegistrationCost,
    registerSubnet,
    registerSubnetNode,
    updateNodeBootnodePeerInfo,
    updateNodeClientPeerInfo,
    updateNodeHotkey,
    updateNodePeerInfo,
    updateNodeUnique,
    updateNonUnique,
    updateValidatorColdkey,
    updateValidatorDelegateRewardRate,
    updateValidatorHotkey,
} from "../src/network"
import { ETH_LOCAL_URL, SUB_LOCAL_URL } from "../src/config";
import { PublicClient } from "viem";
import { ApiPromise, WsProvider } from "@polkadot/api";
import { expect } from "chai";
import { Option } from '@polkadot/types';
import { registerCanonicalValidators } from "../src/validator-fixtures";

// npm test -- -g "test node update parameters-0xdgahRTH"
describe("test node update parameters-0xdgahRTH", () => {
    // init eth part
    const wallet0 = generateRandomEthersWallet();
    const wallet1 = generateRandomEthersWallet();
    const wallet2 = generateRandomEthersWallet();
    const wallet3 = generateRandomEthersWallet();
    const wallet4 = generateRandomEthersWallet();
    const wallet5 = generateRandomEthersWallet();
    const wallet6 = generateRandomEthersWallet();
    const wallet7 = generateRandomEthersWallet();
    const wallet8 = generateRandomEthersWallet();

    const ALL_ACCOUNTS = [
        wallet0.address,
        wallet1.address,
        wallet2.address,
        wallet3.address,
        wallet4.address,
        wallet5.address,
        wallet6.address,
        wallet7.address,
        wallet8.address,
    ]
    const validatorColdkeys = [wallet1, wallet2, wallet3];

    let publicClient: PublicClient;
    // init substrate part

    let papiApi: TypedApi<typeof dev>
    let api: ApiPromise

    const sudoTransferAmount = BigInt(10000e18)
    const stakeAmount = BigInt(100e18)

    const subnetContract = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet0);
    const subnetContract1 = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet1);

    let subnetId: string;
    let subnetNodeId1: string;
    let validatorId: string;

    // sudo account alice as signer
    let alice: PolkadotSigner;
    before(async () => {
        let BOOTNODES: { peerId: string; multiaddr: Uint8Array }[] = [
            {
                peerId: (await generateRandomEd25519PeerId()),
                multiaddr: await generateRandomMultiaddr((await generateRandomEd25519PeerId()))
            }
        ]
        publicClient = await getPublicClient(ETH_LOCAL_URL)
        // init variables got from await and async
        papiApi = await getDevnetApi()

        const provider = new WsProvider(SUB_LOCAL_URL);

        api = await ApiPromise.create({ provider });

        const recipients = ALL_ACCOUNTS.map(address => ({
            address: address,
            balance: BigInt(sudoTransferAmount + BigInt(500))
        }));

        await batchTransferBalanceFromSudo(
            api,
            papiApi,
            recipients
        )

        const initialValidators = await registerCanonicalValidators(
            subnetContract,
            validatorColdkeys,
            api,
        );

        // ==============
        // Register subnet
        // ==============
        let cost = await getCurrentRegistrationCost(subnetContract, api)
        const subnetName = generateRandomString(30)
        const repo = generateRandomString(30)
        const description = generateRandomString(30)
        const misc = generateRandomString(30)
        const minStake = await api.query.network.minSubnetMinStake();
        const maxStake = await api.query.network.networkMaxStakeBalance();
        const delegateStakePercentage = await api.query.network.minDelegateStakePercentage();

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

        subnetId = await subnetContract.getSubnetId(subnetName);

        // ================
        // Add subnet nodes
        // ================

        // ================
        // Subnet node 1
        // ================
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

        validatorId = initialValidators[0].validatorId;

        const unique = generateRandomString(16)
        const nonUnique = generateRandomString(16)

        await registerSubnetNode(
            subnetContract1,
            validatorId,
            subnetId,
            wallet4.address,
            peer_info_1,
            peer_info_2,
            peer_info_3,
            BigInt(minStake.toString()),
            unique,
            nonUnique,
            "1000000000000000000"
        )

        subnetNodeId1 = (
            await api.query.network.totalSubnetNodeUids(subnetId)
        ).toString();
        expect(Number(subnetNodeId1)).to.be.greaterThan(0);
    })

    // Status: passing
    // npm test -- -g "testing update node parameters-0xpdgaa663uF"
    it("testing update node parameters-0xpdgaa663uF", async () => {
        const newDelegateRewardRate = "1"
        const currentBlock = Number((await api.query.system.number()).toString());
        const rewardRateUpdatePeriod = Number((await api.query.network.nodeRewardRateUpdatePeriod()).toString());
        if (currentBlock >= rewardRateUpdatePeriod) {
            await updateValidatorDelegateRewardRate(
                subnetContract1,
                validatorId,
                newDelegateRewardRate
            )
            const validatorData = (await api.query.network.validatorsData(validatorId)).toHuman() as any;
            expect(Number(validatorData.delegateRewardRate)).to.equal(Number(newDelegateRewardRate));
        } else {
            await assert.rejects(() => updateValidatorDelegateRewardRate(
                subnetContract1,
                validatorId,
                newDelegateRewardRate
            ));
        }

        const newUnique = generateRandomString(16)
        await updateNodeUnique(
            subnetContract1,
            subnetId,
            subnetNodeId1,
            newUnique
        )
        let nodeData = (await api.query.network.subnetNodesData(subnetId, subnetNodeId1)).toHuman() as any;
        expect(nodeData.unique).to.equal(newUnique);

        const newNonUnique = generateRandomString(16)
        await updateNonUnique(
            subnetContract1,
            subnetId,
            subnetNodeId1,
            newNonUnique
        )
        nodeData = (await api.query.network.subnetNodesData(subnetId, subnetNodeId1)).toHuman() as any;
        expect(nodeData.nonUnique).to.equal(newNonUnique);

        let newPeerId = await generateRandomEd25519PeerId()
        const newPeerMultiaddr = await generateRandomMultiaddr(newPeerId)
        await updateNodePeerInfo(
            subnetContract1,
            subnetId,
            subnetNodeId1,
            {
                peerId: newPeerId,
                multiaddr: newPeerMultiaddr
            }
        )
        nodeData = (await api.query.network.subnetNodesData(subnetId, subnetNodeId1)).toHuman() as any;
        expect(nodeData.peerInfo.peerId).to.equal(newPeerId);

        newPeerId = await generateRandomEd25519PeerId()
        const newBootnodeMultiaddr = await generateRandomMultiaddr(newPeerId)
        await updateNodeBootnodePeerInfo(
            subnetContract1,
            subnetId,
            subnetNodeId1,
            {
                peerId: newPeerId,
                multiaddr: newBootnodeMultiaddr
            }
        )
        nodeData = (await api.query.network.subnetNodesData(subnetId, subnetNodeId1)).toHuman() as any;
        expect(nodeData.bootnodePeerInfo.peerId).to.equal(newPeerId);

        newPeerId = await generateRandomEd25519PeerId()
        const newClientMultiaddr = await generateRandomMultiaddr(newPeerId)
        await updateNodeClientPeerInfo(
            subnetContract1,
            subnetId,
            subnetNodeId1,
            {
                peerId: newPeerId,
                multiaddr: newClientMultiaddr
            }
        )
        nodeData = (await api.query.network.subnetNodesData(subnetId, subnetNodeId1)).toHuman() as any;
        expect(nodeData.clientPeerInfo.peerId).to.equal(newPeerId);

        const newNodeHotkey = generateRandomEthersWallet();
        await updateNodeHotkey(
            subnetContract1,
            subnetId,
            subnetNodeId1,
            newNodeHotkey.address,
        )
        expect((await api.query.network.subnetNodeIdHotkey(subnetId, subnetNodeId1)).toString()).to.equal(newNodeHotkey.address);

        const newValidatorHotkey = generateRandomEthersWallet();
        await updateValidatorHotkey(
            subnetContract1,
            validatorId,
            newValidatorHotkey.address,
        )
        expect((await api.query.network.validatorIdHotkey(validatorId)).toString()).to.equal(newValidatorHotkey.address);

        const newColdkey = generateRandomEthersWallet();
        await updateValidatorColdkey(
            subnetContract1,
            validatorId,
            newColdkey.address,
        )
        expect((await api.query.network.validatorColdkey(validatorId)).toString()).to.equal(newColdkey.address);

        console.log("✅ Updating node parameters testing complete")
    })
});
