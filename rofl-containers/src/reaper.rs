use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};

/// Guard that re-enables the default process reaper when dropped.
pub struct DisableReaperGuard {
    _internal: (),
}

impl Drop for DisableReaperGuard {
    fn drop(&mut self) {
        // Re-enable default kernel process reaper.
        unsafe {
            let _ = sigaction(
                Signal::SIGCHLD,
                &SigAction::new(SigHandler::SigIgn, SaFlags::empty(), SigSet::empty()),
            );
        }
    }
}

/// Temporarily disables the default process reaper. When the returned guard gets out of scope, the
/// default reaper is re-enabled.
///
/// This assumes that the default reaper has been previously configured by core init.
pub fn disable_default_reaper() -> DisableReaperGuard {
    unsafe {
        let _ = sigaction(
            Signal::SIGCHLD,
            &SigAction::new(SigHandler::SigDfl, SaFlags::empty(), SigSet::empty()),
        );
    }

    DisableReaperGuard { _internal: () }
}
