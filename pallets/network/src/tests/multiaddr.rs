use super::mock::*;
use super::*;
use crate::multiaddr::{
    encode_varint, Multiaddr, MultiaddrError, DNS4, DNS6, DNSADDR, IP4, IP6, P2P, TCP, UDP, WS, WSS,
};
extern crate alloc;
use alloc::string::String;
use alloc::vec;
use sp_core::OpaquePeerId as PeerId;

// verify

// verify ip4
#[test]
fn test_ip4_tcp_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([1u8; 32].to_vec());
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[127, 0, 0, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&30303u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], "/ip4/127.0.0.1");
        assert_eq!(segs[1], "/tcp/30303");
        assert!(segs[2].starts_with("/p2p/"));
    });
}

// verify ip6
#[test]
fn test_ip6_udp_ws_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([2u8; 32].to_vec());
        let mut bytes = vec![];
        encode_varint(IP6, &mut bytes);
        let ip6_bytes = [0u8; 16];
        bytes.extend_from_slice(&ip6_bytes);
        encode_varint(UDP, &mut bytes);
        bytes.extend_from_slice(&53u16.to_be_bytes());
        encode_varint(WS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], "/ip6/0:0:0:0:0:0:0:0");
        assert_eq!(segs[1], "/udp/53");
        assert_eq!(segs[2], "/ws");
        assert!(segs[3].starts_with("/p2p/"));
    });
}

#[test]
fn test_ip4_wss_p2p_from_str() {
    new_test_ext().execute_with(|| {
        let peer = PeerId([3u8; 32].to_vec());
        let peer_str = bs58::encode(&peer.0).into_string();

        let addr_str = format!("/ip4/10.0.0.1/wss/p2p/{}", peer_str);

        let ma = Multiaddr::from_str(&addr_str).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], "/ip4/10.0.0.1");
        assert_eq!(segs[1], "/wss");
        assert!(segs[2].starts_with("/p2p/"));
    });
}

#[test]
fn test_from_str_ip4_tcp_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId([1u8; 32].to_vec());
        let peer_str = bs58::encode(&peer.0).into_string();
        let addr_str = format!("/ip4/127.0.0.1/tcp/30303/p2p/{}", peer_str);

        let ma = Multiaddr::from_str(&addr_str).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], "/ip4/127.0.0.1");
        assert_eq!(segs[1], "/tcp/30303");
        assert!(segs[2].starts_with("/p2p/"));
    });
}

#[test]
fn test_from_str_ip6_udp_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId([2u8; 32].to_vec());
        let peer_str = bs58::encode(&peer.0).into_string();
        let addr_str = format!("/ip6/0:0:0:0:0:0:0:1/udp/8080/p2p/{}", peer_str);

        let ma = Multiaddr::from_str(&addr_str).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], "/ip6/0:0:0:0:0:0:0:1");
        assert_eq!(segs[1], "/udp/8080");
        assert!(segs[2].starts_with("/p2p/"));
    });
}

#[test]
fn test_from_str_ip4_tcp_ws_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId([3u8; 32].to_vec());
        let peer_str = bs58::encode(&peer.0).into_string();
        let addr_str = format!("/ip4/10.0.0.1/tcp/80/ws/p2p/{}", peer_str);

        let ma = Multiaddr::from_str(&addr_str).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], "/ip4/10.0.0.1");
        assert_eq!(segs[1], "/tcp/80");
        assert_eq!(segs[2], "/ws");
        assert!(segs[3].starts_with("/p2p/"));
    });
}

#[test]
fn test_from_str_ip4_tcp_wss_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId([4u8; 32].to_vec());
        let peer_str = bs58::encode(&peer.0).into_string();
        let addr_str = format!("/ip4/10.0.0.1/tcp/443/wss/p2p/{}", peer_str);

        let ma = Multiaddr::from_str(&addr_str).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], "/ip4/10.0.0.1");
        assert_eq!(segs[1], "/tcp/443");
        assert_eq!(segs[2], "/wss");
        assert!(segs[3].starts_with("/p2p/"));
    });
}

#[test]
fn test_from_str_ip6_tcp_ws_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId([5u8; 32].to_vec());
        let peer_str = bs58::encode(&peer.0).into_string();
        let addr_str = format!("/ip6/::1/tcp/80/ws/p2p/{}", peer_str);

        let ma = Multiaddr::from_str(&addr_str).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], "/ip6/0:0:0:0:0:0:0:1");
        assert_eq!(segs[1], "/tcp/80");
        assert_eq!(segs[2], "/ws");
        assert!(segs[3].starts_with("/p2p/"));
    });
}

#[test]
fn test_from_str_ip6_tcp_wss_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId([6u8; 32].to_vec());
        let peer_str = bs58::encode(&peer.0).into_string();
        let addr_str = format!("/ip6/::1/tcp/443/wss/p2p/{}", peer_str);

        let ma = Multiaddr::from_str(&addr_str).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], "/ip6/0:0:0:0:0:0:0:1");
        assert_eq!(segs[1], "/tcp/443");
        assert_eq!(segs[2], "/wss");
        assert!(segs[3].starts_with("/p2p/"));
    });
}

#[test]
fn test_from_str_dns4_tcp_ws_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId([7u8; 32].to_vec());
        let peer_str = bs58::encode(&peer.0).into_string();

        let addr_str = format!("/dns4/node.example.com/tcp/443/ws/p2p/{}", peer_str);

        let ma = Multiaddr::from_str(&addr_str).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], "/dns4/node.example.com");
        assert_eq!(segs[1], "/tcp/443");
        assert_eq!(segs[2], "/ws");
        assert!(segs[3].starts_with("/p2p/"));
    });
}

#[test]
fn test_from_str_dns6_tcp_wss_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId([8u8; 32].to_vec());
        let peer_str = bs58::encode(&peer.0).into_string();

        let addr_str = format!("/dns6/node.example.com/tcp/443/wss/p2p/{}", peer_str);

        let ma = Multiaddr::from_str(&addr_str).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], "/dns6/node.example.com");
        assert_eq!(segs[1], "/tcp/443");
        assert_eq!(segs[2], "/wss");
        assert!(segs[3].starts_with("/p2p/"));
    });
}

#[test]
fn test_from_str_dnsaddr_p2p() {
    new_test_ext().execute_with(|| {
        let peer = PeerId([9u8; 32].to_vec());
        let peer_str = bs58::encode(&peer.0).into_string();

        let addr_str = format!("/dnsaddr/bootstrap.libp2p.io/p2p/{}", peer_str);

        let ma = Multiaddr::from_str(&addr_str).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], "/dnsaddr/bootstrap.libp2p.io");
        assert!(segs[1].starts_with("/p2p/"));
    });
}

#[test]
fn test_extend_multiaddr() {
    new_test_ext().execute_with(|| {
        let peer1 = PeerId::new([4u8; 32].to_vec());
        let peer2 = PeerId::new([5u8; 32].to_vec());

        let mut ma1_bytes = vec![];
        encode_varint(IP4, &mut ma1_bytes);
        ma1_bytes.extend_from_slice(&[192, 168, 0, 1]);
        encode_varint(TCP, &mut ma1_bytes);
        ma1_bytes.extend_from_slice(&8080u16.to_be_bytes());
        encode_varint(P2P, &mut ma1_bytes);
        encode_varint(peer1.0.len() as u64, &mut ma1_bytes);
        ma1_bytes.extend_from_slice(&peer1.0);
        let mut ma1 = Multiaddr::verify(&ma1_bytes).unwrap();

        let mut ma2_bytes = vec![];
        encode_varint(IP4, &mut ma2_bytes);
        ma2_bytes.extend_from_slice(&[10, 0, 0, 1]);
        encode_varint(TCP, &mut ma2_bytes);
        ma2_bytes.extend_from_slice(&30303u16.to_be_bytes());
        encode_varint(P2P, &mut ma2_bytes);
        encode_varint(peer2.0.len() as u64, &mut ma2_bytes);
        ma2_bytes.extend_from_slice(&peer2.0);
        let ma2 = Multiaddr::verify(&ma2_bytes).unwrap();

        ma1.extend(&ma2);
        let segs = ma1.to_vec().unwrap();
        assert!(segs.len() > 0); // just check concatenation works
        assert!(segs[0].starts_with("/ip4/192"));
        assert!(segs.last().unwrap().starts_with("/p2p/"));
    });
}

#[test]
fn test_invalid_multiaddr() {
    new_test_ext().execute_with(|| {
        // missing terminal /p2p
        let bytes = vec![4, 127, 0, 0, 1, 6, 0x1f, 0x90];
        let res = Multiaddr::verify(&bytes);
        assert_eq!(res, Err(MultiaddrError::InvalidProtocol));
    });
}

// ============================================================================
// TCP PROTOCOL TESTS
// ============================================================================

#[test]
fn test_verify_tcp_various_ports() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([3u8; 32].to_vec());

        // Test with port 0
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[192, 168, 1, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&0u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs[1], "/tcp/0");

        // Test with port 65535
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[192, 168, 1, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&65535u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs[1], "/tcp/65535");
    });
}

#[test]
fn test_verify_tcp_truncated() {
    new_test_ext().execute_with(|| {
        // TCP without port bytes (should fail)
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[10, 0, 0, 1]);
        encode_varint(TCP, &mut bytes);
        // Missing 2 bytes for port

        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MultiaddrError::Truncated);
    });
}

#[test]
fn test_verify_tcp_partial_port() {
    new_test_ext().execute_with(|| {
        // TCP with only 1 byte instead of 2
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[10, 0, 0, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.push(0x50); // Only 1 byte instead of 2

        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MultiaddrError::Truncated);
    });
}

// ============================================================================
// UDP PROTOCOL TESTS
// ============================================================================

#[test]
fn test_verify_udp_various_ports() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([4u8; 32].to_vec());

        // Test with standard DNS port
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[8, 8, 8, 8]);
        encode_varint(UDP, &mut bytes);
        bytes.extend_from_slice(&53u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs[0], "/ip4/8.8.8.8");
        assert_eq!(segs[1], "/udp/53");

        // Test with high port number
        let mut bytes = vec![];
        encode_varint(IP6, &mut bytes);
        bytes.extend_from_slice(&[0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        encode_varint(UDP, &mut bytes);
        bytes.extend_from_slice(&49152u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs[1], "/udp/49152");
    });
}

#[test]
fn test_verify_udp_truncated() {
    new_test_ext().execute_with(|| {
        // UDP without port bytes
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[172, 16, 0, 1]);
        encode_varint(UDP, &mut bytes);

        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MultiaddrError::Truncated);
    });
}

// ============================================================================
// DNS4 PROTOCOL TESTS
// ============================================================================

#[test]
fn test_verify_dns4_basic() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([5u8; 32].to_vec());
        let domain = "example.com";

        let mut bytes = vec![];
        encode_varint(DNS4, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&443u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], "/dns4/example.com");
        assert_eq!(segs[1], "/tcp/443");
        assert!(segs[2].starts_with("/p2p/"));
    });
}

#[test]
fn test_verify_dns4_subdomain() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([6u8; 32].to_vec());
        let domain = "node.example.com";

        let mut bytes = vec![];
        encode_varint(DNS4, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&8080u16.to_be_bytes());
        encode_varint(WSS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs[0], "/dns4/node.example.com");
        assert_eq!(segs[1], "/tcp/8080");
        assert_eq!(segs[2], "/wss");
    });
}

#[test]
fn test_verify_dns4_long_domain() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([7u8; 32].to_vec());
        let domain = "very.long.subdomain.example.com";

        let mut bytes = vec![];
        encode_varint(DNS4, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&9000u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs[0], "/dns4/very.long.subdomain.example.com");
    });
}

#[test]
fn test_verify_dns4_truncated_length() {
    new_test_ext().execute_with(|| {
        // DNS4 with invalid length
        let mut bytes = vec![];
        encode_varint(DNS4, &mut bytes);
        encode_varint(100, &mut bytes); // Claims 100 bytes
        bytes.extend_from_slice(b"short"); // But only provides 5

        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MultiaddrError::Truncated);
    });
}

#[test]
fn test_verify_dns4_empty_domain() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([8u8; 32].to_vec());

        // DNS4 with zero-length domain
        let mut bytes = vec![];
        encode_varint(DNS4, &mut bytes);
        encode_varint(0, &mut bytes);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&80u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs[0], "/dns4/");
    });
}

// ============================================================================
// DNS6 PROTOCOL TESTS
// ============================================================================

#[test]
fn test_verify_dns6_basic() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([9u8; 32].to_vec());
        let domain = "ipv6.example.com";

        let mut bytes = vec![];
        encode_varint(DNS6, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&443u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs[0], "/dns6/ipv6.example.com");
        assert_eq!(segs[1], "/tcp/443");
    });
}

#[test]
fn test_verify_dns6_with_ws() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([10u8; 32].to_vec());
        let domain = "ws.ipv6only.net";

        let mut bytes = vec![];
        encode_varint(DNS6, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&8080u16.to_be_bytes());
        encode_varint(WS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs[0], "/dns6/ws.ipv6only.net");
        assert_eq!(segs[2], "/ws");
    });
}

#[test]
fn test_verify_dns6_truncated() {
    new_test_ext().execute_with(|| {
        let mut bytes = vec![];
        encode_varint(DNS6, &mut bytes);
        encode_varint(50, &mut bytes);
        bytes.extend_from_slice(b"too.short.com");

        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MultiaddrError::Truncated);
    });
}

// ============================================================================
// DNSADDR PROTOCOL TESTS
// ============================================================================

#[test]
fn test_verify_dnsaddr_basic() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([11u8; 32].to_vec());
        let domain = "bootstrap.libp2p.io";

        let mut bytes = vec![];
        encode_varint(DNSADDR, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], "/dnsaddr/bootstrap.libp2p.io");
        assert!(segs[1].starts_with("/p2p/"));
    });
}

#[test]
fn test_verify_dnsaddr_with_tcp() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([12u8; 32].to_vec());
        let domain = "node.polkadot.io";

        let mut bytes = vec![];
        encode_varint(DNSADDR, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&30333u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs[0], "/dnsaddr/node.polkadot.io");
        assert_eq!(segs[1], "/tcp/30333");
    });
}

#[test]
fn test_verify_dnsaddr_empty() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([13u8; 32].to_vec());

        let mut bytes = vec![];
        encode_varint(DNSADDR, &mut bytes);
        encode_varint(0, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs[0], "/dnsaddr/");
    });
}

#[test]
fn test_verify_dnsaddr_truncated() {
    new_test_ext().execute_with(|| {
        let mut bytes = vec![];
        encode_varint(DNSADDR, &mut bytes);
        encode_varint(100, &mut bytes);
        bytes.extend_from_slice(b"short.domain.com");

        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
    });
}

// ============================================================================
// P2P PROTOCOL TESTS
// ============================================================================

#[test]
fn test_verify_p2p_different_sizes() {
    new_test_ext().execute_with(|| {
        // Test with 32-byte peer ID
        let peer_32 = PeerId::new([14u8; 32].to_vec());
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[127, 0, 0, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&8080u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer_32.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer_32.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();
        assert!(segs[2].starts_with("/p2p/"));

        // Test with 38-byte peer ID
        let peer_38 = PeerId::new([15u8; 38].to_vec());
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[127, 0, 0, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&8080u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(peer_38.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer_38.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();
        assert!(segs[2].starts_with("/p2p/"));
    });
}

#[test]
fn test_verify_p2p_truncated() {
    new_test_ext().execute_with(|| {
        // P2P claims 32 bytes but provides fewer
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[192, 168, 1, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&9000u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(32, &mut bytes);
        bytes.extend_from_slice(&[1, 2, 3, 4, 5]); // Only 5 bytes

        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MultiaddrError::Truncated);
    });
}

#[test]
fn test_verify_p2p_zero_length() {
    new_test_ext().execute_with(|| {
        // P2P with zero-length peer ID (should technically work with verify but might want to reject)
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[10, 0, 0, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&5000u16.to_be_bytes());
        encode_varint(P2P, &mut bytes);
        encode_varint(0, &mut bytes);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();
        assert_eq!(segs[2], "/p2p/");
    });
}

#[test]
fn test_verify_missing_p2p() {
    new_test_ext().execute_with(|| {
        // Multiaddr that doesn't end with P2P should fail
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[192, 168, 1, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&8080u16.to_be_bytes());

        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MultiaddrError::InvalidProtocol);
    });
}

// ============================================================================
// WS (WebSocket) PROTOCOL TESTS
// ============================================================================

#[test]
fn test_verify_ws_basic() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([16u8; 32].to_vec());

        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[127, 0, 0, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&8080u16.to_be_bytes());
        encode_varint(WS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 4);
        assert_eq!(segs[2], "/ws");
    });
}

#[test]
fn test_verify_ws_with_dns4() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([17u8; 32].to_vec());
        let domain = "ws.example.com";

        let mut bytes = vec![];
        encode_varint(DNS4, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&80u16.to_be_bytes());
        encode_varint(WS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs[0], "/dns4/ws.example.com");
        assert_eq!(segs[1], "/tcp/80");
        assert_eq!(segs[2], "/ws");
    });
}

#[test]
fn test_verify_ws_with_ipv6() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([18u8; 32].to_vec());

        let mut bytes = vec![];
        encode_varint(IP6, &mut bytes);
        bytes.extend_from_slice(&[0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&8080u16.to_be_bytes());
        encode_varint(WS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs[2], "/ws");
    });
}

#[test]
fn test_verify_ws_no_payload() {
    new_test_ext().execute_with(|| {
        // WS should have zero-length payload
        let peer = PeerId::new([19u8; 32].to_vec());

        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[10, 0, 0, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&3000u16.to_be_bytes());
        encode_varint(WS, &mut bytes);
        // WS takes no additional bytes
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        assert!(ma.to_bytes().len() > 0);
    });
}

// ============================================================================
// WSS (WebSocket Secure) PROTOCOL TESTS
// ============================================================================

#[test]
fn test_verify_wss_basic() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([20u8; 32].to_vec());

        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[192, 168, 1, 100]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&443u16.to_be_bytes());
        encode_varint(WSS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 4);
        assert_eq!(segs[2], "/wss");
    });
}

#[test]
fn test_verify_wss_with_dns6() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([21u8; 32].to_vec());
        let domain = "secure.example.com";

        let mut bytes = vec![];
        encode_varint(DNS6, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&443u16.to_be_bytes());
        encode_varint(WSS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs[0], "/dns6/secure.example.com");
        assert_eq!(segs[1], "/tcp/443");
        assert_eq!(segs[2], "/wss");
    });
}

#[test]
fn test_verify_wss_with_dnsaddr() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([22u8; 32].to_vec());
        let domain = "wss.network.io";

        let mut bytes = vec![];
        encode_varint(DNSADDR, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&443u16.to_be_bytes());
        encode_varint(WSS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs[0], "/dnsaddr/wss.network.io");
        assert_eq!(segs[2], "/wss");
    });
}

#[test]
fn test_verify_wss_no_payload() {
    new_test_ext().execute_with(|| {
        // WSS should have zero-length payload
        let peer = PeerId::new([23u8; 32].to_vec());

        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[172, 16, 0, 1]);
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&8443u16.to_be_bytes());
        encode_varint(WSS, &mut bytes);
        // WSS takes no additional bytes
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        assert!(ma.to_bytes().len() > 0);
    });
}

// ============================================================================
// COMBINATION / EDGE CASE TESTS
// ============================================================================

#[test]
fn test_verify_complex_multiaddr() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([24u8; 32].to_vec());
        let domain = "complex.node.example.com";

        // /dnsaddr/complex.node.example.com/tcp/443/wss/p2p/...
        let mut bytes = vec![];
        encode_varint(DNSADDR, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&443u16.to_be_bytes());
        encode_varint(WSS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma = Multiaddr::verify(&bytes).unwrap();
        let segs = ma.to_vec().unwrap();

        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], "/dnsaddr/complex.node.example.com");
        assert_eq!(segs[1], "/tcp/443");
        assert_eq!(segs[2], "/wss");
        assert!(segs[3].starts_with("/p2p/"));
    });
}

#[test]
fn test_verify_invalid_protocol_code() {
    new_test_ext().execute_with(|| {
        // Use an unsupported protocol code
        let mut bytes = vec![];
        encode_varint(IP4, &mut bytes);
        bytes.extend_from_slice(&[127, 0, 0, 1]);
        encode_varint(9999, &mut bytes); // Invalid protocol

        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MultiaddrError::InvalidProtocol);
    });
}

#[test]
fn test_verify_invalid_varint() {
    new_test_ext().execute_with(|| {
        // Create bytes with an incomplete/invalid varint
        let bytes = vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MultiaddrError::InvalidVarint);
    });
}

#[test]
fn test_verify_empty_bytes() {
    new_test_ext().execute_with(|| {
        let bytes = vec![];
        let result = Multiaddr::verify(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MultiaddrError::InvalidProtocol);
    });
}

// ============================================================================
// ROUND-TRIP TESTS (to_bytes -> verify -> to_vec)
// ============================================================================

#[test]
fn test_roundtrip_all_protocols() {
    new_test_ext().execute_with(|| {
        let peer = PeerId::new([25u8; 32].to_vec());
        let domain = "roundtrip.test.com";

        let mut bytes = vec![];
        encode_varint(DNS4, &mut bytes);
        encode_varint(domain.len() as u64, &mut bytes);
        bytes.extend_from_slice(domain.as_bytes());
        encode_varint(TCP, &mut bytes);
        bytes.extend_from_slice(&9999u16.to_be_bytes());
        encode_varint(WS, &mut bytes);
        encode_varint(P2P, &mut bytes);
        encode_varint(peer.0.len() as u64, &mut bytes);
        bytes.extend_from_slice(&peer.0);

        let ma1 = Multiaddr::verify(&bytes).unwrap();
        let bytes2 = ma1.to_bytes();
        let ma2 = Multiaddr::verify(&bytes2).unwrap();

        assert_eq!(ma1, ma2);
        assert_eq!(ma1.to_vec().unwrap(), ma2.to_vec().unwrap());
    });
}
