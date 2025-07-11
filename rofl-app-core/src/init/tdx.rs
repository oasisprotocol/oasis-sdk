//! Initialization specifix to Intel TDX.
use std::{os::unix::fs::PermissionsExt, path::Path, process::Command};

use anyhow::Result;

/// Path to the post-registration init script.
const POST_REGISTRATION_INIT_SCRIPT: &str = "/etc/oasis/init.post-registration";

/// Perform post-registration initialization. This will set up things like external networking
/// support inside the virtual machine.
pub(crate) fn post_registration_init() {
    let _ = run_post_registration_init_script(); // Ignore errors.
}

fn run_post_registration_init_script() -> Result<()> {
    let meta = Path::new(POST_REGISTRATION_INIT_SCRIPT).metadata()?;

    // Only execute when it is an executable file.
    if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
        return Ok(());
    }

    let mut cmd = Command::new(POST_REGISTRATION_INIT_SCRIPT).spawn()?;
    cmd.wait()?;
    Ok(())
}
