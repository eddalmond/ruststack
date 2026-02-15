//! Account and region scoped state management

use dashmap::DashMap;
use std::hash::Hash;

/// Key for account and region scoped state
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountRegionKey {
    pub account_id: String,
    pub region: String,
}

impl AccountRegionKey {
    pub fn new(account_id: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            region: region.into(),
        }
    }
}

/// Thread-safe state store with account/region scoping
pub struct StateStore<T> {
    data: DashMap<AccountRegionKey, T>,
}

impl<T> Default for StateStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> StateStore<T> {
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
        }
    }

    /// Get or create state for an account/region
    pub fn get_or_create(
        &self,
        account_id: &str,
        region: &str,
    ) -> dashmap::mapref::one::RefMut<'_, AccountRegionKey, T>
    where
        T: Default,
    {
        let key = AccountRegionKey::new(account_id, region);
        self.data.entry(key).or_default()
    }

    /// Get state for an account/region if it exists
    pub fn get(
        &self,
        account_id: &str,
        region: &str,
    ) -> Option<dashmap::mapref::one::Ref<'_, AccountRegionKey, T>> {
        let key = AccountRegionKey::new(account_id, region);
        self.data.get(&key)
    }

    /// Remove state for an account/region
    pub fn remove(&self, account_id: &str, region: &str) -> Option<(AccountRegionKey, T)> {
        let key = AccountRegionKey::new(account_id, region);
        self.data.remove(&key)
    }

    /// Clear all state
    pub fn clear(&self) {
        self.data.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestState {
        counter: i32,
    }

    #[test]
    fn test_get_or_create() {
        let store: StateStore<TestState> = StateStore::new();

        {
            let mut state = store.get_or_create("123456789012", "us-east-1");
            state.counter = 42;
        }

        let state = store.get("123456789012", "us-east-1").unwrap();
        assert_eq!(state.counter, 42);
    }

    #[test]
    fn test_different_regions() {
        let store: StateStore<TestState> = StateStore::new();

        store.get_or_create("123456789012", "us-east-1").counter = 1;
        store.get_or_create("123456789012", "us-west-2").counter = 2;

        assert_eq!(
            store.get("123456789012", "us-east-1").unwrap().counter,
            1
        );
        assert_eq!(
            store.get("123456789012", "us-west-2").unwrap().counter,
            2
        );
    }
}
