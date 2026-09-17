//! BABE digest construction for Frontier's hypothetical pending block.
//! No seal or VRF is fabricated: use the configured plain secondary slot author.
use std::sync::Arc;

use scale_codec::Encode;
use sp_api::ProvideRuntimeApi;
use sp_consensus_babe::{
    digests::{PreDigest, SecondaryPlainPreDigest},
    BabeApi, Slot, BABE_ENGINE_ID,
};
use sp_core::U256;
use sp_inherents::InherentData;
use sp_runtime::{
    generic::{Digest, DigestItem},
    traits::{Block as BlockT, Header},
};

pub struct BabeConsensusDataProvider<C> {
    client: Arc<C>,
}

impl<C> BabeConsensusDataProvider<C> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client }
    }
}

impl<B, C> fc_rpc::pending::ConsensusDataProvider<B> for BabeConsensusDataProvider<C>
where
    B: BlockT,
    C: ProvideRuntimeApi<B> + Send + Sync,
    C::Api: BabeApi<B>,
{
    fn create_digest(
        &self,
        parent: &B::Header,
        data: &InherentData,
    ) -> Result<Digest, sp_inherents::Error> {
        let slot: Slot = data
            .get_data(&sp_consensus_babe::inherents::INHERENT_IDENTIFIER)?
            .ok_or_else(|| sp_inherents::Error::Application("missing BABE pending slot".into()))?;
        let api = self.client.runtime_api();
        let mut epoch = api
            .current_epoch(parent.hash())
            .map_err(|e| sp_inherents::Error::Application(Box::new(e)))?;
        // The first block starts epoch zero at its timestamp, even after a delay.
        // After genesis, an epoch boundary (including skipped epochs) uses the
        // next announced authorities/randomness, just as the BABE client does.
        if *parent.number() != Default::default() && slot >= epoch.start_slot + epoch.duration {
            epoch = api
                .next_epoch(parent.hash())
                .map_err(|e| sp_inherents::Error::Application(Box::new(e)))?;
        }
        if epoch.authorities.is_empty() {
            return Err(sp_inherents::Error::Application(
                "empty BABE pending authority set".into(),
            ));
        }
        let hash = (epoch.randomness, slot).using_encoded(sp_core::blake2_256);
        let authority_index =
            (U256::from_big_endian(&hash) % U256::from(epoch.authorities.len())).as_u32();
        Ok(Digest {
            logs: vec![DigestItem::PreRuntime(
                BABE_ENGINE_ID,
                PreDigest::SecondaryPlain(SecondaryPlainPreDigest {
                    authority_index,
                    slot,
                })
                .encode(),
            )],
        })
    }
}
