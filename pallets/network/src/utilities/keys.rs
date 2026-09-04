// Copyright (C) Hypertensor.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use super::*;

const MIN_PEER_ID_LENGTH: usize = 32;
const MAX_PEER_ID_LENGTH: usize = 128;

impl<T: Config> Pallet<T> {
    // Loosely validates Node ID
    pub fn validate_peer_id(peer_id: &PeerId) -> bool {
        let peer_id_0 = &peer_id.0;
        let len = peer_id_0.len();

        if !(MIN_PEER_ID_LENGTH..=MAX_PEER_ID_LENGTH).contains(&len) {
            return false;
        }

        let first = peer_id_0[0];
        let second = peer_id_0[1];

        match (first, second) {
            // (ed25519, using the "identity" multihash) encoded as a raw base58btc multihash
            // '1' → base58btc identity multihash (ed25519)
            (b'1', _) => true,
            // (sha256) encoded as a raw base58btc multihash - 'Qm' → SHA256 base58 multihash
            (b'Q', b'm') => true,
            // (sha256) encoded as a CID - 'f', 'b', 'z', 'm' → CID/base multibase prefixes
            (b'f', _) | (b'b', _) | (b'z', _) | (b'm', _) => true,
            _ => false,
        }
    }
}
