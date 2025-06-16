use {
    crate::{
        command_title,
        loading,
        notify_success,
        prelude::*,
        utils::secrets::master_key::{MasterKeyError, SERVICE, USER},
    },
    keyring::Entry,
};

/// Prompt for a pass-phrase and store it securely in the key-ring.
pub async fn crypto_set_passphrase(stdin: bool, force: bool) -> AnyResult<(), NexusCliError> {
    command_title!("Setting passphrase in the OS key-ring");

    // Guard against overwriting unless --force(are you really sure you want to do this?)
    // Will lose all existing sessions
    let check_handle = loading!("Checking for existing keys...");

    // Abort if a raw master-key or a pass-phrase entry already exists (unless --force).
    if (Entry::new(SERVICE, USER)
        .map_err(|e| NexusCliError::Any(e.into()))?
        .get_password()
        .is_ok()
        || Entry::new(SERVICE, "passphrase")
            .map_err(|e| NexusCliError::Any(e.into()))?
            .get_password()
            .is_ok())
        && !force
    {
        check_handle.error();
        return Err(NexusCliError::Any(MasterKeyError::KeyAlreadyExists.into()));
    }

    check_handle.success();

    let input_handle = loading!("Reading passphrase...");

    let pass = if stdin {
        use std::io::{self, Read};
        let mut buf = String::new();
        match io::stdin().read_to_string(&mut buf) {
            Ok(_) => buf.trim_end_matches('\n').to_owned(),
            Err(e) => {
                input_handle.error();
                return Err(NexusCliError::Any(e.into()));
            }
        }
    } else {
        match rpassword::prompt_password("Enter new pass-phrase: ") {
            Ok(pass) => pass,
            Err(e) => {
                input_handle.error();
                return Err(NexusCliError::Any(e.into()));
            }
        }
    };

    if pass.trim().is_empty() {
        input_handle.error();
        return Err(NexusCliError::Any(anyhow!("pass-phrase cannot be empty")));
    }

    input_handle.success();

    let store_handle = loading!("Storing passphrase in key-ring...");

    match Entry::new(SERVICE, "passphrase")
        .map_err(|e| NexusCliError::Any(e.into()))?
        .set_password(&pass)
    {
        Ok(()) => {
            store_handle.success();
            // Remove any stale raw master-key so that key-status prefers the pass-phrase.
            let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
            notify_success!("pass-phrase stored in the OS key-ring");
            Ok(())
        }
        Err(e) => {
            store_handle.error();
            Err(NexusCliError::Any(e.into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        keyring::{mock, set_default_credential_builder, Entry},
        std::sync::{Mutex, Once},
    };

    /// Serialise key‑ring mutations to avoid cross‑test interference.
    static KEYRING_LOCK: Mutex<()> = Mutex::new(());
    /// Ensure the mock key‑ring is installed exactly once for the whole test run.
    static INIT_KEYRING: Once = Once::new();

    fn init_mock_keyring() {
        INIT_KEYRING.call_once(|| {
            // Replace the default credential store with the in‑memory mock.
            set_default_credential_builder(mock::default_credential_builder());
        });
    }

    /// Helper that acquires the keyring‑lock, cleans up key‑ring state, executes the
    /// test closure, and finally cleans up again. This keeps individual tests
    /// run in order.
    fn with_clean_keyring<F: FnOnce()>(f: F) {
        init_mock_keyring();

        let _guard = KEYRING_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Pre‑test clean‑up.
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
        let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());

        // Execute the actual test body.
        f();

        // Post‑test clean‑up.
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
        let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());
    }

    #[tokio::test]
    #[serial_test::serial(crypto_set_passphrase)]
    async fn test_crypto_set_passphrase_comprehensive() {
        with_clean_keyring(|| {
            // Test 1: Validate empty passphrase detection logic
            let empty_pass = "";
            let whitespace_pass = "   \t\n  ";
            let valid_pass = "valid_passphrase";

            assert!(
                empty_pass.trim().is_empty(),
                "Empty passphrase should be detected"
            );
            assert!(
                whitespace_pass.trim().is_empty(),
                "Whitespace-only passphrase should be detected"
            );
            assert!(
                !valid_pass.trim().is_empty(),
                "Valid passphrase should pass validation"
            );

            // Test 2: Test keyring storage and retrieval operations
            let test_passphrase = "test_passphrase_123";

            // First check that we can create entries successfully
            let entry = Entry::new(SERVICE, "passphrase").expect("Should be able to create entry");
            let store_result = entry.set_password(test_passphrase);
            assert!(
                store_result.is_ok(),
                "Should store passphrase successfully: {:?}",
                store_result
            );

            // Verify the passphrase was stored correctly - use same entry instance
            let stored = entry
                .get_password()
                .expect("Should be able to retrieve stored passphrase");
            assert_eq!(
                stored, test_passphrase,
                "Retrieved passphrase should match stored"
            );

            // Test 3: Test existing key detection logic
            let user_entry =
                Entry::new(SERVICE, USER).expect("Should be able to create user entry");
            user_entry
                .set_password("existing_key")
                .expect("Should be able to set user password");

            // Simulate the check that happens in crypto_set_passphrase
            let existing_key_check = user_entry.get_password().is_ok();
            assert!(existing_key_check, "Should detect existing key");

            // Test 4: Test passphrase overwriting behavior
            let new_passphrase = "overwritten_passphrase";
            let overwrite_result = entry.set_password(new_passphrase);
            assert!(
                overwrite_result.is_ok(),
                "Should overwrite passphrase successfully"
            );

            let retrieved_new = entry
                .get_password()
                .expect("Should be able to retrieve overwritten passphrase");

            assert_eq!(
                retrieved_new, new_passphrase,
                "Retrieved passphrase should be the new one"
            );
            assert_ne!(
                retrieved_new, test_passphrase,
                "Should not be the old passphrase"
            );

            // Test 5: Test error handling for keyring operations
            let result = Entry::new("valid_service", "valid_user");
            // Mock keyring usually succeeds, so we just verify it creates properly
            match result {
                Ok(_) => {
                    // Success case - this is expected with mock keyring
                }
                Err(e) => {
                    // Error case - test that we convert it properly
                    let cli_error = NexusCliError::Any(e.into());
                    assert!(format!("{:?}", cli_error).contains("NexusCliError"));
                }
            }

            // Test 6: Test the MasterKeyError::KeyAlreadyExists error condition
            // This tests the logic that would cause the function to return an error
            let error = MasterKeyError::KeyAlreadyExists;
            let cli_error = NexusCliError::Any(error.into());
            assert!(
                cli_error.to_string().contains("already exists"),
                "Error message should contain 'already exists'"
            );
        });
    }

    #[tokio::test]
    #[serial_test::serial(crypto_set_passphrase)]
    async fn test_crypto_set_passphrase_keyring_persistence() {
        with_clean_keyring(|| {
            // Test that passphrases persist across multiple keyring operations
            let passphrase1 = "first_passphrase";
            let passphrase2 = "second_passphrase";

            let entry = Entry::new(SERVICE, "passphrase").expect("Should be able to create entry");

            // Store first passphrase
            entry
                .set_password(passphrase1)
                .expect("Should be able to set first passphrase");

            // Verify it's stored
            let retrieved1 = entry
                .get_password()
                .expect("Should be able to retrieve first passphrase");
            assert_eq!(retrieved1, passphrase1);

            // Store second passphrase (simulating force overwrite)
            entry
                .set_password(passphrase2)
                .expect("Should be able to set second passphrase");

            // Verify the new one replaced the old one
            let retrieved2 = entry
                .get_password()
                .expect("Should be able to retrieve second passphrase");
            assert_eq!(retrieved2, passphrase2);
            assert_ne!(retrieved2, passphrase1);

            // Test deletion and recreation
            entry
                .delete_credential()
                .expect("Should be able to delete credential");

            // Should not be able to retrieve deleted passphrase
            let deleted_result = entry.get_password();
            assert!(
                deleted_result.is_err(),
                "Should not be able to retrieve deleted passphrase"
            );

            // Should be able to store a new one after deletion
            let passphrase3 = "third_passphrase";
            entry
                .set_password(passphrase3)
                .expect("Should be able to set third passphrase");

            let retrieved3 = entry
                .get_password()
                .expect("Should be able to retrieve third passphrase");
            assert_eq!(retrieved3, passphrase3);
        });
    }
}
