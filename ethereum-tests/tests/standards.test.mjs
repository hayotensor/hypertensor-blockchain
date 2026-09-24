import assert from 'node:assert/strict';
import { after, before, describe, it } from 'node:test';
import { Signature, ZeroAddress, parseEther } from 'ethers';
import { TestChain, prepareLogDirectory } from '../lib/chain.mjs';

const chain = new TestChain();
const unit = parseEther('1');
const supply = 1_000_000n * unit;
const permitTypes = {
  Permit: [
    { name: 'owner', type: 'address' }, { name: 'spender', type: 'address' },
    { name: 'value', type: 'uint256' }, { name: 'nonce', type: 'uint256' },
    { name: 'deadline', type: 'uint256' },
  ],
};

const margin = (gas) => gas + gas / 2n;
const events = (contract, receipt, name) => receipt.logs
  .filter((log) => log.address.toLowerCase() === contract.target.toLowerCase())
  .map((log) => contract.interface.parseLog(log))
  .filter((event) => event?.name === name);

async function nonce(wallet) {
  return BigInt(await chain.provider.send('eth_getTransactionCount', [wallet.address, 'latest']));
}

async function signPermit(token, owner, spender, value, overrides = {}) {
  const block = await chain.provider.getBlock('latest');
  const message = {
    owner: owner.address, spender: spender.address, value,
    nonce: await token.nonces(owner.address), deadline: BigInt(block.timestamp) + 3600n,
    ...overrides.message,
  };
  const domain = {
    name: 'Integration Token', version: '1', chainId: chain.chainId,
    verifyingContract: token.target, ...overrides.domain,
  };
  const signature = Signature.from(await owner.signTypedData(domain, permitTypes, message));
  return [message.owner, message.spender, message.value, message.deadline,
    signature.v, signature.r, signature.s];
}

describe('Ethereum standards through Talaris and eth-rpc', { timeout: 1_200_000 }, () => {
  before(async () => {
    await prepareLogDirectory();
    await chain.start();
  }, { timeout: 600_000 });
  after(async () => { await chain.stop(); });

  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.once(signal, async () => {
      await chain.stop();
      process.exit(signal === 'SIGINT' ? 130 : 143);
    });
  }

  it('ERC-20: delegated transfer updates allowance, balances, logs and historical reads', async () => {
    const [owner, spender, recipient] = chain.wallets;
    const token = await chain.deploy('PermitToken');
    const approval = await chain.send(token.approve(spender.address, 25n * unit));
    const approvalEvents = events(token, approval, 'Approval');
    assert.equal(approvalEvents.length, 1);
    assert.deepEqual([...approvalEvents[0].args], [owner.address, spender.address, 25n * unit]);

    const receipt = await chain.send(token.connect(spender).transferFrom(owner.address, recipient.address, 10n * unit));
    assert.equal(await token.balanceOf(owner.address), supply - 10n * unit);
    assert.equal(await token.balanceOf(recipient.address), 10n * unit);
    assert.equal(await token.allowance(owner.address, spender.address), 15n * unit);
    assert.equal(await token.totalSupply(), supply);
    const transfers = events(token, receipt, 'Transfer');
    assert.equal(transfers.length, 1);
    assert.deepEqual([...transfers[0].args], [owner.address, recipient.address, 10n * unit]);
    // Read state before the delegated transfer through the Ethereum archive interface.
    const previous = { blockTag: approval.blockNumber };
    assert.equal(await token.balanceOf(recipient.address, previous), 0n);
    assert.equal(await token.allowance(owner.address, spender.address, previous), 25n * unit);
  });

  it('ERC-20: insufficient balance rolls back an allowance debit in an included transaction', async () => {
    const [owner, spender, recipient] = chain.wallets;
    const token = await chain.deploy('PermitToken');
    await chain.send(token.approve(spender.address, supply + unit));
    const delegated = token.connect(spender);
    const gas = margin(await delegated.transferFrom.estimateGas(owner.address, recipient.address, unit));
    await assert.rejects(delegated.transferFrom.staticCall(owner.address, recipient.address, supply + unit),
      (error) => error.revert?.name === 'ERC20InsufficientBalance');
    const beforeNonce = await nonce(spender);
    const receipt = await chain.send(delegated.transferFrom(owner.address, recipient.address, supply + unit, { gasLimit: gas }), 0);
    assert.equal(await token.allowance(owner.address, spender.address), supply + unit);
    assert.equal(await token.balanceOf(owner.address), supply);
    assert.equal(await token.balanceOf(recipient.address), 0n);
    assert.equal(await token.totalSupply(), supply);
    assert.equal(receipt.logs.length, 0);
    assert.equal(await nonce(spender), beforeNonce + 1n);
    assert(BigInt(receipt.gasUsed) > 0n);
  });

  it('ERC-20: revoking approval invalidates a previously estimated delegated transfer', async () => {
    const [owner, spender, recipient] = chain.wallets;
    const token = await chain.deploy('PermitToken');
    await chain.send(token.approve(spender.address, 10n * unit));
    const delegated = token.connect(spender);
    const gas = margin(await delegated.transferFrom.estimateGas(owner.address, recipient.address, unit));
    await chain.send(token.approve(spender.address, 0n));
    await assert.rejects(delegated.transferFrom.staticCall(owner.address, recipient.address, unit),
      (error) => error.revert?.name === 'ERC20InsufficientAllowance');
    const receipt = await chain.send(delegated.transferFrom(owner.address, recipient.address, unit, { gasLimit: gas }), 0);
    assert.equal(await token.allowance(owner.address, spender.address), 0n);
    assert.equal(await token.balanceOf(owner.address), supply);
    assert.equal(await token.balanceOf(recipient.address), 0n);
    assert.equal(receipt.logs.length, 0);
  });

  it('ERC-2612: a relayer submits a permit, cannot replay it, and the spender can use it', async () => {
    const [owner, spender, recipient, relayer] = chain.wallets;
    const token = await chain.deploy('PermitToken');
    const args = await signPermit(token, owner, spender, 3n * unit);
    const submitted = token.connect(relayer);
    const gas = margin(await submitted.permit.estimateGas(...args));
    const ownerNonce = await nonce(owner);
    const receipt = await chain.send(submitted.permit(...args));
    assert.equal(await token.nonces(owner.address), 1n);
    assert.equal(await nonce(owner), ownerNonce, 'the relayer, not the permit owner, signs the Ethereum transaction');
    assert.equal(await token.allowance(owner.address, spender.address), 3n * unit);
    assert.equal(await token.allowance(owner.address, relayer.address), 0n);
    assert.deepEqual([...events(token, receipt, 'Approval')[0].args], [owner.address, spender.address, 3n * unit]);

    await assert.rejects(submitted.permit.staticCall(...args), (error) => error.revert?.name === 'ERC2612InvalidSigner');
    const replay = await chain.send(submitted.permit(...args, { gasLimit: gas }), 0);
    assert.equal(replay.logs.length, 0);
    assert.equal(await token.nonces(owner.address), 1n);
    assert.equal(await token.allowance(owner.address, spender.address), 3n * unit);
    await chain.send(token.connect(spender).transferFrom(owner.address, recipient.address, 3n * unit));
    assert.equal(await token.balanceOf(recipient.address), 3n * unit);
    assert.equal(await token.allowance(owner.address, spender.address), 0n);
  });

  it('ERC-2612: rejects wrong chain, wrong verifying contract and expired signatures without consuming permit nonce', async () => {
    const [owner, spender, , relayer] = chain.wallets;
    const token = await chain.deploy('PermitToken');
    const submitted = token.connect(relayer);
    const valid = await signPermit(token, owner, spender, unit);
    const gas = margin(await submitted.permit.estimateGas(...valid));
    const cases = [
      [{ domain: { chainId: chain.chainId + 1n } }, 'ERC2612InvalidSigner'],
      [{ domain: { verifyingContract: spender.address } }, 'ERC2612InvalidSigner'],
      [{ message: { deadline: 0n } }, 'ERC2612ExpiredSignature'],
    ];
    for (const [overrides, errorName] of cases) {
      const args = await signPermit(token, owner, spender, unit, overrides);
      await assert.rejects(submitted.permit.staticCall(...args), (error) => error.revert?.name === errorName);
      const receipt = await chain.send(submitted.permit(...args, { gasLimit: gas }), 0);
      assert.equal(receipt.logs.length, 0);
      assert.equal(await token.nonces(owner.address), 0n);
      assert.equal(await token.allowance(owner.address, spender.address), 0n);
    }
    // Valid control after all failures prevents unrelated deployment/signing bugs passing the test.
    await chain.send(submitted.permit(...valid));
    assert.equal(await token.nonces(owner.address), 1n);
    assert.equal(await token.allowance(owner.address, spender.address), unit);
  });

  it('ERC-721: safe transfer calls the receiver with the original operator, owner and data', async () => {
    const [owner, operator] = chain.wallets;
    const nft = await chain.deploy('TestNFT');
    const receiver = await chain.deploy('NFTReceiver');
    await chain.send(nft.approve(operator.address, 1n));
    const receipt = await chain.send(nft.connect(operator)['safeTransferFrom(address,address,uint256,bytes)'](
      owner.address, receiver.target, 1n, '0x123456'));
    assert.equal(await nft.ownerOf(1n), receiver.target);
    assert.equal(await nft.getApproved(1n), ZeroAddress);
    assert.equal(await receiver.operator(), operator.address);
    assert.equal(await receiver.from(), owner.address);
    assert.equal(await receiver.tokenId(), 1n);
    assert.equal(await receiver.data(), '0x123456');
    const transfers = events(nft, receipt, 'Transfer');
    assert.equal(transfers.length, 1);
    assert.deepEqual([...transfers[0].args], [owner.address, receiver.target, 1n]);
  });

  it('ERC-721: a rejected receiver restores ownership and token approval and removes Transfer logs', async () => {
    const [owner, operator, recipient] = chain.wallets;
    const nft = await chain.deploy('TestNFT');
    const receiver = await chain.deploy('NonReceiver');
    await chain.send(nft.approve(operator.address, 2n));
    const transfer = nft.connect(operator)['safeTransferFrom(address,address,uint256)'];
    const gas = margin(await transfer.estimateGas(owner.address, recipient.address, 2n));
    await assert.rejects(transfer.staticCall(owner.address, receiver.target, 2n),
      (error) => error.revert?.name === 'ERC721InvalidReceiver');
    const receipt = await chain.send(transfer(owner.address, receiver.target, 2n, { gasLimit: gas }), 0);
    assert.equal(await nft.ownerOf(2n), owner.address);
    assert.equal(await nft.getApproved(2n), operator.address);
    assert.equal(await nft.balanceOf(receiver.target), 0n);
    assert.equal(await nft.balanceOf(owner.address), 2n);
    assert.equal(receipt.logs.length, 0);
    await chain.send(transfer(owner.address, recipient.address, 2n));
    assert.equal(await nft.ownerOf(2n), recipient.address);
  });
});
