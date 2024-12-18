use oasis_runtime_sdk::modules::rofl::app::prelude::*;

/// ROFL app environment.
#[async_trait]
pub trait Env: Send + Sync {
    /// ROFL app identifier of the running application.
    fn app_id(&self) -> AppId;
}

pub(crate) struct EnvImpl<A: App> {
    _env: Environment<A>,
}

impl<A: App> EnvImpl<A> {
    pub fn new(env: Environment<A>) -> Self {
        Self { _env: env }
    }
}

impl<A: App> Env for EnvImpl<A> {
    fn app_id(&self) -> AppId {
        A::id()
    }
}
