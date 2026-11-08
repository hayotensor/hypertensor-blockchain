//! Loose, bounded validation of binary multiaddresses.
//!
//! Accepted shape: host / [tcp|udp / port] / [ws|wss] / p2p / peer bytes.
//! The host is ip4, ip6, dns4, dns6, or dnsaddr. DNS and peer payloads are opaque:
//! only their nonempty lengths are checked, not their contents or reachability.
//! Validation has at most four protocol reads and two length reads, with no
//! loops, recursion, allocation, or unchecked cursor arithmetic.

#[cfg(any(feature = "std", feature = "runtime-benchmarks", test))]
extern crate alloc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiaddrError {
    InvalidVarint,
    InvalidProtocol,
    InvalidAddress,
    Truncated,
}

pub const IP4: u64 = 4;
pub const IP6: u64 = 41;
pub const TCP: u64 = 6;
pub const UDP: u64 = 273;
pub const DNS4: u64 = 54;
pub const DNS6: u64 = 55;
pub const DNSADDR: u64 = 56;
pub const P2P: u64 = 421;
pub const WS: u64 = 477;
pub const WSS: u64 = 478;

/// Matches the runtime's NetworkBytes limit; also bounds direct callers.
pub const MAX_MULTIADDR_BYTES: usize = 1024;

pub fn verify(mut bytes: &[u8]) -> Result<(), MultiaddrError> {
    if bytes.len() > MAX_MULTIADDR_BYTES {
        return Err(MultiaddrError::InvalidAddress);
    }

    match read_protocol(&mut bytes)? {
        IP4 => skip(&mut bytes, 4)?,
        IP6 => skip(&mut bytes, 16)?,
        DNS4 | DNS6 | DNSADDR => skip_payload(&mut bytes)?,
        _ => return Err(MultiaddrError::InvalidProtocol),
    }

    let mut protocol = read_protocol(&mut bytes)?;
    if matches!(protocol, TCP | UDP) {
        skip(&mut bytes, 2)?;
        protocol = read_protocol(&mut bytes)?;
    }
    if matches!(protocol, WS | WSS) {
        protocol = read_protocol(&mut bytes)?;
    }
    if protocol != P2P {
        return Err(MultiaddrError::InvalidProtocol);
    }

    skip_payload(&mut bytes)?;
    if !bytes.is_empty() {
        return Err(MultiaddrError::InvalidProtocol);
    }
    Ok(())
}

fn read_protocol(bytes: &mut &[u8]) -> Result<u64, MultiaddrError> {
    if bytes.is_empty() {
        return Err(MultiaddrError::InvalidProtocol);
    }
    read_varint(bytes)
}

/// All supported codes and lengths fit in two bytes. Reject longer and
/// noncanonical encodings without scanning any additional input.
fn read_varint(bytes: &mut &[u8]) -> Result<u64, MultiaddrError> {
    match *bytes {
        [value @ 0..=127, rest @ ..] => {
            *bytes = rest;
            Ok(u64::from(*value))
        }
        [low @ 128..=255, high @ 1..=127, rest @ ..] => {
            *bytes = rest;
            Ok(u64::from(*low & 127) | (u64::from(*high) << 7))
        }
        _ => Err(MultiaddrError::InvalidVarint),
    }
}

fn skip_payload(bytes: &mut &[u8]) -> Result<(), MultiaddrError> {
    // At most 14 bits: the conversion is safe on native and Wasm targets.
    let len = read_varint(bytes)? as usize;
    if len == 0 {
        return Err(MultiaddrError::InvalidAddress);
    }
    skip(bytes, len)
}

fn skip(bytes: &mut &[u8], len: usize) -> Result<(), MultiaddrError> {
    *bytes = bytes.get(len..).ok_or(MultiaddrError::Truncated)?;
    Ok(())
}

/// Fixture/benchmark encoder, excluded from the production Wasm runtime.
#[cfg(any(feature = "std", feature = "runtime-benchmarks", test))]
pub fn encode_varint(value: u64, out: &mut alloc::vec::Vec<u8>) {
    let mut bytes = [
        value as u8 | 128,
        (value >> 7) as u8 | 128,
        (value >> 14) as u8 | 128,
        (value >> 21) as u8 | 128,
        (value >> 28) as u8 | 128,
        (value >> 35) as u8 | 128,
        (value >> 42) as u8 | 128,
        (value >> 49) as u8 | 128,
        (value >> 56) as u8 | 128,
        (value >> 63) as u8,
    ];
    let len = (u64::BITS - value.leading_zeros()).max(1).div_ceil(7) as usize;
    bytes[len - 1] &= 127;
    out.extend_from_slice(&bytes[..len]);
}
