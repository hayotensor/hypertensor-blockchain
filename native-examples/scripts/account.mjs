import { formatEther } from "ethers";
import { mapping, nativeWallet, withChain } from "./helpers.mjs";

const wallet = await nativeWallet();
await withChain(async (api) => {
  const [account, revive] = await Promise.all([
    api.query.System.Account.getValue(wallet.address),
    mapping(api, wallet.address),
  ]);
  console.log(`Native account / funding address: ${wallet.address}`);
  console.log(`Revive address: ${revive.contractAddress}`);
  console.log(`Revive mapping registered: ${revive.registered}`);
  console.log(`Free balance: ${formatEther(account.data.free)} TENSOR`);
  console.log(`Reserved balance: ${formatEther(account.data.reserved)} TENSOR`);
  console.log(`Frozen balance: ${formatEther(account.data.frozen)} TENSOR`);
});
