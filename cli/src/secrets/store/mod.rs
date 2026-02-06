//! CLI integration for at-rest secret storage.
//!
//! The SDK provides a small, keyring-free primitive (`nexus_sdk::secret_store::StoredSecret`) that
//! can serialize a secret as a single string and optionally encrypt it using a 32‑byte key.
//!
//! This module wires that primitive into the CLI:
//! - [`master_key`] stores/loads the 32‑byte master key from the OS keyring.
//! - [`policy`] decides whether secrets should be written encrypted or plaintext for this run.
//! - [`CliKeyProvider`] implements the SDK’s `KeyProvider` trait and bridges policy → key material.
//!
//! The ergonomic CLI type alias is [`Secret`]:
//! ```text
//! Secret<T> = StoredSecret<T, CliKeyProvider, BincodeCodec>
//! ```
//!
//! In other words, any `Secret<T>` fields in persisted CLI state (e.g. `~/.nexus/crypto.toml`) will:
//! - encrypt on serialization when a key is available and policy allows it, and
//! - fall back to plaintext when policy is `off` or when the keyring is unavailable in `auto`
//!   mode.
use {
    crate::cli_conf::SecretsMode,
    nexus_sdk::secret_store::{
        BincodeCodec,
        KeyProvider,
        SecretKey,
        SecretStoreError,
        StoredSecret,
    },
};

pub(crate) mod master_key;
pub(crate) mod policy;

#[derive(Default, Debug, Clone, Copy)]
pub(crate) struct CliKeyProvider;

impl KeyProvider for CliKeyProvider {
    /// Returns the current master key according to the CLI’s secrets policy.
    ///
    /// Semantics:
    /// - `mode=off` => never encrypt (always `Ok(None)`).
    /// - `mode=auto` => encrypt when possible; if the keyring is unavailable for this run, fall
    ///   back to plaintext (`Ok(None)`).
    /// - `mode=require` => encryption is mandatory; if the key is missing/unavailable, return
    ///   `Err(KeyUnavailable)` so deserialization/serialization fails loudly.
    fn key(&self) -> Result<Option<SecretKey>, SecretStoreError> {
        let mode = policy::mode().map_err(|e| SecretStoreError::Provider(e.to_string()))?;

        if mode == SecretsMode::Off {
            return Ok(None);
        }

        if mode == SecretsMode::Auto && policy::keyring_unavailable_for_auto() {
            return Ok(None);
        }

        match master_key::load_master_key() {
            Ok(Some(key)) => Ok(Some(key)),
            Ok(None) => match mode {
                SecretsMode::Require => Err(SecretStoreError::KeyUnavailable),
                SecretsMode::Auto => Ok(None),
                SecretsMode::Off => Ok(None),
            },
            Err(master_key::MasterKeyError::Keyring(_)) if mode == SecretsMode::Auto => Ok(None),
            Err(e) => Err(SecretStoreError::Provider(e.to_string())),
        }
    }
}

/// CLI secret wrapper used for locally persisted crypto state.
///
/// This is the type used by `CryptoConf` to store the identity key and sessions at rest. It is a
/// serde wrapper that serializes as a single string and uses the OS keyring when encryption is
/// enabled.
pub type Secret<T> = StoredSecret<T, CliKeyProvider, BincodeCodec>;

#[cfg(test)]
mod tests {
    use {
        super::*,
        keyring::Entry,
        serde::{Deserialize, Serialize},
        std::sync::Mutex,
    };

    /// Serialise keyring mutations to avoid cross-test interference.
    static KEYRING_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct SimpleData {
        name: String,
        age: u32,
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn test_secret_basic_functionality() {
        let _guard = KEYRING_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        master_key::test_keyring::reset();
        policy::reset_for_tests();

        // Ensure a key exists so this test exercises encryption.
        Entry::new(master_key::SERVICE, master_key::USER)
            .unwrap()
            .set_password(&"00".repeat(master_key::KEY_LEN))
            .unwrap();

        {
            // Basic Secret Creation and Access
            let simple_data = SimpleData {
                name: "Alice".to_string(),
                age: 30,
            };

            let secret = Secret::new(simple_data.clone());
            assert_eq!(secret.name, simple_data.name);
            assert_eq!(secret.age, simple_data.age);
            assert_eq!(*secret, simple_data);

            // Secret Modification
            let mut mutable_secret = Secret::new(SimpleData {
                name: "Bob".to_string(),
                age: 25,
            });

            mutable_secret.age = 26;
            mutable_secret.name = "Bob Smith".to_string();

            assert_eq!(mutable_secret.age, 26);
            assert_eq!(mutable_secret.name, "Bob Smith");
        }
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn test_secret_serialization() {
        let _guard = KEYRING_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        master_key::test_keyring::reset();
        policy::reset_for_tests();

        // Ensure a key exists so this test exercises encryption.
        Entry::new(master_key::SERVICE, master_key::USER)
            .unwrap()
            .set_password(&"11".repeat(master_key::KEY_LEN))
            .unwrap();

        // Test JSON serialization/deserialization
        {
            let data = SimpleData {
                name: "Charlie".to_string(),
                age: 35,
            };

            let secret = Secret::new(data.clone());
            let serialized = serde_json::to_string(&secret).expect("Failed to serialize secret");

            // Verify we emit the encrypted variant.
            assert!(serialized.contains("enc:v1:"));

            // Deserialize and verify data integrity
            let deserialized: Secret<SimpleData> =
                serde_json::from_str(&serialized).expect("Failed to deserialize secret");
            assert_eq!(*deserialized, data);

            // Test different data types
            let string_secret = Secret::new("Hello, World!".to_string());
            let string_serialized = serde_json::to_string(&string_secret).unwrap();
            let string_deserialized: Secret<String> =
                serde_json::from_str(&string_serialized).unwrap();
            assert_eq!(*string_deserialized, "Hello, World!");

            let number_secret = Secret::new(42i64);
            let number_serialized = serde_json::to_string(&number_secret).unwrap();
            let number_deserialized: Secret<i64> =
                serde_json::from_str(&number_serialized).unwrap();
            assert_eq!(*number_deserialized, 42);
        }
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn test_secret_nonce_randomization() {
        let _guard = KEYRING_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        master_key::test_keyring::reset();
        policy::reset_for_tests();

        // Ensure a key exists so this test exercises encryption.
        Entry::new(master_key::SERVICE, master_key::USER)
            .unwrap()
            .set_password(&"22".repeat(master_key::KEY_LEN))
            .unwrap();

        let test_data = SimpleData {
            name: "NonceTest".to_string(),
            age: 99,
        };

        let secret1 = Secret::new(test_data.clone());
        let secret2 = Secret::new(test_data.clone());

        let serialized1 = serde_json::to_string(&secret1).unwrap();
        let serialized2 = serde_json::to_string(&secret2).unwrap();

        // Different encryptions should produce different ciphertexts (due to random nonces)
        assert_ne!(
            serialized1, serialized2,
            "Encryptions should be different due to random nonces"
        );

        // But both should decrypt to the same value
        let deserialized1: Secret<SimpleData> = serde_json::from_str(&serialized1).unwrap();
        let deserialized2: Secret<SimpleData> = serde_json::from_str(&serialized2).unwrap();

        assert_eq!(*deserialized1, test_data);
        assert_eq!(*deserialized2, test_data);
        assert_eq!(*deserialized1, *deserialized2);
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn test_secret_traits() {
        let _guard = KEYRING_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        master_key::test_keyring::reset();
        policy::reset_for_tests();

        // Ensure a key exists so this test exercises encryption.
        Entry::new(master_key::SERVICE, master_key::USER)
            .unwrap()
            .set_password(&"33".repeat(master_key::KEY_LEN))
            .unwrap();

        // Default Implementation
        {
            let default_secret: Secret<String> = Secret::default();
            assert_eq!(*default_secret, String::default());

            // Clone and Equality
            let clone_test_data = SimpleData {
                name: "CloneTest".to_string(),
                age: 77,
            };

            let original_secret = Secret::new(clone_test_data.clone());
            let cloned_secret = original_secret.clone();

            assert_eq!(original_secret, cloned_secret);
            assert_eq!(*original_secret, *cloned_secret);

            // Error Handling
            let invalid_json = r#""invalid-base64-data!@#$%""#;
            let result: Result<Secret<String>, _> = serde_json::from_str(invalid_json);
            assert!(result.is_err(), "Should fail with invalid base64");
        }
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn test_secret_plaintext_mode_when_no_key() {
        let _guard = KEYRING_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        master_key::test_keyring::reset();
        policy::reset_for_tests();

        // Ensure no key is present.
        let _ =
            Entry::new(master_key::SERVICE, master_key::USER).and_then(|e| e.delete_credential());

        let secret = Secret::new("hello".to_string());
        let serialized = serde_json::to_string(&secret).unwrap();
        assert!(serialized.contains("plain:v1:"));

        let roundtrip: Secret<String> = serde_json::from_str(&serialized).unwrap();
        assert_eq!(*roundtrip, "hello");
    }
}
