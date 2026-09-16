import assert from "assert";

export const ETH = BigInt(1000000000000000000); // 10^9
export const GWEI = BigInt(1000000000); // 10^9
export const MAX_TX_FEE = BigInt(21000000) * GWEI; // 100 times EVM to EVM transfer fee
export const BASIS_POINTS = BigInt(10000);

/**
 * Applies an explicit maximum-loss tolerance to a quote. Callers must choose the tolerance; there
 * is intentionally no default that could silently turn slippage protection off.
 */
export function minimumOutputAfterSlippage(
  quotedOutput: bigint,
  maxLossBasisPoints: bigint,
) {
  assert(maxLossBasisPoints >= BigInt(0));
  assert(maxLossBasisPoints < BASIS_POINTS);

  return (quotedOutput * (BASIS_POINTS - maxLossBasisPoints)) / BASIS_POINTS;
}

/** Derive a subnet-pool share floor from the runtime's current exchange-rate preview. */
export async function minimumSubnetDelegateSharesOut(
  contract: any,
  subnetId: string,
  assets: bigint,
  maxLossBasisPoints: bigint,
) {
  const quotedShares = await contract.previewSubnetDelegateStakeDeposit(
    subnetId,
    assets,
  );
  return minimumOutputAfterSlippage(BigInt(quotedShares), maxLossBasisPoints);
}

/** Derive a validator-pool share floor from the runtime's current exchange-rate preview. */
export async function minimumValidatorDelegateSharesOut(
  contract: any,
  validatorId: string,
  assets: bigint,
  maxLossBasisPoints: bigint,
) {
  const quotedShares = await contract.previewValidatorDelegateStakeDeposit(
    validatorId,
    assets,
  );
  return minimumOutputAfterSlippage(BigInt(quotedShares), maxLossBasisPoints);
}

export function compareEthBalanceWithTxFee(balance1: bigint, balance2: bigint) {
  if (balance1 > balance2) {
    assert(balance1 - balance2 < MAX_TX_FEE);
  } else {
    assert(balance2 - balance1 < MAX_TX_FEE);
  }
}
