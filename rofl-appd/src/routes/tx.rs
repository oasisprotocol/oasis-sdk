use std::sync::Arc;

use rocket::State;

use crate::state::Env;

/// Transaction submission endpoint.
#[rocket::post("/submit")]
pub fn submit(env: &State<Arc<dyn Env>>) -> &'static str {
    "TODO"
}
