//! Hand-rolled protobuf codec for the inter-core IPC envelope.
//!
//! This mirrors `schema/v1/ipc.proto` (`IpcEnvelope` and its `oneof`) for cores
//! that cannot run the `buffa` runtime — specifically the AST2700 BootMCU, whose
//! `riscv32imc` core has no atomic compare-exchange (buffa's `bytes`/`once_cell`
//! deps require it). The encoding is standard protobuf proto3 wire format, so it
//! interoperates byte-for-byte with the `buffa` (SSP/TSP) and `protoc-gen-go`
//! (CA35) sides that share the same `.proto`.
//!
//! `no_std`, no allocation: encode/decode operate on caller-provided buffers.
//! Only the small scalar message set the BootMCU needs is implemented; unknown
//! fields are skipped on decode so the schema can grow without breaking it.
//!
//! Field numbers (must match `ipc.proto`):
//! - `IpcEnvelope`: 1 `seq`, 2 `ping`, 3 `pong`, 4 `get_rot_status`, 5 `rot_status`
//! - `Ping`/`Pong`: 1 `nonce`
//! - `RotStatus`: 1 `silicon_rev`, 2 `caliptra_flow_status`,
//!   3 `caliptra_boot_status`, 4 `secure_boot_enabled`, 5 `caliptra_rt_ready`

#![cfg_attr(not(test), no_std)]

/// Largest encoded envelope that fits one 32-byte mailbox slot after the 1-byte
/// length prefix the transport prepends.
pub const MAX_ENVELOPE_LEN: usize = 31;

// Protobuf wire types.
const WT_VARINT: u8 = 0;
const WT_I64: u8 = 1;
const WT_LEN: u8 = 2;
const WT_I32: u8 = 5;

/// Decoded `RotStatus` (see `ipc.proto`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RotStatus {
    pub silicon_rev: u32,
    pub caliptra_flow_status: u32,
    pub caliptra_boot_status: u32,
    pub secure_boot_enabled: bool,
    pub caliptra_rt_ready: bool,
}

/// The `IpcEnvelope.payload` oneof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payload {
    /// No oneof case set.
    None,
    Ping(u32),
    Pong(u32),
    GetRotStatus,
    RotStatus(RotStatus),
}

/// A decoded/encodable `IpcEnvelope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Envelope {
    pub seq: u32,
    pub payload: Payload,
}

impl Envelope {
    pub const fn new(seq: u32, payload: Payload) -> Self {
        Self { seq, payload }
    }
}

// ── varint ──────────────────────────────────────────────────────────────────

/// Append a base-128 varint. Returns false if `buf` is too small.
fn put_varint(buf: &mut [u8], pos: &mut usize, mut v: u64) -> bool {
    loop {
        if *pos >= buf.len() {
            return false;
        }
        let mut byte = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf[*pos] = byte;
        *pos += 1;
        if v == 0 {
            return true;
        }
    }
}

/// Read a base-128 varint. Returns None on truncation or overlong (>10 byte) input.
fn get_varint(buf: &[u8], pos: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        if *pos >= buf.len() || shift >= 64 {
            return None;
        }
        let byte = buf[*pos];
        *pos += 1;
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
    }
}

fn put_tag(buf: &mut [u8], pos: &mut usize, field: u8, wire: u8) -> bool {
    put_varint(buf, pos, ((field as u64) << 3) | wire as u64)
}

/// Encode a proto3 scalar `uint32` field, omitting it when zero (proto3 default).
fn put_u32(buf: &mut [u8], pos: &mut usize, field: u8, v: u32) -> bool {
    if v == 0 {
        return true;
    }
    put_tag(buf, pos, field, WT_VARINT) && put_varint(buf, pos, v as u64)
}

/// Encode a proto3 `bool` field, omitting it when false (proto3 default).
fn put_bool(buf: &mut [u8], pos: &mut usize, field: u8, v: bool) -> bool {
    if !v {
        return true;
    }
    put_tag(buf, pos, field, WT_VARINT) && put_varint(buf, pos, 1)
}

// ── RotStatus body ────────────────────────────────────────────────────────────

fn encode_rot_status(rs: &RotStatus, buf: &mut [u8]) -> Option<usize> {
    let mut pos = 0;
    let ok = put_u32(buf, &mut pos, 1, rs.silicon_rev)
        && put_u32(buf, &mut pos, 2, rs.caliptra_flow_status)
        && put_u32(buf, &mut pos, 3, rs.caliptra_boot_status)
        && put_bool(buf, &mut pos, 4, rs.secure_boot_enabled)
        && put_bool(buf, &mut pos, 5, rs.caliptra_rt_ready);
    if ok {
        Some(pos)
    } else {
        None
    }
}

fn decode_rot_status(buf: &[u8]) -> Option<RotStatus> {
    let mut rs = RotStatus::default();
    let mut pos = 0;
    while pos < buf.len() {
        let tag = get_varint(buf, &mut pos)?;
        let field = (tag >> 3) as u8;
        let wire = (tag & 0x7) as u8;
        match (field, wire) {
            (1, WT_VARINT) => rs.silicon_rev = get_varint(buf, &mut pos)? as u32,
            (2, WT_VARINT) => rs.caliptra_flow_status = get_varint(buf, &mut pos)? as u32,
            (3, WT_VARINT) => rs.caliptra_boot_status = get_varint(buf, &mut pos)? as u32,
            (4, WT_VARINT) => rs.secure_boot_enabled = get_varint(buf, &mut pos)? != 0,
            (5, WT_VARINT) => rs.caliptra_rt_ready = get_varint(buf, &mut pos)? != 0,
            _ => skip_field(buf, &mut pos, wire)?,
        }
    }
    Some(rs)
}

/// Body of a message with a single `nonce` (field 1) — Ping and Pong.
fn encode_nonce(nonce: u32, buf: &mut [u8]) -> Option<usize> {
    let mut pos = 0;
    if put_u32(buf, &mut pos, 1, nonce) {
        Some(pos)
    } else {
        None
    }
}

fn decode_nonce(buf: &[u8]) -> Option<u32> {
    let mut nonce = 0u32;
    let mut pos = 0;
    while pos < buf.len() {
        let tag = get_varint(buf, &mut pos)?;
        let field = (tag >> 3) as u8;
        let wire = (tag & 0x7) as u8;
        match (field, wire) {
            (1, WT_VARINT) => nonce = get_varint(buf, &mut pos)? as u32,
            _ => skip_field(buf, &mut pos, wire)?,
        }
    }
    Some(nonce)
}

/// Advance `pos` past a field of the given wire type. None on truncation or an
/// unsupported/invalid wire type.
fn skip_field(buf: &[u8], pos: &mut usize, wire: u8) -> Option<()> {
    match wire {
        WT_VARINT => {
            get_varint(buf, pos)?;
        }
        WT_I64 => *pos = pos.checked_add(8).filter(|p| *p <= buf.len())?,
        WT_LEN => {
            let len = get_varint(buf, pos)? as usize;
            *pos = pos.checked_add(len).filter(|p| *p <= buf.len())?;
        }
        WT_I32 => *pos = pos.checked_add(4).filter(|p| *p <= buf.len())?,
        _ => return None,
    }
    Some(())
}

/// Write a length-delimited (wire type 2) sub-message: tag, length, body.
fn put_submessage(buf: &mut [u8], pos: &mut usize, field: u8, body: &[u8]) -> bool {
    if !put_tag(buf, pos, field, WT_LEN) || !put_varint(buf, pos, body.len() as u64) {
        return false;
    }
    if *pos + body.len() > buf.len() {
        return false;
    }
    buf[*pos..*pos + body.len()].copy_from_slice(body);
    *pos += body.len();
    true
}

// ── Envelope ──────────────────────────────────────────────────────────────────

/// Encode `env` into `out`, returning the number of bytes written.
///
/// Returns `None` if `out` is too small. A scratch buffer sized for the largest
/// sub-message is used internally (no allocation).
pub fn encode(env: &Envelope, out: &mut [u8]) -> Option<usize> {
    let mut pos = 0;
    if !put_u32(out, &mut pos, 1, env.seq) {
        return None;
    }
    let mut scratch = [0u8; 32];
    match &env.payload {
        Payload::None => {}
        Payload::Ping(n) => {
            let len = encode_nonce(*n, &mut scratch)?;
            if !put_submessage(out, &mut pos, 2, &scratch[..len]) {
                return None;
            }
        }
        Payload::Pong(n) => {
            let len = encode_nonce(*n, &mut scratch)?;
            if !put_submessage(out, &mut pos, 3, &scratch[..len]) {
                return None;
            }
        }
        Payload::GetRotStatus => {
            if !put_submessage(out, &mut pos, 4, &[]) {
                return None;
            }
        }
        Payload::RotStatus(rs) => {
            let len = encode_rot_status(rs, &mut scratch)?;
            if !put_submessage(out, &mut pos, 5, &scratch[..len]) {
                return None;
            }
        }
    }
    Some(pos)
}

/// Decode an `IpcEnvelope` from `buf`. Unknown fields are skipped; the last
/// oneof case present wins (proto3 semantics). Returns `None` on malformed input.
pub fn decode(buf: &[u8]) -> Option<Envelope> {
    let mut env = Envelope {
        seq: 0,
        payload: Payload::None,
    };
    let mut pos = 0;
    while pos < buf.len() {
        let tag = get_varint(buf, &mut pos)?;
        let field = (tag >> 3) as u8;
        let wire = (tag & 0x7) as u8;
        match (field, wire) {
            (1, WT_VARINT) => env.seq = get_varint(buf, &mut pos)? as u32,
            (2, WT_LEN) => env.payload = Payload::Ping(read_len_msg(buf, &mut pos, decode_nonce)?),
            (3, WT_LEN) => env.payload = Payload::Pong(read_len_msg(buf, &mut pos, decode_nonce)?),
            (4, WT_LEN) => {
                let body = read_len_slice(buf, &mut pos)?;
                let _ = body; // empty message
                env.payload = Payload::GetRotStatus;
            }
            (5, WT_LEN) => {
                env.payload = Payload::RotStatus(read_len_msg(buf, &mut pos, decode_rot_status)?)
            }
            _ => skip_field(buf, &mut pos, wire)?,
        }
    }
    Some(env)
}

/// Read a length-delimited slice, advancing `pos` past it.
fn read_len_slice<'a>(buf: &'a [u8], pos: &mut usize) -> Option<&'a [u8]> {
    let len = get_varint(buf, pos)? as usize;
    let end = pos.checked_add(len).filter(|p| *p <= buf.len())?;
    let body = &buf[*pos..end];
    *pos = end;
    Some(body)
}

/// Read a length-delimited sub-message and decode it with `f`.
fn read_len_msg<T>(buf: &[u8], pos: &mut usize, f: fn(&[u8]) -> Option<T>) -> Option<T> {
    let body = read_len_slice(buf, pos)?;
    f(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(env: Envelope) {
        let mut buf = [0u8; 64];
        let n = encode(&env, &mut buf).expect("encode");
        let got = decode(&buf[..n]).expect("decode");
        assert_eq!(env, got);
    }

    #[test]
    fn golden_ping() {
        // Envelope{ seq: 7, ping: Ping{ nonce: 0x1234 } }
        // seq f1 varint: 08 07
        // ping f2 LEN: 12 03  { nonce f1 varint: 08 B4 24 }  (0x1234 = varint B4 24)
        let env = Envelope::new(7, Payload::Ping(0x1234));
        let mut buf = [0u8; 64];
        let n = encode(&env, &mut buf).unwrap();
        assert_eq!(&buf[..n], &[0x08, 0x07, 0x12, 0x03, 0x08, 0xB4, 0x24]);
    }

    #[test]
    fn golden_get_rot_status() {
        // Envelope{ seq: 0, get_rot_status: {} } -> f4 LEN, len 0: 22 00
        let env = Envelope::new(0, Payload::GetRotStatus);
        let mut buf = [0u8; 64];
        let n = encode(&env, &mut buf).unwrap();
        assert_eq!(&buf[..n], &[0x22, 0x00]);
    }

    #[test]
    fn rot_status_defaults_omitted() {
        // All-default RotStatus encodes to an empty body: envelope is f5 LEN 00.
        let env = Envelope::new(1, Payload::RotStatus(RotStatus::default()));
        let mut buf = [0u8; 64];
        let n = encode(&env, &mut buf).unwrap();
        assert_eq!(&buf[..n], &[0x08, 0x01, 0x2A, 0x00]);
    }

    #[test]
    fn roundtrips() {
        roundtrip(Envelope::new(0, Payload::Ping(0)));
        roundtrip(Envelope::new(42, Payload::Pong(0xDEAD_BEEF)));
        roundtrip(Envelope::new(0, Payload::GetRotStatus));
        roundtrip(Envelope::new(
            0xFFFF_FFFF,
            Payload::RotStatus(RotStatus {
                silicon_rev: 0x2750_00A2,
                caliptra_flow_status: 0x0000_080A,
                caliptra_boot_status: 0x1234,
                secure_boot_enabled: false,
                caliptra_rt_ready: true,
            }),
        ));
    }

    #[test]
    fn unknown_fields_skipped() {
        // seq=5, then an unknown field 9 varint, then get_rot_status.
        // 08 05 | 48 63 (f9 varint 0x63) | 22 00
        let buf = [0x08, 0x05, 0x48, 0x63, 0x22, 0x00];
        let env = decode(&buf).unwrap();
        assert_eq!(env.seq, 5);
        assert_eq!(env.payload, Payload::GetRotStatus);
    }

    #[test]
    fn truncated_is_none() {
        assert!(decode(&[0x08]).is_none()); // varint tag with no value
        assert!(decode(&[0x12, 0x05, 0x00]).is_none()); // len says 5, only 1 byte
    }
}
