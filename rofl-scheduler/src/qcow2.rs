//! Simple QCOW2 parser.
use anyhow::{anyhow, Result};
use byteorder::{BigEndian, ByteOrder};

/// QCOW2 file format magic bytes.
const QCOW2_MAGIC: &[u8] = b"QFI\xfb";
/// Minimum allowed size of a QCOW2 header.
const QCOW2_MIN_SIZE: usize = 64;
/// Minimum allowed QCOW2 header version.
const QCOW2_MIN_VERSION: u32 = 2;
/// Maximum allowed QCOW2 header version.
const QCOW2_MAX_VERSION: u32 = 16;
/// Offset of the version field in the QCOW2 header.
const QCOW2_V2_VERSION_OFFSET: usize = 4;
/// Offset of the virtual disk size field in the QCOW2 header.
const QCOW2_V2_SIZE_OFFSET: usize = 24;

/// Parse the virtual disk size from the QCOW2 header.
pub fn parse_virtual_size(buf: &[u8]) -> Result<u64> {
    if buf.len() < QCOW2_MIN_SIZE {
        return Err(anyhow!("QCOW2 header is too small"));
    }
    if &buf[..4] != QCOW2_MAGIC {
        return Err(anyhow!("QCOW2 header is malformed"));
    }

    let version = BigEndian::read_u32(&buf[QCOW2_V2_VERSION_OFFSET..QCOW2_V2_VERSION_OFFSET + 4]);
    if !(QCOW2_MIN_VERSION..=QCOW2_MAX_VERSION).contains(&version) {
        return Err(anyhow!("unsupported QCOW2 header version"));
    }

    let size = BigEndian::read_u64(&buf[QCOW2_V2_SIZE_OFFSET..QCOW2_V2_SIZE_OFFSET + 8]);
    Ok(size)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_virtual_size() {
        let tcs = vec![
            (
                "514649fb00000003000000000000000000000000000000100000000000fd700000000000000000010000000000030000000000000001000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000040000007000000000000000006803f857000001800000646972747920",
                Some(16609280),
            ),
            (
                "514649fb00000003494651252525e7e70000000000000069252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525252525",
                Some(2676586395008836901),
            ),
            (
                "514649fb0000000000000000000000000000",
                None, // Too small.
            ),
            (
                "514649fb00000001000000000000000000000000000000100000000000fd700000000000000000010000000000030000000000000001000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000040000007000000000000000006803f857000001800000646972747920",
                None, // Unsupported version.
            ),
            (
                "514649fb000000ff000000000000000000000000000000100000000000fd700000000000000000010000000000030000000000000001000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000040000007000000000000000006803f857000001800000646972747920",
                None, // Unsupported version.
            ),
        ];

        for tc in tcs {
            let data = hex::decode(tc.0).unwrap();
            let result = parse_virtual_size(&data);
            match tc.1 {
                None => {
                    let _ = result.unwrap_err();
                }
                Some(size) => assert_eq!(result.unwrap(), size),
            }
        }
    }
}
