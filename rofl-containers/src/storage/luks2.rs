//! Helper functions to parse and validate the LUKS2 header.
//!
//! Only the scheme that is required for our use case is currently implemented.
use std::{collections::HashMap, fmt::Display, str::FromStr};

use anyhow::{bail, Context, Result};
use serde::{de, Deserialize, Deserializer};

/// Validate a LUKS2 header to make sure it follows our requirements.
///
/// Currently the following is enforced:
/// * Exactly one key slot, segment and digest with identifier `0`.
/// * No tokens.
/// * Area and segment encryption scheme is `aes-xts-plain64`.
/// * Integrity scheme is set and is `hmac(sha256)`.
///
pub fn validate_header(data: &str) -> Result<()> {
    let header: LuksMeta = serde_json::from_str(data).context("malformed LUKS2 header")?;

    if header.keyslots.len() != 1 {
        bail!("expected exactly one keyslot");
    }

    match header.keyslots.get(&0) {
        Some(LuksKeyslot::Luks2 { key_size, area, .. }) => {
            if *key_size < 64 {
                bail!("invalid key size");
            }

            match area {
                LuksArea::Raw {
                    encryption,
                    key_size,
                    ..
                } => {
                    if encryption != "aes-xts-plain64" {
                        bail!("invalid encryption type");
                    }
                    if *key_size < 64 {
                        bail!("invalid key size");
                    }
                }
            }
        }
        None => bail!("missing key slot 0"),
    }

    if !header.tokens.is_empty() {
        bail!("expected no tokens");
    }

    if header.segments.len() != 1 {
        bail!("expected exactly one segment");
    }

    match header.segments.get(&0) {
        Some(LuksSegment::Crypt {
            encryption,
            integrity,
            ..
        }) => {
            if encryption != "aes-xts-plain64" {
                bail!("invalid encryption type");
            }

            match integrity {
                Some(LuksIntegrity { integrity_type, .. }) => {
                    if integrity_type != "hmac(sha256)" {
                        bail!("invalid integrity protection type");
                    }
                }
                None => bail!("missing integrity protection"),
            }
        }
        None => bail!("missing segment 0"),
    }

    if header.digests.len() != 1 {
        bail!("expected exactly one digest");
    }

    match header.digests.get(&0) {
        Some(LuksDigest::PBKDF2 {
            keyslots, segments, ..
        }) => {
            if !keyslots.iter().all(|k| header.keyslots.contains_key(k)) {
                bail!("bad keyslot reference in digest");
            }
            if !segments.iter().all(|s| header.segments.contains_key(s)) {
                bail!("bad segment reference in digest");
            }
        }
        None => bail!("missing digest 0"),
    }

    Ok(())
}

#[derive(Debug, Deserialize, PartialEq)]
struct LuksMeta {
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    keyslots: HashMap<u8, LuksKeyslot>,
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    tokens: HashMap<u8, LuksToken>,
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    segments: HashMap<u8, LuksSegment>,
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    digests: HashMap<u8, LuksDigest>,
    config: LuksConfig,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
enum LuksKeyslot {
    #[serde(rename = "luks2")]
    Luks2 {
        key_size: u16,
        area: LuksArea,
        kdf: LuksKdf,
        af: LuksAf,
        #[serde(deserialize_with = "deserialize_priority")]
        #[serde(default)]
        priority: Option<LuksPriority>,
    },
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
enum LuksArea {
    #[serde(rename = "raw")]
    Raw {
        encryption: String,
        key_size: u32,
        #[serde(deserialize_with = "from_str")]
        offset: u64,
        #[serde(deserialize_with = "from_str")]
        size: u64,
    },
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
enum LuksKdf {
    #[serde(rename = "pbkdf2")]
    PBKDF2 {
        salt: String,
        hash: String,
        iterations: u32,
    },

    #[serde(rename = "argon2i")]
    Argon2i {
        salt: String,
        time: u32,
        memory: u32,
        cpus: u32,
    },

    #[serde(rename = "argon2id")]
    Argon2id {
        salt: String,
        time: u32,
        memory: u32,
        cpus: u32,
    },
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
enum LuksAf {
    #[serde(rename = "luks1")]
    Luks1 { stripes: u16, hash: String },
}

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord)]
enum LuksPriority {
    Ignore,
    Normal,
    High,
}

#[derive(Debug, Deserialize, PartialEq)]
struct LuksToken {
    // No tokens are currently supported.
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
enum LuksSegment {
    #[serde(rename = "crypt")]
    Crypt {
        #[serde(deserialize_with = "from_str")]
        offset: u64,
        #[serde(deserialize_with = "deserialize_segment_size")]
        size: LuksSegmentSize,
        #[serde(deserialize_with = "from_str")]
        iv_tweak: u64,
        encryption: String,
        sector_size: u16,
        #[serde(default)]
        integrity: Option<LuksIntegrity>,
        #[serde(default)]
        flags: Option<Vec<String>>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
enum LuksSegmentSize {
    Dynamic,
    Fixed(u64),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct LuksIntegrity {
    #[serde(rename(deserialize = "type"))]
    integrity_type: String,
    journal_encryption: String,
    journal_integrity: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
enum LuksDigest {
    #[serde(rename = "pbkdf2")]
    PBKDF2 {
        #[serde(deserialize_with = "vec_from_str")]
        keyslots: Vec<u8>,
        #[serde(deserialize_with = "vec_from_str")]
        segments: Vec<u8>,
        salt: String,
        digest: String,
        hash: String,
        iterations: u32,
    },
}

#[derive(Debug, Deserialize, PartialEq)]
struct LuksConfig {
    #[serde(deserialize_with = "from_str")]
    json_size: u64,
    #[serde(deserialize_with = "from_str")]
    keyslots_size: u64,
    #[serde(default)]
    flags: Option<Vec<String>>,
    #[serde(default)]
    requirements: Option<Vec<String>>,
}

fn from_str<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr,
    T::Err: Display,
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    T::from_str(&s).map_err(de::Error::custom)
}

fn vec_from_str<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: FromStr,
    T::Err: Display,
    D: Deserializer<'de>,
{
    let v = Vec::<String>::deserialize(deserializer)?;
    v.iter()
        .map(|s| T::from_str(s).map_err(de::Error::custom))
        .collect()
}

fn deserialize_priority<'de, D>(deserializer: D) -> Result<Option<LuksPriority>, D::Error>
where
    D: Deserializer<'de>,
{
    let p = match Option::<i32>::deserialize(deserializer)? {
        Some(pr) => pr,
        None => return Ok(None),
    };
    match p {
        0 => Ok(Some(LuksPriority::Ignore)),
        1 => Ok(Some(LuksPriority::Normal)),
        2 => Ok(Some(LuksPriority::High)),
        _ => Err(de::Error::custom(format!("invalid priority {p}"))),
    }
}

fn deserialize_segment_size<'de, D>(deserializer: D) -> Result<LuksSegmentSize, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "dynamic" => Ok(LuksSegmentSize::Dynamic),
        x => Ok(LuksSegmentSize::Fixed(
            u64::from_str(x).map_err(de::Error::custom)?,
        )),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const LUKS2_HEADER_VALID: &str = r#"{
      "keyslots":{
        "0":{
          "type":"luks2",
          "key_size":96,
          "af":{
            "type":"luks1",
            "stripes":4000,
            "hash":"sha256"
          },
          "area":{
            "type":"raw",
            "offset":"32768",
            "size":"385024",
            "encryption":"aes-xts-plain64",
            "key_size":64
          },
          "kdf":{
            "type":"argon2i",
            "time":7,
            "memory":191952,
            "cpus":1,
            "salt":"VaeYp2RiRvTZLqKxggLfN2owbhkNSB9H6yGDhI9d6ko="
          }
        }
      },
      "tokens":{},
      "segments":{
        "0":{
          "type":"crypt",
          "offset":"16777216",
          "size":"dynamic",
          "iv_tweak":"0",
          "encryption":"aes-xts-plain64",
          "sector_size":512,
          "integrity":{
            "type":"hmac(sha256)",
            "journal_encryption":"none",
            "journal_integrity":"none"
          }
        }
      },
      "digests":{
        "0":{
          "type":"pbkdf2",
          "keyslots":[
            "0"
          ],
          "segments":[
            "0"
          ],
          "hash":"sha256",
          "iterations":84344,
          "salt":"CakmJdYBkOgwCHVkoMjUGEQTnNZjym0pa1hl8nWPauM=",
          "digest":"0psj0pfQ4uHA/i/sF2/HUxZnhdO8f1c3GDRuikoZx+Q="
        }
      },
      "config":{
        "json_size":"12288",
        "keyslots_size":"16744448"
      }
    }"#;

    const LUKS2_HEADER_INVALID_CIPHER1: &str = r#"{
      "keyslots":{
        "0":{
          "type":"luks2",
          "key_size":96,
          "af":{
            "type":"luks1",
            "stripes":4000,
            "hash":"sha256"
          },
          "area":{
            "type":"raw",
            "offset":"32768",
            "size":"385024",
            "encryption":"null",
            "key_size":64
          },
          "kdf":{
            "type":"argon2i",
            "time":7,
            "memory":191952,
            "cpus":1,
            "salt":"VaeYp2RiRvTZLqKxggLfN2owbhkNSB9H6yGDhI9d6ko="
          }
        }
      },
      "tokens":{},
      "segments":{
        "0":{
          "type":"crypt",
          "offset":"16777216",
          "size":"dynamic",
          "iv_tweak":"0",
          "encryption":"aes-xts-plain64",
          "sector_size":512,
          "integrity":{
            "type":"hmac(sha256)",
            "journal_encryption":"none",
            "journal_integrity":"none"
          }
        }
      },
      "digests":{
        "0":{
          "type":"pbkdf2",
          "keyslots":[
            "0"
          ],
          "segments":[
            "0"
          ],
          "hash":"sha256",
          "iterations":84344,
          "salt":"CakmJdYBkOgwCHVkoMjUGEQTnNZjym0pa1hl8nWPauM=",
          "digest":"0psj0pfQ4uHA/i/sF2/HUxZnhdO8f1c3GDRuikoZx+Q="
        }
      },
      "config":{
        "json_size":"12288",
        "keyslots_size":"16744448"
      }
    }"#;

    const LUKS2_HEADER_INVALID_CIPHER2: &str = r#"{
      "keyslots":{
        "0":{
          "type":"luks2",
          "key_size":96,
          "af":{
            "type":"luks1",
            "stripes":4000,
            "hash":"sha256"
          },
          "area":{
            "type":"raw",
            "offset":"32768",
            "size":"385024",
            "encryption":"aes-xts-plain64",
            "key_size":64
          },
          "kdf":{
            "type":"argon2i",
            "time":7,
            "memory":191952,
            "cpus":1,
            "salt":"VaeYp2RiRvTZLqKxggLfN2owbhkNSB9H6yGDhI9d6ko="
          }
        }
      },
      "tokens":{},
      "segments":{
        "0":{
          "type":"crypt",
          "offset":"16777216",
          "size":"dynamic",
          "iv_tweak":"0",
          "encryption":"null",
          "sector_size":512,
          "integrity":{
            "type":"hmac(sha256)",
            "journal_encryption":"none",
            "journal_integrity":"none"
          }
        }
      },
      "digests":{
        "0":{
          "type":"pbkdf2",
          "keyslots":[
            "0"
          ],
          "segments":[
            "0"
          ],
          "hash":"sha256",
          "iterations":84344,
          "salt":"CakmJdYBkOgwCHVkoMjUGEQTnNZjym0pa1hl8nWPauM=",
          "digest":"0psj0pfQ4uHA/i/sF2/HUxZnhdO8f1c3GDRuikoZx+Q="
        }
      },
      "config":{
        "json_size":"12288",
        "keyslots_size":"16744448"
      }
    }"#;

    const LUKS2_HEADER_INVALID_CIPHER3: &str = r#"{
      "keyslots":{
        "0":{
          "type":"luks2",
          "key_size":96,
          "af":{
            "type":"luks1",
            "stripes":4000,
            "hash":"sha256"
          },
          "area":{
            "type":"raw",
            "offset":"32768",
            "size":"385024",
            "encryption":"null",
            "key_size":64
          },
          "kdf":{
            "type":"argon2i",
            "time":7,
            "memory":191952,
            "cpus":1,
            "salt":"VaeYp2RiRvTZLqKxggLfN2owbhkNSB9H6yGDhI9d6ko="
          }
        },
        "0":{
          "type":"luks2",
          "key_size":96,
          "af":{
            "type":"luks1",
            "stripes":4000,
            "hash":"sha256"
          },
          "area":{
            "type":"raw",
            "offset":"32768",
            "size":"385024",
            "encryption":"aes-xts-plain64",
            "key_size":64
          },
          "kdf":{
            "type":"argon2i",
            "time":7,
            "memory":191952,
            "cpus":1,
            "salt":"VaeYp2RiRvTZLqKxggLfN2owbhkNSB9H6yGDhI9d6ko="
          }
        }
      },
      "tokens":{},
      "segments":{
        "0":{
          "type":"crypt",
          "offset":"16777216",
          "size":"dynamic",
          "iv_tweak":"0",
          "encryption":"aes-xts-plain64",
          "sector_size":512,
          "integrity":{
            "type":"hmac(sha256)",
            "journal_encryption":"none",
            "journal_integrity":"none"
          }
        }
      },
      "digests":{
        "0":{
          "type":"pbkdf2",
          "keyslots":[
            "0"
          ],
          "segments":[
            "0"
          ],
          "hash":"sha256",
          "iterations":84344,
          "salt":"CakmJdYBkOgwCHVkoMjUGEQTnNZjym0pa1hl8nWPauM=",
          "digest":"0psj0pfQ4uHA/i/sF2/HUxZnhdO8f1c3GDRuikoZx+Q="
        }
      },
      "config":{
        "json_size":"12288",
        "keyslots_size":"16744448"
      }
    }"#;

    const LUKS2_HEADER_INVALID_INTEGRITY1: &str = r#"{
      "keyslots":{
        "0":{
          "type":"luks2",
          "key_size":96,
          "af":{
            "type":"luks1",
            "stripes":4000,
            "hash":"sha256"
          },
          "area":{
            "type":"raw",
            "offset":"32768",
            "size":"385024",
            "encryption":"aes-xts-plain64",
            "key_size":64
          },
          "kdf":{
            "type":"argon2i",
            "time":7,
            "memory":191952,
            "cpus":1,
            "salt":"VaeYp2RiRvTZLqKxggLfN2owbhkNSB9H6yGDhI9d6ko="
          }
        }
      },
      "tokens":{},
      "segments":{
        "0":{
          "type":"crypt",
          "offset":"16777216",
          "size":"dynamic",
          "iv_tweak":"0",
          "encryption":"aes-xts-plain64",
          "sector_size":512
        }
      },
      "digests":{
        "0":{
          "type":"pbkdf2",
          "keyslots":[
            "0"
          ],
          "segments":[
            "0"
          ],
          "hash":"sha256",
          "iterations":84344,
          "salt":"CakmJdYBkOgwCHVkoMjUGEQTnNZjym0pa1hl8nWPauM=",
          "digest":"0psj0pfQ4uHA/i/sF2/HUxZnhdO8f1c3GDRuikoZx+Q="
        }
      },
      "config":{
        "json_size":"12288",
        "keyslots_size":"16744448"
      }
    }"#;

    const LUKS2_HEADER_INVALID_INTEGRITY2: &str = r#"{
      "keyslots":{
        "0":{
          "type":"luks2",
          "key_size":96,
          "af":{
            "type":"luks1",
            "stripes":4000,
            "hash":"sha256"
          },
          "area":{
            "type":"raw",
            "offset":"32768",
            "size":"385024",
            "encryption":"aes-xts-plain64",
            "key_size":64
          },
          "kdf":{
            "type":"argon2i",
            "time":7,
            "memory":191952,
            "cpus":1,
            "salt":"VaeYp2RiRvTZLqKxggLfN2owbhkNSB9H6yGDhI9d6ko="
          }
        }
      },
      "tokens":{},
      "segments":{
        "0":{
          "type":"crypt",
          "offset":"16777216",
          "size":"dynamic",
          "iv_tweak":"0",
          "encryption":"aes-xts-plain64",
          "sector_size":512,
          "integrity":{
            "type":"none",
            "journal_encryption":"none",
            "journal_integrity":"none"
          }
        }
      },
      "digests":{
        "0":{
          "type":"pbkdf2",
          "keyslots":[
            "0"
          ],
          "segments":[
            "0"
          ],
          "hash":"sha256",
          "iterations":84344,
          "salt":"CakmJdYBkOgwCHVkoMjUGEQTnNZjym0pa1hl8nWPauM=",
          "digest":"0psj0pfQ4uHA/i/sF2/HUxZnhdO8f1c3GDRuikoZx+Q="
        }
      },
      "config":{
        "json_size":"12288",
        "keyslots_size":"16744448"
      }
    }"#;

    const LUKS2_HEADER_INVALID_KEYSLOT: &str = r#"{
      "keyslots":{
        "1":{
          "type":"luks2",
          "key_size":96,
          "af":{
            "type":"luks1",
            "stripes":4000,
            "hash":"sha256"
          },
          "area":{
            "type":"raw",
            "offset":"32768",
            "size":"385024",
            "encryption":"aes-xts-plain64",
            "key_size":64
          },
          "kdf":{
            "type":"argon2i",
            "time":7,
            "memory":191952,
            "cpus":1,
            "salt":"VaeYp2RiRvTZLqKxggLfN2owbhkNSB9H6yGDhI9d6ko="
          }
        }
      },
      "tokens":{},
      "segments":{
        "0":{
          "type":"crypt",
          "offset":"16777216",
          "size":"dynamic",
          "iv_tweak":"0",
          "encryption":"aes-xts-plain64",
          "sector_size":512,
          "integrity":{
            "type":"hmac(sha256)",
            "journal_encryption":"none",
            "journal_integrity":"none"
          }
        }
      },
      "digests":{
        "0":{
          "type":"pbkdf2",
          "keyslots":[
            "1"
          ],
          "segments":[
            "0"
          ],
          "hash":"sha256",
          "iterations":84344,
          "salt":"CakmJdYBkOgwCHVkoMjUGEQTnNZjym0pa1hl8nWPauM=",
          "digest":"0psj0pfQ4uHA/i/sF2/HUxZnhdO8f1c3GDRuikoZx+Q="
        }
      },
      "config":{
        "json_size":"12288",
        "keyslots_size":"16744448"
      }
    }"#;

    #[test]
    fn test_validate_header() {
        validate_header(LUKS2_HEADER_VALID).expect("valid header validation should succeed");

        validate_header(LUKS2_HEADER_INVALID_CIPHER1)
            .expect_err("invalid cipher validation should fail");

        validate_header(LUKS2_HEADER_INVALID_CIPHER2)
            .expect_err("invalid cipher validation should fail");

        validate_header(LUKS2_HEADER_INVALID_CIPHER3)
            .expect_err("invalid cipher validation should fail");

        validate_header(LUKS2_HEADER_INVALID_INTEGRITY1)
            .expect_err("invalid integrity mode validation should fail");

        validate_header(LUKS2_HEADER_INVALID_INTEGRITY2)
            .expect_err("invalid integrity mode validation should fail");

        validate_header(LUKS2_HEADER_INVALID_KEYSLOT)
            .expect_err("invalid key slot identifier validation should fail");
    }
}
