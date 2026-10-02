//! The in-memory secret store must stay bounded in long dev sessions.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use assistant_secrets::{
    InMemorySecretStore, MAX_IN_MEMORY_SECRETS, SecretName, SecretStore, SecretValue,
};

#[test]
fn test_in_memory_store_refuses_new_names_past_the_cap() {
    let store = InMemorySecretStore::new();
    let value = SecretValue::new("x").expect("value");
    for index in 0..MAX_IN_MEMORY_SECRETS {
        let name = SecretName::new(format!("test.secret.{index}")).expect("name");
        store.set(&name, &value).expect("write within the cap");
    }
    let overflow = SecretName::new("test.secret.overflow").expect("name");
    assert!(
        store.set(&overflow, &value).is_err(),
        "a new name past the cap must fail closed instead of growing forever"
    );
    // Overwriting an existing name must still work: the cap is on distinct entries.
    let existing = SecretName::new("test.secret.0").expect("name");
    assert!(store.set(&existing, &value).is_ok());
}
