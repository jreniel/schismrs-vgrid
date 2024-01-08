use pretty_env_logger;
use std::sync::Once;

static INIT: Once = Once::new();

pub fn _setup_pretty_env_logger_default() {
    INIT.call_once(|| {
        pretty_env_logger::init();
    });
}

pub use kmeans_hsm::{kmeans_hsm, KMeansHSMCreateError};
pub mod kmeans_hsm;
pub mod transforms;
pub mod vqs;
