pub mod routes;

pub use routes::{ApiState, build_router};

#[cfg(test)]
mod tests;
