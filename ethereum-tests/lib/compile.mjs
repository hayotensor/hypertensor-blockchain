import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve, dirname } from 'node:path';
import solc from 'solc';

export const directory = resolve(dirname(fileURLToPath(import.meta.url)), '..');

export async function compile() {
  const input = {
    language: 'Solidity',
    sources: { 'Standards.sol': { content: await readFile(resolve(directory, 'contracts/Standards.sol'), 'utf8') } },
    settings: {
      optimizer: { enabled: true, runs: 200 },
      evmVersion: 'cancun',
      outputSelection: { '*': { '*': ['abi', 'evm.bytecode.object', 'evm.deployedBytecode.object'] } },
    },
  };
  const output = JSON.parse(solc.compile(JSON.stringify(input), {
    import(path) {
      if (!path.startsWith('@openzeppelin/contracts/') || path.includes('..')) {
        return { error: `Unexpected import: ${path}` };
      }
      return { contents: readFileSync(resolve(directory, 'node_modules', path), 'utf8') };
    },
  }));
  const errors = (output.errors ?? []).filter((error) => error.severity === 'error');
  if (errors.length) throw new Error(errors.map((error) => error.formattedMessage).join('\n'));
  const artifacts = output.contracts['Standards.sol'];
  await mkdir(resolve(directory, 'artifacts'), { recursive: true });
  for (const [name, artifact] of Object.entries(artifacts)) {
    await writeFile(resolve(directory, 'artifacts', `${name}.json`), JSON.stringify({
      ...artifact, compiler: solc.version(), settings: input.settings,
    }, null, 2));
  }
  return artifacts;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await compile();
  console.log(`Compiled Solidity fixtures using ${solc.version()}`);
}
