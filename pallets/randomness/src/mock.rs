// Copyright (C) Hypertensor.
// SPDX-License-Identifier: Apache-2.0

use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU32, ConstU64, OnFinalize, OnInitialize},
};
use sp_consensus_babe::{
    digests::{PreDigest, PrimaryPreDigest, SecondaryPlainPreDigest, SecondaryVRFPreDigest},
    AuthorityPair, Slot,
};
use sp_core::crypto::{Pair, VrfPublic, VrfSecret, Wraps};
use sp_runtime::{BuildStorage, Digest, DigestItem};

use codec::Encode;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Babe: pallet_babe,
        Randomness: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = Babe;
    type MinimumPeriod = ConstU64<1>;
    type WeightInfo = ();
}

pub const EPOCH_DURATION: u64 = 4;

parameter_types! {
    pub static BabeEpochDuration: u64 = EPOCH_DURATION;
}

impl pallet_babe::Config for Test {
    type EpochDuration = BabeEpochDuration;
    type ExpectedBlockTime = ConstU64<2>;
    type EpochChangeTrigger = pallet_babe::SameAuthoritiesForever;
    type DisabledValidators = ();
    type WeightInfo = ();
    type MaxAuthorities = ConstU32<2>;
    type MaxNominators = ConstU32<0>;
    type KeyOwnerProof = sp_session::MembershipProof;
    type EquivocationReportSystem = ();
}

impl crate::Config for Test {}

pub fn new_test_ext() -> sp_io::TestExternalities {
    BabeEpochDuration::set(EPOCH_DURATION);
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    pallet_babe::GenesisConfig::<Test> {
        authorities: vec![(authority().public(), 1)],
        epoch_config: sp_consensus_babe::BabeEpochConfiguration {
            c: (1, 4),
            allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
        },
        ..Default::default()
    }
    .assimilate_storage(&mut storage)
    .unwrap();
    storage.into()
}

fn authority() -> AuthorityPair {
    AuthorityPair::from_seed(&[7; 32])
}

/// Execute BABE hooks using a real sr25519 VRF signature in every primary digest.
/// The mock does not simulate the client's slot lottery, seals, or GRANDPA.
pub fn run_to_block(target: u64) {
    for number in System::block_number() + 1..=target {
        run_block(number, number);
    }
}

pub fn run_block(number: u64, slot: u64) {
    run_block_with_kind(number, slot, BlockKind::Primary);
}

#[derive(Clone, Copy)]
pub enum BlockKind {
    Primary,
    SecondaryPlain,
    SecondaryVrf,
}

pub fn run_block_with_kind(number: u64, slot: u64, kind: BlockKind) {
    let pre_digest = match kind {
        BlockKind::SecondaryPlain => PreDigest::SecondaryPlain(SecondaryPlainPreDigest {
            authority_index: 0,
            slot: slot.into(),
        }),
        BlockKind::Primary | BlockKind::SecondaryVrf => vrf_digest(slot, kind),
    };
    System::initialize(
        &number,
        &Default::default(),
        &Digest {
            logs: vec![DigestItem::PreRuntime(
                sp_consensus_babe::BABE_ENGINE_ID,
                pre_digest.encode(),
            )],
        },
    );
    Babe::on_initialize(number);
    Babe::on_finalize(number);
    System::finalize();
}

fn vrf_digest(slot: u64, kind: BlockKind) -> PreDigest {
    let epoch = (slot - 1) / BabeEpochDuration::get();
    // On an epoch boundary, BABE promotes NextRandomness before processing the
    // VRF in on_finalize. Sign using the seed for the newly active epoch.
    let seed = if epoch == Babe::epoch_index() {
        pallet_babe::Randomness::<Test>::get()
    } else {
        pallet_babe::NextRandomness::<Test>::get()
    };
    let transcript = sp_consensus_babe::make_vrf_transcript(&seed, Slot::from(slot), epoch);
    let sign_data = transcript.into_sign_data();
    let signature = authority().as_ref().vrf_sign(&sign_data);
    assert!(authority()
        .public()
        .as_inner_ref()
        .vrf_verify(&sign_data, &signature));
    match kind {
        BlockKind::Primary => PreDigest::Primary(PrimaryPreDigest {
            authority_index: 0,
            slot: slot.into(),
            vrf_signature: signature,
        }),
        BlockKind::SecondaryVrf => PreDigest::SecondaryVRF(SecondaryVRFPreDigest {
            authority_index: 0,
            slot: slot.into(),
            vrf_signature: signature,
        }),
        BlockKind::SecondaryPlain => unreachable!("plain digests do not have a VRF signature"),
    }
}
