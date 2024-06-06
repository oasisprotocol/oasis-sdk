//! Method access control module types.
use std::collections::{BTreeMap, BTreeSet};

use crate::types::address::Address;

/// A set of addresses that can be used to define access control for a particular method.
pub type Addresses = BTreeSet<Address>;

/// A specific kind of method authorization.
pub enum MethodAuthorization {
    /// Only allow method calls from these addresses;
    /// for other callers, the method call will fail.
    AllowFrom(Addresses),
}

impl MethodAuthorization {
    /// Helper for creating a method authorization type that
    /// only allows callers with the given addresses.
    pub fn allow_from<I: IntoIterator<Item = Address>>(it: I) -> Self {
        Self::AllowFrom(BTreeSet::from_iter(it))
    }

    pub(super) fn is_authorized(&self, address: &Address) -> bool {
        match self {
            Self::AllowFrom(addrs) => addrs.contains(address),
        }
    }
}

/// A set of methods that are subject to access control.
pub type Methods = BTreeMap<String, MethodAuthorization>;

/// A specific kind of access control.
pub enum Authorization {
    /// Control a statically configured set of methods, each with a
    /// statically configured set of addresses that are allowed to call it.
    FilterOnly(Methods),
}

impl Authorization {
    /// Return a new access control configuration.
    pub fn new() -> Self {
        Self::FilterOnly(BTreeMap::new())
    }

    /// Helper for creating a static access control configuration.
    pub fn with_filtered_methods<S, I>(it: I) -> Self
    where
        S: AsRef<str>,
        I: IntoIterator<Item = (S, MethodAuthorization)>,
    {
        Self::FilterOnly(BTreeMap::from_iter(
            it.into_iter()
                .map(|(name, authz)| (name.as_ref().to_string(), authz)),
        ))
    }

    pub(super) fn is_authorized(&self, method: &str, address: &Address) -> bool {
        match self {
            Self::FilterOnly(meths) => meths
                .get(method)
                .map(|authz| authz.is_authorized(address))
                .unwrap_or(true),
        }
    }
}

impl Default for Authorization {
    fn default() -> Self {
        Self::FilterOnly(BTreeMap::default())
    }
}
