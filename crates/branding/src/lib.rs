//! Product identity and folders. Nothing else in the workspace hard-codes the
//! product's name, ids or paths; renaming the dev build for release is a
//! change to [`PRODUCT`] only.
//!
//! The ids, IPC name and URL scheme all differ from the Zed edition's
//! (`dev.forge.super`, `Forge-Super-Editor`, `forgesuper`), so the two can be
//! installed side by side.

use std::path::PathBuf;

pub struct Product {
    /// Full name, for window titles, About and installers.
    pub display: &'static str,
    /// Short name, for menus ("Quit Forge Super").
    pub short: &'static str,
    /// Folder under the platform's config/data folders, e.g.
    /// `%APPDATA%\ForgeSuperDev2`.
    pub dir: &'static [&'static str],
    /// Reverse-DNS application id.
    pub id: &'static str,
    /// Name for single-instance IPC.
    pub ipc: &'static str,
    /// URL scheme. Reserved; not registered with the OS yet.
    pub scheme: &'static str,
    /// Prefix for the environment variables the app sets for the processes
    /// it starts (e.g. `FORGE_SUPER_DEV2_GIT_CREDENTIAL_SOCKET`).
    pub env_prefix: &'static str,
}

pub const PRODUCT: Product = Product {
    display: "Mentenaz Forge Super",
    short: "Forge Super",
    dir: &["ForgeSuperDev2"],
    id: "dev.mentenaz.forge-super.dev2",
    ipc: "Forge-Super-Dev2",
    scheme: "forgesuper-dev2",
    env_prefix: "FORGE_SUPER_DEV2",
};

fn under(base: Option<PathBuf>) -> PathBuf {
    let mut path = base.unwrap_or_else(std::env::temp_dir);
    path.extend(PRODUCT.dir);
    path
}

/// Settings (`settings.json`): `%APPDATA%\ForgeSuperDev2` on
/// Windows, the platform's config folder elsewhere.
pub fn config_dir() -> PathBuf {
    under(dirs::config_dir())
}

/// State, logs and caches: `%LOCALAPPDATA%\ForgeSuperDev2` on
/// Windows, the platform's local data folder elsewhere.
pub fn data_dir() -> PathBuf {
    under(dirs::data_local_dir())
}

pub fn settings_file() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn state_file() -> PathBuf {
    data_dir().join("state.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_end_with_the_product_dir() {
        for dir in [config_dir(), data_dir()] {
            assert!(dir.ends_with(PathBuf::from_iter(PRODUCT.dir)), "{dir:?}");
        }
        assert_eq!(settings_file().file_name().unwrap(), "settings.json");
        assert_eq!(state_file().file_name().unwrap(), "state.json");
    }

    #[cfg(windows)]
    #[test]
    fn windows_uses_appdata_and_localappdata() {
        let appdata = PathBuf::from(std::env::var("APPDATA").unwrap());
        let local = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap());
        assert_eq!(config_dir(), appdata.join("ForgeSuperDev2"));
        assert_eq!(data_dir(), local.join("ForgeSuperDev2"));
    }
}
