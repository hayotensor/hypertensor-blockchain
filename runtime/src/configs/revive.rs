//! Smart contracts share the native balances and accounts of this runtime.

use crate::*;
use frame_support::traits::ConstBool;
use frame_system::EnsureSigned;
use pallet_revive::evm::runtime::EthExtra;
use sp_runtime::FixedU128;

parameter_types! {
    /// EIP-155 chain ID of the selected runtime profile (see `CHAIN_ID` in lib.rs):
    /// the development ID by default, the owner-assigned ID with `--features production`.
    pub const ChainId: u64 = CHAIN_ID;
    pub const DepositPerItem: Balance = deposit(1, 0);
    pub const DepositPerByte: Balance = deposit(0, 1);
    pub const DepositPerChildTrieItem: Balance = tokenomics::POLICY.storage_child_item_deposit;
    pub const CodeHashLockupDepositPercent: Perbill = Perbill::from_percent(tokenomics::CODE_HASH_LOCKUP_PERCENT);
    // Leave room for Network's 50% hook budget and at least 10% for other work.
    // This scales the normal-class max extrinsic, not the entire block weight.
    pub const MaxEthExtrinsicWeight: FixedU128 = FixedU128::from_rational(3, 5);
}

impl pallet_revive::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeHoldReason = RuntimeHoldReason;
    type Time = Timestamp;
    type Balance = Balance;
    type Currency = Balances;
    type WeightInfo = pallet_revive::weights::SubstrateWeight<Self>;
    // Standard EVM and Revive system precompiles are always included by the pallet.
    type Precompiles = network_precompiles::Precompiles<Self>;
    type FindAuthor = <Runtime as pallet_authorship::Config>::FindAuthor;
    type AddressMapper = pallet_revive::AccountId32Mapper<Self>;
    type AllowEVMBytecode = ConstBool<true>;
    type UploadOrigin = EnsureSigned<AccountId>;
    type InstantiateOrigin = EnsureSigned<AccountId>;
    type DepositPerItem = DepositPerItem;
    type DepositPerByte = DepositPerByte;
    type DepositPerChildTrieItem = DepositPerChildTrieItem;
    type CodeHashLockupDepositPercent = CodeHashLockupDepositPercent;
    // Native TENSOR storage deposits; no asset-based deposit backend is needed.
    type Deposit = ();
    type OnBurn = ();
    type RuntimeMemory = ConstU32<{ 128 * 1024 * 1024 }>;
    type PVFMemory = ConstU32<{ 512 * 1024 * 1024 }>;
    type ChainId = ChainId;
    // Both TENSOR and Ethereum transaction values have 18 decimal places.
    type NativeToEthRatio = ConstU32<1>;
    type FeeInfo = pallet_revive::evm::fees::Info<Address, Signature, EthExtraImpl>;
    type GasScale = ConstU32<1000>;
    type MaxEthExtrinsicWeight = MaxEthExtrinsicWeight;
    type DebugEnabled = ConstBool<false>;
    // Native users register a refundable mapping once with Revive::map_account.
    // Ethereum-derived accounts do not need this registration.
    type AutoMap = ConstBool<false>;
}

/// Ethereum transactions use the same nonce, fee payment and weight checks as native calls.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EthExtraImpl;

impl EthExtra for EthExtraImpl {
    type Config = Runtime;
    type ExtensionV0 = TxExtension;
    type ExtensionOtherVersions = sp_runtime::traits::InvalidVersion;

    fn get_eth_extension(nonce: Nonce, tip: Balance) -> TxExtension {
        (
            frame_system::AuthorizeCall::<Runtime>::new(),
            frame_system::CheckNonZeroSender::<Runtime>::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckMortality::<Runtime>::from(generic::Era::Immortal),
            frame_system::CheckNonce::<Runtime>::from(nonce),
            frame_system::CheckWeight::<Runtime>::new(),
            pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
            pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::new_from_eth_transaction(),
            frame_system::WeightReclaim::<Runtime>::new(),
        )
    }

    /// Rejects unprotected (pre-EIP-155) legacy transactions, which carry no chain ID and
    /// would otherwise be replayable on every network. Upstream pallet-revive accepts them
    /// for Nick's-method deployments; this runtime does not. Everything else is unchanged.
    fn try_into_checked_extrinsic(
        payload: &[u8],
        encoded_len: usize,
    ) -> Result<
        generic::CheckedExtrinsic<
            AccountId,
            RuntimeCall,
            TxExtension,
            sp_runtime::traits::InvalidVersion,
        >,
        sp_runtime::transaction_validity::InvalidTransaction,
    > {
        use pallet_revive::evm::TransactionSigned;
        if let Ok(TransactionSigned::TransactionLegacySigned(tx)) =
            TransactionSigned::decode(payload)
        {
            if tx.transaction_legacy_unsigned.chain_id.is_none() {
                return Err(sp_runtime::transaction_validity::InvalidTransaction::Call);
            }
        }
        UpstreamEthExtra::try_into_checked_extrinsic(payload, encoded_len)
    }
}

/// Same as [`EthExtraImpl`] but with pallet-revive's default conversion, so the override above
/// can delegate to it after its check.
struct UpstreamEthExtra;

impl EthExtra for UpstreamEthExtra {
    type Config = Runtime;
    type ExtensionV0 = TxExtension;
    type ExtensionOtherVersions = sp_runtime::traits::InvalidVersion;

    fn get_eth_extension(nonce: Nonce, tip: Balance) -> TxExtension {
        EthExtraImpl::get_eth_extension(nonce, tip)
    }
}

impl network_precompiles::Config for Runtime {
    type PrecompileWeightInfo = network_precompiles::weights::SubstrateWeight<Self>;
    fn network_call(call: pallet_network::Call<Self>) -> RuntimeCall {
        RuntimeCall::Network(call)
    }
}
