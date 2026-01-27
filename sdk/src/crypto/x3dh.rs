#![forbid(unsafe_code)]
#![allow(unused_assignments)]
//! # X3DH: Extended Triple Diffie‑Hellman key agreement
//!
//! This module implements the X3DH protocol as described in Signal's public
//! specification. X3DH enables two parties to establish a shared secret even
//! when one party (typically the Receiver) is offline.  The resulting secret can be
//! used as the root key for a Double‑Ratchet or any other post‑handshake
//! encryption scheme.
//!
//! ## High‑level flow
//!
//! ```text
//! Sender                                    Receiver (that can be offline)
//! ───────────────────────────────────────────────────────────
//! Generate IdentityKey               Generate IdentityKey
//!                                     │
//!                                     ├─► Publish PreKeyBundle ──┐
//! ┌─(1) download bundle ◄─────────────┘                         │
//! │                                                           (server)
//! │  (2) sender_init()                                          │
//! ├─► InitialMessage ───────────────────────────────────────────┤
//!                                     └─► (3) receiver_receive() ════┘
//! ```
//!
//! After step (3) both parties possess the same 32‑byte [`SharedSecret`].
//!
//! ## Example
//!
//! ```
//! use nexus_sdk::crypto::x3dh::{IdentityKey, PreKeyBundle, sender_init, receiver_receive};
//! use x25519_dalek::StaticSecret;
//! use rand::rngs::OsRng;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Long‑term identity keys
//! let sender_id = IdentityKey::generate();
//! let receiver_id = IdentityKey::generate();
//!
//! // 2. Receiver generates a pre-key bundle with a Signed Pre-Key
//! let spk_id = 1;
//! let spk_secret = StaticSecret::random_from_rng(OsRng);
//! let bundle = PreKeyBundle::new(&receiver_id, spk_id, &spk_secret, None, None);
//!
//! // 3. Sender encrypts greeting.
//! let (msg, sender_sk) = sender_init(&sender_id, &bundle, b"hello, Receiver!")?;
//!
//! // 4. Receiver decrypts.
//! let (plain, receiver_sk) = receiver_receive(&receiver_id, &spk_secret, spk_id, None, &msg)?;
//!
//! assert_eq!(plain, b"hello, Receiver!");
//! assert_eq!(&*sender_sk, &*receiver_sk); // handshake success
//! # Ok(()) }
//! ```

use subtle::ConstantTimeEq; // Constant‑time comparison
use {
    // For custom IdentityKey (de)serialisation
    super::secret_bytes::SecretBytes,
    aead::{Aead, KeyInit, Payload},
    chacha20poly1305::{XChaCha20Poly1305, XNonce},
    hkdf::Hkdf,
    rand::rngs::OsRng,
    rand_core::RngCore,
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    sha2::Sha256,
    thiserror::Error,
    x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret},
    xeddsa::{
        xed25519::{PrivateKey as XEdPrivate, PublicKey as XEdPublic},
        Sign,
        Verify,
    },
    zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing},
};

/// Curve identifier for `Encode(PK)` (see section 2.5 of the X3DH spec).
const CURVE_ID_X25519: u8 = 0x05;
/// Maximum ciphertext length accepted in a pre‑key message (**16 KiB**).
const MAX_PRE_KEY_MSG: usize = 16 * 1024;
/// Default string fed into HKDF's `info` field. Overwrite when its needed.
const HKDF_INFO: &[u8] = b"X3DH";

/// Shared secret produced by X3DH.
///
/// The `Zeroizing` wrapper guarantees that the 32‑byte buffer is wiped from
/// memory when dropped, preventing accidental key leakage.
pub type SharedSecret = Zeroizing<[u8; 32]>;

// === Error handling ===

/// Enumeration of all errors that may arise when running X3DH.
#[derive(Debug, Error)]
pub enum X3dhError {
    /// Signature verification of the signed pre‑key (SPK) failed.
    #[error("signature verification failed")]
    SigVerifyFailed,
    /// Authenticated decryption failed (ciphertext corrupt or wrong key).
    #[error("decryption failed")]
    DecryptFailed,
    /// Receiver attempted to decrypt a message that references an OTPK he no longer possesses.
    #[error("OTPK secret missing - refuse to process one-time pre-key message")]
    MissingOneTimeSecret,
    /// The SPK identifier in the message does not match Receiver's current SPK.
    #[error("signed pre-key id mismatch")]
    SpkIdMismatch,
    /// The OTPK identifier in the message does not match the supplied OTPK secret.
    #[error("one-time pre-key id mismatch")]
    OtpkIdMismatch,
    /// Receiver's X25519 and XEdDSA public keys are not the Edwards–Montgomery map of each other.
    #[error("identity DH and Ed keys do not match")]
    IdentityKeyMismatch,
    /// Internal HKDF error (should be unreachable under sane parameters).
    #[error("HKDF output length is wrong")]
    HkdfInvalidLength,
    /// Authenticated encryption error (should be unreachable).
    #[error("AEAD error")]
    Aead,
    /// Ciphertext length exceeded [`MAX_PRE_KEY_MSG`].
    #[error("ciphertext too large")]
    CiphertextTooLarge,
}

impl From<hkdf::InvalidLength> for X3dhError {
    fn from(_: hkdf::InvalidLength) -> Self {
        Self::HkdfInvalidLength
    }
}

// === Helper utilities ===

/// Convenience methods for [`XEdPublic`].
/// Extension trait for XEdPublic
trait XEdPublicExt {
    /// Return the public key as a `[u8; 32]` reference.
    fn as_bytes(&self) -> &[u8; 32];
    /// Create a new public key from raw bytes.
    fn from_bytes(bytes: [u8; 32]) -> Self;
}

impl XEdPublicExt for XEdPublic {
    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn from_bytes(bytes: [u8; 32]) -> Self {
        XEdPublic(bytes)
    }
}

/// Encode a Curve25519 public key as `curve_id || u_coordinate` (33 bytes).
///
/// This is the format used by the X3DH specification.
#[inline]
fn encode_pk(pk: &X25519PublicKey) -> [u8; 33] {
    let mut out = [0u8; 33];
    out[0] = CURVE_ID_X25519;
    out[1..].copy_from_slice(pk.as_bytes());
    out
}

/// HKDF wrapper (SHA‑256) with a 32×`0xff` domain separator.
///
/// * `dhs` – list of raw Diffie‑Hellman outputs (`dh1..dh4`).
/// * `info` – application‑specific label (defaults to [`HKDF_INFO`]).
///
/// Returns a [`SharedSecret`] that is securely zeroised on drop.
fn kdf(dhs: &[&[u8]], info: &[u8]) -> Result<SharedSecret, X3dhError> {
    // Input key material
    let mut ikm = Vec::with_capacity(32 + 32 * dhs.len());
    // Domain separator – mitigates cross‑protocol attacks.
    ikm.extend([0xffu8; 32]);
    for dh in dhs {
        ikm.extend_from_slice(dh);
    }
    // 32‑byte zero salt (per spec §3.2)
    let salt = [0u8; 32];
    let hk = Hkdf::<Sha256>::new(Some(&salt), &ikm);
    // Output key material
    let mut okm = [0u8; 32];
    hk.expand(info, &mut okm)?;
    // Zeroise temporary key material
    ikm.zeroize();
    Ok(Zeroizing::new(okm))
}

// === Long‑term identity keys ===

/// Combined Diffie‑Hellman and XEdDSA identity key pair.
///
/// A single 32‑byte secret scalar serves double purpose – it is interpreted in
/// Montgomery form for X25519 and in Edwards form for XEdDSA.
#[allow(unused_assignments)]
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct IdentityKey {
    /// 32‑byte X25519 secret.
    secret: StaticSecret,
    /// Corresponding Montgomery public key.
    #[zeroize(skip)]
    pub dh_public: X25519PublicKey,
    /// Edwards private key for signatures.
    signing: XEdPrivate,
    /// Edwards public key for verification.
    #[zeroize(skip)]
    pub verify: XEdPublic,
}

impl IdentityKey {
    /// Generate a fresh identity key pair.
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let dh_public = X25519PublicKey::from(&secret);
        let signing = XEdPrivate::from(&secret);
        let verify = XEdPublic::from(&dh_public);
        Self {
            secret,
            dh_public,
            signing,
            verify,
        }
    }

    /// Get a reference to the secret key
    pub fn secret(&self) -> &StaticSecret {
        &self.secret
    }

    /// Create an identity key from an existing secret
    pub fn from_secret(secret: StaticSecret) -> Self {
        let dh_public = X25519PublicKey::from(&secret);
        let signing = XEdPrivate::from(&secret);
        let verify = XEdPublic::from(&dh_public);
        Self {
            secret,
            dh_public,
            signing,
            verify,
        }
    }
}

// Custom Serde for IdentityKey

impl Serialize for IdentityKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialise only the secret scalar using the helper wrapper.
        SecretBytes::from(&self.secret).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for IdentityKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Reconstruct IdentityKey from the secret bytes.
        let secret_bytes = SecretBytes::deserialize(deserializer)?;
        let secret: StaticSecret = secret_bytes.into();
        Ok(IdentityKey::from_secret(secret))
    }
}

// === Pre‑Key material (Receiver‑side) ===

/// Public bundle uploaded by Receiver
///
/// The bundle contains:
/// * the *Signed Pre‑Key* (SPK) plus its identifier and XEdDSA signature,
/// * Receiver's long‑term *Identity Key* (`IK_B`),
/// * **optionally** one *One‑Time Pre‑Key* (OTPK).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreKeyBundle {
    /// Identifier of the signed pre‑key.
    pub spk_id: u32,
    /// SPK public key.
    #[serde(with = "x25519_serde")]
    pub spk_pub: X25519PublicKey,
    /// XEdDSA signature over `Encode(spk_pub)`.
    #[serde(with = "serde_big_array::BigArray")]
    pub spk_sig: [u8; 64],
    /// Raw bytes of `Ed25519(IK_B)`.
    /// It allows for verification of the signature of the SPK.
    pub identity_verify_bytes: [u8; 32],
    /// DH form of Receiver's identity key.
    #[serde(with = "x25519_serde")]
    pub identity_pk: X25519PublicKey,
    /// Identifier of the accompanying OTPK (if any).
    pub otpk_id: Option<u32>,
    /// OTPK public key.
    #[serde(with = "option_x25519_serde")]
    pub otpk_pub: Option<X25519PublicKey>,
}

impl PreKeyBundle {
    /// Assemble a bundle from an Identity Key and freshly generated SPK/OTPK.
    pub fn new(
        identity: &IdentityKey,
        spk_id: u32,
        spk_secret: &StaticSecret,
        otpk_id: Option<u32>,
        otpk_secret: Option<&StaticSecret>,
    ) -> Self {
        let spk_pub = X25519PublicKey::from(spk_secret);
        // Signature proves possession of IK_B
        let spk_sig = identity.signing.sign(&encode_pk(&spk_pub), OsRng);
        let identity_verify_bytes = *identity.verify.as_bytes();

        Self {
            spk_id,
            spk_pub,
            spk_sig,
            identity_verify_bytes,
            identity_pk: identity.dh_public,
            otpk_id,
            otpk_pub: otpk_secret.map(X25519PublicKey::from),
        }
    }

    /// Verify `spk_sig` ⁠and⁠ the Montgomery⇄Edwards mapping for `IK_B`.
    /// Used to verify the integrity of the PreKeyBundle.
    pub fn verify_spk(&self) -> bool {
        let spk_bytes = encode_pk(&self.spk_pub);
        let identity_verify = self.get_identity_verify();

        // Check that `Ed(IK_B)` == Edwards map of `Mont(IK_B)`.
        let expected_verify = XEdPublic::from(&self.identity_pk);
        if identity_verify
            .as_bytes()
            .ct_eq(expected_verify.as_bytes())
            .unwrap_u8()
            == 0
        {
            return false;
        }
        identity_verify.verify(&spk_bytes, &self.spk_sig).is_ok()
    }

    /// Helper: return XEdDSA public key.
    fn get_identity_verify(&self) -> XEdPublic {
        XEdPublic::from_bytes(self.identity_verify_bytes)
    }
}

// === Serde helpers ===

/// Serde (de)serialization for `x25519_dalek::PublicKey`.
///
/// Serializes as a raw 32‑byte string.
pub mod x25519_serde {
    use {
        super::X25519PublicKey,
        serde::{
            de::{Error, SeqAccess, Visitor},
            Deserializer,
            Serializer,
        },
        std::fmt,
    };

    pub fn serialize<S>(key: &X25519PublicKey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(key.as_bytes())
    }
    struct PublicKeyVisitor;

    impl<'de> Visitor<'de> for PublicKeyVisitor {
        type Value = X25519PublicKey;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a 32‑byte X25519 public key")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if v.len() != 32 {
                return Err(E::custom(format!("expected 32 bytes, got {}", v.len())));
            }
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(v);
            Ok(X25519PublicKey::from(bytes))
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut bytes = [0u8; 32];

            for (idx, slot) in bytes.iter_mut().enumerate() {
                let Some(value) = seq.next_element::<u8>()? else {
                    return Err(A::Error::invalid_length(idx, &self));
                };
                *slot = value;
            }

            if seq.next_element::<u8>()?.is_some() {
                return Err(A::Error::invalid_length(33, &self));
            }

            Ok(X25519PublicKey::from(bytes))
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<X25519PublicKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(PublicKeyVisitor)
    }
}

/// Serde (de)serialization for `Option<x25519_dalek::PublicKey>`.
///
/// Serializes as an optional raw 32‑byte string.
pub mod option_x25519_serde {
    use {
        serde::{Deserialize, Deserializer, Serialize, Serializer},
        x25519_dalek::PublicKey as X25519PublicKey,
    };

    pub fn serialize<S>(maybe: &Option<X25519PublicKey>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        maybe.as_ref().map(|pk| pk.as_bytes()).serialize(ser)
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Option<X25519PublicKey>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let maybe_bytes: Option<[u8; 32]> = Option::deserialize(de)?;
        Ok(maybe_bytes.map(X25519PublicKey::from))
    }
}

// === Network message ===

/// First message sent by Sender (aka pre‑key message).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InitialMessage {
    /// Sender's Identity Key (DH form).
    #[serde(with = "x25519_serde")]
    pub ika_pub: X25519PublicKey,
    /// Sender's ephemeral X25519 public key.
    #[serde(with = "x25519_serde")]
    pub ek_pub: X25519PublicKey,
    /// Referenced SPK identifier.
    pub spk_id: u32,
    /// Referenced OTPK identifier (if any).
    pub otpk_id: Option<u32>,
    /// XChaCha20‑Poly1305 nonce.
    pub nonce: [u8; 24],
    /// Ciphertext of the application payload.
    pub ciphertext: Vec<u8>,
}

// === Sender‑side ===

/// Build an [`InitialMessage`] and derive the shared secret.
///
/// This corresponds to section 3.1–3.4 in the X3DH specification.
///
/// # Errors
/// * [`X3dhError::SigVerifyFailed`] – SPK signature invalid.
/// * [`X3dhError::IdentityKeyMismatch`] – `Ed(IK_B)` ≠ map(`IK_B`).
/// * [`X3dhError::HkdfInvalidLength`] – HKDF failure.
/// * [`X3dhError::Aead`] – Encryption failure.
///
/// # Example
/// See the Quick start section in the crate‑level docs.
#[allow(clippy::too_many_arguments)]
pub fn sender_init(
    sender: &IdentityKey,
    bundle: &PreKeyBundle,
    plaintext: &[u8],
) -> Result<(InitialMessage, SharedSecret), X3dhError> {
    // 1. Verify SPK signature and identity binding
    let spk_bytes = encode_pk(&bundle.spk_pub);
    let identity_verify = bundle.get_identity_verify();

    let expected_verify = XEdPublic::from(&bundle.identity_pk);
    if identity_verify
        .as_bytes()
        .ct_eq(expected_verify.as_bytes())
        .unwrap_u8()
        == 0
    {
        return Err(X3dhError::IdentityKeyMismatch);
    }

    identity_verify
        .verify(&spk_bytes, &bundle.spk_sig)
        .map_err(|_| X3dhError::SigVerifyFailed)?;

    // 2. Ephemeral key pair
    let ek_secret = StaticSecret::random_from_rng(OsRng);
    let ek_pub = X25519PublicKey::from(&ek_secret);

    // 3. DH computations
    let mut dh1 = sender.secret.diffie_hellman(&bundle.spk_pub).to_bytes();
    let mut dh2 = ek_secret.diffie_hellman(&bundle.identity_pk).to_bytes();
    let mut dh3 = ek_secret.diffie_hellman(&bundle.spk_pub).to_bytes();
    let mut dh4_opt = bundle
        .otpk_pub
        .as_ref()
        .map(|otpk| ek_secret.diffie_hellman(otpk).to_bytes());

    let mut dh_slices: Vec<&[u8]> = vec![dh1.as_slice(), dh2.as_slice(), dh3.as_slice()];
    if let Some(ref d4) = dh4_opt {
        dh_slices.push(d4.as_slice());
    }
    let sk = kdf(&dh_slices, HKDF_INFO)?;

    // Zeroise temporary DH values
    dh1.zeroize();
    dh2.zeroize();
    dh3.zeroize();
    if let Some(ref mut d4) = dh4_opt {
        d4.zeroize();
    }

    // 4. Associated data: IK_A || IK_B
    let mut ad = Vec::with_capacity(66);
    ad.extend_from_slice(&encode_pk(&sender.dh_public));
    ad.extend_from_slice(&encode_pk(&bundle.identity_pk));

    // 5. Encrypt application payload
    let cipher = XChaCha20Poly1305::new((&*sk).into());
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let mut xnonce = XNonce::default();
    xnonce.copy_from_slice(&nonce);
    let ciphertext = cipher
        .encrypt(
            &xnonce,
            Payload {
                msg: plaintext,
                aad: &ad,
            },
        )
        .map_err(|_| X3dhError::Aead)?;

    let message = InitialMessage {
        ika_pub: sender.dh_public,
        ek_pub,
        spk_id: bundle.spk_id,
        otpk_id: bundle.otpk_id,
        nonce,
        ciphertext,
    };

    Ok((message, sk))
}

// === Receiver‑side ===

/// Decrypt an [`InitialMessage`] received from Sender.
///
/// Returns the plaintext together with the derived [`SharedSecret`].
///
/// # Errors
/// * [`X3dhError::SpkIdMismatch`]
/// * [`X3dhError::MissingOneTimeSecret`]
/// * [`X3dhError::OtpkIdMismatch`]
/// * [`X3dhError::CiphertextTooLarge`]
/// * [`X3dhError::DecryptFailed`]
///
/// # Example
/// Refer to the crate‑level Quick start
pub fn receiver_receive(
    receiver_id: &IdentityKey,
    spk_secret: &StaticSecret,
    spk_id: u32,
    otpk_secret: Option<(&StaticSecret, u32)>,
    msg: &InitialMessage,
) -> Result<(Vec<u8>, SharedSecret), X3dhError> {
    // 0. Check SPK id
    if msg
        .spk_id
        .to_be_bytes()
        .ct_eq(&spk_id.to_be_bytes())
        .unwrap_u8()
        == 0
    {
        return Err(X3dhError::SpkIdMismatch);
    }

    // 1. OTPK bookkeeping
    match (msg.otpk_id, otpk_secret) {
        (None, None) => {}
        (Some(_), None) => return Err(X3dhError::MissingOneTimeSecret),
        (Some(msg_id), Some((_, stored_id)))
            if msg_id
                .to_be_bytes()
                .ct_eq(&stored_id.to_be_bytes())
                .unwrap_u8()
                == 0 =>
        {
            return Err(X3dhError::OtpkIdMismatch);
        }
        _ => {}
    }

    // 2. DH computations
    let mut dh1 = spk_secret.diffie_hellman(&msg.ika_pub).to_bytes();
    let mut dh2 = receiver_id.secret.diffie_hellman(&msg.ek_pub).to_bytes();
    let mut dh3 = spk_secret.diffie_hellman(&msg.ek_pub).to_bytes();
    let mut dh4_opt = otpk_secret.map(|(sk, _)| sk.diffie_hellman(&msg.ek_pub).to_bytes());

    let mut dh_slices: Vec<&[u8]> = vec![dh1.as_slice(), dh2.as_slice(), dh3.as_slice()];
    if let Some(ref d4) = dh4_opt {
        dh_slices.push(d4.as_slice());
    }
    let sk = kdf(&dh_slices, HKDF_INFO)?;

    dh1.zeroize();
    dh2.zeroize();
    dh3.zeroize();
    if let Some(ref mut d4) = dh4_opt {
        d4.zeroize();
    }

    // 3. Associated data (IK_A || IK_B)
    let mut ad = Vec::with_capacity(66);
    ad.extend_from_slice(&encode_pk(&msg.ika_pub));
    ad.extend_from_slice(&encode_pk(&receiver_id.dh_public));

    // 4. Size check
    if msg.ciphertext.len() > MAX_PRE_KEY_MSG {
        return Err(X3dhError::CiphertextTooLarge);
    }

    // 5. Decrypt
    let cipher = XChaCha20Poly1305::new((&*sk).into());
    let mut xnonce = XNonce::default();
    xnonce.copy_from_slice(&msg.nonce);
    let plaintext = cipher
        .decrypt(
            &xnonce,
            Payload {
                msg: &msg.ciphertext,
                aad: &ad,
            },
        )
        .map_err(|_| X3dhError::DecryptFailed)?;

    Ok((plaintext, sk))
}

// === Receiver helpers ===

/// Container that pairs a publicly published [`PreKeyBundle`] with the
/// secrets that Receiver keeps in local storage.
#[allow(unused_assignments)]
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct PreKeyBundleWithSecrets {
    /// The bundle that must be posted to the key server.
    #[zeroize(skip)]
    pub bundle: PreKeyBundle,
    /// Receiver's SPK secret (only one at a time).
    spk_secret: StaticSecret,
    /// Queue of unused OTPK secrets.
    otpk_secrets: Vec<(u32, StaticSecret)>,
}

/// Generate a fresh [`PreKeyBundle`] together with the matching secrets.
///
/// * `spk_id` – monotonically increasing identifier managed by the caller(used to identify the SPK)
/// * `next_otpk_id` – mutable counter for OTPK ids(used to identify the OTPK)
/// * `n_otpks` – how many OTPKs to attach (0‑`n`).
///
///   Returns a bundle with the SPK and the OTPKs.
pub fn receiver_generate_pre_key_bundle(
    identity: &IdentityKey,
    spk_id: u32,
    next_otpk_id: &mut u32,
    n_otpks: usize,
) -> PreKeyBundleWithSecrets {
    let spk_secret = StaticSecret::random_from_rng(OsRng);

    let mut otpk_secrets = Vec::with_capacity(n_otpks);
    for _ in 0..n_otpks {
        let id = *next_otpk_id;
        *next_otpk_id = next_otpk_id.wrapping_add(1);
        let sk = StaticSecret::random_from_rng(OsRng);
        otpk_secrets.push((id, sk));
    }

    let (first_otpk_id, first_otpk_secret) = otpk_secrets
        .first()
        .map(|(id, sk)| (Some(*id), Some(sk)))
        .unwrap_or((None, None));

    let bundle = PreKeyBundle::new(
        identity,
        spk_id,
        &spk_secret,
        first_otpk_id,
        first_otpk_secret,
    );

    PreKeyBundleWithSecrets {
        bundle,
        spk_secret,
        otpk_secrets,
    }
}

/// Convenience helper: create many bundles in one go (useful for batch upload).
pub fn receiver_generate_many_pre_key_bundles(
    identity: &IdentityKey,
    count: usize,
    next_spk_id: &mut u32,
    next_otpk_id: &mut u32,
    otpks_per_bundle: usize,
) -> Vec<PreKeyBundleWithSecrets> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let bundle = receiver_generate_pre_key_bundle(
            identity,
            *next_spk_id,
            next_otpk_id,
            otpks_per_bundle,
        );
        *next_spk_id = next_spk_id.wrapping_add(1);
        out.push(bundle);
    }
    out
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_no_otpk() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();

        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 1u32;

        let bundle = PreKeyBundle::new(&receiver, spk_id, &spk_secret, None, None);

        let plaintext = b"hi Receiver!";
        let (msg, _) = sender_init(&sender, &bundle, plaintext).unwrap();
        let (out, _) = receiver_receive(&receiver, &spk_secret, spk_id, None, &msg).unwrap();
        assert_eq!(plaintext, &out[..]);
    }

    #[test]
    fn fails_without_otpk_secret() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();

        // Receiver SPK
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 7;

        // Receiver OTPK
        let otpk_secret = StaticSecret::random_from_rng(OsRng);
        let otpk_id = 99;

        // Create bundle with OTPK
        let bundle = PreKeyBundle::new(
            &receiver,
            spk_id,
            &spk_secret,
            Some(otpk_id),
            Some(&otpk_secret),
        );

        let msg = sender_init(&sender, &bundle, b"test").unwrap();
        // Receiver forgot to pass OTPK secret – should error
        assert!(matches!(
            receiver_receive(&receiver, &spk_secret, spk_id, None, &msg.0),
            Err(X3dhError::MissingOneTimeSecret)
        ));
    }

    #[test]
    fn roundtrip_with_otpk() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();

        // Receiver creates pre-keys
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 42u32;
        let otpk_secret = StaticSecret::random_from_rng(OsRng);
        let otpk_id = 123u32;

        // Create bundle with OTPK
        let bundle = PreKeyBundle::new(
            &receiver,
            spk_id,
            &spk_secret,
            Some(otpk_id),
            Some(&otpk_secret),
        );

        let plaintext = b"Secret message with one-time key";
        let msg = sender_init(&sender, &bundle, plaintext).unwrap();

        // Receiver processes with the correct OTPK
        let out = receiver_receive(
            &receiver,
            &spk_secret,
            spk_id,
            Some((&otpk_secret, otpk_id)),
            &msg.0,
        )
        .unwrap();

        assert_eq!(plaintext, &out.0[..]);
    }

    #[test]
    fn bundle_can_be_stored() {
        // Demonstrate that bundles can be stored in a collection
        let receiver = IdentityKey::generate();

        // Create multiple bundles
        let mut bundles = Vec::new();

        for i in 0..5 {
            let spk_secret = StaticSecret::random_from_rng(OsRng);
            let bundle = PreKeyBundle::new(&receiver, i as u32, &spk_secret, None, None);

            bundles.push((bundle, spk_secret));
        }

        // Demonstrate we can retrieve and use a bundle
        let (bundle, spk_secret) = &bundles[2];
        let sender = IdentityKey::generate();
        let plaintext = b"Message for stored bundle";

        let msg = sender_init(&sender, bundle, plaintext).unwrap();
        let out = receiver_receive(&receiver, spk_secret, 2, None, &msg.0).unwrap();
        assert_eq!(plaintext, &out.0[..]);
    }

    #[test]
    fn test_signature_verification_failure() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();
        let eve = IdentityKey::generate();

        // Receiver creates SPK
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 1u32;

        // Create legitimate bundle
        let mut bundle = PreKeyBundle::new(&receiver, spk_id, &spk_secret, None, None);

        // Tamper with signature - replace with Eve's signature
        let eve_sig = eve.signing.sign(&encode_pk(&bundle.spk_pub), OsRng);
        bundle.spk_sig = eve_sig;

        // Sender should reject the tampered bundle
        let result = sender_init(&sender, &bundle, b"test message");
        assert!(matches!(result, Err(X3dhError::SigVerifyFailed)));
    }

    #[test]
    fn test_decryption_with_wrong_keys() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();
        let mallory = IdentityKey::generate(); // Attacker

        // Receiver's SPK
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 5u32;

        // Create bundle
        let bundle = PreKeyBundle::new(&receiver, spk_id, &spk_secret, None, None);

        let plaintext = b"secret message";
        let msg = sender_init(&sender, &bundle, plaintext).unwrap();

        // Mallory attempts to decrypt with wrong identity key
        let result = receiver_receive(&mallory, &spk_secret, spk_id, None, &msg.0);
        assert!(matches!(result, Err(X3dhError::DecryptFailed)));

        // Receiver attempts to decrypt with wrong SPK
        let wrong_spk = StaticSecret::random_from_rng(OsRng);
        let result = receiver_receive(&receiver, &wrong_spk, spk_id, None, &msg.0);
        assert!(matches!(result, Err(X3dhError::DecryptFailed)));
    }

    #[test]
    fn test_tampered_ciphertext() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();

        // Receiver SPK
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 3u32;

        // Create bundle
        let bundle = PreKeyBundle::new(&receiver, spk_id, &spk_secret, None, None);

        let plaintext = b"authentic message";
        let mut msg = sender_init(&sender, &bundle, plaintext).unwrap();

        // Try to tamper with the ciphertext
        if !msg.0.ciphertext.is_empty() {
            msg.0.ciphertext[0] ^= 0x01; // Flip a bit
        }

        // Receiver should detect tampering
        let result = receiver_receive(&receiver, &spk_secret, spk_id, None, &msg.0);
        assert!(matches!(result, Err(X3dhError::DecryptFailed)));
    }

    #[test]
    fn test_empty_message() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();

        // Receiver SPK
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 9u32;

        // Create bundle
        let bundle = PreKeyBundle::new(&receiver, spk_id, &spk_secret, None, None);

        // Empty message should work fine
        let plaintext = b"";
        let msg = sender_init(&sender, &bundle, plaintext).unwrap();
        let out = receiver_receive(&receiver, &spk_secret, spk_id, None, &msg.0).unwrap();
        assert_eq!(plaintext, &out.0[..]);
    }

    #[test]
    fn test_large_message() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();

        // Receiver SPK
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 10u32;

        // Create bundle
        let bundle = PreKeyBundle::new(&receiver, spk_id, &spk_secret, None, None);

        // Create a large message (8KB)
        let plaintext = vec![0xaa; 8 * 1024]; // I agree with everything said in the message
        let msg = sender_init(&sender, &bundle, &plaintext).unwrap();
        let out = receiver_receive(&receiver, &spk_secret, spk_id, None, &msg.0).unwrap();
        assert_eq!(plaintext, out.0);
    }

    #[test]
    fn test_binary_data() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();

        // Receiver SPK
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 11u32;

        // Create bundle
        let bundle = PreKeyBundle::new(&receiver, spk_id, &spk_secret, None, None);

        // Binary data with all possible byte values
        let mut plaintext = Vec::with_capacity(256);
        for i in 0..=255u8 {
            plaintext.push(i);
        }

        let msg = sender_init(&sender, &bundle, &plaintext).unwrap();
        let out = receiver_receive(&receiver, &spk_secret, spk_id, None, &msg.0).unwrap();
        assert_eq!(plaintext, out.0);
    }

    #[test]
    fn test_multiple_otpks() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();

        // Receiver creates pre-keys
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 12u32;

        // Create multiple OTPKs
        let otpk_secrets: Vec<(StaticSecret, u32)> = (0..5)
            .map(|i| (StaticSecret::random_from_rng(OsRng), 200 + i))
            .collect();

        // Test each OTPK separately
        for (idx, (otpk_secret, otpk_id)) in otpk_secrets.iter().enumerate() {
            // Create bundle with this OTPK
            let bundle = PreKeyBundle::new(
                &receiver,
                spk_id,
                &spk_secret,
                Some(*otpk_id),
                Some(otpk_secret),
            );

            let plaintext = format!("Message using OTPK #{}", idx).into_bytes();
            let msg = sender_init(&sender, &bundle, &plaintext).unwrap();

            // Receiver processes with the correct OTPK
            let out = receiver_receive(
                &receiver,
                &spk_secret,
                spk_id,
                Some((otpk_secret, *otpk_id)),
                &msg.0,
            )
            .unwrap();

            assert_eq!(plaintext, out.0);
        }
    }

    #[test]
    fn test_different_identity_keys_produce_different_outputs() {
        // Generate two different identity keys for Sender
        let sender1 = IdentityKey::generate();
        let sender2 = IdentityKey::generate();
        let receiver = IdentityKey::generate();

        // Receiver SPK
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 13u32;

        // Create bundle
        let bundle = PreKeyBundle::new(&receiver, spk_id, &spk_secret, None, None);

        let plaintext = b"Same message";

        // Encrypt with different identity keys
        let msg1 = sender_init(&sender1, &bundle, plaintext).unwrap();
        let msg2 = sender_init(&sender2, &bundle, plaintext).unwrap();

        // Ciphertexts should be different even with same plaintext
        assert_ne!(msg1.0.ciphertext, msg2.0.ciphertext);

        // Both should decrypt properly
        let out1 = receiver_receive(&receiver, &spk_secret, spk_id, None, &msg1.0).unwrap();
        let out2 = receiver_receive(&receiver, &spk_secret, spk_id, None, &msg2.0).unwrap();

        assert_eq!(plaintext, &out1.0[..]);
        assert_eq!(plaintext, &out2.0[..]);
    }

    #[test]
    fn test_different_nonce_produces_different_ciphertext() {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();

        // Receiver SPK
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 14u32;

        // Create bundle
        let bundle = PreKeyBundle::new(&receiver, spk_id, &spk_secret, None, None);

        let plaintext = b"test message";

        // Create two messages - they should have different nonces automatically
        let msg1 = sender_init(&sender, &bundle, plaintext).unwrap();
        let msg2 = sender_init(&sender, &bundle, plaintext).unwrap();

        // Nonces should be different
        assert_ne!(msg1.0.nonce, msg2.0.nonce);

        // Ciphertexts should be different even with same plaintext due to different nonces
        assert_ne!(msg1.0.ciphertext, msg2.0.ciphertext);
    }

    #[test]
    fn test_serialization_deserialization() {
        // Test IdentityKey serialization/deserialization
        let identity = IdentityKey::generate();
        let original_dh_public_bytes = *identity.dh_public.as_bytes();
        let original_verify_bytes = *identity.verify.as_bytes();

        // Serialize IdentityKey to binary format using bincode
        let identity_bytes =
            bincode::serialize(&identity).expect("Failed to serialize IdentityKey");

        // Deserialize IdentityKey from binary format
        let deserialized_identity: IdentityKey =
            bincode::deserialize(&identity_bytes).expect("Failed to deserialize IdentityKey");

        // Verify that the deserialized identity key matches the original
        assert_eq!(
            original_dh_public_bytes,
            *deserialized_identity.dh_public.as_bytes()
        );
        assert_eq!(
            original_verify_bytes,
            *deserialized_identity.verify.as_bytes()
        );

        // Test PreKeyBundle serialization/deserialization
        let receiver = IdentityKey::generate();
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_id = 42u32;
        let otpk_secret = StaticSecret::random_from_rng(OsRng);
        let otpk_id = 123u32;

        // Create bundle with OTPK
        let original_bundle = PreKeyBundle::new(
            &receiver,
            spk_id,
            &spk_secret,
            Some(otpk_id),
            Some(&otpk_secret),
        );

        // Serialize/deserialize PreKeyBundle to binary format using bincode
        let bundle_bytes =
            bincode::serialize(&original_bundle).expect("Failed to serialize PreKeyBundle");
        let deserialized_bundle: PreKeyBundle =
            bincode::deserialize(&bundle_bytes).expect("Failed to deserialize PreKeyBundle");

        // Attempt JSON serialization/deserialization as well
        let json_string = serde_json::to_string(&original_bundle)
            .expect("Failed to serialize PreKeyBundle to JSON");
        let deserialized_json_bundle: PreKeyBundle = serde_json::from_str(&json_string)
            .expect("Failed to deserialize PreKeyBundle from JSON");

        // Verify that all fields match
        assert_eq!(original_bundle.spk_id, deserialized_bundle.spk_id);
        assert_eq!(original_bundle.spk_id, deserialized_json_bundle.spk_id);
        assert_eq!(
            original_bundle.spk_pub.as_bytes(),
            deserialized_bundle.spk_pub.as_bytes()
        );
        assert_eq!(
            original_bundle.spk_pub.as_bytes(),
            deserialized_json_bundle.spk_pub.as_bytes()
        );
        assert_eq!(original_bundle.spk_sig, deserialized_bundle.spk_sig);
        assert_eq!(original_bundle.spk_sig, deserialized_json_bundle.spk_sig);
        assert_eq!(
            original_bundle.identity_verify_bytes,
            deserialized_bundle.identity_verify_bytes
        );
        assert_eq!(
            original_bundle.identity_verify_bytes,
            deserialized_json_bundle.identity_verify_bytes
        );
        assert_eq!(
            original_bundle.identity_pk.as_bytes(),
            deserialized_bundle.identity_pk.as_bytes()
        );
        assert_eq!(
            original_bundle.identity_pk.as_bytes(),
            deserialized_json_bundle.identity_pk.as_bytes()
        );
        assert_eq!(original_bundle.otpk_id, deserialized_bundle.otpk_id);
        assert_eq!(original_bundle.otpk_id, deserialized_json_bundle.otpk_id);

        // Check OTPK public key
        match (original_bundle.otpk_pub, deserialized_bundle.otpk_pub) {
            (Some(orig), Some(deser)) => assert_eq!(orig.as_bytes(), deser.as_bytes()),
            (None, None) => {}
            _ => panic!("OTPK public key mismatch between original and deserialized"),
        }

        // Verify that the deserialized bundle still passes signature verification
        assert!(deserialized_bundle.verify_spk());

        // Test that we can use the deserialized bundle in a full X3DH exchange
        let sender = IdentityKey::generate();
        let plaintext = b"test with deserialized bundle";

        let msg = sender_init(&sender, &deserialized_bundle, plaintext).unwrap();
        let out = receiver_receive(
            &receiver,
            &spk_secret,
            spk_id,
            Some((&otpk_secret, otpk_id)),
            &msg.0,
        )
        .unwrap();

        assert_eq!(plaintext, &out.0[..]);
    }
}
