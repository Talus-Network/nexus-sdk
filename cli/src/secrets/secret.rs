use {
    super::master_key,
    aes_gcm::{
        aead::{Aead, KeyInit, OsRng},
        Aes256Gcm,
        Key,
        Nonce,
    },
    base64::{engine::general_purpose, Engine as _},
    bincode,
    rand::RngCore,
    serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer},
};

/// Length of the nonce for AES-GCM (96-bit).
const NONCE_LEN: usize = 12;

/// Wrapper that encrypts/decrypts its inner value transparently when (de)serialised.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Secret<T>(pub T);

impl<T: Default> Default for Secret<T> {
    fn default() -> Self {
        Secret(T::default())
    }
}

impl<T> std::ops::Deref for Secret<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Secret<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Serialize for Secret<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 1. Serialise the inner value to bytes using bincode.
        let plain = bincode::serialize(&self.0).map_err(serde::ser::Error::custom)?;

        // 2. Obtain the master key
        let key_bytes = master_key::get_master_key().map_err(serde::ser::Error::custom)?;
        let key = Key::<Aes256Gcm>::from_slice(&*key_bytes);
        let cipher = Aes256Gcm::new(key);

        // 3. Generate random nonce.
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // 4. Encrypt (AEAD) – tag is appended to the ciphertext returned.
        let mut ciphertext = cipher
            .encrypt(nonce, plain.as_slice())
            .map_err(serde::ser::Error::custom)?;

        // 5. Prepend nonce so we can recover it when decrypting.
        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.append(&mut ciphertext);

        // 6. Base64-encode for TOML friendliness and serialise as a string.
        let encoded = general_purpose::STANDARD.encode(&combined);
        serializer.serialize_str(&encoded)
    }
}

impl<'de, T> Deserialize<'de> for Secret<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Input is expected to be a base64 string.
        let encoded = String::deserialize(deserializer)?;

        // Attempt base64 decode; if it fails we assume this is legacy plaintext
        // TOML that serialised `T` directly.
        let decoded = match general_purpose::STANDARD.decode(&encoded) {
            Ok(bytes) => bytes,
            Err(_) => {
                // Legacy path: try to parse directly from TOML-encoded inner struct.
                // We re-serialise the string as TOML then deserialize T.
                let legacy = toml::from_str::<T>(&encoded).map_err(serde::de::Error::custom)?;
                return Ok(Secret(legacy));
            }
        };

        if decoded.len() < NONCE_LEN {
            return Err(serde::de::Error::custom("ciphertext too short"));
        }

        let (nonce_bytes, ciphertext) = decoded.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);

        let key_bytes = master_key::get_master_key().map_err(serde::de::Error::custom)?;
        let key = Key::<Aes256Gcm>::from_slice(&*key_bytes);
        let cipher = Aes256Gcm::new(key);

        let plain = cipher
            .decrypt(nonce, ciphertext)
            .map_err(serde::de::Error::custom)?;

        let inner: T = bincode::deserialize(&plain).map_err(serde::de::Error::custom)?;
        Ok(Secret(inner))
    }
}
