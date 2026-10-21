mod app_data;
mod capabilities;

pub(crate) use app_data::{
    database_path, default_app_data_dir, default_database_path, default_fingerprint_path,
    fingerprint_path,
};
pub use capabilities::{RuntimeCapabilities, RuntimePlatform};
