use std::convert::TryInto as _;

use oasis_runtime_sdk::core::common::crypto::mrae::deoxysii::{DeoxysII, KEY_SIZE, NONCE_SIZE};

/// DeoxysII encryption and decryption errors.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("malformed encryption key")]
    MalformedKey,
    #[error("malformed nonce")]
    MalformedNonce,
    #[error("unable to decrypt message or authenticate additional data")]
    DecryptionFailed,
}

/// Encrypt and authenticate a message and authenticate additional data using
/// DeoxysII.
pub fn seal(
    key: &[u8],
    nonce: &[u8],
    message: &[u8],
    additional_text: &[u8],
) -> Result<Vec<u8>, Error> {
    let key: [u8; KEY_SIZE] = key.try_into().map_err(|_| Error::MalformedKey)?;
    let nonce: [u8; NONCE_SIZE] = nonce.try_into().map_err(|_| Error::MalformedNonce)?;
    let deoxysii = DeoxysII::new(&key);
    let encrypted = deoxysii.seal(&nonce, message, additional_text);
    Ok(encrypted)
}

/// Decrypt and authenticate a message and authenticate additional data using
/// DeoxysII.
pub fn open(
    key: &[u8],
    nonce: &[u8],
    message: &[u8],
    additional_text: &[u8],
) -> Result<Vec<u8>, Error> {
    let key: [u8; KEY_SIZE] = key.try_into().map_err(|_| Error::MalformedKey)?;
    let nonce: [u8; NONCE_SIZE] = nonce.try_into().map_err(|_| Error::MalformedNonce)?;
    let mut message = message.to_vec();
    let deoxysii = DeoxysII::new(&key);
    let decrypted = deoxysii
        .open(&nonce, &mut message, additional_text)
        .map_err(|_| Error::DecryptionFailed)?;
    Ok(decrypted)
}

#[cfg(test)]
mod test {
    use hex::FromHex;

    use super::*;

    #[test]
    fn basic_roundtrip() {
        let key = <[u8; KEY_SIZE] as FromHex>::from_hex(
            "e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586",
        )
        .unwrap();
        let nonce = b"0123456789abcde";
        let message = b"a message to mangle";
        let ad = b"additional data";

        let ciphered = seal(&key, nonce, message, ad).unwrap();
        assert!(open(&key, nonce, message, b"some other additional data").is_err());
        assert_eq!(open(&key, nonce, &ciphered, ad).unwrap(), message);
    }

    #[test]
    fn arg_checking() {
        let key = <[u8; KEY_SIZE] as FromHex>::from_hex(
            "e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586",
        )
        .unwrap();
        let nonce = b"0123456789abcde";
        let message = b"a message to mangle";
        let ad = b"additional data";

        assert!(matches!(
            seal(&key[..(KEY_SIZE - 1)], nonce, message, ad).unwrap_err(),
            Error::MalformedKey
        ));
        assert!(matches!(
            seal(&key, b"0123456789abcdef", message, ad).unwrap_err(),
            Error::MalformedNonce
        ));
        assert!(seal(&key, nonce, b"", b"").is_ok());
    }
}
