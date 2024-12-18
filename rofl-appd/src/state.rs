use oasis_runtime_sdk::modules::rofl::app::{App, AppId, Environment};

pub trait Env: Send + Sync {
    fn app_id(&self) -> AppId;
}

pub(crate) struct EnvImpl<A: App> {
    env: Environment<A>,
}

impl<A: App> EnvImpl<A> {
    pub fn new(env: Environment<A>) -> Self {
        Self { env }
    }
}

impl<A: App> Env for EnvImpl<A> {
    fn app_id(&self) -> AppId {
        A::id()
    }
}
