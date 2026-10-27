//! Secrets in the system credential store (Windows Credential Manager),
//! kept apart from `settings.json` and `state.json`, which are plain text.
//!
//! A secret is named by a `service` (what it is for, e.g. `"helm"`) and an
//! `account` (which one, e.g. a GitHub login). The store sees them as
//! `<product id>.<service>` and `account`, so the products' secrets don't
//! mix. Backends take a [`CredentialStore`] so tests can use
//! [`MemoryStore`].

use std::collections::HashMap;
use std::sync::Mutex;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("credential store: {0}")]
    Store(String),
}

pub trait CredentialStore: Send + Sync {
    /// The secret, or `None` when nothing is stored for it.
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, CredentialError>;
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), CredentialError>;
    /// Removes the secret; removing one that isn't there is not an error.
    fn delete(&self, service: &str, account: &str) -> Result<(), CredentialError>;
}

/// The system credential store.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemStore;

impl SystemStore {
    fn entry(service: &str, account: &str) -> Result<keyring::Entry, CredentialError> {
        keyring::Entry::new(&format!("{}.{service}", branding::PRODUCT.id), account)
            .map_err(|err| CredentialError::Store(err.to_string()))
    }
}

impl CredentialStore for SystemStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, CredentialError> {
        match Self::entry(service, account)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(CredentialError::Store(err.to_string())),
        }
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), CredentialError> {
        Self::entry(service, account)?
            .set_password(secret)
            .map_err(|err| CredentialError::Store(err.to_string()))
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), CredentialError> {
        match Self::entry(service, account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(CredentialError::Store(err.to_string())),
        }
    }
}

/// Secrets held in memory only, for tests.
#[derive(Default)]
pub struct MemoryStore {
    secrets: Mutex<HashMap<(String, String), String>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(service: &str, account: &str) -> (String, String) {
        (service.to_string(), account.to_string())
    }
}

impl CredentialStore for MemoryStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, CredentialError> {
        Ok(self
            .secrets
            .lock()
            .unwrap()
            .get(&Self::key(service, account))
            .cloned())
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), CredentialError> {
        self.secrets
            .lock()
            .unwrap()
            .insert(Self::key(service, account), secret.to_string());
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), CredentialError> {
        self.secrets
            .lock()
            .unwrap()
            .remove(&Self::key(service, account));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(store: &dyn CredentialStore, service: &str) {
        assert_eq!(store.get(service, "octocat").unwrap(), None);
        store.set(service, "octocat", "s3cret").unwrap();
        store.set(service, "other", "different").unwrap();
        assert_eq!(
            store.get(service, "octocat").unwrap().as_deref(),
            Some("s3cret")
        );
        store.set(service, "octocat", "changed").unwrap();
        assert_eq!(
            store.get(service, "octocat").unwrap().as_deref(),
            Some("changed")
        );
        store.delete(service, "octocat").unwrap();
        store.delete(service, "octocat").unwrap();
        assert_eq!(store.get(service, "octocat").unwrap(), None);
        assert_eq!(
            store.get(service, "other").unwrap().as_deref(),
            Some("different")
        );
        store.delete(service, "other").unwrap();
    }

    #[test]
    fn memory_store_round_trip() {
        round_trip(&MemoryStore::new(), "helm");
    }

    /// Uses the real store with a throwaway service name; every secret is
    /// removed again.
    #[test]
    #[cfg(windows)]
    fn system_store_round_trip() {
        round_trip(&SystemStore, &format!("test-{}", std::process::id()));
    }
}
