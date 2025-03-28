//! Parsing of ORC bundle manifest.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Context, Result};

/// Maximum number of extra kernel options.
const MAX_EXTRA_KERNEL_OPTIONS: usize = 32;

/// An ORC manifest.
#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub version: Version,

    pub id: String,

    pub components: Vec<Component>,

    pub digests: BTreeMap<String, String>,
}

impl Manifest {
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
}

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
)]
#[serde(deny_unknown_fields)]
pub enum Kind {
    #[default]
    #[serde(skip)]
    Invalid,

    #[serde(rename = "ronl")]
    Ronl,

    #[serde(rename = "rofl")]
    Rofl,
}

/// An ORC component.
#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub kind: Kind,

    pub name: String,

    pub version: Version,

    #[serde(default)]
    pub elf: Option<ElfMetadata>,

    #[serde(default)]
    pub sgx: Option<SgxMetadata>,

    #[serde(default)]
    pub tdx: Option<TdxMetadata>,

    pub identity: Vec<Identity>,
}

impl Component {
    pub fn id(&self) -> (Kind, String) {
        (self.kind, self.name.clone())
    }

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
            Kind::Rofl => {}
            _ => return Err(anyhow!("unsupported component kind")),
        }

        Ok(())
    }
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    #[serde(default)]
    pub major: u16,

    #[serde(default)]
    pub minor: u16,

    #[serde(default)]
    pub patch: u16,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElfMetadata {
    pub executable: String,
}

impl ElfMetadata {
    pub fn validate(&self) -> Result<()> {
        if self.executable.is_empty() {
            return Err(anyhow!("executable must be set"));
        }
        Ok(())
    }
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SgxMetadata {
    pub executable: String,
    pub signature: String,
}

impl SgxMetadata {
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

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TdxMetadata {
    pub firmware: String,
    pub kernel: String,
    #[serde(default)]
    pub initrd: String,
    #[serde(default)]
    pub extra_kernel_options: Vec<String>,

    #[serde(default)]
    pub stage2_image: String,
    #[serde(default)]
    pub stage2_format: String,
    #[serde(default)]
    pub stage2_persist: bool,

    pub resources: TdxResources,
}

impl TdxMetadata {
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

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TdxResources {
    pub memory: u64,
    pub cpus: u16,
    #[serde(default)]
    pub gpu: Option<GpuResource>,
}

impl TdxResources {
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

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuResource {
    #[serde(default)]
    pub model: String,
    pub count: u8,
}

impl GpuResource {
    pub fn validate(&self) -> Result<()> {
        if self.count < 1 {
            return Err(anyhow!("GPU count must be at least 1"));
        }
        Ok(())
    }
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    #[serde(default)]
    pub hypervisor: String,
    pub enclave: String,
}
