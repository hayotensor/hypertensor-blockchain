import { defineChain, http, publicActions, createPublicClient } from "viem"
import { privateKeyToAccount, generatePrivateKey } from 'viem/accounts'
import { ApiPromise } from "@polkadot/api";
import { ethers, getAddress } from "ethers"
import { ETH_LOCAL_URL } from "./config"
import IOverwatchNode from "../build/contracts/IOverwatchNode.json";
import Subnet from "../build/contracts/Subnet.json";
import Staking from "../build/contracts/Staking.json";
import PeerId from 'peer-id'
import bs58 from "bs58";

export const SEED_PATH = "subnet-name";
export const TEST_PATH = "subnet-test-name";
export const GENESIS_ACCOUNT = "0x6be02d1d3665660d22ff9624b7be0551ee1ac91b";
export const GENESIS_ACCOUNT_PRIVATE_KEY = "0x99B3C12287537E38C90A9219D4CB074A89A16E9CDB20BF85728EBD97C343E342";

export const OVERWATCH_NODE_CONTRACT_ABI = IOverwatchNode.abi;
export const OVERWATCH_NODE_CONTRACT_ADDRESS = hash(2050);

export const SUBNET_CONTRACT_ABI = Subnet.abi;
export const SUBNET_CONTRACT_ADDRESS = hash(2049);

export const STAKING_CONTRACT_ABI = Staking.abi;
export const STAKING_CONTRACT_ADDRESS = hash(2048);


export type ClientUrlType = 'http://localhost:9944';

export const chain = (id: number, url: string) => defineChain({
    id: id,
    name: 'hypertensor',
    network: 'hypertensor',
    nativeCurrency: {
        name: 'tensor',
        symbol: 'TENSOR',
        decimals: 18,
    },
    rpcUrls: {
        default: {
            http: [url],
        },
    },
    testnet: true,
})


export async function getPublicClient(url: ClientUrlType) {
    const wallet = createPublicClient({
        chain: chain(42, url),
        transport: http(),

    })

    return wallet.extend(publicActions)
}

/**
 * Generates a random Ethereum wallet
 * @returns wallet keyring
 */
export function generateRandomEthWallet() {
    let privateKey = generatePrivateKey().toString();
    privateKey = privateKey.replace('0x', '');

    const account = privateKeyToAccount(`0x${privateKey}`)
    return account
}


export function generateRandomEthersWallet() {
    const account = ethers.Wallet.createRandom();
    const provider = new ethers.JsonRpcProvider(ETH_LOCAL_URL);

    const wallet = new ethers.Wallet(account.privateKey, provider);
    return wallet;
}

export function hash(n: number) {
    const bytes = new Uint8Array(20); // 20 bytes = H160
    const view = new DataView(bytes.buffer);
    view.setBigUint64(12, BigInt(n)); // store in last 8 bytes, big-endian
    const hex = "0x" + Buffer.from(bytes).toString("hex");
    return getAddress(hex); // optional: applies EIP-55 checksum
}

const characters = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';

export function generateRandomString(length: number) {
    let result = ' ';
    const charactersLength = characters.length;
    for (let i = 0; i < length; i++) {
        result += characters.charAt(Math.floor(Math.random() * charactersLength));
    }

    return result;
}

export async function generateRandomEd25519PeerId() {
    const id = await PeerId.create({ bits: 256, keyType: 'Ed25519' })
    return id.toB58String()
}

export async function generateRandomMultiaddr(peerId?: string) {
    const addrPeerId = peerId ?? await generateRandomEd25519PeerId();
    const multiaddr = `/ip4/127.0.0.1/tcp/${generateRandomPort()}/p2p/${addrPeerId}`
    return multiaddrToBytes(multiaddr)
}

export function generateRandomPort() {
    return Math.floor(Math.random() * 65535)
}

export async function waitForBlocks(api: ApiPromise, blockCount = 1) {
    let blocksWaited = 0;
    return new Promise(async (resolve) => {
        const unsubscribe = await api.rpc.chain.subscribeNewHeads((header) => {
            blocksWaited++;
            console.log("waitForBlocks", blocksWaited)
            if (blocksWaited >= blockCount) {
                unsubscribe();
                resolve(header);
            }
        });
    })
}

// MULTIADDR

const IP4 = 4;
const IP6 = 41;
const TCP = 6;
const UDP = 17;
const DNS4 = 54;
const DNS6 = 55;
const DNSADDR = 56;
const P2P = 421;
const WS = 477;
const WSS = 478;

function encodeVarint(value: number) {
    const out = [];
    while (value >= 0x80) {
        out.push((value & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value);
    return out;
}

function parseIPv6(addr: string) {
    const parts = addr.split("::");
    if (parts.length > 2) throw new Error("Invalid IPv6");

    const left = parts[0] ? parts[0].split(":").filter(Boolean) : [];
    const right =
        parts.length === 2 && parts[1]
            ? parts[1].split(":").filter(Boolean)
            : [];

    if (left.length + right.length > 8) {
        throw new Error("Invalid IPv6");
    }

    const segs = new Array(8).fill(0);

    left.forEach((p: string, i: number) => {
        segs[i] = parseInt(p, 16);
    });

    let j = 8 - right.length;
    right.forEach((p: string) => {
        segs[j++] = parseInt(p, 16);
    });

    return segs;
}

export function multiaddrToBytes(addr: string) {
    const out = [];
    const parts = addr.split("/").filter(Boolean);
    let i = 0;

    while (i < parts.length) {
        const proto = parts[i++];

        if (proto === "ip4") {
            const ip = parts[i++];
            const octets = ip.split(".").map(n => parseInt(n, 10));
            out.push(...encodeVarint(IP4));
            out.push(...octets);

        } else if (proto === "ip6") {
            const ip = parts[i++];
            out.push(...encodeVarint(IP6));
            for (const seg of parseIPv6(ip)) {
                out.push((seg >> 8) & 0xff, seg & 0xff);
            }

        } else if (proto === "dns4" || proto === "dns6" || proto === "dnsaddr") {
            const name = parts[i++];
            const code = proto === "dns4" ? DNS4 : proto === "dns6" ? DNS6 : DNSADDR;
            const bytes = new TextEncoder().encode(name);

            out.push(...encodeVarint(code));
            out.push(...encodeVarint(bytes.length));
            out.push(...bytes);

        } else if (proto === "tcp" || proto === "udp") {
            const port = parseInt(parts[i++], 10);
            out.push(...encodeVarint(proto === "tcp" ? TCP : UDP));
            out.push((port >> 8) & 0xff, port & 0xff);

        } else if (proto === "ws") {
            out.push(...encodeVarint(WS));

        } else if (proto === "wss") {
            out.push(...encodeVarint(WSS));

        } else if (proto === "p2p") {
            const peer = parts[i++];
            const peerBytes = bs58.decode(peer);

            out.push(...encodeVarint(P2P));
            out.push(...encodeVarint(peerBytes.length));
            out.push(...peerBytes);

        } else {
            throw new Error(`Unknown protocol: ${proto}`);
        }
    }

    return Uint8Array.from(out);
}
