//! Parsing of ORC bundle manifest.
//!
//! Note that this implements a safe subset used by the ROFL scheduler.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Context, Result};
use oasis_runtime_sdk::core::common::{
    crypto::hash::Hash,
    namespace::Namespace,
    sgx::{EnclaveIdentity, MrEnclave, MrSigner},
};

/// Name of the manifest file inside the ORC archive.
pub const MANIFEST_FILE_NAME: &str = "META-INF/MANIFEST.MF";

/// Maximum number of extra kernel options.
const MAX_EXTRA_KERNEL_OPTIONS: usize = 32;
/// Minimum length of a component name.
const MIN_COMPONENT_NAME_LENGTH: usize = 3;
/// Maximum length of a component name.
const MAX_COMPONENT_NAME_LENGTH: usize = 128;

/// An ORC manifest.
#[derive(
    Clone, Default, Debug, serde::Serialize, serde::Deserialize, cbor::Encode, cbor::Decode,
)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    #[cbor(optional)]
    pub name: String,

    #[serde(default)]
    #[cbor(optional, skip_serializing_if = "Version::is_empty")]
    pub version: Version,

    #[serde(with = "serde_namespace_hex")]
    pub id: Namespace,

    #[cbor(optional)]
    pub components: Vec<Component>,

    #[serde(with = "serde_digests_hex")]
    pub digests: BTreeMap<String, Hash>,
}

impl Manifest {
    /// Validate the manifest for well-formedness.
    pub fn validate(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        for component in &self.components {
            if ids.contains(&component.id()) {
                return Err(anyhow!("duplicate component identifier"));
            }
            ids.insert(component.id());

            component.validate()?;
        }
        Ok(())
    }

    /// Manifest hash.
    pub fn hash(&self) -> Hash {
        Hash::digest_bytes(&cbor::to_vec(self.clone()))
    }
}

/// Component kind.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    cbor::Encode,
    cbor::Decode,
)]
#[serde(deny_unknown_fields)]
pub enum Kind {
    #[default]
    #[serde(skip)]
    #[cbor(skip)]
    Invalid,

    #[serde(rename = "ronl")]
    #[cbor(rename = "ronl")]
    Ronl,

    #[serde(rename = "rofl")]
    #[cbor(rename = "rofl")]
    Rofl,
}

/// An ORC component.
#[derive(
    Clone, Default, Debug, serde::Serialize, serde::Deserialize, cbor::Encode, cbor::Decode,
)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub kind: Kind,

    #[cbor(optional)]
    pub name: String,

    #[cbor(optional, skip_serializing_if = "Version::is_empty")]
    pub version: Version,

    #[serde(default)]
    #[cbor(optional)]
    pub elf: Option<ElfMetadata>,

    #[serde(default)]
    #[cbor(optional)]
    pub sgx: Option<SgxMetadata>,

    #[serde(default)]
    #[cbor(optional)]
    pub tdx: Option<TdxMetadata>,

    #[cbor(optional)]
    pub identity: Vec<Identity>,

    #[serde(default)]
    #[cbor(optional)]
    pub disabled: bool,
}

impl Component {
    /// Unique component identifier.
    pub fn id(&self) -> (Kind, String) {
        (self.kind, self.name.clone())
    }

    /// Validate the component for well-formedness.
    pub fn validate(&self) -> Result<()> {
        if self.sgx.is_some() && self.tdx.is_some() {
            return Err(anyhow!(
                "each component can only include metadata for a single TEE"
            ));
        }
        if let Some(elf) = &self.elf {
            elf.validate().context("elf")?;
        }
        if let Some(sgx) = &self.sgx {
            sgx.validate().context("sgx")?;
        }
        if let Some(tdx) = &self.tdx {
            tdx.validate().context("tdx")?;
        }

        match self.kind {
            Kind::Rofl => {
                if self.name.len() < MIN_COMPONENT_NAME_LENGTH {
                    return Err(anyhow!("ROFL component name is too short"));
                }
                if self.name.len() > MAX_COMPONENT_NAME_LENGTH {
                    return Err(anyhow!("ROFL component name is too long"));
                }
                if self
                    .name
                    .contains(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                {
                    return Err(anyhow!("ROFL component name is invalid"));
                }
            }
            _ => return Err(anyhow!("unsupported component kind")),
        }

        if self.disabled {
            return Err(anyhow!("component should not be disabled"));
        }

        Ok(())
    }
}

/// Version.
#[derive(
    Clone, Default, Debug, serde::Serialize, serde::Deserialize, cbor::Encode, cbor::Decode,
)]
#[serde(deny_unknown_fields)]
pub struct Version {
    #[serde(default)]
    #[cbor(optional)]
    pub major: u16,

    #[serde(default)]
    #[cbor(optional)]
    pub minor: u16,

    #[serde(default)]
    #[cbor(optional)]
    pub patch: u16,
}

impl Version {
    /// Whether all version components are equal to zero.
    pub fn is_empty(&self) -> bool {
        self.major == 0 && self.minor == 0 && self.patch == 0
    }
}

/// ELF metadata.
#[derive(
    Clone, Default, Debug, serde::Serialize, serde::Deserialize, cbor::Encode, cbor::Decode,
)]
#[serde(deny_unknown_fields)]
pub struct ElfMetadata {
    pub executable: String,
}

impl ElfMetadata {
    /// Validate the ELF metadata for well-formedness.
    pub fn validate(&self) -> Result<()> {
        if self.executable.is_empty() {
            return Err(anyhow!("executable must be set"));
        }
        Ok(())
    }
}

/// SGX metadata.
#[derive(
    Clone, Default, Debug, serde::Serialize, serde::Deserialize, cbor::Encode, cbor::Decode,
)]
#[serde(deny_unknown_fields)]
pub struct SgxMetadata {
    pub executable: String,
    pub signature: String,
}

impl SgxMetadata {
    /// Validate the SGX metadata for well-formedness.
    pub fn validate(&self) -> Result<()> {
        if self.executable.is_empty() {
            return Err(anyhow!("executable must be set"));
        }
        if self.signature.is_empty() {
            return Err(anyhow!("signature must be set"));
        }
        Ok(())
    }
}

/// TDX metadata.
#[derive(
    Clone, Default, Debug, serde::Serialize, serde::Deserialize, cbor::Encode, cbor::Decode,
)]
#[serde(deny_unknown_fields)]
pub struct TdxMetadata {
    pub firmware: String,
    #[cbor(optional)]
    pub kernel: String,
    #[serde(default)]
    #[cbor(optional)]
    pub initrd: String,
    #[serde(default)]
    #[cbor(optional)]
    pub extra_kernel_options: Vec<String>,

    #[serde(default)]
    #[cbor(optional)]
    pub stage2_image: String,
    #[serde(default)]
    #[cbor(optional)]
    pub stage2_format: String,
    #[serde(default)]
    #[cbor(optional)]
    pub stage2_persist: bool,

    pub resources: TdxResources,
}

impl TdxMetadata {
    /// Validate the TDX metadata for well-formedness.
    pub fn validate(&self) -> Result<()> {
        if self.firmware.is_empty() {
            return Err(anyhow!("firmware must be set"));
        }
        if self.kernel.is_empty() && !self.stage2_image.is_empty() {
            return Err(anyhow!("kernel must be set if stage 2 image is set"));
        }
        if self.kernel.is_empty() && !self.initrd.is_empty() {
            return Err(anyhow!("kernel must be set if initrd image is set"));
        }
        if self.kernel.is_empty() && !self.extra_kernel_options.is_empty() {
            return Err(anyhow!(
                "kernel must be set if extra kernel options are set"
            ));
        }
        if self.extra_kernel_options.len() > MAX_EXTRA_KERNEL_OPTIONS {
            return Err(anyhow!("too many extra kernel options"));
        }
        match self.stage2_format.as_ref() {
            "qcow2" => {}
            _ => return Err(anyhow!("specified stage 2 format is not supported")),
        }

        self.resources.validate().context("resources")?;
        Ok(())
    }
}

/// Requested TDX resources.
#[derive(
    Clone, Default, Debug, serde::Serialize, serde::Deserialize, cbor::Encode, cbor::Decode,
)]
#[serde(deny_unknown_fields)]
pub struct TdxResources {
    pub memory: u64,
    pub cpus: u16,

    #[serde(default)]
    #[cbor(optional)]
    pub gpu: Option<GpuResource>,
}

impl TdxResources {
    /// Validate the requested TDX resources for well-formedness.
    pub fn validate(&self) -> Result<()> {
        if self.memory < 16 {
            return Err(anyhow!("memory limit must be at least 16M"));
        }
        if self.cpus < 1 {
            return Err(anyhow!("vCPU count must be at least 1"));
        }
        if let Some(gpu) = &self.gpu {
            gpu.validate().context("gpu resource")?;
        }
        Ok(())
    }
}

/// Requested GPU resource.
#[derive(
    Clone, Default, Debug, serde::Serialize, serde::Deserialize, cbor::Encode, cbor::Decode,
)]
#[serde(deny_unknown_fields)]
pub struct GpuResource {
    #[serde(default)]
    #[cbor(optional)]
    pub model: String,
    pub count: u8,
}

impl GpuResource {
    /// Validate the requested GPU resource for well-formedness.
    pub fn validate(&self) -> Result<()> {
        if self.count < 1 {
            return Err(anyhow!("GPU count must be at least 1"));
        }
        Ok(())
    }
}

/// Component identity.
#[derive(
    Clone, Default, Debug, serde::Serialize, serde::Deserialize, cbor::Encode, cbor::Decode,
)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// Optional identifier of the hypervisor.
    #[serde(default)]
    #[cbor(optional)]
    pub hypervisor: String,

    /// Component's enclave identity.
    #[serde(with = "serde_enclave_identity_base64")]
    pub enclave: EnclaveIdentity,
}

mod serde_namespace_hex {
    use super::Namespace;

    pub fn serialize<S>(value: &Namespace, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ser.serialize_str(&format!("{:x}", value))
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Namespace, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::Deserialize;

        let raw = String::deserialize(de)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

mod serde_digests_hex {
    use super::*;

    pub fn serialize<S>(value: &BTreeMap<String, Hash>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::Serialize;

        let raw: BTreeMap<String, String> = value
            .iter()
            .map(|(name, hash)| (name.clone(), format!("{:x}", hash)))
            .collect();
        raw.serialize(ser)
    }

    pub fn deserialize<'de, D>(de: D) -> Result<BTreeMap<String, Hash>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::Deserialize;

        let raw = BTreeMap::<String, String>::deserialize(de)?;
        raw.into_iter()
            .map(|(name, raw_hash)| -> Result<_, _> { Ok((name, raw_hash.parse()?)) })
            .collect::<Result<BTreeMap<String, Hash>, rustc_hex::FromHexError>>()
            .map_err(serde::de::Error::custom)
    }
}

mod serde_enclave_identity_base64 {
    use base64::prelude::*;

    use super::*;

    pub fn serialize<S>(value: &EnclaveIdentity, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let data = [value.mr_enclave.as_ref(), value.mr_signer.as_ref()].concat();

        ser.serialize_str(&BASE64_STANDARD.encode(&data))
    }

    pub fn deserialize<'de, D>(de: D) -> Result<EnclaveIdentity, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::Deserialize;

        let raw = String::deserialize(de)?;
        let raw = BASE64_STANDARD
            .decode(raw)
            .map_err(serde::de::Error::custom)?;
        if raw.len() != MrEnclave::len() + MrSigner::len() {
            return Err(serde::de::Error::custom("malformed enclave identity"));
        }

        Ok(EnclaveIdentity {
            mr_enclave: raw[..MrEnclave::len()].into(),
            mr_signer: raw[MrEnclave::len()..].into(),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const TEST_MANIFEST_1: &str = r#"
{
  "name": "name",
  "id": "0000000000000000000000000000000000000000000000000000000000000000",
  "version": {},
  "components": [
    {
      "kind": "ronl",
      "name": "name",
      "version": {
        "major": 1,
        "minor": 2,
        "patch": 3
      },
      "elf": {
        "executable": "exe"
      },
      "sgx": {
        "executable": "exe",
        "signature": "sig"
      },
      "tdx": {
        "firmware": "firmware",
        "kernel": "kernel",
        "initrd": "initrd",
        "extra_kernel_options": ["opt1", "opt2"],
        "stage2_image": "image",
        "resources": {
          "memory": 1,
          "cpus": 2
        }
      },
      "identity": [
        {
          "hypervisor": "hypervisor",
          "enclave": "AQIDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADBAUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="
        }
      ],
      "disabled": true
    }
  ],
  "digests": {
    "a": "0100000000000000000000000000000000000000000000000000000000000000",
    "b": "0200000000000000000000000000000000000000000000000000000000000000"
  }
}
"#;

    const TEST_MANIFEST_2: &str = r#"
{
  "name": "name",
  "id": "0000000000000000000000000000000000000000000000000000000000000000",
  "version": {},
  "components": [
    {
      "kind": "ronl",
      "name": "name",
      "version": {},
      "tdx": {
        "firmware": "firmware",
        "kernel": "kernel",
        "extra_kernel_options": ["opt1", "opt2"],
        "stage2_image": "image",
        "resources": {
          "memory": 1,
          "cpus": 2
        }
      },
      "identity": [
        {
          "hypervisor": "hypervisor",
          "enclave": "AQIDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADBAUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="
        }
      ]
    }
  ],
  "digests": {
    "a": "0100000000000000000000000000000000000000000000000000000000000000",
    "b": "0200000000000000000000000000000000000000000000000000000000000000"
  }
}
"#;

    #[test]
    fn test_manifest_hash() {
        // Compare against Go test vectors.
        let tcs = [
            (
                TEST_MANIFEST_1,
                "2ddf81b85d08dbecb24571ba75858ec94871800b674d581cf37214c0d56263c3",
            ),
            (
                TEST_MANIFEST_2,
                "07c7b90d8412bf8efa6e96132888ac64dad330b0c1b538458fd05ee7a908617b",
            ),
        ];
        for tc in tcs {
            let manifest: Manifest = serde_json::from_str(tc.0).unwrap();
            let h = manifest.hash();
            assert_eq!(h, tc.1.into());

            // Ensure round-trip works.
            let enc = serde_json::to_string(&manifest).unwrap();
            let manifest: Manifest = serde_json::from_str(&enc).unwrap();
            let h = manifest.hash();
            assert_eq!(h, tc.1.into());
        }
    }
}
