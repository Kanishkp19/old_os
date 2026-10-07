//! Wake-on-LAN (M3, TRD §9): build and send the magic packet. Wake is
/// best-effort — the queue is the guarantee (decision D3, TEST_PLAN RM-06).

use std::net::UdpSocket;

use hh_core::error::{Error, Result};

/// Build the 102-byte magic packet: 6×0xFF + 16×MAC.
pub fn magic_packet(mac: &str) -> Result<[u8; 102]> {
    let parts: Vec<&str> = mac.split([':', '-']).collect();
    if parts.len() != 6 {
        return Err(Error::BadRequest(format!("invalid MAC {mac}")));
    }
    let mut mac_bytes = [0u8; 6];
    for (i, p) in parts.iter().enumerate() {
        mac_bytes[i] = u8::from_str_radix(p, 16)
            .map_err(|_| Error::BadRequest(format!("invalid MAC {mac}")))?;
    }
    let mut pkt = [0xFFu8; 102];
    for i in 0..16 {
        pkt[6 + i * 6..6 + (i + 1) * 6].copy_from_slice(&mac_bytes);
    }
    Ok(pkt)
}

/// Send the magic packet to the subnet broadcast address, UDP port 9.
pub fn send_wake(mac: &str, broadcast: &str) -> Result<()> {
    let pkt = magic_packet(mac)?;
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_broadcast(true)?;
    sock.send_to(&pkt, format!("{broadcast}:9"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_layout() {
        let p = magic_packet("01:23:45:67:89:ab").unwrap();
        assert_eq!(&p[..6], &[0xFF; 6]);
        assert_eq!(&p[6..12], &[0x01, 0x23, 0x45, 0x67, 0x89, 0xab]);
        assert_eq!(&p[96..102], &[0x01, 0x23, 0x45, 0x67, 0x89, 0xab]);
        assert!(magic_packet("bad").is_err());
    }
}
