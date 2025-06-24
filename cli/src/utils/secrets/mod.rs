use {
    self::aes_gcm_encryption::AesGcmEncryption,
    nexus_sdk::secret_core::{secret::GenericSecret, traits::BincodeCodec},
};

pub mod aes_gcm_encryption;
pub mod master_key;
pub mod master_key_provider;

pub type Secret<T> = GenericSecret<T, AesGcmEncryption, BincodeCodec>;

#[cfg(test)]
mod tests {
    use {
        super::*,
        keyring::Entry,
        serde::{Deserialize, Serialize},
        std::{collections::HashMap, env, sync::Mutex},
        tempfile::TempDir,
    };

    /// Serialise env-var mutations to avoid cross-test interference.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_test_env<F: FnOnce() -> R, R>(f: F) -> R {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Clean up any existing keyring entries before test
        let _ =
            Entry::new(master_key::SERVICE, master_key::USER).and_then(|e| e.delete_credential());

        // Set up isolated test environment with unique temp directory
        let tmp = TempDir::new().unwrap();
        let xdg_path = tmp.path().join("xdg_config");
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let original_passphrase = env::var("NEXUS_CLI_STORE_PASSPHRASE").ok();

        // Ensure the directory exists before setting the env var
        std::fs::create_dir_all(&xdg_path).unwrap();

        env::set_var("XDG_CONFIG_HOME", &xdg_path);
        env::set_var(
            "NEXUS_CLI_STORE_PASSPHRASE",
            "test-passphrase-for-comprehensive-test-isolated",
        );

        let result = f();

        // Clean up after test
        let _ =
            Entry::new(master_key::SERVICE, master_key::USER).and_then(|e| e.delete_credential());

        // Restore original environment
        match original_xdg {
            Some(old) => env::set_var("XDG_CONFIG_HOME", old),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
        match original_passphrase {
            Some(old) => env::set_var("NEXUS_CLI_STORE_PASSPHRASE", old),
            None => env::remove_var("NEXUS_CLI_STORE_PASSPHRASE"),
        }

        result
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct SimpleData {
        name: String,
        age: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct ComplexData {
        numbers: Vec<i32>,
        mapping: HashMap<String, String>,
        nested: SimpleData,
        optional: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct FloatData {
        value: f64,
        precision: f32,
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn test_secret_basic_functionality() {
        with_test_env(|| {
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
        });
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn test_secret_serialization() {
        with_test_env(|| {
            // Test JSON serialization/deserialization
            let data = SimpleData {
                name: "Charlie".to_string(),
                age: 35,
            };

            let secret = Secret::new(data.clone());
            let serialized = serde_json::to_string(&secret).expect("Failed to serialize secret");

            // Verify data is encrypted (not visible in plain text)
            assert!(!serialized.contains("Charlie"));

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
        });
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn test_secret_nonce_randomization() {
        with_test_env(|| {
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
        });
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn test_secret_traits() {
        with_test_env(|| {
            // Default Implementation
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
        });
    }
}
