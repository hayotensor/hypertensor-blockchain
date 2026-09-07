import * as assert from "assert";
import { getDevnetApi } from "../src/substrate"
import { dev } from "@polkadot-api/descriptors"
import { PolkadotSigner, TypedApi } from "polkadot-api";
import { ethers } from "ethers"
import { generateRandomEd25519PeerId, generateRandomEthersWallet, generateRandomMultiaddr, generateRandomString, getPublicClient, SUBNET_CONTRACT_ABI, SUBNET_CONTRACT_ADDRESS } from "../src/utils"
import {
    getCurrentRegistrationCost,
    registerSubnet,
    registerSubnetNode,
    registerValidator,
    transferBalanceFromSudo
} from "../src/network"
import { ETH_LOCAL_URL, SUB_LOCAL_URL } from "../src/config";
import { PublicClient } from "viem";
import { ApiPromise, WsProvider } from "@polkadot/api";
import { expect } from "chai";
import { Option } from '@polkadot/types';

// Status: passing
// npm test -- -g "test subnet node entry functions-0xbull3948t92d398"
describe("test subnet node entry functions-0xbull3948t92d398", () => {
    // init eth part
    const wallet0 = generateRandomEthersWallet();
    // subnet registration hotkey
    const wallet1 = generateRandomEthersWallet();
    // node 1 coldkey
    const wallet2 = generateRandomEthersWallet();
    // node 1 hotkey
    const wallet3 = generateRandomEthersWallet();
    // node 2 coldkey
    const wallet4 = generateRandomEthersWallet();
    // node 2 hotkey
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
    const validatorColdkeys = [wallet1, wallet2, wallet3, wallet4];
    const validatorHotkeys = validatorColdkeys.map(() => generateRandomEthersWallet());
    const validatorIds = new Map<string, string>();
    let initialValidators: Array<{ validatorId: string; count: number }>;

    let publicClient: PublicClient;
    let papiApi: TypedApi<typeof dev>
    let api: ApiPromise

    const sudoTransferAmount = BigInt(10000e18)
    let minStakeAmount: string;

    const subnetContract = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet0);

    const subnetContract2 = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet2);
    const subnetContract4 = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet4);

    const subnetName = generateRandomString(30)
    const repo = generateRandomString(30)
    const description = generateRandomString(30)
    const misc = generateRandomString(30)
    let subnetId: string;
    let peer_info_1: { peerId: string; multiaddr: Uint8Array };
    let peer_info_2: { peerId: string; multiaddr: Uint8Array };
    let peer_info_3: { peerId: string; multiaddr: Uint8Array };
    const delegateRewardRate = "0";

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

        // balance for subnet registerer
        await transferBalanceFromSudo(
            api,
            papiApi,
            SUB_LOCAL_URL,
            wallet0.address,
            sudoTransferAmount,
        )

        await transferBalanceFromSudo(
            api,
            papiApi,
            SUB_LOCAL_URL,
            wallet1.address,
            sudoTransferAmount,
        )

        await transferBalanceFromSudo(
            api,
            papiApi,
            SUB_LOCAL_URL,
            wallet2.address,
            sudoTransferAmount,
        )

        await transferBalanceFromSudo(
            api,
            papiApi,
            SUB_LOCAL_URL,
            wallet3.address,
            sudoTransferAmount,
        )

        await transferBalanceFromSudo(
            api,
            papiApi,
            SUB_LOCAL_URL,
            wallet4.address,
            sudoTransferAmount,
        )

        for (let index = 0; index < validatorColdkeys.length; index++) {
            const coldkey = validatorColdkeys[index];
            const validatorContract = new ethers.Contract(
                SUBNET_CONTRACT_ADDRESS,
                SUBNET_CONTRACT_ABI,
                coldkey,
            );
            await registerValidator(validatorContract, validatorHotkeys[index].address);
            const validatorIdOption = await api.query.network.coldkeyValidatorId(coldkey.address) as Option<any>;
            expect(validatorIdOption.isSome).to.equal(true);
            validatorIds.set(coldkey.address, validatorIdOption.unwrap().toString());
        }
        initialValidators = [...validatorIds.values()].map(validatorId => ({
            validatorId,
            count: 1,
        }));

        // ==============
        // Register subnet
        // ==============
        const cost = await getCurrentRegistrationCost(subnetContract, api)
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

        let peer1 = await generateRandomEd25519PeerId()
        peer_info_1 = {
            peerId: peer1,
            multiaddr: await generateRandomMultiaddr(peer1)
        }
        peer_info_2 = {
            peerId: "",
            multiaddr: new Uint8Array()
        }
        peer_info_3 = {
            peerId: "",
            multiaddr: new Uint8Array()
        }

        minStakeAmount = (await api.query.network.minSubnetMinStake()).toString();
    })

    // Status: passing
    // npm test -- -g "testing register subnet node-0x04209fwwWERV3"
    it("testing register subnet node-0x04209fwwWERV3", async () => {
        const unique = generateRandomString(16)
        const nonUnique = generateRandomString(16)

        await registerSubnetNode(
            subnetContract2,
            validatorIds.get(wallet2.address)!,
            subnetId,
            wallet3.address,
            peer_info_1,
            peer_info_2,
            peer_info_3,
            BigInt(minStakeAmount),
            unique,
            nonUnique,
            "1000000000000000000"
        )
        const subnetNodeId = (await api.query.network.totalSubnetNodeUids(subnetId)).toString();
        expect(Number(subnetNodeId)).to.be.greaterThan(0);
        console.log("subnetNodeId", subnetNodeId)

        const subnetNodeDataHuman = (await api.query.network.subnetNodesData(subnetId, subnetNodeId)).toHuman() as any;
        console.log("subnetNodeDataHuman", subnetNodeDataHuman)
        expect(subnetNodeDataHuman.validatorId.toString()).to.equal(validatorIds.get(wallet2.address));
        expect(unique).to.be.equal(subnetNodeDataHuman.unique);
        expect(nonUnique).to.be.equal(subnetNodeDataHuman.nonUnique);
        expect("Validator").to.be.equal(subnetNodeDataHuman.classification.nodeClass);
        expect((await api.query.network.subnetNodeIdHotkey(subnetId, subnetNodeId)).toString()).to.equal(wallet3.address);
        const validatorData = (await api.query.network.validatorsData(validatorIds.get(wallet2.address)!)).toHuman() as any;
        expect(validatorData.delegateRewardRate).to.equal(delegateRewardRate);

        const nodeStake = await api.query.network.nodeSubnetStake(subnetNodeId, subnetId);
        expect(BigInt(nodeStake.toString())).to.be.equal(BigInt(minStakeAmount));

        console.log("✅ Subnet node registration testing complete")
    })

    // Status: passing
    // npm test -- -g "testing register second subnet node-0xf56GRTy2"
    it("testing register second subnet node-0xf56GRTy2", async () => {
        const unique = generateRandomString(16)
        const nonUnique = generateRandomString(16)

        await registerSubnetNode(
            subnetContract4,
            validatorIds.get(wallet4.address)!,
            subnetId,
            wallet5.address,
            peer_info_1,
            peer_info_2,
            peer_info_3,
            BigInt(minStakeAmount),
            unique,
            nonUnique,
            "1000000000000000000"
        )

        const subnetNodeId = (await api.query.network.totalSubnetNodeUids(subnetId)).toString();
        console.log("subnetNodeId", subnetNodeId)
        expect(Number(subnetNodeId)).to.be.greaterThan(0);
        expect((await api.query.network.subnetNodeValidatorId(subnetId, subnetNodeId)).toString()).to.equal(validatorIds.get(wallet4.address));

        console.log("✅ Second subnet node registration testing complete")
    })

});
