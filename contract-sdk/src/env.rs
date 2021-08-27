//! Smart contract environment query interface.
use crate::types::env::{QueryRequest, QueryResponse};

/// Environment query trait.
pub trait Env {
    /// Perform an environment query.
    fn query<Q: Into<QueryRequest>>(&self, query: Q) -> QueryResponse;
}
