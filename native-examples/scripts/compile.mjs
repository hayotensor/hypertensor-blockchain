import { mkdir, readFile, writeFile } from "node:fs/promises";
import solc from "solc";

const source = await readFile(new URL("../contracts/SimpleStorage.sol", import.meta.url), "utf8");
const output = JSON.parse(solc.compile(JSON.stringify({
  language: "Solidity",
  sources: { "SimpleStorage.sol": { content: source } },
  settings: {
    evmVersion: "cancun",
    optimizer: { enabled: true, runs: 200 },
    outputSelection: { "*": { "*": ["abi", "evm.bytecode.object"] } },
  },
})));
for (const diagnostic of output.errors ?? []) console.error(diagnostic.formattedMessage);
if (output.errors?.some(({ severity }) => severity === "error")) {
  throw new Error("Solidity compilation failed.");
}
const contract = output.contracts["SimpleStorage.sol"].SimpleStorage;
const directory = new URL("../artifacts/", import.meta.url);
await mkdir(directory, { recursive: true });
await writeFile(new URL("SimpleStorage.json", directory), JSON.stringify({
  abi: contract.abi,
  bytecode: `0x${contract.evm.bytecode.object}`,
}, null, 2) + "\n");
console.log(`Compiled SimpleStorage with solc ${solc.version()}.`);
