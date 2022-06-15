//! Versioning of the various ABI subfeatures.
//!
//! Oasis ABI sub-versions are indicated by exporting a predefined symbol that denotes the
//! sub-version number in its name. The convention for the symbol name is `__oasis_sv_XXX` where
//! the `XXX` part is the version number in decimal.
//!
//! For example sv1 is indicated by `__oasis_sv_1`.
//!
//! Exporting multiple sub-version symbols is an error and is not allowed.
//!
//! This allows different versions of the contract-sdk to be used in the same network without
//! breaking compatibility with old deployed contracts.
//!
//! # Sub-Versions
//!
//! * _nothing_
//!   * Baseline, no extra supported features.
//! * **sv1**
//!   * Add read-only flag to execution context.
//!   * Add call format to execution context.

#[no_mangle]
pub extern "C" fn __oasis_sv_1() {}
