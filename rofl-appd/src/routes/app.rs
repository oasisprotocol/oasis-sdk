use std::sync::Arc;

use rocket::State;

use crate::state::Env;

#[rocket::get("/id")]
pub fn id(env: &State<Arc<dyn Env>>) -> String {
    env.app_id().to_bech32()
}
