import { encodeAddress } from "@polkadot/util-crypto";

export function printWalletAddresses(wallet) {
  // This runtime maps an Ethereum address to H160 followed by twelve 0xee bytes.
  const accountId = `${wallet.address}${"ee".repeat(12)}`;
  console.log(`Ethereum address: ${wallet.address}`);
  console.log(`Native funding address: ${encodeAddress(accountId, 42)}`);
  console.log(`Native account ID: ${accountId}`);
}
