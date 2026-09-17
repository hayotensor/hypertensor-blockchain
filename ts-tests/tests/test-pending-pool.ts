import { expect } from "chai";

import { GENESIS_ACCOUNT, GENESIS_ACCOUNT_PRIVATE_KEY } from "./config";
import { waitForReceipt, waitForBlock, customRequest, describeWithFrontierAllPools } from "./util";

describeWithFrontierAllPools("Frontier RPC (Pending Pool)", (context) => {
	// Solidity: contract test { function multiply(uint a) public pure returns(uint d) {return a * 7;}}
	const TEST_CONTRACT_BYTECODE =
		"0x6080604052348015600f57600080fd5b5060ae8061001e6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063c6888fa114602d575b600080fd5b605660048036036020811015604157600080fd5b8101908080359060200190929190505050606c565b6040518082815260200191505060405180910390f35b600060078202905091905056fea265627a7a72315820f06085b229f27f9ad48b2ff3dd9714350c1698a37853a30136fa6c5a7762af7364736f6c63430005110032";

	// Allow the fork-aware pool to observe the newly finalized block
	// Before the first block is created, the pool will wrongly report as empty
	// https://github.com/paritytech/polkadot-sdk/issues/8402
	before("create and finalize block 1", async function () {
		await waitForBlock(context.web3);
	});

	it("should return a pending transaction", async function () {
		this.timeout(180000);
		const tx = await context.web3.eth.accounts.signTransaction(
			{
				from: GENESIS_ACCOUNT,
				data: TEST_CONTRACT_BYTECODE,
				value: "0x00",
				gasPrice: "0x3B9ACA00",
				gas: "0x100000",
				nonce: 1, // A missing nonce zero keeps this transaction pending.
			},
			GENESIS_ACCOUNT_PRIVATE_KEY,
		);

		const txHash = (await customRequest(context.web3, "eth_sendRawTransaction", [tx.rawTransaction])).result;

		const pendingTransaction = (await customRequest(context.web3, "eth_getTransactionByHash", [txHash])).result;
		// pending transactions do not know yet to which block they belong to
		expect(pendingTransaction).to.include({
			blockNumber: null,
			hash: txHash,
			r: tx.r,
			s: tx.s,
			v: tx.v,
		});

		const gap = await context.web3.eth.accounts.signTransaction(
			{
				from: GENESIS_ACCOUNT,
				to: "0x1111111111111111111111111111111111111111",
				value: "0x200",
				gasPrice: "0x3B9ACA00",
				gas: "0x5208",
				nonce: 0,
			},
			GENESIS_ACCOUNT_PRIVATE_KEY,
		);
		await customRequest(context.web3, "eth_sendRawTransaction", [gap.rawTransaction]);
		await waitForReceipt(context.web3, txHash);

		const processedTransaction = (await customRequest(context.web3, "eth_getTransactionByHash", [txHash])).result;
		expect(processedTransaction).to.include({
			hash: txHash,
			r: tx.r,
			s: tx.s,
			v: tx.v,
		});
	});
});

describeWithFrontierAllPools("Frontier RPC (Pending Transaction Count)", (context) => {
	const TEST_ACCOUNT = "0x1111111111111111111111111111111111111111";

	// Allow the fork-aware pool to observe the newly finalized block
	// Before the first block is created, the pool will wrongly report as empty
	// https://github.com/paritytech/polkadot-sdk/issues/8402
	before("create and finalize block 1", async function () {
		await waitForBlock(context.web3);
	});

	it("should return pending transaction count", async function () {
		this.timeout(180000);

		// nonce should be 0
		expect(await context.web3.eth.getTransactionCount(GENESIS_ACCOUNT, "latest")).to.eq(0);

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

		{
			const pendingTransactionCount = (
				await customRequest(context.web3, "eth_getBlockTransactionCountByNumber", ["pending"])
			).result;
			expect(pendingTransactionCount).to.eq("0x0");
		}

		async function checkPendingCount(maximum: number) {
			// This pinned Frontier RPC counts the ready pool, including work
			// that may not fit the next block (e.g. at a session boundary).
			// Compare stable pool snapshots, not hypothetical block contents.
			for (let attempt = 0; attempt < 20; attempt++) {
				const before = (await customRequest(context.web3, "txpool_status", [])).result;
				const count = (await customRequest(context.web3, "eth_getBlockTransactionCountByNumber", ["pending"]))
					.result;
				const after = (await customRequest(context.web3, "txpool_status", [])).result;
				if (before.pending !== after.pending) continue;
				expect(BigInt(count)).to.equal(BigInt(after.pending));
				expect(Number(BigInt(count))).to.be.at.most(maximum);
				return;
			}
			throw new Error("Could not observe a stable pending snapshot");
		}

		for (const count of [1, 5]) {
			const hashes: string[] = [];
			for (let i = 0; i < count; i++) hashes.push(await sendTransaction());
			await checkPendingCount(count);
			const receipts = await Promise.all(hashes.map((hash) => waitForReceipt(context.web3, hash)));
			await checkPendingCount(0);
			for (const blockNumber of new Set(receipts.map((receipt) => receipt.blockNumber))) {
				const actual = (
					await customRequest(context.web3, "eth_getBlockTransactionCountByNumber", [blockNumber])
				).result;
				expect(Number(BigInt(actual))).to.equal(
					receipts.filter((receipt) => receipt.blockNumber === blockNumber).length,
				);
			}
		}
	});
});
