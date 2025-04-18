use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::{
    core::{
        common::namespace::Namespace,
        consensus::{state::roothash::ImmutableState as RoothashState, verifier::Verifier},
        protocol::Protocol,
        storage::mkvs,
        types::HostStorageEndpoint,
    },
    storage,
};

/// A store for a specific state root that talks to the runtime host.
pub struct HostStore {
    tree: mkvs::Tree,
}

impl HostStore {
    /// Create a new host store for the given host and root.
    pub fn new(host: Arc<Protocol>, root: mkvs::Root) -> Self {
        Self {
            tree: new_mkvs_tree_for_root(host, root),
        }
    }

    /// Create a new host store for the given host and root at the given round.
    ///
    /// The corresponding root hash is fetched by looking it up in consensus layer state, verified
    /// by the passed verifier to be correct.
    pub async fn new_for_round(
        host: Arc<Protocol>,
        consensus_verifier: &Arc<dyn Verifier>,
        runtime_id: Namespace,
        round: u64,
    ) -> Result<Self> {
        Ok(Self {
            tree: new_mkvs_tree_for_round(
                host,
                consensus_verifier,
                runtime_id,
                round,
                mkvs::RootType::State,
            )
            .await?,
        })
    }
}

impl storage::Store for HostStore {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.tree.get(key).unwrap()
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) {
        self.tree.insert(key, value).unwrap();
    }

    fn remove(&mut self, key: &[u8]) {
        self.tree.remove(key).unwrap();
    }

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        Box::new(self.tree.iter())
    }

    fn prefetch_prefixes(&mut self, prefixes: Vec<mkvs::Prefix>, limit: u16) {
        self.tree.prefetch_prefixes(&prefixes, limit).unwrap();
    }
}

/// Create a new MKVS tree for the given host and root.
pub fn new_mkvs_tree_for_root(host: Arc<Protocol>, root: mkvs::Root) -> mkvs::Tree {
    let read_syncer = mkvs::sync::HostReadSyncer::new(host, HostStorageEndpoint::Runtime);
    mkvs::Tree::builder()
        .with_capacity(10_000, 1024 * 1024)
        .with_root(root)
        .build(Box::new(read_syncer))
}

/// Create a new MKVS tree for the given host and runtime at the given round.
///
/// The corresponding root hash is fetched by looking it up in consensus layer state, verified
/// by the passed verifier to be correct.
pub async fn new_mkvs_tree_for_round(
    host: Arc<Protocol>,
    consensus_verifier: &Arc<dyn Verifier>,
    runtime_id: Namespace,
    round: u64,
    root_type: mkvs::RootType,
) -> Result<mkvs::Tree> {
    // Fetch latest consensus layer state.
    let state = consensus_verifier.latest_state().await?;
    // Fetch latest roots for the given namespace.
    let roots = tokio::task::spawn_blocking(move || {
        let roothash = RoothashState::new(&state);
        roothash.round_roots(runtime_id, round)
    })
    .await??
    .ok_or(anyhow!("root not found"))?;

    let root = mkvs::Root {
        namespace: runtime_id,
        version: round,
        root_type,
        hash: match root_type {
            mkvs::RootType::State => roots.state_root,
            mkvs::RootType::IO => roots.io_root,
            _ => return Err(anyhow!("unsupported root type")),
        },
    };

    Ok(new_mkvs_tree_for_root(host, root))
}
