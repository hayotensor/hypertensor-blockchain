//! Defer ABI parsing until the precompile can reserve its decoding weight.
//!
//! Revive calls `Interface::abi_decode_validate` before `Precompile::call`. Using the generated
//! interface there would decode attacker-controlled arrays before our first gas charge. This
//! envelope only copies the wire bytes; typed parsing happens in `decode`, after charging.
use crate::{convert::revert, dispatch::check_context, weights::WeightInfo, Config};
use alloc::vec::Vec;
use core::marker::PhantomData;
use pallet_revive::precompiles::{
    alloy::sol_types::{self, abi::AbiDecoderConfig, SolInterface},
    Error, Ext,
};

pub struct MeteredInput<I> {
    wire: Vec<u8>,
    interface: PhantomData<I>,
}

impl<I: SolInterface> MeteredInput<I> {
    pub fn decode<T: Config>(&self, env: &mut impl Ext<T = T>) -> Result<I, Error> {
        env.charge(T::PrecompileWeightInfo::entry())?;
        let len = u32::try_from(self.wire.len()).map_err(|_| revert("Network: input too large"))?;
        env.charge(T::PrecompileWeightInfo::input(len))?;
        check_context(env, false)?;
        self.decode_charged()
    }

    /// Only call after reserving entry/input weight (benchmarks also use this exact parser).
    pub(crate) fn decode_charged(&self) -> Result<I, Error> {
        if self.wire.len() < 4 {
            return Err(revert("Network: malformed ABI"));
        }
        if !I::valid_selector(self.selector()) {
            return Err(revert("Network: unknown selector"));
        }
        // Canonical offsets prevent repeated/overlapping tails from amplifying work. Bound token
        // allocation to the paid-for wire size, rather than Alloy's default 1 GiB allowance.
        let config = AbiDecoderConfig::new()
            .strict(true)
            .memory_limit(self.wire.len().saturating_mul(4));
        I::abi_decode_with_config(&self.wire, config).map_err(|error| match error {
            sol_types::Error::MemoryLimitExceeded(_) | sol_types::Error::Reserve(_) => {
                sp_runtime::DispatchError::Exhausted.into()
            }
            _ => revert("Network: malformed ABI"),
        })
    }
}

// This is a transport envelope, not a second ABI. Selector metadata and encoding retain the
// generated interface's wire format. Deliberately accept malformed inputs here so they also pay
// for validation inside `Precompile::call` and return the same errors in every domain.
impl<I: SolInterface> SolInterface for MeteredInput<I> {
    const NAME: &'static str = I::NAME;
    const MIN_DATA_LENGTH: usize = 0;
    const COUNT: usize = I::COUNT;

    fn selector(&self) -> [u8; 4] {
        self.wire
            .get(..4)
            .and_then(|s| s.try_into().ok())
            .unwrap_or_default()
    }
    fn selector_at(i: usize) -> Option<[u8; 4]> {
        I::selector_at(i)
    }
    fn valid_selector(selector: [u8; 4]) -> bool {
        I::valid_selector(selector)
    }
    fn abi_decode_raw(selector: [u8; 4], data: &[u8]) -> sol_types::Result<Self> {
        let mut wire = Vec::with_capacity(4 + data.len());
        wire.extend_from_slice(&selector);
        wire.extend_from_slice(data);
        Ok(Self {
            wire,
            interface: PhantomData,
        })
    }
    fn abi_decode_raw_with_config(
        selector: [u8; 4],
        data: &[u8],
        _config: AbiDecoderConfig,
    ) -> sol_types::Result<Self> {
        Self::abi_decode_raw(selector, data)
    }
    fn abi_decode_with_config(data: &[u8], _config: AbiDecoderConfig) -> sol_types::Result<Self> {
        Ok(Self {
            wire: data.to_vec(),
            interface: PhantomData,
        })
    }
    fn abi_encoded_size(&self) -> usize {
        self.wire.len().saturating_sub(4)
    }
    fn abi_encode_raw(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self.wire.get(4..).unwrap_or_default());
    }
}
