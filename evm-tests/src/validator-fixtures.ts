import { ApiPromise } from "@polkadot/api";
import { Option } from "@polkadot/types";
import { Contract, Wallet } from "ethers";
import { registerValidator } from "./network";

export type InitialValidator = {
  validatorId: string;
  count: number;
};

/** Register coldkeys as canonical validators and return the subnet-registration ABI tuples. */
export async function registerCanonicalValidators(
  contract: Contract,
  coldkeys: Wallet[],
  api: ApiPromise,
): Promise<InitialValidator[]> {
  const initialValidators: InitialValidator[] = [];

  for (const coldkey of coldkeys) {
    const validatorContract = contract.connect(coldkey) as Contract;
    await registerValidator(validatorContract, Wallet.createRandom().address);

    const validatorId = (await api.query.network.coldkeyValidatorId(
      coldkey.address,
    )) as Option<any>;
    if (validatorId.isNone) {
      throw new Error(
        `Canonical validator ID was not created for ${coldkey.address}`,
      );
    }

    initialValidators.push({
      validatorId: validatorId.unwrap().toString(),
      count: 1,
    });
  }

  return initialValidators;
}
