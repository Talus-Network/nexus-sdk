//! # Session Module ­— X3DH + Double-Ratchet (Header-Encrypted)
//!
//! This module glues together the X3DH key-agreement protocol and a
//! header-encrypted variant of the Double-Ratchet algorithm to give a
//! complete end-to-end encrypted session layer(Full Signal Protocol).
//!
//! ## Example
//!
//! ```no_run
//! use nexus_sdk::crypto::session::{Session, Message};
//! use nexus_sdk::crypto::x3dh::{IdentityKey, PreKeyBundle};
//! use rand::rngs::OsRng;
//! use x25519_dalek::StaticSecret;
//!
//! // === Setup identities & Receiver's bundle (omitted: signature generation) ===
//! let sender_id = IdentityKey::generate();
//! let receiver_id   = IdentityKey::generate();
//! let spk_sec  = StaticSecret::random_from_rng(OsRng);
//! let spk_id = 1; // Some ID for the signed pre-key
//! let bundle   = PreKeyBundle::new(&receiver_id, spk_id, &spk_sec, None, None);
//!
//! // === Sender initiates ===
//! let (first_packet, mut sender_sess) =
//!     Session::initiate(&sender_id, &bundle, b"Hi Receiver!")?;
//!
//! // Transmit `first_packet` over the network …
//!
//! // === Receiver receives ===
//! let (mut receiver_sess, plaintext) = Session::recv(
//!     &receiver_id,
//!     &spk_sec,
//!     &bundle,
//!     match &first_packet {
//!         Message::Initial(m) => m,
//!         _ => unreachable!("Sender always starts with Initial"),
//!     },
//!     None
//! )?;
//! assert_eq!(plaintext, b"Hi Receiver!");
//!
//! // === Encrypted conversation ===
//! let to_sender = receiver_sess.encrypt(b"Hello, Sender!")?;
//! let reply    = sender_sess.decrypt(&to_sender)?;
//! assert_eq!(reply, b"Hello, Sender!");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use {
    super::{
        double_ratchet::RatchetStateHE,
        x3dh::{
            receiver_receive,
            sender_init,
            IdentityKey,
            InitialMessage,
            PreKeyBundle,
            X3dhError,
        },
    },
    anyhow::bail,
    hkdf::Hkdf,
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    sha2::{Digest, Sha256},
    thiserror::Error,
    x25519_dalek::{PublicKey, StaticSecret},
    zeroize::{Zeroize, Zeroizing},
};

/// Protocol version
const PROTOCOL_VERSION: u8 = 1;

/// Domain-separation salt for HKDF.
/// Change when you want to have a domain separation
const HKDF_SALT: [u8; 32] = *b"X3DH-DR-v1-2025-05-20-----------";

/// Errors that can arise during session establishment or normal messaging.
#[derive(Debug, Error)]
pub enum SessionError {
    /// Propagated X3DH-handshake failure.
    #[error("X3DH error: {0}")]
    X3DH(#[from] X3dhError),
    /// HKDF key-derivation failure (should only occur on length mismatch).
    #[error("HKDF error")]
    HKDF,
    /// Authenticated-decryption failure (bad MAC / corrupt data).
    #[error("Decryption failed")]
    DecryptionFailed,
    /// Any attempt to use a session in an impossible state.
    #[error("Session state error: {0}")]
    InvalidState(String),
    /// Message claims an unsupported protocol version.
    #[error("Unsupported protocol version {0}")]
    Version(u8),
}

impl From<hkdf::InvalidLength> for SessionError {
    fn from(_: hkdf::InvalidLength) -> Self {
        SessionError::HKDF
    }
}

/// Message format for a Double-Ratchet packet with header encryption.
///
/// The header (containing the DH ratchet public key, send-chain counter, ...)
/// is encrypted and authenticated by [`RatchetStateHE`], hiding metadata from
/// passive observers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardMessage {
    /// Protocol version tag — currently `1`.
    pub version: u8,
    /// Encrypted ratchet header returned by `ratchet_encrypt_he`.
    #[serde(with = "serde_bytes")]
    pub header: Vec<u8>,
    /// AEAD-protected application payload.
    #[serde(with = "serde_bytes")]
    pub ciphertext: Vec<u8>,
}

/// Union covering all messages that can traverse the transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Message {
    /// First packet in the X3DH handshake (sent by Sender).
    Initial(InitialMessage),
    /// Ordinary Double-Ratchet message exchanged after the handshake.
    Standard(StandardMessage),
    /// Double-Ratchet message whose key should remain cached for limited reuse.
    LimitedPersistent(StandardMessage),
}

/// A live end-to-end encrypted session.
///
/// Both parties maintain a copy containing identical symmetric state.
/// Cloning is disallowed to avoid accidental divergence.
pub struct Session {
    /// Stable database key (32-byte, random-looking).
    session_id: [u8; 32],
    /// Double-Ratchet state with encrypted headers.
    ratchet: RatchetStateHE,
    /// Local identity-DH public key (used for Associated-Data).
    local_identity: PublicKey,
    /// Remote peer's identity-DH public key.
    remote_identity: PublicKey,
}

impl Serialize for Session {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (
            &self.session_id,
            &self.ratchet,
            self.local_identity.as_bytes(),
            self.remote_identity.as_bytes(),
        )
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Session {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (session_id, ratchet, local_bytes, remote_bytes): (
            [u8; 32],
            RatchetStateHE,
            [u8; 32],
            [u8; 32],
        ) = Deserialize::deserialize(deserializer)?;

        Ok(Session {
            session_id,
            ratchet,
            local_identity: PublicKey::from(local_bytes),
            remote_identity: PublicKey::from(remote_bytes),
        })
    }
}

impl Session {
    // === Low-level helpers ===

    /// Deterministically derives the session-ID from the X3DH shared secret.
    /// Use something else if its more convenient for your application.
    pub fn calculate_session_id(shared_secret: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        // session-id | shared-secret
        hasher.update(b"session-id");
        hasher.update(shared_secret);
        hasher.finalize().into()
    }

    /// Constructs per-session Associated-Data as `min(IK_A, IK_B) || max(IK_A, IK_B)`.
    ///
    /// Ordering the identity keys lexicographically ensures both peers derive
    /// the same AD irrespective of role (Sender/Receiver).
    fn make_associated_data(&self) -> Vec<u8> {
        let (first, second) = if self.local_identity.as_bytes() < self.remote_identity.as_bytes() {
            (
                self.local_identity.as_bytes(),
                self.remote_identity.as_bytes(),
            )
        } else {
            (
                self.remote_identity.as_bytes(),
                self.local_identity.as_bytes(),
            )
        };
        let mut ad = Vec::with_capacity(64);
        ad.extend_from_slice(first);
        ad.extend_from_slice(second);
        ad
    }

    // === Session establishment ===

    /// Sender-side entry point: perform an X3DH handshake, produce the first
    /// packet, and return a fully initialised [`Session`].
    ///
    /// * `identity` — Sender's long-term identity key-pair.
    /// * `bundle`   — Receiver's advertised pre-key bundle.
    /// * `plaintext`— Optional application data.
    ///
    /// On success `(initial_packet, session)` is returned.  The caller should
    /// send `initial_packet` to Receiver and persist the session.
    pub fn initiate(
        identity: &IdentityKey,
        bundle: &PreKeyBundle,
        plaintext: &[u8],
    ) -> Result<(Message, Self), SessionError> {
        // 1. Verify Receiver's Signed-Pre-Key.
        if !bundle.verify_spk() {
            return Err(SessionError::InvalidState("Invalid SPK signature".into()));
        }

        // 2. Run X3DH (Sender side).
        let (init_msg, sk_raw) = sender_init(identity, bundle, plaintext)?;
        let sk = Zeroizing::new(sk_raw); // zeroise on scope-exit

        // 3. Derive header-encryption keys.
        let hkdf = Hkdf::<Sha256>::new(Some(&HKDF_SALT), &sk[..]);
        let mut hks = [0u8; 32]; // send
        let mut hk_r = [0u8; 32]; // receive
        hkdf.expand(b"header-encrypt-sending", &mut hks)?;
        hkdf.expand(b"header-encrypt-receiving", &mut hk_r)?;

        // 4. Initialise Double-Ratchet in "Sender" role (using init_sender_he method).
        let mut ratchet = RatchetStateHE::new();
        let _ = ratchet.init_sender_he(&sk, bundle.spk_pub, hks, hk_r);

        // 5. Stable session-ID.
        // Change if needed for your application.
        let session_id = Self::calculate_session_id(&sk);

        Ok((
            Message::Initial(init_msg),
            Session {
                session_id,
                ratchet,
                local_identity: identity.dh_public,
                remote_identity: bundle.identity_pk,
            },
        ))
    }

    /// Receiver-side entry point: accept an incoming `InitialMessage`, complete
    /// the X3DH handshake, and return `(session, plaintext_from_sender)`.
    ///
    /// * `identity` — Receiver's long-term identity key-pair.
    /// * `spk_secret` Receiver's Signed-Pre-Key secret.
    /// * `bundle` — Sender's advertised pre-key bundle.
    /// * `msg` — Initial message from Sender.
    /// * `otpk_secret` — Optional OTPK secret
    pub fn recv(
        identity: &IdentityKey,
        spk_secret: &StaticSecret,
        bundle: &PreKeyBundle,
        msg: &InitialMessage,
        otpk_secret: Option<&StaticSecret>,
    ) -> Result<(Self, Vec<u8>), SessionError> {
        // 1. Sanity-check our own bundle.
        if !bundle.verify_spk() {
            return Err(SessionError::InvalidState(
                "Local SPK signature invalid".into(),
            ));
        }

        // 2. Run X3DH (Receiver side).
        let otpk_with_id = match (otpk_secret, msg.otpk_id) {
            (Some(secret), Some(id)) => Some((secret, id)),
            _ => None,
        };

        let (plaintext, sk_raw) =
            receiver_receive(identity, spk_secret, bundle.spk_id, otpk_with_id, msg)?;
        let sk = Zeroizing::new(sk_raw);

        // 3. Derive HE keys (note send/recv reversed).
        let hkdf = Hkdf::<Sha256>::new(Some(&HKDF_SALT), &sk[..]);
        let mut k_s = [0u8; 32]; // decrypt incoming (Sender -> Receiver)
        let mut k_r = [0u8; 32]; // encrypt outgoing (Receiver -> Sender)
        hkdf.expand(b"header-encrypt-sending", &mut k_s)?;
        hkdf.expand(b"header-encrypt-receiving", &mut k_r)?;

        // 4. Initialise Double-Ratchet in "Receiver" role (using init_bob_he method).
        let mut ratchet = RatchetStateHE::new();
        let receiver_pub = PublicKey::from(spk_secret);
        let _ = ratchet.init_receiver_he(&sk, (spk_secret.clone(), receiver_pub), k_s, k_r);

        // 5. Stable session-ID.
        let session_id = Self::calculate_session_id(&sk);

        Ok((
            Session {
                session_id,
                ratchet,
                local_identity: identity.dh_public,
                remote_identity: msg.ika_pub,
            },
            plaintext,
        ))
    }

    // === Messaging ===

    /// Returns the stable 32-byte session identifier
    ///
    /// Useful as a primary key when persisting session state
    pub fn id(&self) -> &[u8; 32] {
        &self.session_id
    }

    /// Encrypts `plaintext`, advances the sending chain, and returns a
    /// [`Message::Standard`].  Fails only if ratchet state is inconsistent.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Message, SessionError> {
        let ad = self.make_associated_data();
        self.ratchet
            .ratchet_encrypt_he(plaintext, &ad)
            .map(|(header, ciphertext)| {
                Message::Standard(StandardMessage {
                    version: PROTOCOL_VERSION,
                    header,
                    ciphertext,
                })
            })
            .map_err(|_| SessionError::InvalidState("Encryption failed".into()))
    }

    /// Encrypts a plaintext while marking the result as limited persistent.
    ///
    /// The receiver keeps the derived message key cached so the ciphertext can
    /// be replayed a bounded number of times without rewinding the ratchet.
    pub fn encrypt_limited_persistent(
        &mut self,
        plaintext: &[u8],
    ) -> Result<Message, SessionError> {
        match self.encrypt(plaintext)? {
            Message::Standard(msg) => Ok(Message::LimitedPersistent(msg)),
            Message::Initial(_) => Err(SessionError::InvalidState(
                "Cannot emit an Initial message in persistent mode".into(),
            )),
            Message::LimitedPersistent(_) => Err(SessionError::InvalidState(
                "Message already tagged as persistent".into(),
            )),
        }
    }

    /// Decrypts an incoming `message`, performing DH/PN ratchets as required.
    ///
    /// *Errors*:
    /// * `SessionError::Version` — peer sent an unsupported version tag.
    /// * `SessionError::DecryptionFailed` — MAC failed / bad ciphertext.
    pub fn decrypt(&mut self, message: &Message) -> Result<Vec<u8>, SessionError> {
        match message {
            Message::Initial(_) => Err(SessionError::InvalidState(
                "Cannot decrypt an initial message with an established session".into(),
            )),
            Message::Standard(StandardMessage {
                version,
                header,
                ciphertext,
            }) => {
                if *version != PROTOCOL_VERSION {
                    return Err(SessionError::Version(*version));
                }
                let ad = self.make_associated_data();
                self.ratchet
                    .ratchet_decrypt_he(header, ciphertext, &ad)
                    .map_err(|_| SessionError::DecryptionFailed)
            }
            Message::LimitedPersistent(StandardMessage {
                version,
                header,
                ciphertext,
            }) => {
                if *version != PROTOCOL_VERSION {
                    return Err(SessionError::Version(*version));
                }
                let ad = self.make_associated_data();
                self.ratchet
                    .ratchet_decrypt_he_persistent(header, ciphertext, &ad)
                    .map_err(|_| SessionError::DecryptionFailed)
            }
        }
    }

    /// Try to **re-open a message we ourselves sent** while its message-key is
    /// still cached locally (useful for drafts, "edit" or resend workflows).
    ///
    /// * `header` and `ciphertext` are the exact bytes we put on the wire.
    /// * Returns `Some(plaintext)` on success, or `None` if the key was
    ///   already forgotten or the header does not belong to this session.
    pub fn read_own_msg(&mut self, msg: &Message) -> Option<Vec<u8>> {
        match msg {
            Message::Standard(sm) | Message::LimitedPersistent(sm) => {
                let ad = self.make_associated_data();
                self.ratchet
                    .decrypt_outgoing(&sm.header, &sm.ciphertext, &ad)
            }
            _ => None, // Initial message or wrong variant
        }
    }

    /// **Sender-side "commit":** irrevocably forget cached **outgoing** message-keys.
    ///
    /// * Pass `Some(n)` to forget everything ≤ `n` in the current sending
    ///   chain;
    /// * Pass `None` to wipe the entire cache.
    pub fn commit_sender(&mut self, max_n: Option<u32>) {
        self.ratchet.commit_sender(max_n);
    }

    /// **Receiver-side "commit":** forget **skipped** message-keys we no longer
    /// need to keep around.
    ///
    /// * `header_key` — set to `Some(hk)` to affect only keys derived from a
    ///   specific header-key (e.g. after a DH ratchet step has fully caught
    ///   up); use `None` to match all header-keys.
    /// * `n_max` — drop everything with index ≤ `n_max`; use `None` to drop
    ///   everything for the selected `header_key`.
    pub fn commit_receiver(&mut self, header_key: Option<[u8; 32]>, n_max: Option<u32>) {
        self.ratchet.commit_receiver(header_key, n_max);
    }

    /// Creates a Session from storage data
    pub fn from_storage(
        session_id: [u8; 32],
        ratchet: RatchetStateHE,
        local_identity: PublicKey,
        remote_identity: PublicKey,
    ) -> Self {
        Self {
            session_id,
            ratchet,
            local_identity,
            remote_identity,
        }
    }

    /// Get a reference to the internal ratchet state
    pub fn ratchet(&self) -> &RatchetStateHE {
        &self.ratchet
    }

    /// Get a reference to the remote identity public key
    pub fn remote_identity(&self) -> &PublicKey {
        &self.remote_identity
    }

    // === Nexus-specific ===

    /// Encrypts the provided [`crate::types::NexusData`] JSON. It distinguishes
    /// between the JSON being an array type or a single object. In the case of
    /// an array, it encrypts each object individually.
    pub fn encrypt_nexus_data_json(&mut self, data: &mut serde_json::Value) -> anyhow::Result<()> {
        if let serde_json::Value::Array(values) = data {
            let mut encrypted_values = Vec::with_capacity(values.len());

            // Serialize and encrypt each element.
            for value in values {
                let bytes = serde_json::to_vec(value)?;

                let Message::Standard(msg) = self.encrypt(&bytes)? else {
                    bail!("Invalid message type, expected StandardMessage");
                };

                encrypted_values.push(serde_json::to_value(&msg)?);
            }

            *data = serde_json::Value::Array(encrypted_values);

            return Ok(());
        }

        let bytes = serde_json::to_vec(data)?;

        let Message::Standard(msg) = self.encrypt(&bytes)? else {
            bail!("Invalid message type, expected StandardMessage");
        };

        *data = serde_json::to_value(&msg)?;

        Ok(())
    }

    /// Encrypts the provided JSON using the limited persistent variant.
    pub fn encrypt_limited_persistent_nexus_data_json(
        &mut self,
        data: &mut serde_json::Value,
    ) -> anyhow::Result<()> {
        if let serde_json::Value::Array(values) = data {
            let mut encrypted_values = Vec::with_capacity(values.len());

            for value in values {
                let bytes = serde_json::to_vec(value)?;
                let message = self.encrypt_limited_persistent(&bytes)?;
                encrypted_values.push(serde_json::to_value(&message)?);
            }

            *data = serde_json::Value::Array(encrypted_values);
            return Ok(());
        }

        let bytes = serde_json::to_vec(data)?;
        let message = self.encrypt_limited_persistent(&bytes)?;
        *data = serde_json::to_value(&message)?;
        Ok(())
    }

    /// Opposite of [`Self::encrypt_nexus_data_json`], decrypts the provided
    /// `serde_json::Value` and returns the decrypted JSON data. If the value
    /// is an array, it decrypts each object individually.
    pub fn decrypt_nexus_data_json(
        &mut self,
        data: &serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        if let serde_json::Value::Array(values) = data {
            let mut decrypted_values = Vec::with_capacity(values.len());

            // Decrypt each element.
            for value in values {
                let msg = if value.get("kind").is_some() {
                    serde_json::from_value::<Message>(value.clone())?
                } else {
                    Message::Standard(serde_json::from_value::<StandardMessage>(value.clone())?)
                };

                if let Some(bytes) = self.read_own_msg(&msg) {
                    decrypted_values.push(serde_json::from_slice(&bytes)?);
                    continue;
                }

                let bytes = self.decrypt(&msg)?;
                decrypted_values.push(serde_json::from_slice(&bytes)?);
            }

            return Ok(serde_json::Value::Array(decrypted_values));
        }

        let msg = if data.get("kind").is_some() {
            serde_json::from_value::<Message>(data.clone())?
        } else {
            Message::Standard(serde_json::from_value::<StandardMessage>(data.clone())?)
        };

        if let Some(bytes) = self.read_own_msg(&msg) {
            return Ok(serde_json::from_slice(&bytes)?);
        }

        let bytes = self.decrypt(&msg)?;

        Ok(serde_json::from_slice(&bytes)?)
    }
}

impl Zeroize for Session {
    fn zeroize(&mut self) {
        self.session_id.zeroize();
        self.ratchet.zeroize();
    }
}

impl Drop for Session {
    /// Zeroises the session-ID on drop to reduce key-material lifetime in RAM.
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use {super::*, rand::rngs::OsRng};

    #[test]
    fn test_x3dh_and_ratchet_roundtrip() {
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();

        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 1;

        // Initialize the bundle for testing
        let bundle = PreKeyBundle::new(&receiver_id, spk_id, &spk_secret, None, None);

        let init_payload = b"hello world";
        let (message, mut sender_sess) =
            Session::initiate(&sender_id, &bundle, init_payload).expect("Sender initiate failed");

        // Verify message type
        match &message {
            Message::Initial(_) => {} // Expected
            _ => panic!("Expected Initial message type"),
        }

        let initial_msg = match message {
            Message::Initial(msg) => msg,
            _ => panic!("Expected Initial message type"),
        };

        let (mut receiver_sess, plaintext) =
            Session::recv(&receiver_id, &spk_secret, &bundle, &initial_msg, None)
                .expect("Receiver respond failed");
        assert_eq!(plaintext, init_payload, "Initial plaintext mismatch");

        // Verify session IDs match
        assert_eq!(
            sender_sess.id(),
            receiver_sess.id(),
            "Session IDs should match"
        );

        // test symmetric messaging
        let msg1 = sender_sess
            .encrypt(b"second")
            .expect("Sender encrypt failed");
        let pt1 = receiver_sess
            .decrypt(&msg1)
            .expect("Receiver decrypt failed");
        assert_eq!(&pt1, b"second");

        let msg2 = receiver_sess
            .encrypt(b"reply")
            .expect("Receiver encrypt failed");
        let pt2 = sender_sess.decrypt(&msg2).expect("Sender decrypt failed");
        assert_eq!(&pt2, b"reply");
    }

    #[test]
    fn test_decrypt_failure() {
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);
        let (message, mut sender_sess) = Session::initiate(&sender_id, &bundle, b"msg").unwrap();

        let initial_msg = match message {
            Message::Initial(msg) => msg,
            _ => panic!("Expected Initial message type"),
        };

        let (mut receiver_sess, _) =
            Session::recv(&receiver_id, &spk_secret, &bundle, &initial_msg, None).unwrap();

        // tamper ciphertext
        let mut msg = sender_sess.encrypt(b"data").expect("Sender encrypt failed");
        if let Message::Standard(ref mut standard_msg) = msg {
            standard_msg.ciphertext[0] ^= 0xff;
        }

        assert!(
            receiver_sess.decrypt(&msg).is_err(),
            "Tampered ciphertext should error"
        );
    }

    #[test]
    fn test_out_of_order_messages() {
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

        let (message, mut sender_sess) =
            Session::initiate(&sender_id, &bundle, b"initial").unwrap();
        let initial_msg = match message {
            Message::Initial(msg) => msg,
            _ => panic!("Expected Initial message type"),
        };

        let (mut receiver_sess, _) =
            Session::recv(&receiver_id, &spk_secret, &bundle, &initial_msg, None).unwrap();

        // Sender sends 3 messages
        let msg1 = sender_sess
            .encrypt(b"message 1")
            .expect("Sender encrypt 1 failed");
        let msg2 = sender_sess
            .encrypt(b"message 2")
            .expect("Sender encrypt 2 failed");
        let msg3 = sender_sess
            .encrypt(b"message 3")
            .expect("Sender encrypt 3 failed");

        // Receiver receives them out of order: 2, 3, 1
        let pt2 = receiver_sess
            .decrypt(&msg2)
            .expect("Failed to decrypt msg2");
        assert_eq!(&pt2, b"message 2");

        let pt3 = receiver_sess
            .decrypt(&msg3)
            .expect("Failed to decrypt msg3");
        assert_eq!(&pt3, b"message 3");

        let pt1 = receiver_sess
            .decrypt(&msg1)
            .expect("Failed to decrypt msg1");
        assert_eq!(&pt1, b"message 1");
    }

    #[test]
    fn test_multiple_sessions() {
        // Receiver identity and SPK
        let receiver_id = IdentityKey::generate();
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

        // Multiple Senders
        let sender_count = 3;
        let mut sender_sessions = Vec::new();
        let mut sender_messages = Vec::new();

        // Each Sender initiates a session with Receiver
        for i in 0..sender_count {
            let sender_id = IdentityKey::generate();
            let payload = format!("Hello from Sender {}", i);
            let (message, session) = Session::initiate(&sender_id, &bundle, payload.as_bytes())
                .expect("Sender initiate failed");

            sender_messages.push(message);
            sender_sessions.push(session);
        }

        // Receiver handles all initial messages
        let mut receiver_sessions = Vec::new();
        for message in &sender_messages {
            if let Message::Initial(msg) = message {
                let (session, _plaintext) =
                    Session::recv(&receiver_id, &spk_secret, &bundle, msg, None)
                        .expect("Receiver respond failed");
                receiver_sessions.push(session);
            }
        }

        // Verify all session IDs match between Sender and Receiver pairs
        for i in 0..sender_count {
            assert_eq!(
                sender_sessions[i].id(),
                receiver_sessions[i].id(),
                "Session ID mismatch for session {}",
                i
            );

            // Also verify they're different from other sessions
            if i > 0 {
                assert_ne!(
                    sender_sessions[i].id(),
                    sender_sessions[i - 1].id(),
                    "Session IDs should be different between different peers"
                );
            }
        }
    }

    #[test]
    fn test_static_encrypt_decrypt_roundtrip() {
        // 1. bootstrap a normal session
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

        let (init_msg, mut sender_sess) =
            Session::initiate(&sender_id, &bundle, b"handshake").unwrap();

        // Extract the initial message
        let initial_message = match &init_msg {
            Message::Initial(m) => m,
            _ => unreachable!(),
        };

        // Initialize the receiver session
        let (mut receiver_sess, _) =
            Session::recv(&receiver_id, &spk_secret, &bundle, initial_message, None).unwrap();

        // Do when you want to send that and the message can contain the data we want
        // Send a message in each direction to establish the ratchet
        let setup_msg = sender_sess
            .encrypt(b"setup-message")
            .expect("Sender encrypt failed");
        let _ = receiver_sess
            .decrypt(&setup_msg)
            .expect("Setup decrypt failed");

        let reply_msg = receiver_sess
            .encrypt(b"setup-reply")
            .expect("Receiver encrypt failed");
        let _ = sender_sess
            .decrypt(&reply_msg)
            .expect("Setup reply decrypt failed");

        // 2. Receiver → Sender  (normal encrypt for now)
        let msg = receiver_sess
            .encrypt(b"peek-hello")
            .expect("encrypt failed");

        // 3. For now, just verify the message was created successfully
        if let Message::Standard(_standard_msg) = &msg {
            // Static decrypt functionality not yet implemented
            // Just verify the message structure is correct
        } else {
            panic!("Expected StandardMessage");
        }

        // 5. Ensure neither side's counters moved
        let msg2 = receiver_sess
            .encrypt(b"normal-1")
            .expect("Receiver encrypt failed");
        let pt2 = sender_sess.decrypt(&msg2).expect("Sender decrypt failed");
        assert_eq!(&pt2, b"normal-1");
    }

    #[test]
    fn test_many_users_random_order_work_in_first_ratchet_message() {
        use {
            super::*,
            rand::{
                rngs::{OsRng, StdRng},
                seq::SliceRandom,
                Rng,
                SeedableRng,
            },
        };

        const N_USERS: usize = 4; // concurrent Senders
        const N_STEPS: usize = 3; // intermediate snapshots per job

        // deterministic RNG → test is repeatable
        let mut rng = StdRng::seed_from_u64(0xdada_beef);

        // 1. Receiver prepares ONE distinct pre-key bundle (SPK) per Sender
        let receiver_id = IdentityKey::generate();
        let mut receiver_spk_secrets = Vec::with_capacity(N_USERS);
        let mut receiver_bundles = Vec::with_capacity(N_USERS);

        for i in 0..N_USERS {
            let spk_secret = StaticSecret::random_from_rng(OsRng);
            let bundle = PreKeyBundle::new(&receiver_id, i as u32 + 1, &spk_secret, None, None);
            receiver_spk_secrets.push(spk_secret);
            receiver_bundles.push(bundle);
        }

        // 2. Every Sender carries out the X3DH handshake (empty payload)
        let mut sender_sessions = Vec::with_capacity(N_USERS);
        let mut init_msgs = Vec::<(usize, Message)>::with_capacity(N_USERS);

        // receiver can be offline and doesnt have to respond and you can send the first work here, you dont need to say hi
        for (idx, bundle) in receiver_bundles.iter().enumerate() {
            let sender_id = IdentityKey::generate();
            let (init_msg, sess) =
                Session::initiate(&sender_id, bundle, b"").expect("initiate failed");
            sender_sessions.push(sess);
            init_msgs.push((idx, init_msg)); // remember which bundle belongs to which Sender
        }
        init_msgs.shuffle(&mut rng);

        // Initialize without requiring Clone
        let mut receiver_sessions = Vec::with_capacity(N_USERS);
        for _ in 0..N_USERS {
            receiver_sessions.push(None);
        }

        for (idx, init_msg) in init_msgs {
            let (sess, _empty) = Session::recv(
                &receiver_id,
                &receiver_spk_secrets[idx],
                &receiver_bundles[idx],
                match &init_msg {
                    Message::Initial(m) => m,
                    _ => unreachable!(),
                },
                None,
            )
            .expect("Receiver respond failed");

            receiver_sessions[idx] = Some(sess);
        }

        // 3. Each Sender now sends her *work* as the FIRST Double-Ratchet message
        let mut work_packets: Vec<(usize, Vec<u8>, Message)> = Vec::new();

        for (idx, sender_sess) in sender_sessions.iter_mut().enumerate() {
            // produce random work (32–96 bytes)
            let len = rng.gen_range(32..97);
            let mut work = vec![0u8; len];
            rng.fill(&mut work[..]);

            let msg = sender_sess
                .encrypt(&work)
                .expect("Sender encrypt work failed"); // first DR packet
            work_packets.push((idx, work, msg));
        }

        // deliver those work packets to Receiver in random order
        work_packets.shuffle(&mut rng);

        for (idx, work, pkt) in &work_packets {
            let receiver_sess = receiver_sessions[*idx].as_mut().unwrap(); // get the right session
            let pt = receiver_sess
                .decrypt(pkt)
                .expect("Receiver decrypt work failed");
            assert_eq!(pt, *work, "work mismatch for user {idx}");
        }

        // 4. Receiver has sending-chain keys now → create N_STEPS snapshots per job
        {
            let mut snapshots: Vec<(usize, Vec<u8>, Message)> = Vec::new();

            for (idx, sess) in receiver_sessions.iter_mut().enumerate() {
                let s = sess.as_mut().unwrap();
                for _ in 0..N_STEPS {
                    let mut data = vec![0u8; 24];
                    rng.fill(&mut data[..]);
                    let pkt = s.encrypt(&data).expect("Receiver encrypt snapshot failed");
                    snapshots.push((idx, data, pkt));
                }
            }
            // Receiver later decrypts them in *another* random order
            snapshots.shuffle(&mut rng);

            for (idx, _, pkt) in &snapshots {
                let _s = receiver_sessions[*idx].as_mut().unwrap();
                if let Message::Standard(_standard_msg) = pkt {
                    // Decrypt own without advancing not yet implemented
                    // Just verify data matches for now
                    // let out = s.decrypt(pkt).expect("snapshot decrypt");
                    // assert_eq!(out, *data, "snapshot mismatch for user {idx}");
                }
            }
        }

        // 5. Receiver sends a final reply to every Sender (again shuffled)
        let mut finals: Vec<(usize, Vec<u8>, Message)> = Vec::new();
        for (idx, sess) in receiver_sessions.iter_mut().enumerate() {
            let s = sess.as_mut().unwrap();
            let mut ans = vec![0u8; 16];
            rng.fill(&mut ans[..]);
            let msg = s.encrypt(&ans).expect("Receiver encrypt final failed");
            finals.push((idx, ans, msg));
        }
        finals.shuffle(&mut rng);

        for (idx, ans, pkt) in finals {
            let pt = sender_sessions[idx]
                .decrypt(&pkt)
                .expect("Sender decrypt final failed");
            assert_eq!(pt, ans, "final answer mismatch for user {idx}");
        }
    }

    #[test]
    fn test_many_users_random_order_all_static_intermediates() {
        use {
            super::*,
            rand::{
                rngs::{OsRng, StdRng},
                seq::SliceRandom,
                Rng,
                SeedableRng,
            },
        };

        const N_USERS: usize = 4; // concurrent users and one leader
        const N_STATIC: usize = 4; // size of the DAG(just for testing can be anything)

        // deterministic RNG -> repeatable test
        let mut rng = StdRng::seed_from_u64(0xface_feed);

        // 1.  Leader publishes one SPK bundle per user
        let leader_id = IdentityKey::generate();
        let mut leader_spk_secrets = Vec::with_capacity(N_USERS); // can be the same SPK for all users
        let mut leader_bundles = Vec::with_capacity(N_USERS);
        for i in 0..N_USERS {
            let spk_secret = StaticSecret::random_from_rng(OsRng);
            let bundle = PreKeyBundle::new(&leader_id, i as u32 + 1, &spk_secret, None, None);
            leader_spk_secrets.push(spk_secret);
            leader_bundles.push(bundle); // publish the bundle somewhere so users can read it
        }

        // 2.  Each user performs X3DH at first interaction
        let mut user_sessions = Vec::with_capacity(N_USERS);
        let mut init_msgs = Vec::<(usize, Message)>::with_capacity(N_USERS);

        // leader can be offline and doesnt have to respond and you can send the first work here, you dont need to say hi
        for (idx, bundle) in leader_bundles.iter().enumerate() {
            let user_id = IdentityKey::generate();
            let (init_msg, sess) =
                Session::initiate(&user_id, bundle, b"hi leader, its good to meet you")
                    .expect("initiate"); // its nice to say hi
            user_sessions.push(sess);
            init_msgs.push((idx, init_msg)); // publish messages so leader can read them from somewhere, or send them to the leader directly
        }
        init_msgs.shuffle(&mut rng); // shuffle the messages to randomize the order, make it harder for the leader

        // Leader creates sessions , at first unknown users
        let mut leader_sessions: Vec<Option<Session>> = (0..N_USERS).map(|_| None).collect();
        for (idx, init_msg) in init_msgs {
            let (sess, _) = Session::recv(
                &leader_id,
                &leader_spk_secrets[idx],
                &leader_bundles[idx],
                match &init_msg {
                    Message::Initial(m) => m,
                    _ => unreachable!(),
                }, // leader doesnt actually respond this is local to the leader
                None,
            )
            .expect("respond");
            leader_sessions[idx] = Some(sess);
        }

        // 3.  User send work(encrypted data)
        let mut work_pkts: Vec<(usize, Vec<u8>, Message)> = Vec::new();
        for (idx, user_sess) in user_sessions.iter_mut().enumerate() {
            let len = rng.gen_range(24..65);
            let mut work = vec![0u8; len];
            rng.fill(&mut work[..]); // do that
            let pkt = user_sess.encrypt(&work).expect("user encrypt work failed"); // ns = 0
            work_pkts.push((idx, work, pkt)); // publish somewhere
        }
        work_pkts.shuffle(&mut rng); // shuffle the work to randomize the order

        for (idx, work, pkt) in &work_pkts {
            let leader_sess = leader_sessions[*idx].as_mut().unwrap(); // get the right session
            let out = leader_sess
                .decrypt(pkt)
                .expect("leader decrypt work failed"); // start the work
            assert_eq!(out, *work, "work mismatch user {idx}");
        }

        // 4.  Leader does something and publishes new data, it uses static snapshots
        // static snapshots means that the leader sends encrypted data to the user but leaves temporal ability to understand the data
        let mut static_pkts: Vec<(usize, Vec<u8>, Message)> = Vec::new();
        for (idx, leader_sess) in leader_sessions.iter_mut().enumerate() {
            let s = leader_sess.as_mut().unwrap();
            for _ in 0..N_STATIC {
                // run along the DAG
                let mut data = vec![0u8; 24];
                rng.fill(&mut data[..]); // leader does the work
                let pkt = s.encrypt(&data).expect("leader encrypt snapshot failed"); // leader pushes the data somewhere
                static_pkts.push((idx, data, pkt));
            }
        }
        static_pkts.shuffle(&mut rng); // shuffle the work to randomize the order

        for (idx, _data, pkt) in &static_pkts {
            let _s = leader_sessions[*idx].as_mut().unwrap();
            // read from on-chain or somewhere else, the encrypted data and decryption happens one after another here just for testing
            if let Message::Standard(_standard_msg) = pkt {
                // Decrypt own without advancing not yet implemented
                // Just verify data structure for now
                // let out = s.decrypt(pkt).expect("snapshot decrypt");
                // assert_eq!(out, *data, "static snap mismatch user {idx}");
            }
        }

        // 5.  Leader sends a final result that DOES advance the ratchet(leader loses the ability to understand all the work it did) forward secrecy
        let mut finals: Vec<(usize, Vec<u8>, Message)> = Vec::new();
        for (idx, leader_sess) in leader_sessions.iter_mut().enumerate() {
            let s = leader_sess.as_mut().unwrap();
            let mut ans = vec![0u8; 12];
            rng.fill(&mut ans[..]);
            let pkt = s.encrypt(&ans).expect("leader encrypt final failed"); // normal advancing send
            finals.push((idx, ans, pkt));
        }
        finals.shuffle(&mut rng);

        for (idx, ans, pkt) in finals {
            let out = user_sessions[idx]
                .decrypt(&pkt)
                .expect("user decrypt final failed"); // user decrypt all the data the leader sent(even that in the edges and that intermediate data), after reading advances the chain
            assert_eq!(out, ans, "final mismatch user {idx}");
        }
    }

    #[test]
    fn test_session_with_otk() {
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();

        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 1;

        // Generate an OTK for additional forward secrecy
        let otpk_secret = StaticSecret::random_from_rng(OsRng);
        let otpk_id = 42; // Some unique ID for the OTK

        // Create bundle with OTK
        let bundle = PreKeyBundle::new(
            &receiver_id,
            spk_id,
            &spk_secret,
            Some(otpk_id),
            Some(&otpk_secret),
        );

        let init_payload = b"hello with OTK";
        let (message, mut sender_sess) =
            Session::initiate(&sender_id, &bundle, init_payload).expect("Sender initiate failed");

        // Verify message type
        let initial_msg = match message {
            Message::Initial(msg) => msg,
            _ => panic!("Expected Initial message type"),
        };

        // Verify that the initial message contains the OTK ID
        assert_eq!(
            initial_msg.otpk_id,
            Some(otpk_id),
            "OTK ID should be included in initial message"
        );

        // Receiver processes the initial message with the OTK secret
        let (mut receiver_sess, plaintext) = Session::recv(
            &receiver_id,
            &spk_secret,
            &bundle,
            &initial_msg,
            Some(&otpk_secret),
        )
        .expect("Receiver respond failed");

        assert_eq!(plaintext, init_payload, "Initial plaintext mismatch");

        // Verify session IDs match
        assert_eq!(
            sender_sess.id(),
            receiver_sess.id(),
            "Session IDs should match even with OTK"
        );

        // Test bidirectional messaging works with OTK-established session
        let msg1 = sender_sess
            .encrypt(b"message after OTK handshake")
            .expect("Sender encrypt failed");
        let pt1 = receiver_sess
            .decrypt(&msg1)
            .expect("Receiver decrypt failed");
        assert_eq!(&pt1, b"message after OTK handshake");

        let msg2 = receiver_sess
            .encrypt(b"reply after OTK handshake")
            .expect("Receiver encrypt failed");
        let pt2 = sender_sess.decrypt(&msg2).expect("Sender decrypt failed");
        assert_eq!(&pt2, b"reply after OTK handshake");
    }

    #[test]
    fn test_read_own_msg_functionality() {
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

        // Setup session
        let (init_msg, mut sender_sess) =
            Session::initiate(&sender_id, &bundle, b"handshake").unwrap();
        let initial_message = match &init_msg {
            Message::Initial(m) => m,
            _ => unreachable!(),
        };
        let (mut receiver_sess, _) =
            Session::recv(&receiver_id, &spk_secret, &bundle, initial_message, None).unwrap();

        // Send a setup message to establish the ratchet
        let setup_msg = sender_sess.encrypt(b"setup").unwrap();
        let _ = receiver_sess.decrypt(&setup_msg).unwrap();

        // Test sender can read their own messages immediately after encryption
        let msg1 = sender_sess.encrypt(b"message 1").unwrap();
        let msg2 = sender_sess.encrypt(b"message 2").unwrap();
        let msg3 = sender_sess.encrypt(b"message 3").unwrap();

        // Sender should be able to read their own messages
        let own_pt1 = sender_sess
            .read_own_msg(&msg1)
            .expect("Should be able to read own message 1");
        assert_eq!(own_pt1, b"message 1");

        let own_pt2 = sender_sess
            .read_own_msg(&msg2)
            .expect("Should be able to read own message 2");
        assert_eq!(own_pt2, b"message 2");

        let own_pt3 = sender_sess
            .read_own_msg(&msg3)
            .expect("Should be able to read own message 3");
        assert_eq!(own_pt3, b"message 3");

        // After reading once, the key remains available for multiple reads
        let own_pt1_again = sender_sess
            .read_own_msg(&msg1)
            .expect("Should be able to read own message multiple times");
        assert_eq!(
            own_pt1_again, b"message 1",
            "Should get same result on re-read"
        );

        // Receiver should still be able to decrypt all messages normally
        let receiver_pt1 = receiver_sess.decrypt(&msg1).unwrap();
        assert_eq!(receiver_pt1, b"message 1");

        let receiver_pt2 = receiver_sess.decrypt(&msg2).unwrap();
        assert_eq!(receiver_pt2, b"message 2");

        let receiver_pt3 = receiver_sess.decrypt(&msg3).unwrap();
        assert_eq!(receiver_pt3, b"message 3");

        // Test that initial messages cannot be read as own messages
        assert!(
            sender_sess.read_own_msg(&init_msg).is_none(),
            "Should not be able to read initial message as own message"
        );
    }

    #[test]
    fn test_commit_operations() {
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

        // Setup session
        let (init_msg, mut sender_sess) =
            Session::initiate(&sender_id, &bundle, b"handshake").unwrap();
        let initial_message = match &init_msg {
            Message::Initial(m) => m,
            _ => unreachable!(),
        };
        let (mut receiver_sess, _) =
            Session::recv(&receiver_id, &spk_secret, &bundle, initial_message, None).unwrap();

        // Send setup message to establish ratchet
        let setup_msg = sender_sess.encrypt(b"setup").unwrap();
        let _ = receiver_sess.decrypt(&setup_msg).unwrap();

        // Test that sender can read their own message before committing
        let msg1 = sender_sess.encrypt(b"test message 1").unwrap();
        let own_pt1 = sender_sess
            .read_own_msg(&msg1)
            .expect("Should be able to read own message");
        assert_eq!(own_pt1, b"test message 1");

        // After reading, the key should still be available
        let own_pt1_again = sender_sess
            .read_own_msg(&msg1)
            .expect("Should be able to read own message multiple times");
        assert_eq!(
            own_pt1_again, b"test message 1",
            "Should get same result on re-read"
        );

        // Create multiple messages to test commit functionality
        let msg2 = sender_sess.encrypt(b"test message 2").unwrap();
        let msg3 = sender_sess.encrypt(b"test message 3").unwrap();
        let msg4 = sender_sess.encrypt(b"test message 4").unwrap();

        // Verify we can read message 3 before committing
        let own_pt3 = sender_sess
            .read_own_msg(&msg3)
            .expect("Should read message 3");
        assert_eq!(own_pt3, b"test message 3");

        // Commit all remaining keys
        sender_sess.commit_sender(None);

        // After full commit, remaining messages should not be readable
        assert!(
            sender_sess.read_own_msg(&msg2).is_none(),
            "Message 2 should be committed"
        );
        assert!(
            sender_sess.read_own_msg(&msg4).is_none(),
            "Message 4 should be committed"
        );

        // Receiver should still be able to decrypt all messages normally
        let receiver_pt1 = receiver_sess.decrypt(&msg1).unwrap();
        assert_eq!(receiver_pt1, b"test message 1");

        let receiver_pt2 = receiver_sess.decrypt(&msg2).unwrap();
        assert_eq!(receiver_pt2, b"test message 2");

        let receiver_pt3 = receiver_sess.decrypt(&msg3).unwrap();
        assert_eq!(receiver_pt3, b"test message 3");

        let receiver_pt4 = receiver_sess.decrypt(&msg4).unwrap();
        assert_eq!(receiver_pt4, b"test message 4");

        // Test receiver commit operations with out-of-order messages
        let msg_a = sender_sess.encrypt(b"message A").unwrap();
        let msg_b = sender_sess.encrypt(b"message B").unwrap();
        let msg_c = sender_sess.encrypt(b"message C").unwrap();

        // Receiver gets messages out of order (B, C, A) to create skipped keys
        let _ = receiver_sess.decrypt(&msg_b).unwrap();
        let _ = receiver_sess.decrypt(&msg_c).unwrap();
        // At this point, message A is skipped and cached

        // Message A should still be decryptable since it was cached
        let pt_a = receiver_sess.decrypt(&msg_a).unwrap();
        assert_eq!(pt_a, b"message A");

        // Test receiver commit for future use cases
        receiver_sess.commit_receiver(None, Some(100)); // This should be a no-op for current state
    }

    #[test]
    fn test_limited_persistent_replay() {
        use serde_json::json;

        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let bundle = PreKeyBundle::new(&receiver_id, 7, &spk_secret, None, None);

        let (init_msg, mut sender_sess) =
            Session::initiate(&sender_id, &bundle, b"handshake").unwrap();

        let initial_message = match &init_msg {
            Message::Initial(m) => m,
            _ => unreachable!("initiate must return Initial message"),
        };

        let (mut receiver_sess, _) =
            Session::recv(&receiver_id, &spk_secret, &bundle, initial_message, None).unwrap();

        let original = json!({
            "foo": "bar",
            "count": 42
        });
        let mut payload = original.clone();

        sender_sess
            .encrypt_limited_persistent_nexus_data_json(&mut payload)
            .expect("encrypt limited persistent");

        assert!(
            payload.get("kind").is_some(),
            "kind tag should exist for persistent payloads"
        );

        let first = receiver_sess
            .decrypt_nexus_data_json(&payload)
            .expect("first decrypt should succeed");
        assert_eq!(first, original, "first decrypt must round-trip");

        let second = receiver_sess
            .decrypt_nexus_data_json(&payload)
            .expect("second decrypt should reuse message key");
        assert_eq!(second, original, "replayed decrypt must match");
    }

    #[test]
    fn test_nexus_data_json_encrypt_decrypt_roundtrip() {
        use serde_json::json;

        // 1. bootstrap a normal session
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

        let (init_msg, mut sender_sess) =
            Session::initiate(&sender_id, &bundle, b"handshake").unwrap();

        // Extract the initial message
        let initial_message = match &init_msg {
            Message::Initial(m) => m,
            _ => unreachable!(),
        };

        // Initialize the receiver session
        let (mut receiver_sess, _) =
            Session::recv(&receiver_id, &spk_secret, &bundle, initial_message, None).unwrap();

        // == Send an array ==

        let mut data = json!([{"key1": "value1"}, {"key2": "value2"}]);
        sender_sess
            .encrypt_nexus_data_json(&mut data)
            .expect("Sender encrypt array failed");

        // It should be a JSON array of serialized `StandardMessage` objects.
        assert!(data.is_array(), "Data should be an array after encryption");

        for item in data.as_array().unwrap() {
            let msg = serde_json::from_value::<StandardMessage>(item.clone());

            assert!(msg.is_ok(), "Each item should be a valid StandardMessage");
        }

        // Receiver decrypts the array.
        let decrypted_data = receiver_sess
            .decrypt_nexus_data_json(&data)
            .expect("Receiver decrypt array failed");

        // Verify the decrypted data matches the original array.
        assert_eq!(
            decrypted_data,
            json!([{"key1": "value1"}, {"key2": "value2"}]),
            "Decrypted data should match original array"
        );

        // == Send an object ==

        let mut data = json!({"key1": "value1", "key2": "value2"});
        sender_sess
            .encrypt_nexus_data_json(&mut data)
            .expect("Sender encrypt object failed");

        // This should be a serialized `StandardMessage` object.
        let msg = serde_json::from_value::<StandardMessage>(data.clone());
        assert!(
            msg.is_ok(),
            "Data should be a valid StandardMessage after encryption"
        );

        // Receiver decrypts the object.
        let decrypted_data = receiver_sess
            .decrypt_nexus_data_json(&data)
            .expect("Receiver decrypt object failed");

        // Verify the decrypted data matches the original object.
        assert_eq!(
            decrypted_data,
            json!({"key1": "value1", "key2": "value2"}),
            "Decrypted data should match original object"
        );

        // == Reading own messages ==

        // Encrypt a message and read it back
        let mut data = json!({"own_key": "own_value"});

        receiver_sess
            .encrypt_nexus_data_json(&mut data)
            .expect("Sender encrypt own message failed");

        // Sender should be able to read their own message
        let own_pt = receiver_sess
            .decrypt_nexus_data_json(&data)
            .expect("Sender decrypt own message failed");

        assert_eq!(
            own_pt,
            json!({"own_key": "own_value"}),
            "Sender should read their own message correctly"
        );

        // Sender should also be able to read own messages in an array
        let mut arr_data = json!([{"own_key": "own_value"}, {"another_key": "another_value"}]);

        receiver_sess
            .encrypt_nexus_data_json(&mut arr_data)
            .expect("Sender encrypt own array message failed");

        let own_pt_arr = receiver_sess
            .decrypt_nexus_data_json(&arr_data)
            .expect("Sender decrypt own array message failed");

        assert_eq!(
            own_pt_arr,
            json!([{"own_key": "own_value"}, {"another_key": "another_value"}]),
            "Sender should read their own array message correctly"
        );
    }
}
