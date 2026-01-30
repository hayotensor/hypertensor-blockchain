import * as assert from "assert";
import { getDevnetApi } from "../src/substrate"
import { dev } from "@polkadot-api/descriptors"
import { PolkadotSigner, TypedApi } from "polkadot-api";
import { ethers } from "ethers"
import { generateRandomEd25519PeerId, generateRandomEthersWallet, generateRandomMultiaddr, generateRandomString, getPublicClient, STAKING_CONTRACT_ABI, STAKING_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, SUBNET_CONTRACT_ADDRESS } from "../src/utils"
import {
    addToStake,
    batchTransferBalanceFromSudo,
    getCurrentRegistrationCost,
    registerSubnet,
    registerSubnetNode,
    removeStake,
    waitForFinalizedBalance
} from "../src/network"
import { ETH_LOCAL_URL, SUB_LOCAL_URL } from "../src/config";
import { PublicClient } from "viem";
import { ApiPromise, WsProvider } from "@polkadot/api";
import { expect } from "chai";
import { Option } from '@polkadot/types';

// npm test -- -g "test node staking-0x65683fx2"
describe("test node staking-0x65683fx2", () => {
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
    const initialColdkeys = [
        {
            coldkey: wallet1.address,
            count: 1
        },
        {
            coldkey: wallet2.address,
            count: 1
        },
        {
            coldkey: wallet3.address,
            count: 1
        },
        {
            coldkey: wallet4.address,
            count: 1
        },
        {
            coldkey: wallet5.address,
            count: 1
        },
        {
            coldkey: wallet6.address,
            count: 1
        },
        {
            coldkey: wallet7.address,
            count: 1
        },
        {
            coldkey: wallet8.address,
            count: 1
        },
    ];

    let publicClient: PublicClient;
    // init substrate part

    let papiApi: TypedApi<typeof dev>
    let api: ApiPromise

    const sudoTransferAmount = BigInt(10000e18)
    const stakeAmount = BigInt(100e18)

    const subnetContract = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet0);
    const subnetContract1 = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet1);
    const subnetContract2 = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet2);

    const stakingContract1 = new ethers.Contract(STAKING_CONTRACT_ADDRESS, STAKING_CONTRACT_ABI, wallet1);
    const stakingContract2 = new ethers.Contract(STAKING_CONTRACT_ADDRESS, STAKING_CONTRACT_ABI, wallet2);

    let subnetId: string;
    let subnetNodeId1: string;
    let subnetNodeId2: string;

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
            initialColdkeys,
            BOOTNODES,
            cost,
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

        let delegateAccount1 = {
            accountId: wallet1.address,
            rate: BigInt(0)
        }

        const delegateRewardRate = "0";

        const unique = generateRandomString(16)
        const nonUnique = generateRandomString(16)

        await registerSubnetNode(
            subnetContract1,
            subnetId,
            wallet4.address,
            peer_info_1,
            peer_info_2,
            peer_info_3,
            delegateRewardRate,
            BigInt(minStake.toString()),
            unique,
            nonUnique,
            delegateAccount1,
            "1000000000000000000"
        )

        let subnetNodeId1Fetched = await api.query.network.hotkeySubnetNodeId(subnetId, wallet4.address);

        const subnetNodeId1Opt = subnetNodeId1Fetched as Option<any>;
        expect(subnetNodeId1Opt.isSome);

        let subnetNode1Exists: boolean = false;
        if (subnetNodeId1Opt.isSome) {
            subnetNode1Exists = true;
            const subnetNodeId2Unwrapped = subnetNodeId1Opt.unwrap();
            const human = subnetNodeId2Unwrapped.toHuman();
            subnetNodeId1 = human?.toString();
            expect(Number(subnetNodeId1)).to.be.greaterThan(0);
        }
        expect(subnetNode1Exists);

        // ================
        // Subnet node 2
        // ================
        let peer4 = await generateRandomEd25519PeerId()
        let peer_info_4 = {
            peerId: peer4,
            multiaddr: await generateRandomMultiaddr(peer4)
        }
        let peer_info_5 = {
            peerId: "",
            multiaddr: new Uint8Array()
        }
        let peer_info_6 = {
            peerId: "",
            multiaddr: new Uint8Array()
        }

        let delegateAccount2 = {
            accountId: wallet1.address,
            rate: BigInt(0)
        }


        const unique2 = generateRandomString(16)

        await registerSubnetNode(
            subnetContract2,
            subnetId,
            wallet5.address,
            peer_info_4,
            peer_info_5,
            peer_info_6,
            delegateRewardRate,
            BigInt(minStake.toString()),
            unique2,
            nonUnique,
            delegateAccount2,
            "1000000000000000000"
        )

        let subnetNodeId2Fetched = await api.query.network.hotkeySubnetNodeId(subnetId, wallet5.address);

        const subnetNodeId2Opt = subnetNodeId2Fetched as Option<any>;
        expect(subnetNodeId2Opt.isSome);

        let subnetNode2Exists: boolean = false;
        if (subnetNodeId2Opt.isSome) {
            subnetNode2Exists = true;
            const subnetNodeId2Unwrapped = subnetNodeId2Opt.unwrap();
            const human = subnetNodeId2Unwrapped.toHuman();
            subnetNodeId2 = human?.toString();
            expect(Number(subnetNodeId2)).to.be.greaterThan(0);
        }
        expect(subnetNode2Exists);
    })

    // Status: passing
    // npm test -- -g "testing add subnet node stake-0xpqlaz0185"
    it("testing add subnet node stake-0xpqlaz0185", async () => {
        let accountSubnetStakePre = await api.query.network.accountSubnetStake(wallet1.address, subnetId);

        const beforeFinalizedBalance = (await papiApi.query.System.Account.getValue(wallet1.address)).data.free

        await addToStake(
            stakingContract1,
            subnetId,
            subnetNodeId1,
            wallet4.address,
            stakeAmount
        )

        let accountSubnetStakePost = await api.query.network.accountSubnetStake(wallet4.address, subnetId);

        expect(Number(accountSubnetStakePre.toString())).to.be.lessThan(Number(accountSubnetStakePost.toString()));

        const afterFinalizedBalance = await waitForFinalizedBalance(
            papiApi,
            wallet1.address,
            beforeFinalizedBalance
        );

        expect(Number(afterFinalizedBalance.toString())).to.be.lessThan(Number(beforeFinalizedBalance.toString()))

        console.log("✅ Add stake testing complete")
    })

    // Status: passing
    // npm test -- -g "testing remove subnet node stake-0xnvhgyt926v"
    it("testing remove subnet node stake-0xnvhgyt926v", async () => {
        let accountSubnetStakePre = await api.query.network.accountSubnetStake(wallet5.address, subnetId);

        console.log("adding stake")
        // =========
        // Add stake
        // =========
        await addToStake(
            stakingContract2,
            subnetId,
            subnetNodeId2,
            wallet5.address,
            stakeAmount
        )

        let accountSubnetStakePost = await api.query.network.accountSubnetStake(wallet5.address, subnetId);
        expect(Number(accountSubnetStakePre.toString())).to.be.lessThan(Number(accountSubnetStakePost.toString()))

        // =========
        // Remove stake
        // =========
        await removeStake(
            stakingContract2,
            subnetId,
            wallet5.address,
            stakeAmount
        )

        let accountSubnetStakeAfterRemoval = await api.query.network.accountSubnetStake(wallet5.address, subnetId);
        expect(Number(accountSubnetStakePre.toString())).to.be.equal(Number(accountSubnetStakeAfterRemoval.toString()));

        const unbondings = (await api.query.network.stakeUnbondingLedger(wallet2.address)).toHuman();
        console.log("unbondings", unbondings)
        const unbondingBalance = Object.values(unbondings!)[0];
        const unbondingBalanceWithoutCommas = unbondingBalance.replace(/,/g, "");
        expect(Number(unbondingBalanceWithoutCommas.toString())).to.be.equal(Number(stakeAmount.toString()));

        console.log("✅ Remove stake testing complete")
    })
});