use std::sync::Arc;

use rocket::State;

use crate::state::Env;

/// Key generation endpoint.
#[rocket::post("/generate")]
pub fn generate(env: &State<Arc<dyn Env>>) -> &'static str {
    "TODO"
}
