import { expect } from "chai";

import { waitForBlock, describeWithFrontier, customRequest } from "./util";

// Consensus advances independently of RPC requests. Compare a tag to native
// heads read around it so a block/finality update between calls is allowed.
describeWithFrontier("Frontier RPC (BlockNumber tags)", (context) => {
	before("Wait for GRANDPA finality", async function () {
		await waitForBlock(context.web3);
	});

	it("`earliest` returns genesis", async function () {
		expect((await context.web3.eth.getBlock("earliest")).number).to.equal(0);
	});

	async function nativeNumber(finalized: boolean) {
		const params = finalized ? [(await customRequest(context.web3, "chain_getFinalizedHead", [])).result] : [];
		const header = (await customRequest(context.web3, "chain_getHeader", params)).result;
		return Number(BigInt(header.number));
	}

	for (const tag of ["latest", "finalized", "safe"]) {
		it(`\`${tag}\` follows the corresponding native head`, async function () {
			const finalized = tag !== "latest";
			const before = await nativeNumber(finalized);
			const block = await context.web3.eth.getBlock(tag);
			const after = await nativeNumber(finalized);
			expect(block.number).to.be.within(before, after);
			expect(block.number).to.be.greaterThan(0);
		});
	}
});
