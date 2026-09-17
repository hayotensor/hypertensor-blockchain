use crate as pallet_author_subsidy;
use crate::*;
use frame_support::{derive_impl, parameter_types, traits::Everything, ConsensusEngineId};
use sp_core::{sr25519, ConstU128, Pair, H256};
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

pub type AccountId = fp_account::AccountId20;
type Block = frame_system::mocking::MockBlockU32<Test>;
frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        AuthorSubsidy: pallet_author_subsidy,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountData = pallet_balances::AccountData<u128>;
    type BaseCallFilter = Everything;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type ExistentialDeposit = ConstU128<500>;
    type AccountStore = System;
}

pub const AUTHOR_BLOCK_EMISSIONS: u128 = 1_000_000_000_000_000_000_000 / 5_256_000;
pub const AUTHOR_SUBSIDY_WEIGHT: Weight = Weight::from_parts(123_456, 789);
pub const SKIPPED_SUBSIDY_WEIGHT: Weight = Weight::from_parts(12_345, 678);
parameter_types! {
    pub const AuthorBlockEmissions: u128 = AUTHOR_BLOCK_EMISSIONS;
    pub storage Authorities: Vec<sr25519::Public> = Vec::new();
}
pub struct IsAuthority;
impl Contains<sr25519::Public> for IsAuthority {
    fn contains(key: &sr25519::Public) -> bool {
        Authorities::get().contains(key)
    }
}
pub struct MockFindAuthor;
impl FindAuthor<H160> for MockFindAuthor {
    fn find_author<'a, I>(digests: I) -> Option<H160>
    where
        I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
    {
        for (engine, mut data) in digests {
            if engine == *b"babe" {
                let key = sr25519::Public::decode(&mut data).ok()?;
                if !IsAuthority::contains(&key) {
                    return None;
                }
                return AuthorSubsidy::reward_address_at(&key, System::block_number());
            }
        }
        None
    }
}
pub struct TestWeightInfo;
impl WeightInfo for TestWeightInfo {
    fn on_initialize() -> Weight {
        AUTHOR_SUBSIDY_WEIGHT
    }
    fn on_initialize_skipped() -> Weight {
        SKIPPED_SUBSIDY_WEIGHT
    }
    fn set_reward_address() -> Weight {
        Weight::from_parts(1_000_000, 1_000)
    }
    fn update_reward_address() -> Weight {
        Self::set_reward_address()
    }
}
impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type FindAuthor = MockFindAuthor;
    type AddressMapping = pallet_evm::IdentityAddressMapping;
    type IsBabeAuthority = IsAuthority;
    type WeightInfo = TestWeightInfo;
    type AuthorBlockEmissions = AuthorBlockEmissions;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = MockBenchmarkHelper;
}

pub fn alice() -> sr25519::Pair {
    sr25519::Pair::from_string("//Alice", None).unwrap()
}
pub fn bob() -> sr25519::Pair {
    sr25519::Pair::from_string("//Bob", None).unwrap()
}
pub fn author_digest(key: sr25519::Public) {
    System::initialize(
        &System::block_number(),
        &System::block_hash(System::block_number().saturating_sub(1)),
        &sp_runtime::generic::Digest {
            logs: vec![sp_runtime::generic::DigestItem::PreRuntime(
                *b"babe",
                key.encode(),
            )],
        },
    );
}
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut ext: sp_io::TestExternalities = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into();
    #[cfg(feature = "runtime-benchmarks")]
    ext.register_extension(sp_keystore::KeystoreExt::new(
        sp_keystore::testing::MemoryKeystore::new(),
    ));
    ext.execute_with(|| {
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(0, H256::repeat_byte(42));
        Authorities::set(&vec![alice().public(), bob().public()]);
        author_digest(alice().public());
    });
    ext
}
#[cfg(feature = "runtime-benchmarks")]
pub struct MockBenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl BenchmarkHelper for MockBenchmarkHelper {
    fn setup_author(authority: sr25519::Public) {
        Authorities::set(&vec![authority]);
        author_digest(authority);
    }
}
