// Run against an isolated --chain local Alice/Bob network. Uses existing npm dependencies.
// This intentionally submits staking transactions and changes the test validator set.
const assert = require('node:assert/strict');
const { ethers } = require('ethers');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { blake2AsU8a } = require('@polkadot/util-crypto');
const artifact = require('./build/contracts/NposSmoke.json');

const rpc = process.env.NPOS_RPC || 'http://127.0.0.1:9944';
const ws = process.env.NPOS_WS || rpc.replace('http', 'ws');
const ALITH_KEY = '0x5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133';
const ALITH = '0xf24ff3a9cf04c71dbc94d0b566f7a27b94566cac';
const BOB = '0x3cd0a705a2dc65e5b1e1205896baa2be8a07c6e0';
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

async function until(label, fn, timeout = 180_000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    const result = await fn();
    if (result) return result;
    await sleep(1500);
  }
  throw new Error(`Timed out: ${label}`);
}

async function main() {
  const provider = new ethers.JsonRpcProvider(rpc, undefined, { cacheTimeout: -1 });
  const subscriptions = new ethers.WebSocketProvider(ws);
  const wallet = new ethers.Wallet(ALITH_KEY, provider);
  const nominator = ethers.Wallet.createRandom().connect(provider);
  const api = await ApiPromise.create({ provider: new WsProvider(ws), types: {
    AccountId: 'AccountId20', Address: 'AccountId20', LookupSource: 'AccountId20',
  } });
  try {
    assert.equal((await provider.send('eth_chainId', [])), '0x2a');
    assert.equal(wallet.address.toLowerCase(), ALITH);
    assert.equal((await api.query.session.validators()).length, 2, 'use a fresh local chain with two validators');
    const generatedKeys = await api.rpc.author.rotateKeys();
    assert.equal(generatedKeys.toU8a(true).length, 64, 'BABE + GRANDPA key bundle');
    assert((await api.rpc.author.hasSessionKeys(generatedKeys)).isTrue);
    const initialBlock = BigInt(await provider.send('eth_blockNumber', []));
    assert(BigInt(await provider.send('eth_getBalance', [wallet.address, 'latest'])) > 0n);
    const firstFinalized = (await api.rpc.chain.getHeader(await api.rpc.chain.getFinalizedHead())).number.toNumber();
    let ethereumHeads = 0;
    await subscriptions.on('block', () => { ethereumHeads++; });
    await until('GRANDPA finality advances', async () =>
      (await api.rpc.chain.getHeader(await api.rpc.chain.getFinalizedHead())).number.toNumber() > firstFinalized + 1);
    const authors = new Set();
    const unsubHeads = await api.rpc.chain.subscribeFinalizedHeads(async header => {
      for (const digest of header.digest.logs) {
        if (digest.isPreRuntime && digest.asPreRuntime[0].toHex() === '0x42414245') {
          const bytes = digest.asPreRuntime[1].toU8a(true);
          authors.add(Buffer.from(bytes).readUInt32LE(1));
        }
      }
    });

    const factory = new ethers.ContractFactory(artifact.abi, artifact.bytecode, wallet);
    const contract = await factory.deploy();
    await contract.waitForDeployment();
    const deployment = await contract.deploymentTransaction().wait();
    assert.equal(deployment.from.toLowerCase(), ALITH);
    const address = await contract.getAddress();
    assert.notEqual(await provider.send('eth_getCode', [address, 'latest']), '0x');
    const call = { from: wallet.address, to: address, data: contract.interface.encodeFunctionData('set', [73]) };
    assert(BigInt(await provider.send('eth_estimateGas', [call])) >= 21_000n);
    // Broadcast an explicitly serialized, normally signed Ethereum transaction.
    const raw = await wallet.signTransaction(await wallet.populateTransaction(call));
    const hash = await provider.send('eth_sendRawTransaction', [raw]);
    const receipt = await until('Ethereum receipt indexing', () => provider.send('eth_getTransactionReceipt', [hash]));
    assert.equal(receipt.status, '0x1');
    assert.equal(receipt.from.toLowerCase(), ALITH);
    assert.equal(await contract.value(), 73n);
    const valueData = contract.interface.encodeFunctionData('value');
    assert.equal(BigInt(await provider.send('eth_call', [{ to: address, data: valueData }, 'pending'])), 73n);
    const logs = await provider.send('eth_getLogs', [{ address, fromBlock: receipt.blockNumber, toBlock: receipt.blockNumber }]);
    assert.equal(logs.length, 1);
    assert.equal(contract.interface.parseLog(logs[0]).args.sender.toLowerCase(), ALITH);
    assert(await provider.send('eth_getBlockByNumber', ['pending', false]));
    assert((await provider.send('eth_feeHistory', ['0x2', 'latest', [50]])).baseFeePerGas.length > 0);
    const filter = await provider.send('eth_newFilter', [{ address }]);
    assert(Array.isArray(await provider.send('eth_getFilterChanges', [filter])));
    assert(await provider.send('eth_uninstallFilter', [filter]));
    await until('Ethereum newHeads pubsub', () => ethereumHeads > 0);
    console.log('PASS Ethereum H160 signing, deployment, call, estimate, receipt, code, logs, pending, fees and filters');

    // Sign the repository's native extrinsic payload with its EthereumSignature
    // scheme: SCALE SignedPayload (blake2 if >256 bytes), then secp256k1 over keccak.
    async function submit(tx, signer) {
      const nonce = (await api.rpc.system.accountNextIndex(signer.address)).toNumber();
      const payload = api.registry.createType('ExtrinsicPayload', {
        method: tx.method.toHex(), era: '0x00', nonce, tip: 0,
        specVersion: api.runtimeVersion.specVersion,
        transactionVersion: api.runtimeVersion.transactionVersion,
        genesisHash: api.genesisHash, blockHash: api.genesisHash,
      }, { version: tx.version });
      let bytes = payload.toU8a({ method: true });
      if (bytes.length > 256) bytes = blake2AsU8a(bytes);
      const signature = signer.signingKey.sign(ethers.keccak256(bytes));
      const rawSignature = ethers.concat([signature.r, signature.s, ethers.toBeHex(signature.yParity, 1)]);
      tx.addSignature(signer.address, rawSignature, payload.toJSON());
      return new Promise((resolve, reject) => {
        let unsubscribe;
        const timer = setTimeout(() => { unsubscribe?.(); reject(new Error('staking transaction timeout')); }, 120_000);
        tx.send(result => {
          if (result.dispatchError) {
            clearTimeout(timer); unsubscribe?.(); reject(new Error(result.dispatchError.toString()));
          } else if (result.status.isFinalized) {
            clearTimeout(timer); unsubscribe?.(); resolve(result);
          }
        }).then(fn => { unsubscribe = fn; }).catch(error => { clearTimeout(timer); reject(error); });
      });
    }
    await (await wallet.sendTransaction({ to: nominator.address, value: ethers.parseEther('2000') })).wait();
    const bondResult = await submit(api.tx.staking.bond(ethers.parseEther('100').toString(), 'Stash'), nominator);
    assert(bondResult.events.some(({ event }) => event.section === 'staking' && event.method === 'Bonded'));
    await submit(api.tx.staking.bondExtra(ethers.parseEther('10').toString()), nominator);
    await submit(api.tx.staking.nominate([ALITH, BOB]), nominator);
    const nominatedAt = (await api.query.staking.activeEra()).unwrap().index.toNumber();
    await until('both validators author finalized BABE blocks', () => authors.size === 2);
    unsubHeads();
    console.log(`Bonded and nominated at era ${nominatedAt}; waiting for election activation`);
    // The next era can already be queued when the nomination is submitted.
    const electedEra = await until('nomination is elected into an active era', async () => {
      const era = (await api.query.staking.activeEra()).unwrap().index.toNumber();
      if (era <= nominatedAt) return false;
      const exposures = await api.query.staking.erasStakersPaged.entries(era);
      return exposures.some(([, exposure]) => exposure.isSome && exposure.unwrap().others.some(n =>
        n.who.toString().toLowerCase() === nominator.address.toLowerCase())) && era;
    }, 900_000);
    console.log(`Nomination elected in era ${electedEra}; chilling Alice`);
    assert((await api.query.session.currentIndex()).toNumber() > 0);
    await submit(api.tx.staking.chill(), wallet);
    await until('chilled validator leaves staking-managed sessions', async () =>
      !(await api.query.session.validators()).some(id => id.toString().toLowerCase() === ALITH), 900_000);
    assert.deepEqual((await api.query.session.validators()).map(id => id.toString().toLowerCase()), [BOB]);
    console.log('Staking rotated the active validator set to Bob; checking reward and unbond');
    const beforeReward = BigInt((await api.query.system.account(nominator.address)).data.free.toString());
    await submit(api.tx.staking.payoutStakers(BOB, electedEra), nominator);
    assert(BigInt((await api.query.system.account(nominator.address)).data.free.toString()) > beforeReward);
    await submit(api.tx.staking.chill(), nominator);
    await submit(api.tx.staking.unbond(ethers.parseEther('110').toString()), nominator);
    assert((await api.query.staking.ledger(nominator.address)).unwrap().unlocking.length > 0);
    const finalizedAfter = (await api.rpc.chain.getHeader(await api.rpc.chain.getFinalizedHead())).number.toNumber();
    await until('GRANDPA continues after validator removal', async () =>
      (await api.rpc.chain.getHeader(await api.rpc.chain.getFinalizedHead())).number.toNumber() > finalizedAfter + 1);
    assert(BigInt(await provider.send('eth_blockNumber', [])) > initialBlock);
    assert.equal(await contract.value(), 73n);
    assert.equal(BigInt(await provider.send('eth_call', [{ to: address, data: valueData }, 'pending'])), 73n);
    assert(await provider.send('eth_getBlockByNumber', ['pending', false]));
    console.log('PASS two BABE authors, GRANDPA finality, bond/extra, nomination/election, era/session rotation, chill, payout and unbond');
  } finally {
    await api.disconnect();
    await subscriptions.destroy();
    provider.destroy();
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
