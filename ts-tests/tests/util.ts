import Web3 from "web3";
import { ethers, getAddress } from "ethers";
import { JsonRpcResponse } from "web3-core-helpers";
import { spawn, ChildProcess } from "child_process";

import { NODE_BINARY_NAME, CHAIN_ID } from "./config";

export const PORT = Number(process.env.FRONTIER_P2P_PORT || 19931);
export const RPC_PORT = Number(process.env.FRONTIER_RPC_PORT || 19932);

// export const PORT = 33137;
// export const RPC_PORT = 9944;

export const DISPLAY_LOG = process.env.FRONTIER_LOG || false;
export const FRONTIER_LOG = process.env.FRONTIER_LOG || "info";
export const FRONTIER_BUILD = process.env.FRONTIER_BUILD || "release";
export const FRONTIER_BACKEND_TYPE = process.env.FRONTIER_BACKEND_TYPE || "key-value";

export const BINARY_PATH = `../target/${FRONTIER_BUILD}/${NODE_BINARY_NAME}`;
export const SPAWNING_TIME = Number(process.env.FRONTIER_SPAWNING_TIME || 60000);

export async function customRequest(web3: Web3, method: string, params: any[]) {
	return new Promise<JsonRpcResponse>((resolve, reject) => {
		(web3.currentProvider as any).send(
			{
				jsonrpc: "2.0",
				id: 1,
				method,
				params,
			},
			(error: Error | null, result?: JsonRpcResponse) => {
				if (error) {
					reject(
						`Failed to send custom request (${method} (${params.join(",")})): ${
							error.message || error.toString()
						}`
					);
				}
				resolve(result);
			}
		);
	});
}

// Wait for a newly authored block and, by default, its GRANDPA finality.
// Production continues while waiting: return the requested block, not a later head.
export async function waitForBlock(web3: Web3, finalize: boolean = true) {
	const best = await customRequest(web3, "eth_blockNumber", []);
	const target = BigInt(best.result as string) + BigInt(1);
	const deadline = Date.now() + 120_000;
	while (Date.now() < deadline) {
		const response = await customRequest(web3, "eth_getBlockByNumber", [finalize ? "finalized" : "latest", false]);
		const block = response.result as any;
		if (block && BigInt(block.number) >= target) return web3.eth.getBlock(Number(target));
		await new Promise<void>((resolve) => setTimeout(resolve, 1000));
	}
	throw new Error(`Timed out waiting for block ${target}`);
}

// A session-boundary block can be full of consensus work and contain no user
// transactions. Observe inclusion directly instead of assuming the next block.
export async function waitForReceipt(web3: Web3, hash: string) {
	const deadline = Date.now() + 120_000;
	while (Date.now() < deadline) {
		const receipt = await web3.eth.getTransactionReceipt(hash);
		if (receipt) {
			const finalized = await web3.eth.getBlock("finalized");
			if (finalized && finalized.number >= receipt.blockNumber) return receipt;
		}
		await new Promise<void>((resolve) => setTimeout(resolve, 1000));
	}
	throw new Error(`Timed out waiting for finalized transaction ${hash}`);
}

export async function startFrontierNode(
	provider?: string,
	additionalArgs: string[] = []
): Promise<{
	web3: Web3;
	binary: ChildProcess;
	ethersjs: ethers.JsonRpcProvider;
}> {
	var web3;
	if (!provider || provider == "http") {
		web3 = new Web3(`http://127.0.0.1:${RPC_PORT}`);
	}

	const cmd = BINARY_PATH;
	const args = [
		`--chain=eth_dev`,
		`--validator`,
		`--alice`,
		// Independent suites reuse Alice's development keys; prevent peer discovery
		// from connecting their otherwise identical genesis chains.
		`--no-mdns`,
		`--no-telemetry`,
		`--no-prometheus`,
		`--force-authoring`,
		`-l${FRONTIER_LOG}`,
		`--port=${PORT}`,
		`--rpc-port=${RPC_PORT}`,
		`--frontier-backend-type=${FRONTIER_BACKEND_TYPE}`,
		`--tmp`,
		`--unsafe-force-node-key-generation`,
		...additionalArgs,
	];
	const binary = spawn(cmd, args);

	const binaryLogs: string[] = [];
	try {
		await new Promise<void>((resolve, reject) => {
			// Probe HTTP for both providers; readiness must not depend on log verbosity.
			const readiness = new Web3(
				new Web3.providers.HttpProvider(`http://127.0.0.1:${RPC_PORT}`, {
					timeout: 5000,
					keepAlive: false,
				})
			);
			let finished = false;
			let retryTimer: ReturnType<typeof setTimeout>;
			let lastRpcError: unknown;
			const fail = (error: Error) => {
				if (finished) return;
				cleanup();
				reject(new Error(`${error.message}\nCommand: ${cmd} ${args.join(" ")}\n${binaryLogs.join("")}`));
			};
			const onError = (error: Error) => fail(error);
			const onExit = (code: number, signal: string) =>
				fail(new Error(`Test node exited before readiness (${code ?? signal})`));
			const timer = setTimeout(
				() => fail(new Error(`Timed out starting the test node. Last RPC error: ${lastRpcError}`)),
				SPAWNING_TIME - 2000
			);
			const cleanup = () => {
				finished = true;
				clearTimeout(timer);
				clearTimeout(retryTimer);
				binary.off("error", onError);
				binary.off("exit", onExit);
				binary.stderr.off("data", onData);
				binary.stdout.off("data", onData);
			};
			const onData = (chunk: Buffer) => {
				const message = chunk.toString();
				if (DISPLAY_LOG) console.log(message);
				binaryLogs.push(message);
				if (binaryLogs.length > 200) binaryLogs.shift();
			};
			const pollRpc = async () => {
				try {
					// This also warms up the EVM runtime before the tests begin.
					await readiness.eth.getChainId();
				} catch (error) {
					if (finished) return;
					lastRpcError = error;
					retryTimer = setTimeout(pollRpc, 250);
					return;
				}
				if (finished) return;
				cleanup();
				if (DISPLAY_LOG) {
					binary.stderr.on("data", (data) => console.log(data.toString()));
					binary.stdout.on("data", (data) => console.log(data.toString()));
				}
				resolve();
			};
			binary.once("error", onError);
			binary.once("exit", onExit);
			binary.stderr.on("data", onData);
			binary.stdout.on("data", onData);
			void pollRpc();
		});
	} catch (error) {
		await stopFrontierNode(binary);
		throw error;
	}

	if (provider == "ws") {
		web3 = new Web3(`ws://127.0.0.1:${RPC_PORT}`);
	}

	let ethersjs = new ethers.JsonRpcProvider(`http://127.0.0.1:${RPC_PORT}`, {
		chainId: CHAIN_ID,
		name: "frontier-dev",
	});

	return { web3, binary, ethersjs };
}

async function stopFrontierNode(binary?: ChildProcess) {
	if (!binary?.pid || binary.exitCode !== null || binary.signalCode !== null) return;
	// Wait for database/port release before starting the next suite.
	await new Promise<void>((resolve, reject) => {
		const timer = setTimeout(() => {
			binary.kill("SIGKILL");
			reject(new Error("Test node did not shut down within 30 seconds"));
		}, 30_000);
		binary.once("exit", () => {
			clearTimeout(timer);
			resolve();
		});
		binary.kill("SIGINT");
	});
}

export function describeWithFrontier(
	title: string,
	cb: (context: { web3: Web3 }) => void,
	provider?: string,
	additionalArgs: string[] = []
) {
	describe(title, function () {
		this.timeout(180_000);
		let context: {
			web3: Web3;
			ethersjs: ethers.JsonRpcProvider;
		} = { web3: null, ethersjs: null };
		let binary: ChildProcess;
		// Making sure the Frontier node has started
		before("Starting Frontier Test Node", async function () {
			this.timeout(SPAWNING_TIME);
			const init = await startFrontierNode(provider, additionalArgs);
			context.web3 = init.web3;
			context.ethersjs = init.ethersjs;
			binary = init.binary;
		});

		after(async function () {
			context.ethersjs?.destroy();
			(context.web3?.currentProvider as any)?.disconnect?.();
			await stopFrontierNode(binary);
		});

		cb(context);
	});
}

export function describeWithFrontierFaTp(title: string, cb: (context: { web3: Web3 }) => void) {
	describeWithFrontier(title, cb, undefined, [`--pool-type=fork-aware`]);
}

export function describeWithFrontierSsTp(title: string, cb: (context: { web3: Web3 }) => void) {
	describeWithFrontier(title, cb, undefined, [`--pool-type=single-state`]);
}

export function describeWithFrontierAllPools(title: string, cb: (context: { web3: Web3 }) => void) {
	describeWithFrontierSsTp(`[SsTp] ${title}`, cb);
	describeWithFrontierFaTp(`[FaTp] ${title}`, cb);
}

export function describeWithFrontierWs(title: string, cb: (context: { web3: Web3 }) => void) {
	describeWithFrontier(title, cb, "ws");
}

export function hash(n: number) {
	const bytes = new Uint8Array(20); // 20 bytes = H160
	const view = new DataView(bytes.buffer);
	view.setBigUint64(12, BigInt(n)); // store in last 8 bytes, big-endian
	const hex = "0x" + Buffer.from(bytes).toString("hex");
	return getAddress(hex); // optional: applies EIP-55 checksum
}
