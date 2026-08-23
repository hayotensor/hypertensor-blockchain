// Copyright (C) Hypertensor.
// SPDX-License-Identifier: Apache-2.0

use crate::{mock::*, Event};
use frame_support::traits::Hooks;
use pallet_evm::AddressMapping;

#[test]
fn on_initialize_credits_the_author_emits_an_event_and_reports_its_weight() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let who = <Test as crate::Config>::AddressMapping::into_account_id(mock_author());
        let issuance_before = Balances::total_issuance();

        let weight = AuthorSubsidy::on_initialize(1);

        assert_eq!(weight, AUTHOR_SUBSIDY_WEIGHT);
        assert_eq!(Balances::free_balance(&who), AUTHOR_BLOCK_EMISSIONS);
        assert_eq!(
            Balances::total_issuance(),
            issuance_before + AUTHOR_BLOCK_EMISSIONS
        );
        System::assert_last_event(RuntimeEvent::AuthorSubsidy(Event::AuthorSubsidy {
            who,
            subsidy: AUTHOR_BLOCK_EMISSIONS,
        }));
    });
}
