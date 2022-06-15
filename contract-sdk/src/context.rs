//! Contract execution context.
use crate::{
    env::{Crypto, Env},
    event::Event,
    storage::{ConfidentialStore, PublicStore},
    types::{address::Address, message::Message, token, CallFormat, InstanceId},
};

/// Execution context.
pub trait Context {
    /// The public store.
    type PublicStore: PublicStore;
    /// The confidential store.
    type ConfidentialStore: ConfidentialStore;
    /// The environment.
    type Env: Env + Crypto;

    /// Contract instance identifier.
    fn instance_id(&self) -> InstanceId;

    /// Contract instance address.
    fn instance_address(&self) -> &Address;

    /// Caller address.
    fn caller_address(&self) -> &Address;

    /// Tokens deposited by the caller.
    fn deposited_tokens(&self) -> &[token::BaseUnits];

    /// Whether the call is read-only and must not make any storage modifications.
    fn is_read_only(&self) -> bool;

    /// Call format.
    fn call_format(&self) -> CallFormat;

    /// Emits a message.
    fn emit_message(&mut self, msg: Message);

    /// Emits an event.
    fn emit_event<E: Event>(&mut self, event: E);

    /// Public contract store.
    fn public_store(&mut self) -> &mut Self::PublicStore;

    /// Confidential contract store.
    fn confidential_store(&mut self) -> &mut Self::ConfidentialStore;

    /// Environment.
    fn env(&self) -> &Self::Env;
}
