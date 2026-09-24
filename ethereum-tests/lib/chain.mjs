import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createServer } from 'node:net';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { openSync, closeSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { encodeAddress } from '@polkadot/util-crypto';
import { JsonRpcProvider, Wallet, ContractFactory, parseEther } from 'ethers';
import JSONbig from 'json-bigint';
import { compile, directory } from './compile.mjs';

const exec = promisify(execFile);
const bigJson = JSONbig({ useNativeBigInt: true });
const timeout = 120_000;

async function freePort() {
  const server = createServer();
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return port;
}

async function eventually(description, operation, check, limit = timeout) {
  const deadline = Date.now() + limit;
  let last;
  do {
    check();
    try {
      const value = await operation();
      if (value) return value;
    } catch (error) {
      last = error;
    }
    await delay(500);
  } while (Date.now() < deadline);
  throw new Error(`Timed out waiting for ${description}${last ? `: ${last.message}` : ''}`);
}

async function rpc(url, method, params = []) {
  const response = await fetch(url, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
    signal: AbortSignal.timeout(5000),
  });
  if (!response.ok) throw new Error(`${method}: HTTP ${response.status}`);
  const result = await response.json();
  if (result.error) throw new Error(`${method}: ${JSON.stringify(result.error)}`);
  return result.result;
}

export class TestChain {
  children = [];
  wallets = [];
  stopped = false;

  async start() {
    const node = process.env.TALARIS_NODE ?? resolve(directory, '../target/release/hypertensor-node');
    const adapter = process.env.ETH_RPC ?? 'eth-rpc';
    // Fail before spawning anything if required executables are unavailable.
    const [nodeVersion, adapterVersion] = await Promise.all([
      exec(node, ['--version']), exec(adapter, ['--version']),
    ]);
    this.artifacts = await compile();
    this.basePath = await mkdtemp(resolve(tmpdir(), 'talaris-ethereum-tests-'));
    this.logPath = await mkdtemp(resolve(directory, 'logs/run-'));
    const ports = new Set();
    while (ports.size < 3) ports.add(await freePort());
    const [nativePort, ethPort, p2pPort] = [...ports];
    this.nativeUrl = `http://127.0.0.1:${nativePort}`;
    this.url = `http://127.0.0.1:${ethPort}`;
    this.provider = new JsonRpcProvider(this.url, undefined, { cacheTimeout: -1, batchMaxCount: 1 });
    this.provider.pollingInterval = 500;
    this.wallets = Array.from({ length: 4 }, () => Wallet.createRandom().connect(this.provider));

    const { stdout } = await exec(node, ['build-spec', '--chain', 'dev', '--disable-default-bootnode'], {
      maxBuffer: 64 * 1024 * 1024, timeout,
    });
    // Ordinary JSON.parse would round the u128 genesis balances.
    const spec = bigJson.parse(stdout);
    spec.id = 'ethereum-integration-tests';
    spec.name = 'Disposable Ethereum integration tests';
    spec.bootNodes = [];
    spec.telemetryEndpoints = null;
    for (const wallet of this.wallets) {
      const native = encodeAddress(`${wallet.address}${'ee'.repeat(12)}`, 42);
      spec.genesis.runtimeGenesis.patch.balances.balances.push([native, parseEther('10000')]);
    }
    const specPath = resolve(this.basePath, 'spec.json');
    await writeFile(specPath, bigJson.stringify(spec), { mode: 0o600 });

    this.launch('node', node, [
      '--chain', specPath, '--base-path', resolve(this.basePath, 'node'),
      '--alice', '--validator', '--force-authoring', '--reserved-only', '--no-mdns',
      '--unsafe-force-node-key-generation', // Fresh, disposable P2P identity for this run.
      '--listen-addr', `/ip4/127.0.0.1/tcp/${p2pPort}`,
      '--rpc-port', String(nativePort), '--rpc-methods', 'safe',
      '--no-telemetry', '--no-prometheus', '--state-pruning', 'archive', '--blocks-pruning', 'archive',
    ]);
    await this.ready('node RPC', async () => rpc(this.nativeUrl, 'system_health'));
    await this.ready('block production', async () => {
      const header = await rpc(this.nativeUrl, 'chain_getHeader');
      return BigInt(header.number) > 0n;
    });

    this.launch('eth-rpc', adapter, [
      '--node-rpc-url', `ws://127.0.0.1:${nativePort}`, '--rpc-port', String(ethPort),
      '--eth-pruning', 'archive', '--base-path', resolve(this.basePath, 'eth-rpc'), '--no-prometheus',
    ]);
    await this.ready('Ethereum RPC', async () => rpc(this.url, 'eth_chainId'));
    this.chainId = BigInt(await rpc(this.url, 'eth_chainId'));
    // eth_getBalance excludes the native existential deposit from spendable funds.
    await this.ready('Ethereum genesis funding', async () =>
      BigInt(await rpc(this.url, 'eth_getBalance', [this.wallets[0].address, 'latest'])) >= parseEther('1000'));
    const provenance = {
      node: nodeVersion.stdout.trim(), adapter: adapterVersion.stdout.trim(),
      genesisHash: await rpc(this.nativeUrl, 'chain_getBlockHash', [0]),
      runtime: await rpc(this.nativeUrl, 'state_getRuntimeVersion'), chainId: String(this.chainId),
    };
    await writeFile(resolve(this.logPath, 'environment.json'), JSON.stringify(provenance, null, 2));
    console.log(`Disposable chain ready; logs: ${this.logPath}`);
  }

  launch(name, executable, args) {
    this.assertRunning();
    const logFile = resolve(this.logPath, `${name}.log`);
    const fd = openSync(logFile, 'w', 0o600);
    const child = spawn(executable, args, { stdio: ['ignore', fd, fd] });
    closeSync(fd);
    child.on('error', (error) => { child.startError = error; });
    this.children.push({ name, child, logFile });
    return child;
  }

  assertRunning() {
    if (this.stopped) throw new Error('Test chain has been stopped');
    for (const { name, child, logFile } of this.children) {
      if (child.startError || child.exitCode !== null || child.signalCode !== null) {
        throw new Error(`${name} stopped: ${child.startError ?? child.exitCode ?? child.signalCode}. See ${logFile}`);
      }
    }
  }

  async ready(description, operation) {
    return eventually(description, operation, () => this.assertRunning());
  }

  async receipt(hash, status = 1) {
    const receipt = await this.ready(`finalized receipt ${hash}`, async () => {
      const value = await rpc(this.url, 'eth_getTransactionReceipt', [hash]);
      if (!value) return false;
      const finalized = await rpc(this.url, 'eth_getBlockByNumber', ['finalized', false]);
      return finalized && BigInt(finalized.number) >= BigInt(value.blockNumber) ? value : false;
    });
    assert.equal(Number(BigInt(receipt.status)), status, `receipt ${hash} status`);
    const block = await rpc(this.url, 'eth_getBlockByNumber', [receipt.blockNumber, false]);
    assert.equal(block.hash, receipt.blockHash, 'receipt must belong to the canonical finalized block');
    return receipt;
  }

  async deploy(name, wallet = this.wallets[0]) {
    const artifact = this.artifacts[name];
    const contract = await new ContractFactory(artifact.abi, `0x${artifact.evm.bytecode.object}`, wallet).deploy();
    await this.receipt(contract.deploymentTransaction().hash);
    return contract;
  }

  async send(transaction, status = 1) {
    const tx = await transaction;
    return this.receipt(tx.hash, status);
  }

  async stop() {
    if (this.stopped) return;
    this.stopped = true;
    this.provider?.destroy();
    for (const { child } of this.children.toReversed()) {
      if (!child.pid || child.exitCode !== null || child.signalCode !== null) continue;
      const ended = new Promise((resolve) => child.once('exit', resolve));
      child.kill('SIGTERM');
      const deadline = setTimeout(() => child.kill('SIGKILL'), 5000);
      await ended;
      clearTimeout(deadline);
    }
    if (this.basePath) await rm(this.basePath, { recursive: true, force: true });
  }
}

export async function prepareLogDirectory() {
  await mkdir(resolve(directory, 'logs'), { recursive: true });
}
