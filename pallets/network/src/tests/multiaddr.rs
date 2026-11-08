extern crate alloc;

use crate::multiaddr::{
    encode_varint, verify, MultiaddrError, DNS4, DNS6, DNSADDR, IP4, IP6, MAX_MULTIADDR_BYTES, P2P,
    TCP, UDP, WS, WSS,
};
use alloc::{vec, vec::Vec};

fn host(protocol: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_varint(protocol, &mut bytes);
    if matches!(protocol, DNS4 | DNS6 | DNSADDR) {
        encode_varint(payload.len() as u64, &mut bytes);
    }
    bytes.extend_from_slice(payload);
    bytes
}

fn address(host: &[u8], transport: Option<u64>, websocket: Option<u64>, peer: &[u8]) -> Vec<u8> {
    let mut bytes = host.to_vec();
    if let Some(protocol) = transport {
        encode_varint(protocol, &mut bytes);
        bytes.extend_from_slice(&30303u16.to_be_bytes());
    }
    if let Some(protocol) = websocket {
        encode_varint(protocol, &mut bytes);
    }
    encode_varint(P2P, &mut bytes);
    encode_varint(peer.len() as u64, &mut bytes);
    bytes.extend_from_slice(peer);
    bytes
}

fn hosts() -> [Vec<u8>; 5] {
    [
        host(IP4, &[127, 0, 0, 1]),
        host(IP6, &[0; 16]),
        host(DNS4, b"node.example.com"),
        host(DNS6, b"node.example.com"),
        host(DNSADDR, b"bootstrap.example.com"),
    ]
}

#[test]
fn accepts_all_supported_shapes() {
    for host in hosts() {
        for transport in [None, Some(TCP), Some(UDP)] {
            for websocket in [None, Some(WS), Some(WSS)] {
                let bytes = address(&host, transport, websocket, &[1; 32]);
                assert_eq!(verify(&bytes), Ok(()), "{bytes:?}");
            }
        }
    }
}

#[test]
fn accepts_standard_udp_wire_code_and_port_extremes() {
    // Literal wire bytes keep this independent of the encoder/constants.
    assert_eq!(
        verify(&[4, 127, 0, 0, 1, 0x91, 2, 0, 0, 0xa5, 3, 1, 42]),
        Ok(())
    );
    assert_eq!(
        verify(&[4, 127, 0, 0, 1, 6, 255, 255, 0xa5, 3, 1, 42]),
        Ok(())
    );
    assert_eq!(
        verify(&[4, 127, 0, 0, 1, 17, 0, 0, 0xa5, 3, 1, 42]),
        Err(MultiaddrError::InvalidProtocol)
    );
}

#[test]
fn payloads_are_opaque_and_not_scanned_for_protocols() {
    let payload = [0xff, 0, 0xa5, 3, 0xdd, 3];
    assert_eq!(
        verify(&address(&host(DNS4, &payload), None, None, &payload)),
        Ok(())
    );
}

#[test]
fn accepts_one_and_two_byte_payload_lengths() {
    for len in [1, 32, 38, 127, 128, 255] {
        let payload = vec![42; len];
        assert_eq!(
            verify(&address(
                &host(DNS4, &payload),
                Some(TCP),
                Some(WS),
                &payload
            )),
            Ok(())
        );
    }
}

#[test]
fn bounds_the_entire_address_including_direct_calls() {
    let bytes = address(&host(DNS4, &[b'a'; 889]), None, None, &[42; 128]);
    assert_eq!(bytes.len(), MAX_MULTIADDR_BYTES);
    assert_eq!(verify(&bytes), Ok(()));
    let oversized = address(&host(DNS4, &[b'a'; 890]), None, None, &[42; 128]);
    assert_eq!(oversized.len(), MAX_MULTIADDR_BYTES + 1);
    assert_eq!(verify(&oversized), Err(MultiaddrError::InvalidAddress));
    assert_eq!(
        verify(&[0xff; MAX_MULTIADDR_BYTES + 1]),
        Err(MultiaddrError::InvalidAddress)
    );
}

#[test]
fn rejects_empty_host_or_peer_payloads() {
    for protocol in [DNS4, DNS6, DNSADDR] {
        assert_eq!(
            verify(&address(&host(protocol, b""), None, None, &[42])),
            Err(MultiaddrError::InvalidAddress)
        );
    }
    assert_eq!(
        verify(&address(&host(IP4, &[127, 0, 0, 1]), None, None, &[])),
        Err(MultiaddrError::InvalidAddress)
    );
}

#[test]
fn rejects_missing_or_unsupported_protocols() {
    for bytes in [
        vec![],
        vec![0],
        vec![99],
        vec![6, 0, 80],
        vec![0xa5, 3, 1, 42],
        vec![4, 127, 0, 0, 1],
        vec![4, 127, 0, 0, 1, 6, 0, 80],
    ] {
        assert_eq!(
            verify(&bytes),
            Err(MultiaddrError::InvalidProtocol),
            "{bytes:?}"
        );
    }
}

#[test]
fn rejects_repeated_reordered_or_trailing_components() {
    let host = host(IP4, &[127, 0, 0, 1]);
    let peer = [0xa5, 3, 1, 42];
    for middle in [
        vec![4, 127, 0, 0, 1],          // second host
        vec![6, 0, 80, 6, 0, 80],       // repeated transport
        vec![6, 0, 80, 0x91, 2, 0, 80], // both transports
        vec![0xdd, 3, 0xde, 3],         // both WebSocket markers
        vec![0xdd, 3, 6, 0, 80],        // transport after WebSocket
        peer.to_vec(),                  // peer before final peer
        vec![99],                       // unknown protocol
    ] {
        let bytes = [host.as_slice(), middle.as_slice(), &peer].concat();
        assert_eq!(
            verify(&bytes),
            Err(MultiaddrError::InvalidProtocol),
            "{bytes:?}"
        );
    }
    let valid = address(&host, Some(TCP), Some(WSS), &[42]);
    for suffix in [vec![0], peer.to_vec(), valid.clone()] {
        assert_eq!(
            verify(&[valid.as_slice(), suffix.as_slice()].concat()),
            Err(MultiaddrError::InvalidProtocol)
        );
    }
    let repeated = [0xdd, 3].repeat(MAX_MULTIADDR_BYTES / 2);
    assert!(verify(&repeated).is_err());
}

#[test]
fn rejects_truncated_fixed_and_variable_payloads() {
    for bytes in [
        vec![4, 127, 0, 0],
        vec![41, 0],
        vec![4, 127, 0, 0, 1, 6],
        vec![4, 127, 0, 0, 1, 6, 0],
        vec![4, 127, 0, 0, 1, 0x91, 2, 0],
        vec![54, 100, b'a'],
        vec![55, 100, b'a'],
        vec![56, 100, b'a'],
        vec![4, 127, 0, 0, 1, 0xa5, 3, 32, 42],
    ] {
        assert_eq!(verify(&bytes), Err(MultiaddrError::Truncated), "{bytes:?}");
    }
}

#[test]
fn rejects_every_truncated_prefix_of_valid_addresses() {
    for host in hosts() {
        for transport in [None, Some(TCP), Some(UDP)] {
            for websocket in [None, Some(WS), Some(WSS)] {
                let bytes = address(&host, transport, websocket, &[42; 128]);
                for end in 0..bytes.len() {
                    assert!(verify(&bytes[..end]).is_err(), "{bytes:?}, cut at {end}");
                }
            }
        }
    }
}

#[test]
fn rejects_incomplete_noncanonical_and_overlong_varints() {
    for malformed in [
        vec![],
        vec![128],
        vec![128, 0],
        vec![132, 0],
        vec![128, 128, 1],
        vec![255; 10],
    ] {
        // Check host protocol, DNS length, transport protocol, and peer length.
        for prefix in [
            vec![],
            vec![54],
            vec![4, 127, 0, 0, 1],
            vec![4, 127, 0, 0, 1, 0xa5, 3],
        ] {
            let bytes = [prefix.as_slice(), malformed.as_slice()].concat();
            let error = if malformed.is_empty() && (prefix.is_empty() || prefix.len() == 5) {
                MultiaddrError::InvalidProtocol
            } else {
                MultiaddrError::InvalidVarint
            };
            assert_eq!(verify(&bytes), Err(error), "{bytes:?}");
        }
    }
}

#[test]
fn rejects_lengths_that_previously_could_wrap_or_truncate() {
    for len in [
        16_383,
        16_384,
        u32::MAX as u64 - 5, // DNS cursor + length wraps to zero on Wasm.
        u32::MAX as u64,
        1u64 << 32,    // A narrowing cast must not turn this into an empty payload.
        u64::MAX - 10, // DNS cursor + length wraps to zero on 64-bit native.
        u64::MAX,
    ] {
        for prefix in [vec![54], vec![55], vec![56], vec![4, 127, 0, 0, 1, 0xa5, 3]] {
            let mut bytes = prefix;
            encode_varint(len, &mut bytes);
            bytes.extend_from_slice(&[0xa5, 3, 1, 42]);
            assert!(verify(&bytes).is_err(), "length {len}, {bytes:?}");
        }
    }
}

#[test]
fn arbitrary_bytes_and_mutated_addresses_do_not_panic() {
    let mut state = 0x6a09e667f3bcc909u64;
    let mut bytes = [0u8; MAX_MULTIADDR_BYTES];
    for sample in 0..8192 {
        let len = sample % (MAX_MULTIADDR_BYTES + 1);
        for byte in &mut bytes[..len] {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *byte = state as u8;
        }
        let _ = verify(&bytes[..len]);
    }
    // Mutate every byte of a maximum-length valid address through all values.
    let mut bytes = address(&host(DNS4, &[b'a'; 889]), None, None, &[42; 128]);
    for index in 0..bytes.len() {
        let original = bytes[index];
        for value in 0..=255 {
            bytes[index] = value;
            let _ = verify(&bytes);
        }
        bytes[index] = original;
    }
}

#[test]
fn fixture_encoder_handles_full_u64_range() {
    for (value, expected) in [
        (0, vec![0]),
        (127, vec![127]),
        (128, vec![128, 1]),
        (273, vec![0x91, 2]),
        (421, vec![0xa5, 3]),
        (16_383, vec![255, 127]),
        (16_384, vec![128, 128, 1]),
        (
            u64::MAX,
            vec![255, 255, 255, 255, 255, 255, 255, 255, 255, 1],
        ),
    ] {
        let mut bytes = Vec::new();
        encode_varint(value, &mut bytes);
        assert_eq!(bytes, expected);
    }
}
