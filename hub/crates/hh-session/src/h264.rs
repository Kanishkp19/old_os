//! Bounded RFC 6184 H.264 RTP reassembly and SPS dimension validation.
//! Only packetization mode 1 (single NAL, STAP-A, FU-A) is accepted.
pub const MAX_ACCESS_UNIT: usize = 4 * 1024 * 1024;
#[derive(Default)]
pub struct Assembler { data: Vec<u8>, timestamp: Option<u32>, next_sequence: Option<u16>, fragment: bool, damaged: bool }
impl Assembler {
    pub fn push(&mut self, sequence: u16, timestamp: u32, marker: bool, payload: &[u8]) -> Result<Option<Vec<u8>>, String> {
        if self.timestamp != Some(timestamp) {
            self.data.clear(); self.fragment = false; self.damaged = false; self.timestamp = Some(timestamp);
        }
        if self.next_sequence.is_some_and(|next| sequence != next) { self.data.clear(); self.fragment = false; self.damaged = true; }
        self.next_sequence = Some(sequence.wrapping_add(1));
        let result = self.append(payload);
        if result.is_err() { self.data.clear(); self.fragment = false; self.damaged = true; }
        result?;
        if marker {
            if self.fragment || self.damaged { self.data.clear(); return Err("incomplete H264 access unit".into()); }
            return Ok((!self.data.is_empty()).then(|| std::mem::take(&mut self.data)));
        }
        Ok(None)
    }
    fn nal(&mut self, nal: &[u8]) -> Result<(), String> {
        if nal.is_empty() || nal[0] & 0x80 != 0 { return Err("invalid H264 NAL".into()); }
        let kind = nal[0] & 31;
        if !matches!(kind, 1 | 5..=12) { return Err("unsupported H264 NAL".into()); }
        if kind == 7 { validate_sps(nal)?; }
        if self.data.len().saturating_add(nal.len()).saturating_add(4) > MAX_ACCESS_UNIT { return Err("H264 frame too large".into()); }
        self.data.extend_from_slice(&[0, 0, 0, 1]); self.data.extend_from_slice(nal); Ok(())
    }
    fn append(&mut self, payload: &[u8]) -> Result<(), String> {
        let first = *payload.first().ok_or("empty H264 payload")?;
        match first & 31 {
            1..=23 => { if self.fragment { return Err("interrupted H264 fragment".into()); } self.nal(payload) }
            24 => {
                if self.fragment { return Err("interrupted H264 fragment".into()); }
                let mut rest = &payload[1..];
                while !rest.is_empty() {
                    if rest.len() < 2 { return Err("truncated H264 aggregate".into()); }
                    let size = u16::from_be_bytes([rest[0], rest[1]]) as usize; rest = &rest[2..];
                    if size == 0 || size > rest.len() { return Err("invalid H264 aggregate length".into()); }
                    self.nal(&rest[..size])?; rest = &rest[size..];
                }
                Ok(())
            }
            28 => {
                if payload.len() < 3 || first & 0x80 != 0 { return Err("truncated H264 fragment".into()); }
                let header = payload[1]; let start = header & 0x80 != 0; let end = header & 0x40 != 0;
                if header & 0x20 != 0 || start && end || !matches!(header & 31, 1 | 5..=12) { return Err("invalid H264 fragment flags".into()); }
                // Parameter sets are small and must use a single NAL/STAP-A;
                // accepting fragmented SPS would bypass pre-decode bounds.
                if header & 31 == 7 { return Err("fragmented SPS unsupported".into()); }
                if start {
                    if self.fragment { return Err("nested H264 fragment".into()); }
                    self.nal(&[(first & 0xe0) | (header & 31)])?; self.fragment = true;
                } else if !self.fragment { return Err("orphan H264 fragment".into()); }
                if self.data.len().saturating_add(payload.len() - 2) > MAX_ACCESS_UNIT { return Err("H264 frame too large".into()); }
                self.data.extend_from_slice(&payload[2..]); if end { self.fragment = false; } Ok(())
            }
            _ => Err("unsupported H264 packetization".into()),
        }
    }
}
struct Bits { bytes: Vec<u8>, offset: usize }
impl Bits {
    fn read(&mut self, n: usize) -> Result<u32, String> {
        if n > 32 || self.offset.saturating_add(n) > self.bytes.len() * 8 { return Err("truncated H264 SPS".into()); }
        let mut value = 0;
        for _ in 0..n { value = (value << 1) | u32::from((self.bytes[self.offset / 8] >> (7 - self.offset % 8)) & 1); self.offset += 1; }
        Ok(value)
    }
    fn ue(&mut self) -> Result<u32, String> {
        let mut zeros = 0;
        while self.read(1)? == 0 { zeros += 1; if zeros > 24 { return Err("H264 SPS value too large".into()); } }
        Ok(((1u32 << zeros) - 1) + self.read(zeros)?)
    }
    fn se(&mut self) -> Result<i32, String> { let n = self.ue()?; Ok(if n % 2 == 0 { -(n as i32 / 2) } else { (n as i32 + 1) / 2 }) }
}
/// Reject unsupported or oversized SPS before native decoder allocation.
pub fn validate_sps(nal: &[u8]) -> Result<(), String> {
    if nal.len() > 4096 || nal.len() < 4 { return Err("invalid H264 SPS size".into()); }
    let mut rbsp = Vec::with_capacity(nal.len()); let mut zeros = 0;
    for byte in &nal[1..] {
        if zeros >= 2 && *byte == 3 { zeros = 0; continue; }
        rbsp.push(*byte); zeros = if *byte == 0 { zeros + 1 } else { 0 };
    }
    let mut b = Bits { bytes: rbsp, offset: 0 };
    let profile = b.read(8)?; b.read(8)?; b.read(8)?;
    if b.ue()? > 31 { return Err("invalid H264 SPS id".into()); }
    let mut chroma = 1;
    if matches!(profile, 100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135) {
        chroma = b.ue()?;
        if chroma != 1 { return Err("only 8-bit H264 4:2:0 supported".into()); }
        if b.ue()? != 0 || b.ue()? != 0 { return Err("only 8-bit H264 supported".into()); }
        b.read(1)?;
        if b.read(1)? != 0 {
            for index in 0..8 {
                if b.read(1)? != 0 {
                    let mut last = 8; let mut next = 8;
                    for _ in 0..if index < 6 { 16 } else { 64 } {
                        if next != 0 { next = (last + b.se()? + 256) % 256; }
                        if next != 0 { last = next; }
                    }
                }
            }
        }
    } else if !matches!(profile, 66 | 77 | 88) { return Err("unsupported H264 profile".into()); }
    if b.ue()? > 12 { return Err("invalid H264 frame number".into()); }
    match b.ue()? {
        0 => { if b.ue()? > 12 { return Err("invalid H264 picture order".into()); } }
        1 => { b.read(1)?; b.se()?; b.se()?; let n = b.ue()?; if n > 256 { return Err("H264 SPS cycle too large".into()); } for _ in 0..n { b.se()?; } }
        2 => {}
        _ => return Err("invalid H264 picture order".into()),
    }
    if b.ue()? > 16 { return Err("too many H264 reference frames".into()); }
    b.read(1)?;
    let w = b.ue()?.checked_add(1).and_then(|n| n.checked_mul(16)).ok_or("invalid H264 width")?;
    let h = b.ue()?.checked_add(1).ok_or("invalid H264 height")?;
    let progressive = b.read(1)?;
    if progressive == 0 { return Err("interlaced H264 unsupported".into()); }
    b.read(1)?;
    let h = h.checked_mul(16).ok_or("invalid H264 height")?;
    let (mut cw, mut ch) = (0, 0);
    if b.read(1)? != 0 {
        cw = b.ue()?.checked_add(b.ue()?).and_then(|n| n.checked_mul(if chroma == 1 { 2 } else { 1 })).ok_or("invalid H264 crop")?;
        ch = b.ue()?.checked_add(b.ue()?).and_then(|n| n.checked_mul(2)).ok_or("invalid H264 crop")?;
    }
    let width = w.checked_sub(cw).ok_or("invalid H264 crop")?; let height = h.checked_sub(ch).ok_or("invalid H264 crop")?;
    // Bound coded dimensions too: malicious crops must not hide huge buffers.
    if w > 1920 || h > 1920 || w.saturating_mul(h) > 1920 * 1088 || width < 16 || height < 16 {
        return Err("cast dimensions exceed 1080p budget".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn reconstructs_fua_and_wraps_sequence() {
        let mut a = Assembler::default();
        assert!(a.push(65535, 1, false, &[0x7c, 0x85, 1, 2]).unwrap().is_none());
        assert_eq!(a.push(0, 1, true, &[0x7c, 0x45, 3]).unwrap().unwrap(), vec![0,0,0,1,0x65,1,2,3]);
    }
    #[test] fn loss_discards_access_unit_and_recovers_next_timestamp() {
        let mut a = Assembler::default();
        a.push(1, 1, false, &[0x7c,0x85,1]).unwrap();
        assert!(a.push(3, 1, true, &[0x7c,0x45,2]).is_err());
        assert!(a.push(4, 2, true, &[0x65,3]).unwrap().is_some());
    }
    #[test] fn malformed_aggregate_and_sps_are_rejected() {
        let mut a = Assembler::default();
        assert!(a.push(1, 1, true, &[24,0,255,1]).is_err());
        assert!(validate_sps(&[7,0,0,0]).is_err());
        assert!(a.push(2, 2, true, &[0x7c,0x87,1]).is_err());
    }
}

/// Convert Annex-B or 4-byte length-prefixed H264 to the Annex-B samples
/// expected by WebRTC's H264 payloader. Both formats stay bounded.
pub fn normalize_access_unit(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.is_empty() || bytes.len() > MAX_ACCESS_UNIT { return Err("invalid H264 access unit size".into()); }
    if bytes.starts_with(&[0, 0, 1]) || bytes.starts_with(&[0, 0, 0, 1]) {
        return Ok(bytes.to_vec());
    }
    let mut rest = bytes; let mut output = Vec::with_capacity(bytes.len());
    while !rest.is_empty() {
        if rest.len() < 4 { return Err("truncated H264 length prefix".into()); }
        let length = u32::from_be_bytes([rest[0],rest[1],rest[2],rest[3]]) as usize; rest = &rest[4..];
        if length == 0 || length > rest.len() || output.len().saturating_add(length).saturating_add(4) > MAX_ACCESS_UNIT { return Err("invalid H264 NAL length".into()); }
        output.extend_from_slice(&[0,0,0,1]); output.extend_from_slice(&rest[..length]); rest = &rest[length..];
    }
    Ok(output)
}
/// Normalize the MF sequence header, including AVCDecoderConfigurationRecord.
pub fn normalize_sequence_header(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.is_empty() { return Ok(Vec::new()); }
    if bytes.len() > 65536 { return Err("H264 sequence header too large".into()); }
    if bytes[0] != 1 { return normalize_access_unit(bytes); }
    if bytes.len() < 7 || bytes[4] & 3 != 3 { return Err("unsupported H264 configuration record".into()); }
    let mut rest = &bytes[6..]; let mut output = Vec::new();
    let append = |rest: &mut &[u8], output: &mut Vec<u8>| -> Result<(), String> {
        if rest.len() < 2 { return Err("truncated H264 configuration".into()); }
        let length = u16::from_be_bytes([rest[0],rest[1]]) as usize; *rest = &rest[2..];
        if length == 0 || length > rest.len() { return Err("invalid H264 configuration NAL".into()); }
        output.extend_from_slice(&[0,0,0,1]); output.extend_from_slice(&rest[..length]); *rest = &rest[length..]; Ok(())
    };
    for _ in 0..(bytes[5] & 31) { append(&mut rest, &mut output)?; }
    let count = *rest.first().ok_or("missing H264 picture parameter sets")?; rest = &rest[1..];
    for _ in 0..count { append(&mut rest, &mut output)?; }
    Ok(output)
}
/// A selected encoder must produce an independently decodable first packet,
/// with bounded SPS/PPS and an IDR, before it can be reported as hardware.
pub fn validate_initial_packet(bytes: &[u8]) -> Result<(), String> {
    let mut starts = Vec::new(); let mut i = 0;
    while i + 3 <= bytes.len() {
        let prefix = if bytes[i..].starts_with(&[0,0,0,1]) { 4 } else if bytes[i..].starts_with(&[0,0,1]) { 3 } else { i += 1; continue; };
        starts.push((i, i + prefix)); i += prefix;
    }
    let (mut sps, mut pps, mut idr) = (false, false, false);
    for (index, (_, start)) in starts.iter().enumerate() {
        let end = starts.get(index + 1).map(|s| s.0).unwrap_or(bytes.len());
        if *start >= end { return Err("empty encoder NAL".into()); }
        let nal = &bytes[*start..end];
        if nal[0] & 0x80 != 0 { return Err("invalid encoder NAL".into()); }
        match nal[0] & 31 {
            7 => { validate_sps(nal)?; sps = true; },
            8 => pps = true,
            5 => idr = true,
            _ => {},
        }
    }
    if !sps || !pps || !idr { return Err("encoder omitted initial SPS/PPS/IDR".into()); }
    Ok(())
}
#[cfg(test)]
mod normalization_tests {
    use super::*;
    #[test] fn normalizes_avcc_and_rejects_invalid_lengths() {
        assert_eq!(normalize_access_unit(&[0,0,0,2,0x65,1]).unwrap(), vec![0,0,0,1,0x65,1]);
        assert!(normalize_access_unit(&[0,0,0,100,0x65]).is_err());
        assert!(validate_initial_packet(&[0,0,0,1,0x65,1]).is_err());
    }
    #[test] fn configuration_record_parameter_sets_are_annex_b() {
        let header = [1,66,0,31,255,225,0,2,0x67,0,1,0,2,0x68,0];
        assert_eq!(normalize_sequence_header(&header).unwrap(), vec![0,0,0,1,0x67,0,0,0,0,1,0x68,0]);
    }
}
