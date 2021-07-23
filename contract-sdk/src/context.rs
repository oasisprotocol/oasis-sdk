//! Contract execution context.
use crate::types::{address::Address, ExecutionContext, InstanceId};

pub trait Context {
    fn instance_id(&self) -> InstanceId;

    fn instance_address(&self) -> Address;
}

pub(crate) struct Internal {
    ec: ExecutionContext,
}

impl From<ExecutionContext> for Internal {
    fn from(ec: ExecutionContext) -> Self {
        Self { ec }
    }
}

impl Context for Internal {
    fn instance_id(&self) -> InstanceId {
        self.ec.instance_id
    }

    fn instance_address(&self) -> Address {
        self.ec.instance_address
    }
}
