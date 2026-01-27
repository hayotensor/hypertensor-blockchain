#![allow(dead_code)]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use bs58;

/* Errors */
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiaddrError {
    InvalidVarint,
    InvalidProtocol,
    InvalidAddress,
    Truncated,
}

/* Protocol codes */
pub const IP4: u64 = 4;
pub const IP6: u64 = 41;
pub const TCP: u64 = 6;
pub const UDP: u64 = 17;
pub const DNS4: u64 = 54;
pub const DNS6: u64 = 55;
pub const DNSADDR: u64 = 56;
pub const P2P: u64 = 421;
pub const WS: u64 = 477;
pub const WSS: u64 = 478;

/* Multiaddr */
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Multiaddr {
    bytes: Vec<u8>,
}

impl Multiaddr {
    /* =====================
    Verify multiaddr bytes
    ===================== */
    pub fn verify(bytes: &[u8]) -> Result<Self, MultiaddrError> {
        let mut i = 0;
        let mut last_proto = 0;

        while i < bytes.len() {
            let (proto, read) = decode_varint(&bytes[i..]).ok_or(MultiaddrError::InvalidVarint)?;
            i += read;

            match proto {
                IP4 => advance(bytes, &mut i, 4)?,
                IP6 => advance(bytes, &mut i, 16)?,
                TCP | UDP => advance(bytes, &mut i, 2)?,
                WS | WSS => advance(bytes, &mut i, 0)?, // WebSockets have no payload
                DNS4 | DNS6 | DNSADDR => {
                    // DNS protocols have a length-prefixed string
                    let (len, read) =
                        decode_varint(&bytes[i..]).ok_or(MultiaddrError::InvalidVarint)?;
                    i += read;
                    advance(bytes, &mut i, len as usize)?;
                }
                P2P => {
                    let (len, read) =
                        decode_varint(&bytes[i..]).ok_or(MultiaddrError::InvalidVarint)?;
                    i += read;
                    advance(bytes, &mut i, len as usize)?;
                }
                _ => return Err(MultiaddrError::InvalidProtocol),
            }

            last_proto = proto;
        }

        // Ensure multiaddr ends with /p2p
        if last_proto != P2P {
            return Err(MultiaddrError::InvalidProtocol);
        }

        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }

    /* =====================
    Access raw bytes
    ===================== */
    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /* =====================
    Extend / join
    ===================== */
    pub fn extend(&mut self, other: &Multiaddr) {
        self.bytes.extend_from_slice(&other.bytes);
    }

    /* =====================
    Format to vec of string segments
    Example: ["/ip4/127.0.0.1", "/tcp/38960", "/p2p/12D3Koo..."]
    ===================== */
    pub fn to_vec(&self) -> Result<Vec<String>, MultiaddrError> {
        let mut i = 0;
        let mut out = Vec::new();

        while i < self.bytes.len() {
            let (proto, read) =
                decode_varint(&self.bytes[i..]).ok_or(MultiaddrError::InvalidVarint)?;
            i += read;

            match proto {
                IP4 => {
                    let a = &self.bytes[i..i + 4];
                    i += 4;
                    out.push(format!("/ip4/{}.{}.{}.{}", a[0], a[1], a[2], a[3]));
                }
                IP6 => {
                    let a = &self.bytes[i..i + 16];
                    i += 16;
                    let segs: [u16; 8] = [
                        u16::from_be_bytes([a[0], a[1]]),
                        u16::from_be_bytes([a[2], a[3]]),
                        u16::from_be_bytes([a[4], a[5]]),
                        u16::from_be_bytes([a[6], a[7]]),
                        u16::from_be_bytes([a[8], a[9]]),
                        u16::from_be_bytes([a[10], a[11]]),
                        u16::from_be_bytes([a[12], a[13]]),
                        u16::from_be_bytes([a[14], a[15]]),
                    ];
                    out.push(format!(
                        "/ip6/{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
                        segs[0], segs[1], segs[2], segs[3], segs[4], segs[5], segs[6], segs[7]
                    ));
                }
                TCP | UDP => {
                    let port = u16::from_be_bytes([self.bytes[i], self.bytes[i + 1]]);
                    i += 2;
                    out.push(format!(
                        "/{}/{}",
                        if proto == TCP { "tcp" } else { "udp" },
                        port
                    ));
                }
                DNS4 | DNS6 | DNSADDR => {
                    let (len, read) =
                        decode_varint(&self.bytes[i..]).ok_or(MultiaddrError::InvalidVarint)?;
                    i += read;

                    let name_bytes = &self.bytes[i..i + len as usize];
                    i += len as usize;

                    let name = core::str::from_utf8(name_bytes)
                        .map_err(|_| MultiaddrError::InvalidAddress)?;

                    let proto_str = match proto {
                        DNS4 => "dns4",
                        DNS6 => "dns6",
                        DNSADDR => "dnsaddr",
                        _ => unreachable!(),
                    };

                    out.push(format!("/{}/{}", proto_str, name));
                }
                WS => out.push("/ws".into()),
                WSS => out.push("/wss".into()),
                P2P => {
                    let (len, read) =
                        decode_varint(&self.bytes[i..]).ok_or(MultiaddrError::InvalidVarint)?;
                    i += read;
                    let peer_bytes = &self.bytes[i..i + len as usize];
                    i += len as usize;

                    let encoded = bs58::encode(peer_bytes).into_string();
                    out.push(format!("/p2p/{}", encoded));
                }
                _ => return Err(MultiaddrError::InvalidProtocol),
            }
        }

        Ok(out)
    }

    #[cfg(feature = "std")]
    pub fn from_str(s: &str) -> Result<Self, MultiaddrError> {
        let mut bytes = Vec::new();
        let mut parts = s.split('/').filter(|p| !p.is_empty());

        while let Some(proto) = parts.next() {
            match proto {
                "ip4" => {
                    let addr = parts.next().ok_or(MultiaddrError::InvalidAddress)?;
                    let octets: Vec<u8> = addr
                        .split('.')
                        .map(|b| b.parse::<u8>().map_err(|_| MultiaddrError::InvalidAddress))
                        .collect::<Result<_, _>>()?;

                    if octets.len() != 4 {
                        return Err(MultiaddrError::InvalidAddress);
                    }

                    encode_varint(IP4, &mut bytes);
                    bytes.extend_from_slice(&octets);
                }

                "ip6" => {
                    let addr = parts.next().ok_or(MultiaddrError::InvalidAddress)?;
                    let segs = parse_ipv6(addr)?;
                    encode_varint(IP6, &mut bytes);
                    for s in segs {
                        bytes.extend_from_slice(&s.to_be_bytes());
                    }
                }

                "dns4" | "dns6" | "dnsaddr" => {
                    let name = parts.next().ok_or(MultiaddrError::InvalidAddress)?;
                    let proto_code = match proto {
                        "dns4" => DNS4,
                        "dns6" => DNS6,
                        _ => DNSADDR,
                    };

                    encode_varint(proto_code, &mut bytes);
                    encode_varint(name.len() as u64, &mut bytes);
                    bytes.extend_from_slice(name.as_bytes());
                }

                "tcp" | "udp" => {
                    let port = parts.next().ok_or(MultiaddrError::InvalidAddress)?;
                    let port: u16 = port.parse().map_err(|_| MultiaddrError::InvalidAddress)?;

                    encode_varint(if proto == "tcp" { TCP } else { UDP }, &mut bytes);
                    bytes.extend_from_slice(&port.to_be_bytes());
                }

                "ws" => {
                    encode_varint(WS, &mut bytes);
                }

                "wss" => {
                    encode_varint(WSS, &mut bytes);
                }

                "p2p" => {
                    let peer = parts.next().ok_or(MultiaddrError::InvalidAddress)?;
                    let peer_bytes = bs58::decode(peer)
                        .into_vec()
                        .map_err(|_| MultiaddrError::InvalidAddress)?;

                    encode_varint(P2P, &mut bytes);
                    encode_varint(peer_bytes.len() as u64, &mut bytes);
                    bytes.extend_from_slice(&peer_bytes);
                }

                _ => return Err(MultiaddrError::InvalidProtocol),
            }
        }

        Multiaddr::verify(&bytes)
    }
}

/* ============================================================
   Helpers
============================================================ */
/// Parses IPv6 addresses with support for "::" compression
fn parse_ipv6(addr: &str) -> Result<[u16; 8], MultiaddrError> {
    let mut segs = [0u16; 8];
    let parts: Vec<&str> = addr.split("::").collect();
    if parts.len() > 2 {
        return Err(MultiaddrError::InvalidAddress);
    }

    // Collect left and right parts as Vec<&str> so they have the same type
    let left: Vec<&str> = parts[0].split(':').filter(|s| !s.is_empty()).collect();
    let right: Vec<&str> = if parts.len() == 2 {
        parts[1].split(':').filter(|s| !s.is_empty()).collect()
    } else {
        Vec::new()
    };

    if left.len() + right.len() > 8 {
        return Err(MultiaddrError::InvalidAddress);
    }

    // Fill left segments
    for (i, p) in left.iter().enumerate() {
        segs[i] = u16::from_str_radix(p, 16).map_err(|_| MultiaddrError::InvalidAddress)?;
    }

    // Fill right segments
    let mut j = 8 - right.len();
    for p in right {
        segs[j] = u16::from_str_radix(p, 16).map_err(|_| MultiaddrError::InvalidAddress)?;
        j += 1;
    }

    Ok(segs)
}

pub fn advance(bytes: &[u8], i: &mut usize, len: usize) -> Result<(), MultiaddrError> {
    if *i + len > bytes.len() {
        Err(MultiaddrError::Truncated)
    } else {
        *i += len;
        Ok(())
    }
}

pub fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

pub fn decode_varint(input: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0u64;
    let mut shift = 0;
    let mut i = 0;

    while i < input.len() {
        let byte = input[i];
        value |= ((byte & 0x7F) as u64) << shift;
        i += 1;

        if byte & 0x80 == 0 {
            return Some((value, i));
        }

        shift += 7;
        if shift > 63 {
            return None;
        }
    }

    None
}