use crate::{mock::*, Error, Event, RewardAddressRecord, RewardAddresses, REWARD_ADDRESS_DOMAIN};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::Hooks};
use sp_core::{sr25519, Pair, H160, H256};

fn address(id: u64) -> H160 {
    H160::from_low_u64_be(id)
}
fn proof(key: &sr25519::Pair, dest: H160, nonce: u64, until: u32) -> sr25519::Signature {
    key.sign(&AuthorSubsidy::reward_address_payload(
        &key.public(),
        dest,
        nonce,
        until,
    ))
}
fn configure(key: &sr25519::Pair, dest: H160, nonce: u64) {
    assert_ok!(AuthorSubsidy::set_reward_address(
        RuntimeOrigin::signed(dest.into()),
        key.public(),
        dest,
        nonce,
        100,
        proof(key, dest, nonce, 100),
    ));
}

#[test]
fn configuration_requires_both_accounts_and_activates_next_block() {
    new_test_ext().execute_with(|| {
        let key = alice();
        configure(&key, address(1), 0);
        assert_eq!(AuthorSubsidy::next_nonce(&key.public()), 1);
        assert_eq!(AuthorSubsidy::reward_address_at(&key.public(), 1), None);
        System::assert_last_event(RuntimeEvent::AuthorSubsidy(Event::RewardAddressScheduled {
            aura_key: key.public(),
            reward_address: address(1),
            activation_block: 2,
            nonce: 0,
        }));
        assert_eq!(AuthorSubsidy::on_initialize(1), SKIPPED_SUBSIDY_WEIGHT);
        assert_eq!(Balances::total_issuance(), 0);
        System::set_block_number(2);
        assert_eq!(AuthorSubsidy::on_initialize(2), AUTHOR_SUBSIDY_WEIGHT);
        assert_eq!(
            Balances::free_balance(AccountId::from(address(1))),
            AUTHOR_BLOCK_EMISSIONS
        );
        assert_eq!(Balances::total_issuance(), AUTHOR_BLOCK_EMISSIONS);
        System::assert_last_event(RuntimeEvent::AuthorSubsidy(Event::AuthorSubsidy {
            who: address(1).into(),
            subsidy: AUTHOR_BLOCK_EMISSIONS,
        }));
    });
}

#[test]
fn rejects_wrong_origin_unknown_authority_zero_address_expiry_and_nonce() {
    new_test_ext().execute_with(|| {
        let key = alice();
        let dest = address(1);
        let sig = proof(&key, dest, 0, 100);
        for origin in [RuntimeOrigin::none(), RuntimeOrigin::root()] {
            assert_noop!(
                AuthorSubsidy::set_reward_address(origin, key.public(), dest, 0, 100, sig),
                sp_runtime::DispatchError::BadOrigin
            );
        }
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(address(2).into()),
                key.public(),
                dest,
                0,
                100,
                sig
            ),
            Error::<Test>::WrongRewardAccount
        );
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                key.public(),
                H160::zero(),
                0,
                100,
                sig
            ),
            Error::<Test>::ZeroRewardAddress
        );
        Authorities::set(&vec![bob().public()]);
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                key.public(),
                dest,
                0,
                100,
                sig
            ),
            Error::<Test>::UnknownAuraAuthority
        );
        Authorities::set(&vec![key.public()]);
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                key.public(),
                dest,
                1,
                100,
                sig
            ),
            Error::<Test>::InvalidNonce
        );
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                key.public(),
                dest,
                0,
                0,
                sig
            ),
            Error::<Test>::ExpiredProof
        );
        assert_eq!(AuthorSubsidy::next_nonce(&key.public()), 0);
        assert!(RewardAddresses::<Test>::get(key.public()).is_none());
    });
}

#[test]
fn aura_proof_binds_every_payload_field_and_domain() {
    new_test_ext().execute_with(|| {
        let key = alice();
        let dest = address(1);
        let payload = AuthorSubsidy::reward_address_payload(&key.public(), dest, 0, 100);
        assert_eq!(
            payload,
            (
                REWARD_ADDRESS_DOMAIN.to_vec(),
                H256::repeat_byte(42),
                key.public(),
                dest,
                0u64,
                100u32
            )
                .encode()
        );
        let sig = key.sign(&payload);
        // Both destination and extrinsic signer change: EVM authorization alone is insufficient.
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(address(2).into()),
                key.public(),
                address(2),
                0,
                100,
                sig
            ),
            Error::<Test>::InvalidAuraSignature
        );
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                bob().public(),
                dest,
                0,
                100,
                sig
            ),
            Error::<Test>::InvalidAuraSignature
        );
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                key.public(),
                dest,
                0,
                101,
                sig
            ),
            Error::<Test>::InvalidAuraSignature
        );
        let wrong_nonce_sig = proof(&key, dest, 1, 100);
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                key.public(),
                dest,
                0,
                100,
                wrong_nonce_sig
            ),
            Error::<Test>::InvalidAuraSignature
        );
        let wrong_domain = (
            b"other-pallet".to_vec(),
            H256::repeat_byte(42),
            key.public(),
            dest,
            0u64,
            100u32,
        )
            .encode();
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                key.public(),
                dest,
                0,
                100,
                key.sign(&wrong_domain)
            ),
            Error::<Test>::InvalidAuraSignature
        );
        frame_system::BlockHash::<Test>::insert(0, H256::repeat_byte(43));
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                key.public(),
                dest,
                0,
                100,
                sig
            ),
            Error::<Test>::InvalidAuraSignature
        );
        assert_eq!(AuthorSubsidy::next_nonce(&key.public()), 0);
    });
}

#[test]
fn updates_preserve_current_block_and_replays_cannot_restore_old_addresses() {
    new_test_ext().execute_with(|| {
        let key = alice();
        configure(&key, address(1), 0);
        configure(&key, address(2), 1);
        assert_eq!(AuthorSubsidy::reward_address_at(&key.public(), 1), None);
        assert_eq!(
            AuthorSubsidy::reward_address_at(&key.public(), 2),
            Some(address(2))
        );
        System::set_block_number(2);
        configure(&key, address(3), 2);
        configure(&key, address(4), 3);
        assert_eq!(
            AuthorSubsidy::reward_address_at(&key.public(), 2),
            Some(address(2))
        );
        assert_eq!(
            AuthorSubsidy::reward_address_at(&key.public(), 3),
            Some(address(4))
        );
        let original = proof(&key, address(1), 0, 100);
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(address(1).into()),
                key.public(),
                address(1),
                0,
                100,
                original
            ),
            Error::<Test>::InvalidNonce
        );
        AuthorSubsidy::on_initialize(2);
        System::set_block_number(3);
        AuthorSubsidy::on_initialize(3);
        assert_eq!(
            Balances::free_balance(AccountId::from(address(2))),
            AUTHOR_BLOCK_EMISSIONS
        );
        assert_eq!(
            Balances::free_balance(AccountId::from(address(4))),
            AUTHOR_BLOCK_EMISSIONS
        );
        assert_eq!(Balances::free_balance(AccountId::from(address(1))), 0);
        assert_eq!(Balances::free_balance(AccountId::from(address(3))), 0);
    });
}

#[test]
fn missing_author_or_mapping_never_mints_and_removed_authorities_do_not_receive_rewards() {
    new_test_ext().execute_with(|| {
        assert_eq!(AuthorSubsidy::on_initialize(1), SKIPPED_SUBSIDY_WEIGHT);
        assert!(System::events().is_empty());
        configure(&alice(), address(1), 0);
        System::set_block_number(2);
        System::initialize(&2, &System::parent_hash(), &Default::default());
        AuthorSubsidy::on_initialize(2);
        author_digest(alice().public());
        Authorities::set(&vec![bob().public()]);
        AuthorSubsidy::on_initialize(2);
        assert_eq!(Balances::total_issuance(), 0);
        assert_eq!(Balances::free_balance(AccountId::from(H160::zero())), 0);
        // Keep the nonce even when an authority temporarily leaves the set.
        assert_eq!(AuthorSubsidy::next_nonce(&alice().public()), 1);
    });
}

#[test]
fn independent_authorities_can_choose_the_same_wallet_and_expiry_is_inclusive() {
    new_test_ext().execute_with(|| {
        for key in [alice(), bob()] {
            assert_ok!(AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(address(1).into()),
                key.public(),
                address(1),
                0,
                1,
                proof(&key, address(1), 0, 1)
            ));
        }
        System::set_block_number(2);
        AuthorSubsidy::on_initialize(2);
        author_digest(bob().public());
        System::set_block_number(3);
        AuthorSubsidy::on_initialize(3);
        assert_eq!(
            Balances::free_balance(AccountId::from(address(1))),
            2 * AUTHOR_BLOCK_EMISSIONS
        );
    });
}

#[test]
fn arithmetic_overflows_fail_without_mutating_the_record() {
    new_test_ext().execute_with(|| {
        let key = alice();
        let dest = address(1);
        RewardAddresses::<Test>::insert(
            key.public(),
            RewardAddressRecord {
                current_address: None,
                pending_address: dest,
                activation_block: 1,
                next_nonce: u64::MAX,
            },
        );
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                key.public(),
                dest,
                u64::MAX,
                100,
                proof(&key, dest, u64::MAX, 100)
            ),
            Error::<Test>::NonceOverflow
        );
        RewardAddresses::<Test>::remove(key.public());
        System::set_block_number(u32::MAX);
        assert_noop!(
            AuthorSubsidy::set_reward_address(
                RuntimeOrigin::signed(dest.into()),
                key.public(),
                dest,
                0,
                u32::MAX,
                proof(&key, dest, 0, u32::MAX)
            ),
            Error::<Test>::BlockNumberOverflow
        );
    });
}
