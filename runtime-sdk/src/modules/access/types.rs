//! Access module types.
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    types::address::Address,
};

pub type Addresses = BTreeSet<Address>;

pub enum MethodAuthorization {
    AllowOnly(Addresses),
}

impl MethodAuthorization {
    pub(super) fn is_authorized(&self, address: &Address) -> bool {
        match self {
            Self::AllowOnly(addrs) => addrs.contains(address),
        }
    }
}

pub type Methods = BTreeMap<String, MethodAuthorization>;

pub enum Authorization {
    FilterOnly(Methods),
}

impl Authorization {
    pub fn new() -> Self {
        Self::FilterOnly(BTreeMap::new())
    }

    pub(super) fn is_authorized(&self, method: &str, address: &Address) -> bool {
        match self {
            Self::FilterOnly(meths) => {
                meths.get(method).map(|authz| authz.is_authorized(address)).unwrap_or(true)
            }
        }
    }
}
