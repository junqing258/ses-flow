mod controllers;
mod models;
mod router;
mod services;
mod views;

pub use models::{FORMAL_PLUGIN_ID, FORMAL_PLUGIN_RUNNER_TYPE, PLUGIN_ID, PLUGIN_RUNNER_TYPE};
pub use router::build_app;

#[cfg(test)]
mod tests;
