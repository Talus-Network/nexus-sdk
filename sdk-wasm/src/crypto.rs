// CLI v0.3.0 Crypto Implementation for WASM
// Direct port of CLI crypto with localStorage instead of OS keyring

use {
    aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm,
        Nonce,
    },
    argon2::{Algorithm, Argon2, Params, Version},
    base64::{self, Engine as _},
    nexus_sdk::crypto::{
        session::{Message, Session},
        x3dh::{IdentityKey, PreKeyBundle},
    },
    rand::{self, RngCore},
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
    wasm_bindgen::prelude::*,
    zeroize::Zeroizing,
};

// === Constants (matching CLI) ===

const SERVICE: &str = "nexus-cli-store";
const USER: &str = "master-key";
const PASSPHRASE_USER: &str = "passphrase";

const KEY_LEN: usize = 32; // 256-bit master key
const SALT_LEN: usize = 16; // 128-bit salt

// Argon2id parameters (matching CLI exactly)
const ARGON2_MEMORY_KIB: u32 = 64 * 1024; // 64 MiB
const ARGON2_ITERATIONS: u32 = 4;

// LocalStorage keys (simulating keyring)
fn get_storage_key(user: &str) -> String {
    format!("nexus-{}-{}", SERVICE, user)
}

const SALT_KEY: &str = "nexus-argon2-salt";
const CRYPTO_CONF_KEY: &str = "nexus-crypto-conf";

// === CryptoConf Structure (matching CLI) ===

#[derive(Serialize, Deserialize, Default)]
struct CryptoConf {
    identity_key: Option<EncryptedData>,
    sessions: Option<EncryptedData>,
}

#[derive(Serialize, Deserialize, Clone)]
struct EncryptedData {
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

// === LocalStorage Helper (simulating keyring) ===

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn storage_get(key: &str) -> Option<String> {
    local_storage()?.get_item(key).ok()?
}

fn storage_set(key: &str, value: &str) -> Result<(), String> {
    local_storage()
        .ok_or("LocalStorage not available")?
        .set_item(key, value)
        .map_err(|_| "Failed to set item".to_string())
}

fn storage_remove(key: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(key);
    }
}

// === Master Key Management (CLI v0.3.0 parity) ===

/// Get master key with 3-tier resolution (matching CLI exactly)
fn get_master_key() -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
    // 1. Check passphrase in storage (simulating keyring)
    if let Some(passphrase) = storage_get(&get_storage_key(PASSPHRASE_USER)) {
        return derive_from_passphrase(&passphrase);
    }

    // 2. Check raw key in storage
    if let Some(hex_key) = storage_get(&get_storage_key(USER)) {
        let bytes = hex::decode(&hex_key).map_err(|e| format!("Hex decode: {}", e))?;
        if bytes.len() == KEY_LEN {
            let mut key_array = [0u8; KEY_LEN];
            key_array.copy_from_slice(&bytes);
            return Ok(Zeroizing::new(key_array));
        }
        // Corrupt entry, clean up
        storage_remove(&get_storage_key(USER));
    }

    Err("No persistent master key found; run crypto_init_key() or crypto_set_passphrase()".into())
}

/// Derive master key from passphrase using Argon2id (CLI parity)
fn derive_from_passphrase(passphrase: &str) -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
    let salt = get_or_create_salt()?;

    let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_ITERATIONS, 1, Some(KEY_LEN))
        .map_err(|e| format!("Argon2 params: {}", e))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(passphrase.as_bytes(), &salt, &mut *key)
        .map_err(|e| format!("Argon2 hash: {}", e))?;

    Ok(key)
}

/// Get or create salt (matching CLI salt.bin logic)
fn get_or_create_salt() -> Result<[u8; SALT_LEN], String> {
    if let Some(salt_b64) = storage_get(SALT_KEY) {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&salt_b64)
            .map_err(|e| format!("Salt decode: {}", e))?;

        if bytes.len() == SALT_LEN {
            let mut salt = [0u8; SALT_LEN];
            salt.copy_from_slice(&bytes);
            return Ok(salt);
        }
        // Invalid salt, will recreate
        storage_remove(SALT_KEY);
    }

    // Generate new salt
    let mut salt = [0u8; SALT_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);

    // Store in localStorage
    let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);
    storage_set(SALT_KEY, &salt_b64)?;

    Ok(salt)
}

// === Secret<T> Encryption (CLI parity) ===

/// Encrypt data with master key using AES-256-GCM
fn encrypt_with_master_key(
    plaintext: &[u8],
    master_key: &[u8; KEY_LEN],
) -> Result<EncryptedData, String> {
    // Generate random nonce
    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

    let cipher = Aes256Gcm::new(master_key.into());
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {}", e))?;

    Ok(EncryptedData {
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    })
}

/// Decrypt data with master key
fn decrypt_with_master_key(
    encrypted: &EncryptedData,
    master_key: &[u8; KEY_LEN],
) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(master_key.into());
    let nonce = Nonce::from_slice(&encrypted.nonce);

    cipher
        .decrypt(nonce, encrypted.ciphertext.as_ref())
        .map_err(|e| format!("Decryption failed: {}", e))
}

// === CryptoConf Management ===

fn load_crypto_conf() -> CryptoConf {
    storage_get(CRYPTO_CONF_KEY)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_crypto_conf(conf: &CryptoConf) -> Result<(), String> {
    let json = serde_json::to_string(conf).map_err(|e| format!("Serialize: {}", e))?;
    storage_set(CRYPTO_CONF_KEY, &json)
}

// === CLI Commands (WASM exports) ===

/// CLI: nexus crypto init-key [--force]
#[wasm_bindgen]
pub fn crypto_init_key(force: bool) -> String {
    let result = (|| -> Result<String, String> {
        // Check for existing keys (unless --force)
        if !force {
            if storage_get(&get_storage_key(PASSPHRASE_USER)).is_some()
                || storage_get(&get_storage_key(USER)).is_some()
            {
                return Err("KeyAlreadyExists: A different persistent key already exists; re-run with force=true".into());
            }
        }

        // Generate random 32-byte key
        let mut key = [0u8; KEY_LEN];
        rand::rngs::OsRng.fill_bytes(&mut key);
        let key_hex = hex::encode(key);

        // Store in localStorage (simulating keyring)
        storage_set(&get_storage_key(USER), &key_hex)?;

        // Remove any stale passphrase
        storage_remove(&get_storage_key(PASSPHRASE_USER));

        // Clear crypto conf (sessions invalidated)
        save_crypto_conf(&CryptoConf::default())?;

        Ok(serde_json::json!({
            "success": true,
            "message": "32-byte master key saved to localStorage",
            "key_preview": format!("{}...", &key_hex[..16]),
            "cli_compatible": true
        })
        .to_string())
    })();

    match result {
        Ok(json) => json,
        Err(e) => serde_json::json!({
            "success": false,
            "error": e
        })
        .to_string(),
    }
}

/// CLI: nexus crypto set-passphrase [--force]
#[wasm_bindgen]
pub fn crypto_set_passphrase(passphrase: String, force: bool) -> String {
    let result = (|| -> Result<String, String> {
        if passphrase.trim().is_empty() {
            return Err("Passphrase cannot be empty".into());
        }

        // Check for existing keys (unless --force)
        if !force {
            if storage_get(&get_storage_key(USER)).is_some()
                || storage_get(&get_storage_key(PASSPHRASE_USER)).is_some()
            {
                return Err("KeyAlreadyExists: A different persistent key already exists; re-run with force=true".into());
            }
        }

        // Store passphrase
        storage_set(&get_storage_key(PASSPHRASE_USER), &passphrase)?;

        // Remove any stale raw key
        storage_remove(&get_storage_key(USER));

        // Clear crypto conf (sessions invalidated)
        save_crypto_conf(&CryptoConf::default())?;

        Ok(serde_json::json!({
            "success": true,
            "message": "Passphrase stored in localStorage",
            "cli_compatible": true
        })
        .to_string())
    })();

    match result {
        Ok(json) => json,
        Err(e) => serde_json::json!({
            "success": false,
            "error": e
        })
        .to_string(),
    }
}

/// CLI: nexus crypto key-status
#[wasm_bindgen]
pub fn crypto_key_status() -> String {
    if storage_get(&get_storage_key(PASSPHRASE_USER)).is_some() {
        serde_json::json!({
            "exists": true,
            "source": "passphrase (localStorage)",
            "cli_compatible": true
        })
        .to_string()
    } else if let Some(hex) = storage_get(&get_storage_key(USER)) {
        serde_json::json!({
            "exists": true,
            "source": "raw-key (localStorage)",
            "preview": format!("{}...", &hex[..std::cmp::min(16, hex.len())]),
            "cli_compatible": true
        })
        .to_string()
    } else {
        serde_json::json!({
            "exists": false,
            "cli_compatible": true
        })
        .to_string()
    }
}

/// CLI: nexus crypto generate-identity-key
#[wasm_bindgen]
pub fn crypto_generate_identity_key() -> String {
    let result = (|| -> Result<String, String> {
        let master_key = get_master_key()?;

        // Generate fresh identity key
        let identity_key = IdentityKey::generate();

        // Serialize
        let bytes = bincode::serialize(&identity_key).map_err(|e| format!("Serialize: {}", e))?;

        // Encrypt with master key
        let encrypted = encrypt_with_master_key(&bytes, &master_key)?;

        // Save to CryptoConf
        let mut conf = load_crypto_conf();
        conf.identity_key = Some(encrypted);
        conf.sessions = None; // Invalidate sessions
        save_crypto_conf(&conf)?;

        Ok(serde_json::json!({
            "success": true,
            "message": "Identity key generated and stored (encrypted)",
            "warning": "All existing sessions have been invalidated",
            "cli_compatible": true
        })
        .to_string())
    })();

    match result {
        Ok(json) => json,
        Err(e) => serde_json::json!({
            "success": false,
            "error": e
        })
        .to_string(),
    }
}

/// Get identity key from CryptoConf
fn get_identity_key() -> Result<IdentityKey, String> {
    let master_key = get_master_key()?;
    let conf = load_crypto_conf();

    let encrypted = conf
        .identity_key
        .ok_or("No identity key found; run crypto_generate_identity_key() first")?;

    let bytes = decrypt_with_master_key(&encrypted, &master_key)?;

    bincode::deserialize(&bytes).map_err(|e| format!("Deserialize identity key: {}", e))
}

// === X3DH Session (with persistent identity key) ===

#[wasm_bindgen]
pub fn crypto_auth(peer_bundle_bytes: &[u8]) -> String {
    let result = (|| -> Result<String, String> {
        // Load persistent identity key
        let identity_key = get_identity_key()?;

        // Deserialize peer bundle
        let peer_bundle: PreKeyBundle = bincode::deserialize(peer_bundle_bytes)
            .map_err(|e| format!("Deserialize bundle: {}", e))?;

        // X3DH handshake
        let first_message = b"nexus auth";
        let (initial_msg, session) = Session::initiate(&identity_key, &peer_bundle, first_message)
            .map_err(|e| format!("X3DH failed: {}", e))?;

        let initial_message = match initial_msg {
            Message::Initial(msg) => msg,
            _ => return Err("Expected Initial message".into()),
        };

        // Serialize initial message
        let initial_message_bytes = bincode::serialize(&initial_message)
            .map_err(|e| format!("Serialize message: {}", e))?;

        // Store session
        let session_id = *session.id();
        save_session(session_id, session)?;

        Ok(serde_json::json!({
            "success": true,
            "session_id": hex::encode(session_id),
            "initial_message_b64": base64::engine::general_purpose::STANDARD.encode(&initial_message_bytes),
            "cli_compatible": true
        })
        .to_string())
    })();

    match result {
        Ok(json) => json,
        Err(e) => serde_json::json!({
            "success": false,
            "error": e
        })
        .to_string(),
    }
}

// === Session Management ===

thread_local! {
    pub(crate) static SESSIONS: std::cell::RefCell<HashMap<[u8; 32], Session>> = std::cell::RefCell::new(HashMap::new());
}

fn save_session(session_id: [u8; 32], session: Session) -> Result<(), String> {
    SESSIONS.with(|sessions| {
        sessions.borrow_mut().insert(session_id, session);
    });

    // Persist to CryptoConf
    persist_sessions()
}

pub(crate) fn persist_sessions() -> Result<(), String> {
    let master_key = get_master_key()?;
    let mut conf = load_crypto_conf();

    // Serialize all sessions
    let sessions_bytes = SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();
        bincode::serialize(&*sessions).map_err(|e| format!("Serialize sessions: {}", e))
    })?;

    // Encrypt
    let encrypted = encrypt_with_master_key(&sessions_bytes, &master_key)?;
    conf.sessions = Some(encrypted);

    save_crypto_conf(&conf)
}

pub(crate) fn load_sessions() -> Result<(), String> {
    let master_key = get_master_key()?;
    let conf = load_crypto_conf();

    if let Some(encrypted) = conf.sessions {
        let bytes = decrypt_with_master_key(&encrypted, &master_key)?;
        let sessions_map: HashMap<[u8; 32], Session> =
            bincode::deserialize(&bytes).map_err(|e| format!("Deserialize sessions: {}", e))?;

        SESSIONS.with(|sessions| {
            *sessions.borrow_mut() = sessions_map;
        });
    }

    Ok(())
}

#[wasm_bindgen]
pub fn get_session_count() -> usize {
    SESSIONS.with(|sessions| sessions.borrow().len())
}

/// Encrypt input ports with active session (CLI parity)
#[wasm_bindgen]
pub fn encrypt_entry_ports(input_json: &str, encrypted_ports_json: &str) -> String {
    let result = (|| -> Result<String, String> {
        // Load sessions if not already loaded
        let _ = load_sessions();

        let mut input_data: serde_json::Value =
            serde_json::from_str(input_json).map_err(|e| e.to_string())?;
        let encrypted_ports: HashMap<String, Vec<String>> =
            serde_json::from_str(encrypted_ports_json).map_err(|e| e.to_string())?;

        if encrypted_ports.is_empty() {
            return Ok(serde_json::json!({
                "success": true,
                "input_data": input_data,
                "encrypted_count": 0
            })
            .to_string());
        }

        let mut encrypted_count = 0;

        SESSIONS.with(|sessions| {
            let mut sessions = sessions.borrow_mut();

            if sessions.is_empty() {
                return Err("No active sessions; run crypto_auth() first".to_string());
            }

            let (_session_id, session) = sessions.iter_mut().next().ok_or("No sessions")?;

            for (vertex, ports) in &encrypted_ports {
                for port in ports {
                    if let Some(slot) = input_data.get_mut(vertex).and_then(|v| v.get_mut(port)) {
                        let plaintext = slot.take();
                        let bytes = serde_json::to_vec(&plaintext).map_err(|e| e.to_string())?;

                        let msg = session
                            .encrypt(&bytes)
                            .map_err(|e| format!("Encrypt: {}", e))?;

                        let Message::Standard(pkt) = msg else {
                            return Err("Expected StandardMessage".into());
                        };

                        // CLI parity: serialize StandardMessage directly as JSON object
                        *slot = serde_json::to_value(&pkt).map_err(|e| e.to_string())?;
                        encrypted_count += 1;
                    }
                }
            }

            session.commit_sender(None);
            Ok(())
        })?;

        // Persist updated sessions
        persist_sessions()?;

        Ok(serde_json::json!({
            "success": true,
            "input_data": input_data,
            "encrypted_count": encrypted_count
        })
        .to_string())
    })();

    match result {
        Ok(json) => json,
        Err(e) => serde_json::json!({
            "success": false,
            "error": e
        })
        .to_string(),
    }
}

/// Decrypt output data with active session (CLI parity)
#[wasm_bindgen]
pub fn decrypt_output_data(encrypted_json: &str) -> String {
    let result = (|| -> Result<String, String> {
        // Load sessions if not already loaded
        let _ = load_sessions();

        // CLI parity: StandardMessage is serialized as JSON object, not bincode
        let standard_msg: nexus_sdk::crypto::session::StandardMessage =
            serde_json::from_str(encrypted_json)
                .map_err(|e| format!("Parse StandardMessage: {}", e))?;

        let decrypted_data = SESSIONS.with(|sessions| {
            let mut sessions = sessions.borrow_mut();

            if sessions.is_empty() {
                return Err("No active sessions".to_string());
            }

            let (_session_id, session) = sessions.iter_mut().next().ok_or("No sessions")?;

            let decrypted_bytes = session
                .decrypt(&Message::Standard(standard_msg))
                .map_err(|e| format!("Decrypt: {}", e))?;

            let data: serde_json::Value = serde_json::from_slice(&decrypted_bytes)
                .map_err(|e| format!("Parse JSON: {}", e))?;

            session.commit_receiver(None, None);
            Ok(data)
        })?;

        // Persist updated sessions
        persist_sessions()?;

        Ok(serde_json::json!({
            "success": true,
            "data": decrypted_data
        })
        .to_string())
    })();

    match result {
        Ok(json) => json,
        Err(e) => serde_json::json!({
            "success": false,
            "error": e
        })
        .to_string(),
    }
}

/// Clear all crypto data (for testing)
#[wasm_bindgen]
pub fn crypto_clear_all() -> String {
    storage_remove(&get_storage_key(USER));
    storage_remove(&get_storage_key(PASSPHRASE_USER));
    storage_remove(SALT_KEY);
    storage_remove(CRYPTO_CONF_KEY);

    SESSIONS.with(|sessions| sessions.borrow_mut().clear());

    serde_json::json!({
        "success": true,
        "message": "All crypto data cleared"
    })
    .to_string()
}
