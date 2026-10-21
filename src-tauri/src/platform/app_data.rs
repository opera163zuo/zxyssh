#[cfg(not(any(target_os = "android", target_os = "ios")))]
use anyhow::Context;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const APP_BUNDLE_IDENTIFIER: &str = "com.zxyssh.desktop";
pub(crate) const DATABASE_FILE_NAME: &str = "zxyssh.db";
pub(crate) const FINGERPRINT_FILE_NAME: &str = "ssh_fingerprints.json";

pub(crate) fn database_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(DATABASE_FILE_NAME)
}

pub(crate) fn fingerprint_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(FINGERPRINT_FILE_NAME)
}

/// Same value as the GUI's `app.path().app_data_dir()`, resolved without a
/// running Tauri runtime so headless entry points (CLI daemon, import) share
/// one data directory with the desktop application.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) fn default_app_data_dir() -> Result<PathBuf> {
    let base_dirs =
        directories::BaseDirs::new().context("Could not determine platform base directories")?;
    Ok(base_dirs.data_dir().join(APP_BUNDLE_IDENTIFIER))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) fn default_app_data_dir() -> Result<PathBuf> {
    anyhow::bail!(
        "The default {} data directory can only be resolved on desktop platforms",
        APP_BUNDLE_IDENTIFIER
    );
}

/// Default database location for entry points without an AppHandle. Creates the
/// directory and performs the same one-time legacy migration as GUI startup.
pub(crate) fn default_database_path() -> Result<PathBuf> {
    prepared_default_app_data_dir().map(|dir| database_path(&dir))
}

/// Default fingerprint-store location for entry points without an AppHandle.
pub(crate) fn default_fingerprint_path() -> Result<PathBuf> {
    prepared_default_app_data_dir().map(|dir| fingerprint_path(&dir))
}

fn prepared_default_app_data_dir() -> Result<PathBuf> {
    let dir = default_app_data_dir()?;
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
mod tests {
    use super::*;

    #[test]
    fn bundle_identifier_matches_tauri_configuration() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let raw = fs::read_to_string(manifest_dir.join("tauri.conf.json"))
            .expect("tauri.conf.json must exist next to the crate manifest");
        let config: serde_json::Value =
            serde_json::from_str(&raw).expect("tauri.conf.json must be valid JSON");

        assert_eq!(
            APP_BUNDLE_IDENTIFIER,
            config["identifier"].as_str().expect("identifier field"),
            "APP_BUNDLE_IDENTIFIER and the default data dir must track the Tauri bundle identifier"
        );
    }
}
