import { expect } from "chai";
import { step } from "mocha-steps";

import { ETH_BLOCK_GAS_LIMIT, GENESIS_ACCOUNT, GENESIS_ACCOUNT_PRIVATE_KEY } from "./config";
import { waitForBlock, waitForReceipt, describeWithFrontier, customRequest } from "./util";

describeWithFrontier("Frontier RPC (Block)", (context) => {
	let previousBlock;
	// Those tests are dependent of each other in the given order.
	// The reason is to avoid having to restart the node each time
	// Running them individually will result in failure

	step("should retain the genesis block while authoring", async function () {
		expect((await context.web3.eth.getBlock(0)).number).to.equal(0);
	});

	it("should return genesis block by number", async function () {
		const block = await context.web3.eth.getBlock(0);
		expect(block).to.include({
			author: "0x0000000000000000000000000000000000000000",
			difficulty: "0",
			extraData: "0x",
			gasLimit: ETH_BLOCK_GAS_LIMIT,
			gasUsed: 0,
			logsBloom:
				"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
			miner: "0x0000000000000000000000000000000000000000",
			number: 0,
			receiptsRoot: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
			size: 505,
			timestamp: 0,
			totalDifficulty: "0",
		});

		expect(block.nonce).to.eql("0x0000000000000000");
		expect(block.hash).to.be.a("string").lengthOf(66);
		expect(block.parentHash).to.be.a("string").lengthOf(66);
		expect(block.timestamp).to.be.a("number");
		previousBlock = block;
	});

	step("should have empty uncles and correct sha3Uncles", async function () {
		const block = await context.web3.eth.getBlock(0);
		expect(block.uncles).to.be.a("array").empty;
		expect(block.sha3Uncles).to.equal("0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347");
	});

	step("should have empty transactions and correct transactionRoot", async function () {
		const block = await context.web3.eth.getBlock(0);
		expect(block.transactions).to.be.a("array").empty;
		expect(block).to.include({
			transactionsRoot: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
		});
	});

	let firstBlockCreated = false;
	step("should advance and finalize after block production", async function () {
		const before = await context.web3.eth.getBlockNumber();
		const block = await waitForBlock(context.web3);
		expect(block.number).to.be.greaterThan(before);
		firstBlockCreated = true;
	});

	step("should have valid timestamp after block production", async function () {
		const block = await context.web3.eth.getBlock("latest");
		expect(Number(block.timestamp)).to.be.within(
			Math.floor(Date.now() / 1000) - 120,
			Math.ceil(Date.now() / 1000) + 30,
		);
	});

	it("genesis block should be already available by hash", async function () {
		const block = await context.web3.eth.getBlock(previousBlock.hash);
		expect(block).to.include({
			author: "0x0000000000000000000000000000000000000000",
			difficulty: "0",
			extraData: "0x",
			gasLimit: ETH_BLOCK_GAS_LIMIT,
			gasUsed: 0,
			logsBloom:
				"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
			miner: "0x0000000000000000000000000000000000000000",
			number: 0,
			receiptsRoot: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
			size: 505,
			timestamp: 0,
			totalDifficulty: "0",
		});

		expect(block.nonce).to.eql("0x0000000000000000");
		expect(block.hash).to.be.a("string").lengthOf(66);
		expect(block.parentHash).to.be.a("string").lengthOf(66);
		expect(block.timestamp).to.be.a("number");
	});

	step("retrieve block information", async function () {
		expect(firstBlockCreated).to.be.true;

		const block = await context.web3.eth.getBlock("latest");
		expect(block).to.include({
			author: "0x0000000000000000000000000000000000000000",
			difficulty: "0",
			extraData: "0x",
			gasLimit: ETH_BLOCK_GAS_LIMIT,
			gasUsed: 0,
			//hash: "0x14fe6f7c93597f79b901f8b5d7a84277a90915b8d355959b587e18de34f1dc17",
			logsBloom:
				"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
			miner: "0x0000000000000000000000000000000000000000",
			//parentHash: "0x04540257811b46d103d9896e7807040e7de5080e285841c5430d1a81588a0ce4",
			receiptsRoot: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
			totalDifficulty: "0",
			//transactions: [],
			transactionsRoot: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
			//uncles: []
		});
		previousBlock = block;

		expect(block.transactions).to.be.a("array").empty;
		expect(block.uncles).to.be.a("array").empty;
		expect(block.nonce).to.eql("0x0000000000000000");
		expect(block.hash).to.be.a("string").lengthOf(66);
		expect(block.parentHash).to.be.a("string").lengthOf(66);
		expect(block.timestamp).to.be.a("number");
	});

	step("get block by hash", async function () {
		const latest_block = await context.web3.eth.getBlock("latest");
		const block = await context.web3.eth.getBlock(latest_block.hash);
		expect(block.hash).to.be.eq(latest_block.hash);
	});

	step("get block by number", async function () {
		const block = await context.web3.eth.getBlock(1);
		expect(block).not.null;
	});

	it("should include previous block hash as parent", async function () {
		const block = await waitForBlock(context.web3);
		const parent = await context.web3.eth.getBlock(block.number - 1);
		expect(block.hash).to.not.equal(parent.hash);
		expect(block.parentHash).to.equal(parent.hash);
	});
});

describeWithFrontier("Frontier RPC (Pending Block)", (context) => {
	const TEST_ACCOUNT = "0x1111111111111111111111111111111111111111";

	it("should return pending block", async function () {
		var nonce = 0;
		let sendTransaction = async () => {
			const tx = await context.web3.eth.accounts.signTransaction(
				{
					from: GENESIS_ACCOUNT,
					to: TEST_ACCOUNT,
					value: "0x200", // Must be higher than ExistentialDeposit
					gasPrice: "0x3B9ACA00",
					gas: "0x100000",
					nonce: nonce,
				},
				GENESIS_ACCOUNT_PRIVATE_KEY,
			);
			nonce = nonce + 1;
			return (await customRequest(context.web3, "eth_sendRawTransaction", [tx.rawTransaction])).result;
		};

		const hashes: string[] = [];
		for (let i = 0; i < 5; i++) {
			hashes.push(await sendTransaction());
		}

		// test still invalid future transactions can be safely applied (they are applied, just not overlayed)
		nonce = nonce + 100;
		const futureHash = await sendTransaction();

		// Read the hypothetical pending block while BABE continues authoring.
		let pending_transactions = [];
		{
			const pending = (await customRequest(context.web3, "eth_getBlockByNumber", ["pending", false])).result;
			expect(pending.hash).to.be.null;
			expect(pending.miner).to.be.null;
			expect(pending.nonce).to.be.null;
			expect(pending.totalDifficulty).to.be.null;
			pending_transactions = pending.transactions;
			expect(pending_transactions).not.to.include(futureHash);
			expect(hashes).to.include.members(pending_transactions);
		}

		for (const hash of hashes) {
			const receipt = await waitForReceipt(context.web3, hash);
			const block = await context.web3.eth.getBlock(receipt.blockHash);
			expect(block.transactions).to.include(hash);
		}
		expect(await context.web3.eth.getTransactionReceipt(futureHash)).to.be.null;
	});
});

describeWithFrontier("Frontier RPC (BlockReceipts)", (context) => {
	const TEST_ACCOUNT = "0x1111111111111111111111111111111111111111";
	let receiptBlock: number;

	it("should return empty for a block without transactions", async function () {
		const block = await waitForBlock(context.web3);
		const result = await customRequest(context.web3, "eth_getBlockReceipts", [block.number]);
		expect(result.result).to.be.an("array").that.is.empty;
	});

	it("should return every included transaction receipt", async function () {
		const nonce = await context.web3.eth.getTransactionCount(GENESIS_ACCOUNT, "pending");
		const hashes: string[] = [];
		for (let i = 0; i < 5; i++) {
			const tx = await context.web3.eth.accounts.signTransaction(
				{
					from: GENESIS_ACCOUNT,
					to: TEST_ACCOUNT,
					value: "0x200",
					gasPrice: "0x3B9ACA00",
					gas: "0x100000",
					nonce: nonce + i,
				},
				GENESIS_ACCOUNT_PRIVATE_KEY,
			);
			hashes.push((await customRequest(context.web3, "eth_sendRawTransaction", [tx.rawTransaction])).result);
		}
		const receipts = await Promise.all(hashes.map((hash) => waitForReceipt(context.web3, hash)));
		for (const number of new Set(receipts.map((receipt) => receipt.blockNumber))) {
			const result = await customRequest(context.web3, "eth_getBlockReceipts", [number]);
			expect(result.result.map((receipt) => receipt.transactionHash)).to.have.members(
				receipts.filter((receipt) => receipt.blockNumber === number).map((receipt) => receipt.transactionHash),
			);
		}
		receiptBlock = receipts[0].blockNumber;
	});

	it("should support block number, tag and hash", async function () {
		const block = await context.web3.eth.getBlock(receiptBlock);
		const byNumber = (await customRequest(context.web3, "eth_getBlockReceipts", [receiptBlock])).result;
		const byHash = (
			await customRequest(context.web3, "eth_getBlockReceipts", [
				{ blockHash: block.hash, requireCanonical: true },
			])
		).result;
		expect(byHash).to.deep.equal(byNumber);
		expect(byNumber).not.to.be.empty;
		expect((await customRequest(context.web3, "eth_getBlockReceipts", ["earliest"])).result).to.be.empty;
		// A tag can already refer to an empty successor of our transaction block.
		for (const tag of ["finalized", "latest"]) {
			const result = (await customRequest(context.web3, "eth_getBlockReceipts", [tag])).result;
			expect(result).to.be.an("array");
			for (const receipt of result) {
				const canonical = await context.web3.eth.getBlock(Number(BigInt(receipt.blockNumber)));
				expect(receipt.blockHash).to.equal(canonical.hash);
				expect(canonical.transactions).to.include(receipt.transactionHash);
			}
		}
	});
});
