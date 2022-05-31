//! Domain separation context helpers.
use std::sync::Mutex;

use once_cell::sync::Lazy;

use oasis_core_runtime::common::{crypto::hash::Hash, namespace::Namespace};

const CHAIN_CONTEXT_SEPARATOR: &[u8] = b" for chain ";

static CHAIN_CONTEXT: Lazy<Mutex<Option<Vec<u8>>>> = Lazy::new(Default::default);

/// Return the globally configured chain domain separation context.
///
/// The returned domain separation context is computed as:
///
/// ```plain
/// <base> || " for chain " || <chain-context>
/// ```
///
/// # Panics
///
/// This function will panic in case the global chain domain separation context was not previously
/// set using `set_chain_context`.
///
pub fn get_chain_context_for(base: &[u8]) -> Vec<u8> {
    let guard = CHAIN_CONTEXT.lock().unwrap();
    let chain_context = match guard.as_ref() {
        Some(cc) => cc,
        None => {
            drop(guard); // Avoid poisioning the global lock.
            panic!("chain domain separation context must be configured");
        }
    };

    let mut ctx = vec![0; base.len() + CHAIN_CONTEXT_SEPARATOR.len() + chain_context.len()];
    ctx[..base.len()].copy_from_slice(base);
    ctx[base.len()..base.len() + CHAIN_CONTEXT_SEPARATOR.len()]
        .copy_from_slice(CHAIN_CONTEXT_SEPARATOR);
    ctx[base.len() + CHAIN_CONTEXT_SEPARATOR.len()..].copy_from_slice(chain_context);
    ctx
}

/// Configure the global chain domain separation context.
///
/// The domain separation context is computed as:
///
/// ```plain
/// Base-16(H(<runtime-id> || <consensus-chain-context>))
/// ```
///
/// # Panics
///
/// This function will panic in case the global chain domain separation context was already set.
///
pub fn set_chain_context(runtime_id: Namespace, consensus_chain_context: &str) {
    let ctx = hex::encode(&Hash::digest_bytes_list(&[
        runtime_id.as_ref(),
        consensus_chain_context.as_bytes(),
    ]));
    let mut guard = CHAIN_CONTEXT.lock().unwrap();
    if let Some(ref existing) = *guard {
        if cfg!(any(test, feature = "test")) && existing == ctx.as_bytes() {
            return;
        }
        let ex = String::from_utf8(existing.clone()).unwrap();
        drop(guard); // Avoid poisioning the global lock.
        panic!("chain domain separation context already set: {}", ex,);
    }
    *guard = Some(ctx.into_bytes());
}

#[cfg(test)]
mod test {
    use super::*;

    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(Default::default);

    fn reset_chain_context() {
        *CHAIN_CONTEXT.lock().unwrap() = None;
    }

    #[test]
    fn test_chain_context() {
        let _guard = TEST_GUARD.lock().unwrap();
        reset_chain_context();
        set_chain_context(
            "8000000000000000000000000000000000000000000000000000000000000000".into(),
            "643fb06848be7e970af3b5b2d772eb8cfb30499c8162bc18ac03df2f5e22520e",
        );

        let ctx = get_chain_context_for(b"oasis-runtime-sdk/tx: v0");
        assert_eq!(&String::from_utf8(ctx).unwrap(), "oasis-runtime-sdk/tx: v0 for chain ca4842870b97a6d5c0d025adce0b6a0dec94d2ba192ede70f96349cfbe3628b9");
    }

    #[test]
    fn test_chain_context_not_configured() {
        let _guard = TEST_GUARD.lock().unwrap();
        reset_chain_context();

        let result = std::panic::catch_unwind(|| get_chain_context_for(b"test"));
        assert!(result.is_err());
    }

    #[test]
    fn test_chain_context_already_configured() {
        let _guard = TEST_GUARD.lock().unwrap();
        reset_chain_context();
        set_chain_context(
            "8000000000000000000000000000000000000000000000000000000000000000".into(),
            "643fb06848be7e970af3b5b2d772eb8cfb30499c8162bc18ac03df2f5e22520e",
        );

        let result = std::panic::catch_unwind(|| {
            set_chain_context(
                "8000000000000000000000000000000000000000000000000000000000000001".into(),
                "643fb06848be7e970af3b5b2d772eb8cfb30499c8162bc18ac03df2f5e22520e",
            )
        });
        assert!(result.is_err());
    }
}
