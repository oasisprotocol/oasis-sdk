//! A simple TLS record parser that extracts the SNI extension data from TLS hello records.
use std::io::{self, Cursor, Seek};

use anyhow::{anyhow, Result};
use byteorder::{BigEndian, ReadBytesExt};

pub(crate) const TLS_MAX_RECORD_SIZE: usize = 16 * 1024;
const TLS_RECORD_HEADER_LENGTH: usize = 5;
const TLS_HANDSHAKE_HEADER_LENGTH: usize = 4;
const TLS_TYPE_HANDSHAKE: u8 = 22;
const TLS_MESSAGE_TYPE_CLIENT_HELLO: u8 = 1;
const TLS_EXTENSION_TYPE_SNI: u16 = 0;
const TLS_SNI_TYPE_HOSTNAME: u8 = 0;

/// Attempt to parse the SNI hostname from the given buffer.
///
/// The buffer should contain a TLS ClientHello handshake record and may be incomplete.
/// In case there is not enough data available in the buffer, the function will return
/// `Ok(None)`, indicating that the current bytes did not contain the SNI extension.
pub fn parse(buf: &[u8]) -> Result<Option<String>> {
    if buf.len() < TLS_RECORD_HEADER_LENGTH + TLS_HANDSHAKE_HEADER_LENGTH {
        return Ok(None);
    }
    let buf = &buf[..buf.len().min(TLS_MAX_RECORD_SIZE)];

    // TLS record header (5 bytes):
    //   type:   u8
    //   major:  u8
    //   minor:  u8
    //   length: u16
    if buf[0] != TLS_TYPE_HANDSHAKE {
        return Err(anyhow!("not a valid TLS handshake record"));
    }

    // TLS handshake header (4 bytes):
    //   message type:   u8
    //   message length: [u8; 3]
    if buf[5] != TLS_MESSAGE_TYPE_CLIENT_HELLO {
        return Err(anyhow!("not a valid TLS ClientHello message"));
    }
    let buf = &buf[TLS_RECORD_HEADER_LENGTH + TLS_HANDSHAKE_HEADER_LENGTH..];

    // ClientHello.
    match parse_hello(buf) {
        Ok(result) => Ok(result),
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn parse_hello(buf: &[u8]) -> io::Result<Option<String>> {
    let mut buf = Cursor::new(buf);

    // Header.
    buf.seek_relative(34)?; // Skip constant-size fields.

    // Skip session ID.
    let length = buf.read_u8()?;
    if length > 32 {
        return Err(io::Error::other("corrupted session ID"));
    }
    buf.seek_relative(length as i64)?;
    // Skip cipher suite.
    let length = buf.read_u16::<BigEndian>()?;
    if length < 2 || length % 2 != 0 {
        return Err(io::Error::other("corrupted cipher suite"));
    }
    buf.seek_relative(length as i64)?;
    // Skip compression method.
    let length = buf.read_u8()?;
    if length < 1 {
        return Err(io::Error::other("corrupted compression method"));
    }
    buf.seek_relative(length as i64)?;

    // Extensions.
    let _extensions_length = buf.read_u16::<BigEndian>()?;
    loop {
        // Extension header:
        //   type:   u16
        //   length: u16
        let extension_type = buf.read_u16::<BigEndian>()?;
        let extension_length = buf.read_u16::<BigEndian>()?;
        if extension_type != TLS_EXTENSION_TYPE_SNI {
            // Skip over non-SNI extensions.
            buf.seek_relative(extension_length as i64)?;
            continue;
        }

        // SNI extension header:
        //   server name list length: u16
        let _list_length = buf.read_u16::<BigEndian>()?;
        loop {
            let name_type = buf.read_u8()?;
            let name_length = buf.read_u16::<BigEndian>()?;
            if name_type != TLS_SNI_TYPE_HOSTNAME {
                // Skip over non-hostname SNI.
                buf.seek_relative(name_length as i64)?;
                continue;
            }

            let name_length = name_length as usize;
            if buf.get_ref().len() - (buf.position() as usize) < name_length {
                return Err(io::Error::other("corrupted SNI extension: bad name length"));
            }
            let buf = &buf.get_ref()[buf.position() as usize..];
            let name = str::from_utf8(&buf[..name_length])
                .map_err(|_| io::Error::other("corrupted SNI extension: bad name"))?;
            return Ok(Some(name.to_string()));
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Valid record, including an SNI but no ALPN extension.
    const RECORD_SNI: &[u8] = &[
        22, 3, 1, 1, 54, 1, 0, 1, 50, 3, 3, 203, 69, 166, 24, 168, 5, 235, 3, 40, 94, 250, 34, 63,
        198, 156, 194, 25, 13, 0, 80, 200, 213, 125, 74, 215, 165, 193, 219, 143, 84, 201, 35, 32,
        232, 149, 249, 110, 18, 24, 36, 194, 152, 145, 10, 139, 7, 175, 172, 173, 61, 56, 71, 185,
        191, 71, 213, 156, 229, 62, 54, 91, 75, 253, 9, 104, 0, 72, 19, 2, 19, 3, 19, 1, 19, 4,
        192, 44, 192, 48, 204, 169, 204, 168, 192, 173, 192, 43, 192, 47, 192, 172, 192, 35, 192,
        39, 192, 10, 192, 20, 192, 9, 192, 19, 0, 157, 192, 157, 0, 156, 192, 156, 0, 61, 0, 60, 0,
        53, 0, 47, 0, 159, 204, 170, 192, 159, 0, 158, 192, 158, 0, 107, 0, 103, 0, 57, 0, 51, 0,
        255, 1, 0, 0, 161, 0, 0, 0, 16, 0, 14, 0, 0, 11, 101, 120, 97, 109, 112, 108, 101, 46, 110,
        101, 116, 0, 11, 0, 4, 3, 0, 1, 2, 0, 10, 0, 22, 0, 20, 0, 29, 0, 23, 0, 30, 0, 25, 0, 24,
        1, 0, 1, 1, 1, 2, 1, 3, 1, 4, 0, 35, 0, 0, 0, 22, 0, 0, 0, 23, 0, 0, 0, 13, 0, 34, 0, 32,
        4, 3, 5, 3, 6, 3, 8, 7, 8, 8, 8, 9, 8, 10, 8, 11, 8, 4, 8, 5, 8, 6, 4, 1, 5, 1, 6, 1, 3, 3,
        3, 1, 0, 43, 0, 5, 4, 3, 4, 3, 3, 0, 45, 0, 2, 1, 1, 0, 51, 0, 38, 0, 36, 0, 29, 0, 32,
        240, 147, 220, 154, 241, 161, 127, 109, 148, 66, 113, 35, 83, 38, 72, 28, 160, 33, 215,
        192, 53, 121, 246, 185, 203, 110, 197, 32, 128, 254, 152, 97,
    ];

    // Valid record, including an SNI and an ALPN extension.
    const RECORD_SNI_ALPN: &[u8] = &[
        22, 3, 1, 1, 71, 1, 0, 1, 67, 3, 3, 200, 84, 240, 198, 191, 79, 87, 134, 132, 184, 32, 142,
        147, 79, 172, 138, 254, 33, 184, 196, 224, 73, 186, 162, 178, 28, 93, 80, 154, 180, 197,
        117, 32, 105, 182, 50, 2, 25, 6, 98, 98, 89, 78, 89, 134, 43, 34, 138, 16, 244, 31, 185,
        254, 246, 209, 12, 203, 31, 69, 37, 134, 237, 216, 165, 5, 0, 72, 19, 2, 19, 3, 19, 1, 19,
        4, 192, 44, 192, 48, 204, 169, 204, 168, 192, 173, 192, 43, 192, 47, 192, 172, 192, 35,
        192, 39, 192, 10, 192, 20, 192, 9, 192, 19, 0, 157, 192, 157, 0, 156, 192, 156, 0, 61, 0,
        60, 0, 53, 0, 47, 0, 159, 204, 170, 192, 159, 0, 158, 192, 158, 0, 107, 0, 103, 0, 57, 0,
        51, 0, 255, 1, 0, 0, 178, 0, 0, 0, 16, 0, 14, 0, 0, 11, 101, 120, 97, 109, 112, 108, 101,
        46, 110, 101, 116, 0, 11, 0, 4, 3, 0, 1, 2, 0, 10, 0, 22, 0, 20, 0, 29, 0, 23, 0, 30, 0,
        25, 0, 24, 1, 0, 1, 1, 1, 2, 1, 3, 1, 4, 0, 35, 0, 0, 0, 16, 0, 13, 0, 11, 10, 97, 99, 109,
        101, 45, 116, 108, 115, 47, 49, 0, 22, 0, 0, 0, 23, 0, 0, 0, 13, 0, 34, 0, 32, 4, 3, 5, 3,
        6, 3, 8, 7, 8, 8, 8, 9, 8, 10, 8, 11, 8, 4, 8, 5, 8, 6, 4, 1, 5, 1, 6, 1, 3, 3, 3, 1, 0,
        43, 0, 5, 4, 3, 4, 3, 3, 0, 45, 0, 2, 1, 1, 0, 51, 0, 38, 0, 36, 0, 29, 0, 32, 205, 54,
        119, 60, 111, 182, 114, 106, 157, 109, 117, 208, 183, 128, 208, 86, 101, 69, 206, 87, 119,
        236, 20, 71, 211, 71, 215, 186, 239, 195, 3, 21,
    ];

    // Valid record, no extension.
    const RECORD_NO_EXT: &[u8] = &[
        22, 3, 1, 1, 34, 1, 0, 1, 30, 3, 3, 174, 236, 43, 233, 60, 1, 225, 235, 52, 225, 121, 90,
        72, 102, 153, 32, 127, 186, 243, 82, 5, 211, 126, 210, 140, 62, 55, 13, 105, 153, 87, 230,
        32, 242, 103, 97, 74, 54, 19, 236, 162, 139, 127, 239, 150, 191, 164, 241, 242, 223, 41,
        73, 93, 70, 173, 109, 216, 49, 64, 180, 72, 158, 82, 151, 159, 0, 72, 19, 2, 19, 3, 19, 1,
        19, 4, 192, 44, 192, 48, 204, 169, 204, 168, 192, 173, 192, 43, 192, 47, 192, 172, 192, 35,
        192, 39, 192, 10, 192, 20, 192, 9, 192, 19, 0, 157, 192, 157, 0, 156, 192, 156, 0, 61, 0,
        60, 0, 53, 0, 47, 0, 159, 204, 170, 192, 159, 0, 158, 192, 158, 0, 107, 0, 103, 0, 57, 0,
        51, 0, 255, 1, 0, 0, 141, 0, 11, 0, 4, 3, 0, 1, 2, 0, 10, 0, 22, 0, 20, 0, 29, 0, 23, 0,
        30, 0, 25, 0, 24, 1, 0, 1, 1, 1, 2, 1, 3, 1, 4, 0, 35, 0, 0, 0, 22, 0, 0, 0, 23, 0, 0, 0,
        13, 0, 34, 0, 32, 4, 3, 5, 3, 6, 3, 8, 7, 8, 8, 8, 9, 8, 10, 8, 11, 8, 4, 8, 5, 8, 6, 4, 1,
        5, 1, 6, 1, 3, 3, 3, 1, 0, 43, 0, 5, 4, 3, 4, 3, 3, 0, 45, 0, 2, 1, 1, 0, 51, 0, 38, 0, 36,
        0, 29, 0, 32, 87, 236, 148, 113, 132, 227, 66, 188, 129, 107, 224, 171, 174, 68, 70, 34,
        200, 235, 65, 252, 62, 213, 12, 28, 115, 126, 46, 52, 72, 108, 158, 10,
    ];

    #[test]
    fn test_parse_sni() {
        let result = parse(RECORD_SNI).unwrap();
        assert_eq!(result, Some("example.net".to_string()));

        let result = parse(RECORD_SNI_ALPN).unwrap();
        assert_eq!(result, Some("example.net".to_string()));

        let result = parse(RECORD_NO_EXT).unwrap();
        assert_eq!(result, None);
    }
}
