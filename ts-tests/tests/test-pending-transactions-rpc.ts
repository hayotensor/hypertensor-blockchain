import { expect } from "chai";
import { step } from "mocha-steps";

import { GENESIS_ACCOUNT, GENESIS_ACCOUNT_PRIVATE_KEY } from "./config";
import { waitForReceipt, waitForBlock, customRequest, describeWithFrontierAllPools } from "./util";

describeWithFrontierAllPools("Frontier RPC (Pending Transactions)", (context) => {
	const TEST_ACCOUNT = "0x1111111111111111111111111111111111111111";

	// Helper function to create and send a transaction
	async function sendTransaction(nonce?: number, options = {}) {
		const defaultTxParams: {
			from: string;
			to: string;
			data: string;
			value: string;
			gasPrice: string;
			gas: string;
			nonce?: number;
		} = {
			from: GENESIS_ACCOUNT,
			to: TEST_ACCOUNT,
			data: "0x00",
			value: "0x200", // Must be higher than ExistentialDeposit
			gasPrice: "0x3B9ACA00",
			gas: "0x100000",
		};

		// Use next available nonce if not provided
		const txParams = { ...defaultTxParams, ...options };
		if (nonce !== undefined) {
			txParams.nonce = nonce;
		}

		const tx = await context.web3.eth.accounts.signTransaction(txParams, GENESIS_ACCOUNT_PRIVATE_KEY);

		const result = await customRequest(context.web3, "eth_sendRawTransaction", [tx.rawTransaction]);
		return {
			hash: result.result,
			...txParams,
		};
	}

	// Helper to get pending transactions
	async function getPendingTransactions() {
		const response = await customRequest(context.web3, "eth_pendingTransactions", []);
		return response.result || [];
	}

	step("should return empty array when no transactions are pending", async function () {
		const pendingTransactions = await getPendingTransactions();
		expect(pendingTransactions).to.be.an("array").that.is.empty;
	});

	step("should return pending transactions when transactions are in mempool", async function () {
		// First, create a block to clear previous pending transactions
		await waitForBlock(context.web3);

		const readyTransactionCount = 3;
		const futureTransactionCount = 2;
		const transactions = [];

		// Get initial nonce
		const initialNonce = await context.web3.eth.getTransactionCount(GENESIS_ACCOUNT);

		// Submit regular transactions with sequential nonces
		for (let i = 0; i < readyTransactionCount; i++) {
			const currentNonce = initialNonce + i;
			const tx = await sendTransaction(currentNonce);
			transactions.push(tx);
		}

		// Submit future transactions with gaps in nonces
		for (let i = 0; i < futureTransactionCount; i++) {
			// Create a gap by skipping some nonces
			const gapSize = i * 2 + 1;
			const futureNonce = initialNonce + readyTransactionCount + gapSize;
			const tx = await sendTransaction(futureNonce);
			transactions.push(tx);
		}

		// Check pending transactions through RPC
		const pendingTransactions = await getPendingTransactions();

		// Verify the response
		expect(pendingTransactions).to.be.an("array");
		expect(pendingTransactions.length).to.be.at.most(transactions.length);

		// Verify transaction hashes match what we submitted
		const pendingHashes = pendingTransactions.map((tx) => tx.hash);
		const submittedHashes = transactions.map((tx) => tx.hash);
		expect(submittedHashes).to.include.members(pendingHashes);
		// Ready transactions may already be mined while these RPCs run. Future
		// transactions with nonce gaps must remain visible in the pool.
		expect(pendingHashes).to.include.members(transactions.slice(readyTransactionCount).map((tx) => tx.hash));
		for (const hash of submittedHashes.filter((hash) => !pendingHashes.includes(hash))) {
			expect((await waitForReceipt(context.web3, hash)).transactionHash).to.equal(hash);
		}
	});

	step("should remove transactions from pending transactions when block is created", async function () {
		// First, create a block to clear previous pending transactions
		await waitForBlock(context.web3);

		// Get current nonce
		const nonce = await context.web3.eth.getTransactionCount(GENESIS_ACCOUNT);

		// Submit a transaction
		const submitted = await sendTransaction(nonce, {
			gasPrice: context.web3.utils.toWei("1", "gwei"),
		});

		await waitForReceipt(context.web3, submitted.hash);
		const pendingAfter = await getPendingTransactions();
		expect(pendingAfter.map((tx) => tx.hash)).not.to.include(submitted.hash);
	});
});
