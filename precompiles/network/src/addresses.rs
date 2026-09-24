//! The two final bytes are reserved by Revive for builtin precompiles.
use core::num::NonZeroU16;
use pallet_revive::precompiles::AddressMatcher;

pub const STAKING: u16 = 0x1001;
pub const VALIDATORS: u16 = 0x1002;
pub const SUBNETS: u16 = 0x1003;
pub const SUBNET_NODES: u16 = 0x1004;
pub const CONSENSUS: u16 = 0x1005;
pub const OVERWATCH: u16 = 0x1006;
/// Reserved only: no precompile is registered at this address.
pub const GOVERNANCE: u16 = 0x1007;

pub const fn matcher(id: u16) -> AddressMatcher {
    AddressMatcher::Fixed(NonZeroU16::new(id).expect("nonzero precompile ID"))
}
pub const fn address(id: u16) -> [u8; 20] {
    matcher(id).base_address()
}
