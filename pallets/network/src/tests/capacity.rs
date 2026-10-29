//! Deterministic admission-weight arithmetic, not a timing benchmark.
//! Run with `cargo test --release -p pallet-network capacity_weight_report -- --ignored --nocapture`.
use crate::weights::WeightInfo;
use frame_support::weights::{constants::RocksDbWeight, Weight};

type W = ();

fn mandatory_slot(h: u32) -> Weight {
    let db = RocksDbWeight::get();
    let accepted = W::emission_step(h)
        .saturating_add(
            W::emission_step_accepted_queue_mutations(64)
                .max(W::emission_step_accepted_queue_mutations_front(64)),
        )
        .saturating_add(W::emission_step_accepted_below_min_weight_reputation(h))
        .saturating_add(if h > 3 {
            W::emission_step_accepted_non_attestor_reputation(h - 3)
        } else {
            Weight::zero()
        });
    let settlement = accepted
        .max(W::emission_step_rejected(h))
        .max(W::emission_step_emergency(h.max(64)))
        .max(W::emission_step_missing())
        .saturating_add(db.reads_writes(3, 3))
        .saturating_add(Weight::from_parts(0, 8192 + 4 * (512 + 64)));
    let election = W::elect_validator(h)
        .max(W::elect_validator_emergency(64))
        .max(W::elect_validator_expired(h, 64));
    W::on_initialize_base()
        .saturating_add(W::advance_overwatch_epoch_noop())
        .saturating_add(W::emission_slot_selector())
        .saturating_add(W::emission_step_selectors())
        .saturating_add(settlement)
        .saturating_add(db.reads(2))
        .saturating_add(election)
}

#[test]
fn proposal_summary_weight_report() {
    let limit = Weight::from_parts(1_000_000_000_000, 16 * 1024 * 1024);
    for n in (64..=512).step_by(64) {
        let slot = mandatory_slot(n);
        assert!(slot.all_lte(limit));
        println!("proposal_summary_weight,mandatory_slot,{n},{},{}", slot.ref_time(), slot.proof_size());
    }
    let scans = W::pending_admitted_removal_scan(512).saturating_add(W::pending_queued_removal_scan(64));
    let cleanup = W::remove_admitted_subnet_node_dispatch_small(512, 64)
        .max(W::remove_admitted_subnet_node_dispatch_large(512, 512));
    for (name, weight) in [
        ("proposal_dispatch", W::propose_attestation().max(W::propose_attestation_emergency())
            .saturating_add(scans.saturating_mul(3)).saturating_add(cleanup)
            .saturating_add(RocksDbWeight::get().writes(2))),
        ("attest_dispatch", W::attest().saturating_add(scans.saturating_mul(2))),
    ] {
        println!("proposal_summary_weight,{name},512,{},{}", weight.ref_time(), weight.proof_size());
    }
}

#[test]
#[ignore = "research output; extrapolated rows are not supported protocol limits"]
fn capacity_weight_report() {
    let limit = Weight::from_parts(1_000_000_000_000, 16 * 1024 * 1024);
    println!("capacity_model,kind,n,ref_time_ps,proof_bytes");
    for n in [
        3, 64, 128, 256, 384, 512, 640, 768, 800, 896, 1024, 1536, 2048,
    ] {
        let slot = mandatory_slot(n);
        println!(
            "capacity_model,mandatory_slot,{n},{},{}",
            slot.ref_time(),
            slot.proof_size()
        );
    }
    assert!(mandatory_slot(512).all_lte(limit));
    for (name, limit) in [
        ("time", Weight::from_parts(limit.ref_time(), u64::MAX)),
        ("proof", Weight::from_parts(u64::MAX, limit.proof_size())),
    ] {
        let largest = (3..10_000)
            .take_while(|n| mandatory_slot(*n).all_lte(limit))
            .last()
            .unwrap();
        println!("capacity_model,extrapolated_{name}_ceiling,{largest},0,0");
    }
    let scans =
        W::pending_admitted_removal_scan(512).saturating_add(W::pending_queued_removal_scan(64));
    let active_cleanup = W::remove_admitted_subnet_node_dispatch_small(512, 64)
        .max(W::remove_admitted_subnet_node_dispatch_large(512, 512));
    let proposal = W::propose_attestation()
        .max(W::propose_attestation_emergency())
        .saturating_add(scans.saturating_mul(3))
        .saturating_add(active_cleanup)
        .saturating_add(RocksDbWeight::get().writes(2));
    let attest = W::attest().saturating_add(scans.saturating_mul(2));
    for (name, weight) in [
        ("proposal_dispatch", proposal),
        ("attest_dispatch", attest),
        ("hook_limit", limit),
    ] {
        println!(
            "capacity_model,{name},512,{},{}",
            weight.ref_time(),
            weight.proof_size()
        );
    }
}
