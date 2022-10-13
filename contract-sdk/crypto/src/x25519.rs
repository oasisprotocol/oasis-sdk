use hmac::{Hmac, Mac as _, NewMac as _};
use sha2::Sha512Trunc256;
use x25519_dalek::{PublicKey, StaticSecret};

pub use oasis_runtime_sdk::core::common::crypto::mrae::deoxysii::KEY_SIZE;

/// x25519 key derivation errors.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("malformed public key")]
    MalformedPublicKey,
    #[error("malformed private key")]
    MalformedPrivateKey,
    #[error("key derivation function failure")]
    KeyDerivationFunctionFailure,
}

/// Derive a symmetric encryption key from the provided public/private key
/// pair.
pub fn derive_symmetric(public_key: &[u8], private_key: &[u8]) -> Result<[u8; KEY_SIZE], Error> {
    if public_key.len() != 32 {
        return Err(Error::MalformedPublicKey);
    }
    if private_key.len() != 32 {
        return Err(Error::MalformedPrivateKey);
    }

    let mut public = [0u8; 32];
    let mut private = [0u8; 32];
    public.copy_from_slice(public_key);
    private.copy_from_slice(private_key);

    let public = PublicKey::from(public);
    let private = StaticSecret::from(private);

    let mut kdf = Hmac::<Sha512Trunc256>::new_from_slice(b"MRAE_Box_Deoxys-II-256-128")
        .map_err(|_| Error::KeyDerivationFunctionFailure)?;
    kdf.update(private.diffie_hellman(&public).as_bytes());

    let mut derived_key = [0u8; KEY_SIZE];
    let digest = kdf.finalize();
    derived_key.copy_from_slice(&digest.into_bytes()[..KEY_SIZE]);

    Ok(derived_key)
}

#[cfg(test)]
mod test {
    use hex::FromHex;

    use super::*;

    #[test]
    fn derive_symmetric_basic() {
        let key_short = <[u8; 31] as FromHex>::from_hex(
            "00000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        let key_ok = <[u8; 32] as FromHex>::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        let key_long = <[u8; 33] as FromHex>::from_hex(
            "000000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        assert!(matches!(
            derive_symmetric(&key_short, &key_ok).unwrap_err(),
            Error::MalformedPublicKey
        ));
        assert!(matches!(
            derive_symmetric(&key_long, &key_ok).unwrap_err(),
            Error::MalformedPublicKey
        ));
        assert!(matches!(
            derive_symmetric(&key_ok, &key_long).unwrap_err(),
            Error::MalformedPrivateKey
        ));
        assert!(matches!(
            derive_symmetric(&key_short, &key_long).unwrap_err(),
            Error::MalformedPublicKey
        ));

        // Well-known key pair, taken from the unit test in
        // runtime-sdk::modules::evm::precompile::confidential.
        let public = <[u8; 32] as FromHex>::from_hex(
            "3046db3fa70ce605457dc47c48837ebd8bd0a26abfde5994d033e1ced68e2576",
        )
        .unwrap();
        let private = <[u8; 32] as FromHex>::from_hex(
            "c07b151fbc1e7a11dff926111188f8d872f62eba0396da97c0a24adb75161750",
        )
        .unwrap();
        let expected = <[u8; KEY_SIZE] as FromHex>::from_hex(
            "e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586",
        )
        .unwrap();

        let derived = derive_symmetric(&public, &private).unwrap();
        assert_eq!(derived, expected);
    }
}
