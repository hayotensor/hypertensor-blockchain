import * as assert from "assert";
import { getDevnetApi } from "../src/substrate"
import { dev } from "@polkadot-api/descriptors"
import { PolkadotSigner, TypedApi } from "polkadot-api";
import { ethers } from "ethers"
import { generateRandomEd25519PeerId, generateRandomEthersWallet, generateRandomMultiaddr, generateRandomString, getPublicClient, SUBNET_CONTRACT_ABI, SUBNET_CONTRACT_ADDRESS } from "../src/utils"
import {
    batchTransferBalanceFromSudo,
    getCurrentRegistrationCost,
    registerSubnet,
    registerSubnetNode,
    registerValidator,
    updateValidatorIdentity,
} from "../src/network"
import { ETH_LOCAL_URL, SUB_LOCAL_URL } from "../src/config";
import { PublicClient } from "viem";
import { ApiPromise, WsProvider } from "@polkadot/api";
import { expect } from "chai";
import { Option } from '@polkadot/types';

// npm test -- -g "test node update parameters-0xdgahRTH"
describe("test identities-0xDANBre34", () => {
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
    const validatorColdkeys = [wallet1, wallet2, wallet3, wallet4, wallet5, wallet6, wallet7, wallet8];
    const validatorHotkeys = validatorColdkeys.map(() => generateRandomEthersWallet());
    const subnetNodeHotkey = generateRandomEthersWallet();

    let publicClient: PublicClient;
    // init substrate part
    let papiApi: TypedApi<typeof dev>
    let api: ApiPromise

    const sudoTransferAmount = BigInt(10000e18)
    const subnetContract = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet0);
    const subnetContract1 = new ethers.Contract(SUBNET_CONTRACT_ADDRESS, SUBNET_CONTRACT_ABI, wallet1);

    let subnetId: string;
    let subnetNodeId1: string;
    let validatorId: string;
    let initialValidators: Array<{ validatorId: string; count: number }>;

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

        initialValidators = [];
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
            const registeredValidatorId = validatorIdOption.unwrap().toString();
            initialValidators.push({ validatorId: registeredValidatorId, count: 1 });
            if (coldkey.address === wallet1.address) {
                validatorId = registeredValidatorId;
            }
        }

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

        const unique = generateRandomString(16)
        const nonUnique = generateRandomString(16)

        await registerSubnetNode(
            subnetContract1,
            validatorId,
            subnetId,
            subnetNodeHotkey.address,
            peer_info_1,
            peer_info_2,
            peer_info_3,
            BigInt(minStake.toString()),
            unique,
            nonUnique,
            "1000000000000000000"
        )

        subnetNodeId1 = (await api.query.network.totalSubnetNodeUids(subnetId)).toString();
        expect(Number(subnetNodeId1)).to.be.greaterThan(0);
        expect((await api.query.network.subnetNodeValidatorId(subnetId, subnetNodeId1)).toString()).to.equal(validatorId);
    })

    // Status: passing
    // npm test -- -g "testing register and remove identity-0xA4hdrg"
    it("testing register and remove identity-0xA4hdrg", async () => {
        const newName = generateRandomString(16)
        const newUrl = generateRandomString(16)
        const newImage = generateRandomString(16)
        const newDiscord = generateRandomString(16)
        const newX = generateRandomString(16)
        const newTelegram = generateRandomString(16)
        const newGithub = generateRandomString(16)
        const newHuggingFace = generateRandomString(16)
        const newDescription = generateRandomString(16)
        const newMisc = generateRandomString(16)

        await updateValidatorIdentity(
            subnetContract1,
            validatorId,
            true,
            newName,
            newUrl,
            newImage,
            newDiscord,
            newX,
            newTelegram,
            newGithub,
            newHuggingFace,
            newDescription,
            newMisc,
        )

        let validatorData = (await api.query.network.validatorsData(validatorId)).toHuman() as any;
        expect(validatorData.identity.name).to.equal(newName);
        expect(validatorData.identity.url).to.equal(newUrl);
        expect(validatorData.identity.image).to.equal(newImage);
        expect(validatorData.identity.discord).to.equal(newDiscord);
        expect(validatorData.identity.x).to.equal(newX);
        expect(validatorData.identity.telegram).to.equal(newTelegram);
        expect(validatorData.identity.github).to.equal(newGithub);
        expect(validatorData.identity.huggingFace).to.equal(newHuggingFace);
        expect(validatorData.identity.description).to.equal(newDescription);
        expect(validatorData.identity.misc).to.equal(newMisc);

        await updateValidatorIdentity(
            subnetContract1,
            validatorId,
            false,
            "", "", "", "", "", "", "", "", "", "",
        );
        validatorData = (await api.query.network.validatorsData(validatorId)).toHuman() as any;
        expect(validatorData.identity).to.equal(null);

        console.log("✅ Registering identity testing complete")
    })
});
