import { expect } from "chai";
import { AbiItem } from "web3-utils";

import Test from "../build/contracts/Test.json";
import {
	GENESIS_ACCOUNT,
	GENESIS_ACCOUNT_PRIVATE_KEY,
	FIRST_CONTRACT_ADDRESS,
	BLOCK_HASH_COUNT,
	ETH_BLOCK_GAS_LIMIT,
} from "./config";
import { waitForReceipt, waitForBlock, customRequest, describeWithFrontier } from "./util";

describeWithFrontier("Frontier RPC (Contract Methods)", (context) => {
	const TEST_CONTRACT_BYTECODE = Test.bytecode;
	const TEST_CONTRACT_ABI = Test.abi as AbiItem[];
	let deploymentHash: string;

	// Those test are ordered. In general this should be avoided, but due to the time it takes
	// to spin up a frontier node, it saves a lot of time.

	before("create the contract", async function () {
		this.timeout(180000);
		const tx = await context.web3.eth.accounts.signTransaction(
			{
				from: GENESIS_ACCOUNT,
				data: TEST_CONTRACT_BYTECODE,
				value: "0x00",
				gasPrice: "0x3B9ACA00",
				gas: "0x100000",
			},
			GENESIS_ACCOUNT_PRIVATE_KEY
		);
		deploymentHash = (await customRequest(context.web3, "eth_sendRawTransaction", [tx.rawTransaction])).result;
		await waitForReceipt(context.web3, deploymentHash);
	});

	it("get transaction by hash", async () => {
		const txHash = deploymentHash;
		const tx = await context.web3.eth.getTransaction(txHash);
		expect(tx.hash).to.equal(txHash);
	});

	it("should return contract method result", async function () {
		const contract = new context.web3.eth.Contract(TEST_CONTRACT_ABI, FIRST_CONTRACT_ADDRESS, {
			from: GENESIS_ACCOUNT,
			gasPrice: "0x3B9ACA00",
		});

		expect(await contract.methods.multiply(3).call()).to.equal("21");
	});
	it("should get correct environmental block number", async function () {
		// Solidity `block.number` is expected to return the same height at which the runtime call was made.
		const contract = new context.web3.eth.Contract(TEST_CONTRACT_ABI, FIRST_CONTRACT_ADDRESS, {
			from: GENESIS_ACCOUNT,
			gasPrice: "0x3B9ACA00",
		});
		let block = await context.web3.eth.getBlock("latest");
		expect(await contract.methods.currentBlock().call({}, block.number)).to.eq(block.number.toString());
		await waitForBlock(context.web3);
		block = await context.web3.eth.getBlock("latest");
		expect(await contract.methods.currentBlock().call({}, block.number)).to.eq(block.number.toString());
	});

	it("should get correct environmental block hash", async function () {
        // Expiry requires 256 real slots; allow for missed slots and finality.
        this.timeout((BLOCK_HASH_COUNT + 10) * 12_000);
        const contract = new context.web3.eth.Contract(TEST_CONTRACT_ABI, FIRST_CONTRACT_ADDRESS, {
            from: GENESIS_ACCOUNT, gasPrice: "0x3B9ACA00",
        });
        const first = await context.web3.eth.getBlock("latest");
        let current = first;
        while (current.number <= first.number + BLOCK_HASH_COUNT) {
            current = await waitForBlock(context.web3);
            const parent = await context.web3.eth.getBlock(current.number - 1);
            expect(await contract.methods.blockHash(parent.number).call({}, current.number)).to.eq(parent.hash);
        }
        expect(await contract.methods.blockHash(first.number).call({}, current.number)).to.eq(
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        );
	});

	it("should get correct environmental block gaslimit", async function () {
		const contract = new context.web3.eth.Contract(TEST_CONTRACT_ABI, FIRST_CONTRACT_ADDRESS, {
			from: GENESIS_ACCOUNT,
			gasPrice: "0x3B9ACA00",
		});
		expect(await contract.methods.gasLimit().call()).to.eq(ETH_BLOCK_GAS_LIMIT.toString());
	});

	// Requires error handling
	it.skip("should fail for missing parameters", async function () {
		const contract = new context.web3.eth.Contract(
			[{ ...TEST_CONTRACT_ABI[0], inputs: [] }],
			FIRST_CONTRACT_ADDRESS,
			{
				from: GENESIS_ACCOUNT,
				gasPrice: "0x3B9ACA00",
			}
		);
		await contract.methods
			.multiply()
			.call()
			.catch((err) =>
				expect(err.message).to.equal(`Returned error: VM Exception while processing transaction: revert.`)
			);
	});

	// Requires error handling
	it.skip("should fail for too many parameters", async function () {
		const contract = new context.web3.eth.Contract(
			[
				{
					...TEST_CONTRACT_ABI[0],
					inputs: [
						{ internalType: "uint256", name: "a", type: "uint256" },
						{ internalType: "uint256", name: "b", type: "uint256" },
					],
				},
			],
			FIRST_CONTRACT_ADDRESS,
			{
				from: GENESIS_ACCOUNT,
				gasPrice: "0x3B9ACA00",
			}
		);
		await contract.methods
			.multiply(3, 4)
			.call()
			.catch((err) =>
				expect(err.message).to.equal(`Returned error: VM Exception while processing transaction: revert.`)
			);
	});

	// Requires error handling
	it.skip("should fail for invalid parameters", async function () {
		const contract = new context.web3.eth.Contract(
			[
				{
					...TEST_CONTRACT_ABI[0],
					inputs: [{ internalType: "address", name: "a", type: "address" }],
				},
			],
			FIRST_CONTRACT_ADDRESS,
			{ from: GENESIS_ACCOUNT, gasPrice: "0x3B9ACA00" }
		);
		await contract.methods
			.multiply("0x0123456789012345678901234567890123456789")
			.call()
			.catch((err) =>
				expect(err.message).to.equal(`Returned error: VM Exception while processing transaction: revert.`)
			);
	});
});
