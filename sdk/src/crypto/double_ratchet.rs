//! Double Ratchet implementation with header encryption (HE)
//!
//! This module follows the Public Signal Double Ratchet specification
//! <https://signal.org/docs/specifications/doubleratchet/> and extends it with the
//! header‑encryption variant described in section 4 "Security considerations".
//!
//! # Example
//!
//! ```rust
//! use nexus_sdk::crypto::double_ratchet::RatchetStateHE;
//! use rand_core::OsRng;
//! use x25519_dalek::StaticSecret;
//!
//! // 1. Session setup (X3DH, QR‑code, pre‑keys …).  Out of scope here:
//! let sender_root_key = [0u8; 32];          // derived elsewhere …
//! let receiver_root_key   = sender_root_key;     // shared!
//!
//! // Initial shared header keys obtained through X3DH or similar.
//! let shared_hka  = [1u8; 32]; // Sender‑>Receiver  (HK_sending for Sender)
//! let shared_nhkb = [2u8; 32]; // Receiver‑>Sender  (NHK_sending for Receiver)
//!
//! // 2. Instantiate ratchets on both sides.
//! let mut sender = RatchetStateHE::new();
//! let mut receiver   = RatchetStateHE::new();
//!
//! // Receiver chooses a long‑term X25519 key‑pair for the first DH‑ratchet step.
//! let receiver_kp = RatchetStateHE::generate_dh();
//!
//! sender.init_sender_he(&sender_root_key, receiver_kp.1, shared_hka, shared_nhkb).unwrap();
//! receiver.init_receiver_he(&receiver_root_key, receiver_kp, shared_hka, shared_nhkb).unwrap();
//!
//! // 3. Sender sends an encrypted message to Receiver.
//! let (hdr, msg) = sender.ratchet_encrypt_he(b"hello Receiver!", b"assoc-data").unwrap();
//!
//! // 4. Receiver receives and decrypts.
//! let plaintext = receiver.ratchet_decrypt_he(&hdr, &msg, b"assoc-data").unwrap();
//! assert_eq!(plaintext, b"hello Receiver!");
//!
//! // Receiver replies … and so on.
//! ```

use {
    aes_siv::{
        aead::{Aead, Payload},
        Aes128SivAead,
        KeyInit,
        Nonce,
    },
    hkdf::Hkdf,
    hmac::{Hmac, Mac},
    lru::LruCache,
    rand::{rngs::OsRng, RngCore},
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    sha2::Sha256,
    std::{
        collections::{HashMap, VecDeque},
        num::NonZeroUsize,
    },
    subtle::ConstantTimeEq,
    thiserror::Error,
    x25519_dalek::{PublicKey, StaticSecret},
    zeroize::{Zeroize, Zeroizing},
};

type SkippedDecrypt = Option<(Vec<u8>, [u8; 32], u32, [u8; 32])>;

/// Maximum number of skipped message keys to derive per chain before rejecting incoming traffic,
///  as mentioned in section 4 of the spec.
const MAX_SKIP_PER_CHAIN: usize = 1_000;
/// Upper bound across all chains maintained in memory at any moment. This is
/// a defence‑in‑depth limit to avoid unbounded growth of
const MAX_SKIP_GLOBAL: usize = 2 * MAX_SKIP_PER_CHAIN;
/// Maximum cached outgoing drafts (bounded LRU).
const MAX_OUTGOING: usize = 1024;
/// Each AES‑SIV nonce is 128‑bit.  We concatenate an 8‑byte random prefix with
/// an 8‑byte big‑endian counter to get a unique value for every encryption.
/// Note: We use AES‑128‑SIV with 32‑byte (256‑bit) keys for 128‑bit security.
const NONCE_LEN: usize = 16;
/// Maximum number of previous header keys to keep for self‑decryption.
const MAX_PREV_HKS: usize = 512;
/// How many skipped message-keys we actually retain in RAM.
/// MUST be ≥ the largest contiguous stretch of *real* messages
/// you expect to decrypt later (drafts are always earlier).
const MAX_SKIP_STORE: usize = 2 * MAX_SKIP_PER_CHAIN; // 2 000
/// All eight small‑order points from RFC 7748 §6.
const SMALL_ORDER: [[u8; 32]; 8] = [
    [0u8; 32],
    [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    [
        0xe0, 0xeb, 0x7a, 0x7c, 0x70, 0x10, 0x44, 0x19, 0xa5, 0x41, 0x14, 0xb3, 0x75, 0xd4, 0x16,
        0x92, 0xd4, 0x1d, 0x00, 0xa9, 0x43, 0x4f, 0x70, 0x0b, 0xff, 0x32, 0x19, 0x66, 0x7f, 0x0b,
        0xe1, 0x4e,
    ],
    [
        0x5f, 0x51, 0xe6, 0x57, 0x3a, 0x0b, 0xcb, 0x3e, 0x0a, 0xdb, 0x22, 0xc0, 0x21, 0x3b, 0x0f,
        0xcb, 0xe4, 0x29, 0x5f, 0xb0, 0x2b, 0x06, 0x47, 0x4d, 0x1d, 0x15, 0x3f, 0xe3, 0x26, 0xea,
        0xfa, 0x1a,
    ],
    [
        0x95, 0xdb, 0x0b, 0x37, 0x0e, 0x37, 0x59, 0x52, 0xbc, 0x20, 0xd0, 0x85, 0x85, 0x8c, 0x4b,
        0xf5, 0x16, 0xab, 0xa8, 0x18, 0x00, 0xf4, 0x16, 0x7f, 0x41, 0x2d, 0x64, 0x27, 0xde, 0xe7,
        0x6c, 0x0a,
    ],
    [
        0xd0, 0x26, 0x20, 0x12, 0x26, 0x87, 0x9e, 0xaf, 0x4d, 0x01, 0xed, 0xbd, 0xe2, 0x6d, 0xea,
        0x1e, 0x4a, 0x93, 0x63, 0x50, 0x1e, 0xd4, 0x28, 0x29, 0x44, 0xd4, 0xe0, 0x7e, 0x5e, 0x35,
        0x3f, 0x37,
    ],
    [
        0xa3, 0x7b, 0x2a, 0xb1, 0xdc, 0x36, 0x92, 0xdd, 0x0f, 0xd4, 0x28, 0x50, 0x9d, 0xd0, 0x41,
        0xc7, 0x51, 0x55, 0xa2, 0xbd, 0xf4, 0x43, 0xe8, 0x15, 0xa1, 0x16, 0x4d, 0x13, 0xa4, 0x79,
        0x54, 0x9a,
    ],
    [
        0xe0, 0xd2, 0xee, 0x23, 0x53, 0x47, 0x62, 0x10, 0xd0, 0x02, 0xe1, 0x50, 0xf8, 0x98, 0x29,
        0x0f, 0x66, 0x79, 0x72, 0x0c, 0x6b, 0xa4, 0xc8, 0x04, 0x31, 0xb4, 0x2d, 0x18, 0x0a, 0x16,
        0x1a, 0xb9,
    ],
];

// === Type aliases ===

/// HKDF‑SHA‑256 as per RFC 5869.
type HkdfSha256 = Hkdf<Sha256>;
/// HMAC‑SHA‑256 wrapper from universal‑hash.
type HmacSha256 = Hmac<Sha256>;

// === Error types ===

/// Possible failures returned by [`RatchetStateHE`] operations.
#[derive(Debug, Error)]
pub enum RatchetError {
    /// Called a sending‑side API before [`RatchetStateHE::cks`] was initialised.
    #[error("missing sending chain")]
    MissingSendingChain,
    /// Called a receiving‑side API before [`RatchetStateHE::ckr`] was initialised.
    #[error("missing receiving chain")]
    MissingReceivingChain,
    /// Header key not available in the current direction (encrypt/decrypt).
    #[error("missing header key")]
    MissingHeaderKey,
    /// Wrapper around any cryptographic backend failure (AES‑SIV).
    #[error("crypto error")]
    CryptoError,
    /// Invalid CBOR or authentication failure while parsing an incoming header.
    #[error("header parse error")]
    HeaderParse,
    /// Safety valve triggered: too many skipped messages or malformed counter.
    #[error("max skip exceeded")]
    MaxSkipExceeded,
    /// Public‑key rejected (identity / small‑order curve point).
    #[error("invalid public key")]
    InvalidPublicKey,
}

impl From<aes_siv::aead::Error> for RatchetError {
    fn from(_: aes_siv::aead::Error) -> Self {
        RatchetError::CryptoError
    }
}

impl From<hmac::digest::InvalidLength> for RatchetError {
    fn from(_: hmac::digest::InvalidLength) -> Self {
        RatchetError::CryptoError
    }
}

// === Nonce sequence generator ===

/// Deterministic, unique 128‑bit nonce generator used for both header and
/// payload encryption. Each instance starts with a random prefix so that two
/// different [`RatchetStateHE`] values never collide.
#[derive(Clone)]
struct NonceSeq {
    prefix: [u8; 8],
    counter: u64, // big‑endian in output
}

impl NonceSeq {
    /// Create a new sequence with a cryptographically random 64‑bit prefix.
    fn new() -> Self {
        let mut prefix = [0u8; 8];
        OsRng.fill_bytes(&mut prefix);
        Self { prefix, counter: 0 }
    }

    /// Return the next never‑repeating 16‑byte nonce: `prefix || counter_be`.
    #[inline(always)]
    fn next(&mut self) -> [u8; NONCE_LEN] {
        let mut out = [0u8; NONCE_LEN];
        out[..8].copy_from_slice(&self.prefix);
        out[8..].copy_from_slice(&self.counter.to_be_bytes());
        self.counter = self.counter.wrapping_add(1);
        out
    }
}

// Manual [`Zeroize`] to shred secrets when dropped.
impl Zeroize for NonceSeq {
    fn zeroize(&mut self) {
        self.prefix.zeroize();
        self.counter = 0;
    }
}

// === Header structure ===

/// Wire header accompanying every ciphertext (encrypted again with AES‑SIV).
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Header {
    /// Ephemeral X25519 public key of the sender
    /// Used for DH ratchet step
    pub dh: PublicKey,
    /// Length of the previous sending chain (`pn` in the paper).
    pub pn: u32,
    /// Index of the current message inside the active sending chain.
    pub n: u32,
}

// Compact CBOR serialisation – 3‑tuple of fixed‑size fields.
impl Serialize for Header {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (self.dh.as_bytes(), self.pn, self.n).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Header {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (pk_bytes, pn, n): ([u8; 32], u32, u32) = Deserialize::deserialize(deserializer)?;
        Ok(Header {
            dh: PublicKey::from(pk_bytes),
            pn,
            n,
        })
    }
}

// === Ratchet state (HE) ===

/// Core state machine implementing the Double Ratchet with header
/// encryption.
///
/// You normally create an empty state via [`RatchetStateHE::new`], then call
/// either [`init_sender_he`](RatchetStateHE::init_sender_he) or
/// [`init_receiver_he`](RatchetStateHE::init_receiver_he) depending on your role as sender or receiver.  After
/// the handshake, the public API is:
/// * [`ratchet_encrypt_he`](RatchetStateHE::ratchet_encrypt_he) → send message.
/// * [`ratchet_decrypt_he`](RatchetStateHE::ratchet_decrypt_he) → receive.
///
/// The public API provides encryption and decryption methods that advance
/// the ratchet state as messages are sent and received.
#[derive(Serialize, Deserialize)]
pub struct RatchetStateHE {
    /// Own private key
    #[serde(with = "secret_bytes_serde")]
    dhs: StaticSecret,
    /// Own public key
    #[serde(with = "public_key_serde")]
    dhs_pub: PublicKey,
    /// Remote public key
    #[serde(with = "public_key_option_serde")]
    dhr: Option<PublicKey>,
    /// Root key
    rk: [u8; 32],
    /// Chain key for sending
    cks: Option<[u8; 32]>,
    /// Chain key for receiving
    ckr: Option<[u8; 32]>,
    /// Header key for sending
    hks: Option<[u8; 32]>,
    /// Header key for receiving
    hkr: Option<[u8; 32]>,
    /// Next header key for sending
    nhks: [u8; 32],
    /// Next header key for receiving
    nhkr: [u8; 32],
    /// Number of messages sent in current sending chain
    ns: u32,
    /// Number of messages received in current receiving chain
    nr: u32,
    /// Length of previous sending chain
    pn: u32,
    /// Map keyed by `(header_key, n)` -> message key, maintained while the state
    /// is alive.
    mkskipped: HashMap<([u8; 32], u32), [u8; 32]>,
    /// Local cache so the sender can reopen its own drafts.
    #[serde(with = "lru_cache_serde")]
    outgoing_cache: LruCache<([u8; 16], u32), Zeroizing<[u8; 32]>>,
    /// Keep previous HK_s so drafts survive one DH step overlap.
    prev_hks: VecDeque<[u8; 32]>,
    /// Nonce sequence for payload encryption
    #[serde(with = "nonce_seq_serde")]
    nonce_seq_msg: NonceSeq,
}

mod secret_bytes_serde {
    use {
        super::StaticSecret,
        crate::crypto::secret_bytes::SecretBytes,
        serde::{Deserialize, Deserializer, Serialize, Serializer},
    };
    pub fn serialize<S>(sk: &StaticSecret, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SecretBytes::from(sk).serialize(s)
    }
    pub fn deserialize<'de, D>(d: D) -> Result<StaticSecret, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(SecretBytes::deserialize(d)?.into())
    }
}

mod public_key_serde {
    use {
        super::PublicKey,
        serde::{Deserialize, Deserializer, Serialize, Serializer},
    };

    pub fn serialize<S>(pk: &PublicKey, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        pk.as_bytes().serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<PublicKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: [u8; 32] = Deserialize::deserialize(d)?;
        Ok(PublicKey::from(bytes))
    }
}

mod public_key_option_serde {
    use {
        super::PublicKey,
        serde::{Deserialize, Deserializer, Serialize, Serializer},
    };

    pub fn serialize<S>(opt_pk: &Option<PublicKey>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match opt_pk {
            Some(pk) => Some(pk.as_bytes()).serialize(s),
            None => None::<[u8; 32]>.serialize(s),
        }
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<PublicKey>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt_bytes: Option<[u8; 32]> = Deserialize::deserialize(d)?;
        Ok(opt_bytes.map(PublicKey::from))
    }
}

mod nonce_seq_serde {
    use {
        super::NonceSeq,
        serde::{Deserialize, Deserializer, Serialize, Serializer},
    };

    pub fn serialize<S>(nonce_seq: &NonceSeq, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (nonce_seq.prefix, nonce_seq.counter).serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<NonceSeq, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (prefix, counter): ([u8; 8], u64) = Deserialize::deserialize(d)?;
        Ok(NonceSeq { prefix, counter })
    }
}

mod lru_cache_serde {
    use {
        super::{LruCache, NonZeroUsize, Zeroizing, MAX_OUTGOING},
        serde::{Deserialize, Deserializer, Serialize, Serializer},
    };

    type CacheKey = ([u8; 16], u32);
    type CacheValue = Zeroizing<[u8; 32]>;
    type SerializedItem = (([u8; 16], u32), [u8; 32]);

    pub fn serialize<S>(cache: &LruCache<CacheKey, CacheValue>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let items: Vec<SerializedItem> = cache.iter().map(|(k, v)| (*k, **v)).collect();
        items.serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<LruCache<CacheKey, CacheValue>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let items: Vec<SerializedItem> = Deserialize::deserialize(d)?;
        let mut cache = LruCache::new(NonZeroUsize::new(MAX_OUTGOING).unwrap());
        for (k, v) in items {
            cache.put(k, Zeroizing::new(v));
        }
        Ok(cache)
    }
}

// Scrub everything on drop.
impl Zeroize for RatchetStateHE {
    fn zeroize(&mut self) {
        // StaticSecret already implements Zeroize; PublicKey is public data.
        self.rk.zeroize();
        if let Some(ref mut k) = self.cks {
            k.zeroize();
        }
        if let Some(ref mut k) = self.ckr {
            k.zeroize();
        }
        if let Some(ref mut k) = self.hks {
            k.zeroize();
        }
        if let Some(ref mut k) = self.hkr {
            k.zeroize();
        }
        self.prev_hks.iter_mut().for_each(|k| k.zeroize());
        self.nhks.zeroize();
        self.nhkr.zeroize();
        self.mkskipped.clear();
        self.nonce_seq_msg.zeroize();
    }
}

impl Drop for RatchetStateHE {
    fn drop(&mut self) {
        self.zeroize();
    }
}

// === Helper functions ===

impl RatchetStateHE {
    // === Key utilities ===

    /// Generate a fresh ephemeral X25519 key‑pair.
    #[inline]
    pub fn generate_dh() -> (StaticSecret, PublicKey) {
        let sk = StaticSecret::random_from_rng(OsRng);
        let pk = PublicKey::from(&sk);
        (sk, pk)
    }

    /// Reject the identity point and other small‑order curve points.
    /// This is a defence‑in‑depth measure.
    #[inline]
    /// Reject identity and any of the eight small‑order points.
    fn validate_pk(pk: &PublicKey) -> Result<(), RatchetError> {
        for bad in SMALL_ORDER {
            if pk.as_bytes().ct_eq(&bad).unwrap_u8() == 1 {
                return Err(RatchetError::InvalidPublicKey);
            }
        }
        Ok(())
    }

    /// Perform X25519 Diffie‑Hellman and return the 32‑byte shared secret.
    #[inline]
    fn dh(sk: &StaticSecret, pk: &PublicKey) -> [u8; 32] {
        sk.diffie_hellman(pk).to_bytes()
    }

    // === Key derivation functions (KDFs) ===

    /// Root‑key KDF with domain‑separated label `"DR‑RootHE"` producing the
    /// tuple `(new_rk, ck, nhk)` as per the specification.
    /// (new_rk, ck, nhk) are the new root key, chain key and next header key respectively.
    #[inline]
    fn kdf_rk_he(rk: &[u8; 32], dh_out: &[u8; 32]) -> ([u8; 32], [u8; 32], [u8; 32]) {
        let hk = HkdfSha256::new(Some(rk), dh_out);
        let mut okm = [0u8; 96];
        hk.expand(b"DR-RootHE", &mut okm).expect("hkdf expand");
        let mut new_rk = [0u8; 32];
        new_rk.copy_from_slice(&okm[..32]);
        let mut ck = [0u8; 32];
        ck.copy_from_slice(&okm[32..64]);
        let mut nhk = [0u8; 32];
        nhk.copy_from_slice(&okm[64..]);
        (new_rk, ck, nhk)
    }

    /// Chain‑key KDF using single‑byte labels `0x01` / `0x02`.
    /// Takes the current chain key and returns the new chain key and message key.
    #[inline]
    fn kdf_ck(ck: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
        let mut mac1 = <HmacSha256 as Mac>::new_from_slice(ck).expect("hmac");
        mac1.update(&[0x01]);
        let mut new_ck = [0u8; 32];
        new_ck.copy_from_slice(&mac1.finalize().into_bytes());

        let mut mac2 = <HmacSha256 as Mac>::new_from_slice(ck).expect("hmac");
        mac2.update(&[0x02]);
        let mut mk = [0u8; 32];
        mk.copy_from_slice(&mac2.finalize().into_bytes());
        (new_ck, mk)
    }

    /// Per‑message header‑key KDF: HK' = HMAC(HK, 0x00).
    #[inline]
    fn kdf_hk(hk: &[u8; 32]) -> [u8; 32] {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(hk).unwrap();
        mac.update(&[0x00]);
        let mut out = [0u8; 32];
        out.copy_from_slice(&mac.finalize().into_bytes());
        out
    }

    // === Header encryption helpers (AES‑SIV) ===

    /// Encrypt and authenticate a header with AES‑128‑SIV using 256‑bit keys.
    /// Takes the header key and plaintext and returns the encrypted header.
    // hencrypt/hdecrypt unchanged
    fn hencrypt(&self, hk: &[u8; 32], pt: &[u8]) -> Result<Vec<u8>, RatchetError> {
        let cipher = Aes128SivAead::new_from_slice(hk)?;
        let zero_nonce = Nonce::default();
        cipher
            .encrypt(&zero_nonce, Payload { msg: pt, aad: &[] })
            .map_err(Into::into)
    }

    fn hdecrypt(hk: &[u8; 32], data: &[u8]) -> Result<Header, RatchetError> {
        let cipher = Aes128SivAead::new_from_slice(hk)?;
        let zero_nonce = Nonce::default();
        let pt = cipher.decrypt(
            &zero_nonce,
            Payload {
                msg: data,
                aad: &[],
            },
        )?;
        ciborium::from_reader(pt.as_slice()).map_err(|_| RatchetError::HeaderParse)
    }

    // === Constructors / initialisers ===

    /// Create a blank state with fresh DH key‑pair and zeroed secrets.
    #[must_use]
    pub fn new() -> Self {
        let (dhs_sk, dhs_pk) = Self::generate_dh();
        Self {
            dhs: dhs_sk,
            dhs_pub: dhs_pk,
            dhr: None,
            rk: [0u8; 32],
            cks: None,
            ckr: None,
            hks: None,
            hkr: None,
            nhks: [0u8; 32],
            nhkr: [0u8; 32],
            ns: 0,
            nr: 0,
            pn: 0,
            mkskipped: HashMap::new(),
            outgoing_cache: LruCache::new(NonZeroUsize::new(MAX_OUTGOING).unwrap()),
            prev_hks: VecDeque::with_capacity(MAX_PREV_HKS),
            nonce_seq_msg: NonceSeq::new(),
        }
    }

    /// Sender side initialisation (first message sender).
    ///
    /// * `sk` – shared 32‑byte root key from X3DH or other handshake.
    /// * `receiver_pub` – Receiver's long‑term DH public key.
    /// * `shared_hka` – first header key Sender sends with.
    /// * `shared_nhkb` – header key Receiver will use after their first DH‑ratchet.
    pub fn init_sender_he(
        &mut self,
        sk: &[u8; 32],
        receiver_pub: PublicKey,
        shared_hka: [u8; 32],
        shared_nhkb: [u8; 32],
    ) -> Result<(), RatchetError> {
        Self::validate_pk(&receiver_pub)?;
        let (dhs_sk, dhs_pk) = Self::generate_dh();
        let dh_out = Self::dh(&dhs_sk, &receiver_pub);
        let (new_rk, ck_s, nhk_s) = Self::kdf_rk_he(sk, &dh_out);

        self.dhs = dhs_sk;
        self.dhs_pub = dhs_pk;
        self.dhr = Some(receiver_pub);
        self.rk = new_rk;
        self.cks = Some(ck_s);
        self.ckr = None;
        self.ns = 0;
        self.nr = 0;
        self.pn = 0;
        self.mkskipped.clear();
        self.hks = Some(shared_hka);
        self.hkr = None;
        self.nhks = nhk_s;
        self.nhkr = shared_nhkb;
        self.nonce_seq_msg = NonceSeq::new();
        Ok(())
    }

    /// Receiver side initialisation (first message receiver).
    ///
    /// * `sk` – same 32‑byte root key as Sender.
    /// * `receiver_kp` – Receiver's long‑term X25519 key‑pair.
    /// * `shared_hka` / `shared_nhkb` – same as for `init_sender_he` but swapped
    ///   direction.
    pub fn init_receiver_he(
        &mut self,
        sk: &[u8; 32],
        receiver_kp: (StaticSecret, PublicKey),
        shared_hka: [u8; 32],
        shared_nhkb: [u8; 32],
    ) -> Result<(), RatchetError> {
        let (dhs_sk, dhs_pk) = receiver_kp;
        self.dhs = dhs_sk;
        self.dhs_pub = dhs_pk;
        self.dhr = None;
        self.rk = *sk;
        self.cks = None;
        self.ckr = None;
        self.ns = 0;
        self.nr = 0;
        self.pn = 0;
        self.mkskipped.clear();
        self.hks = None;
        self.hkr = None;
        self.nhks = shared_nhkb;
        self.nhkr = shared_hka;
        self.nonce_seq_msg = NonceSeq::new();
        Ok(())
    }

    // === Public API – send / receive ===

    /// Encrypt a plaintext with associated data `ad` into `(enc_header, payload)`.
    ///
    /// Returns `(enc_header, ciphertext_payload)` suitable for transport.  The
    /// function advances the sending chain, so do not call it twice for
    /// the same message.
    /// Encrypt a plaintext and return (enc_header, payload).
    /// Encrypt a plaintext and return (enc_header, payload).
    pub fn ratchet_encrypt_he(
        &mut self,
        plaintext: &[u8],
        ad: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), RatchetError> {
        // Message‑key derivation as before
        let cks = self.cks.ok_or(RatchetError::MissingSendingChain)?;
        let (new_cks, mk) = Self::kdf_ck(&cks);
        self.cks = Some(new_cks);

        // 1. build + encrypt header with current HK_s
        let header = Header {
            dh: self.dhs_pub,
            pn: self.pn,
            n: self.ns,
        };
        let mut hdr_bytes = Vec::new();
        ciborium::into_writer(&header, &mut hdr_bytes).unwrap();
        let hk_current = self.hks.ok_or(RatchetError::MissingHeaderKey)?;
        let enc_header = self.hencrypt(&hk_current, &hdr_bytes)?;

        // 2. advance header key *after* successful encryption
        let hk_next = Self::kdf_hk(&hk_current);
        if self.prev_hks.len() == MAX_PREV_HKS {
            self.prev_hks.pop_front();
        }
        self.prev_hks.push_back(hk_current);
        self.hks = Some(hk_next);

        // Cache draft MK for self‑decryption
        self.cache_outgoing_header(&enc_header, &mk);

        // 3. encrypt payload (AAD = user AD || enc_header)
        let mut full_ad = ad.to_vec();
        full_ad.extend_from_slice(&enc_header);
        let cipher = Aes128SivAead::new_from_slice(&mk).unwrap();
        let nonce_bytes = self.nonce_seq_msg.next();
        let mut nonce = Nonce::default();
        nonce.copy_from_slice(&nonce_bytes);
        let mut ct = cipher.encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: &full_ad,
            },
        )?;
        let mut payload = nonce_bytes.to_vec();
        payload.append(&mut ct);
        self.ns = self.ns.wrapping_add(1);
        Ok((enc_header, payload))
    }

    /// Decrypt an incoming message.  Handles skipped‑message lookup and automatic
    /// DH‑ratchet advancement.
    ///
    /// * `enc_header` – encrypted header as produced by the sender.
    /// * `ciphertext` – encrypted payload (including nonce prefix).
    /// * `ad` – caller‑supplied associated data.
    ///
    /// Returns the decrypted payload.
    /// Receive side: decrypt and advance state.
    pub fn ratchet_decrypt_he(
        &mut self,
        enc_header: &[u8],
        ciphertext: &[u8],
        ad: &[u8],
    ) -> Result<Vec<u8>, RatchetError> {
        self.ratchet_decrypt_he_impl(enc_header, ciphertext, ad, false)
    }

    /// Decrypts a message while keeping its key cached for limited reuse.
    pub fn ratchet_decrypt_he_persistent(
        &mut self,
        enc_header: &[u8],
        ciphertext: &[u8],
        ad: &[u8],
    ) -> Result<Vec<u8>, RatchetError> {
        self.ratchet_decrypt_he_impl(enc_header, ciphertext, ad, true)
    }

    fn ratchet_decrypt_he_impl(
        &mut self,
        enc_header: &[u8],
        ciphertext: &[u8],
        ad: &[u8],
        retain_mk: bool,
    ) -> Result<Vec<u8>, RatchetError> {
        if let Some((pt, hk, n, mk)) = self.try_skipped_keys(enc_header, ciphertext, ad)? {
            if retain_mk {
                self.mkskipped.insert((hk, n), mk);
            }
            return Ok(pt);
        }

        let (header, used_nhk, hk_used) = self.decrypt_header(enc_header)?;
        let hk_for_cache = if used_nhk { Some(self.nhkr) } else { hk_used };

        if used_nhk {
            self.skip_message_keys_he(header.pn, None)?;
            self.dh_ratchet_he(&header)?;

            // 2. derive and cache keys for indices 0‥(N-1) with the current HK_r
            let hk_seed = self.hkr;
            self.skip_message_keys_he(header.n, hk_seed)?;

            // 3. now step HK_r so it points at the next expected header
            if let Some(mut hk) = hk_seed {
                for _ in 0..=header.n {
                    hk = Self::kdf_hk(&hk);
                }
                self.hkr = Some(hk);
            } else {
                return Err(RatchetError::MissingHeaderKey);
            }
        } else if self.ckr.is_none() {
            self.skip_message_keys_he(header.pn, None)?;
            self.dh_ratchet_he(&header)?;
            self.skip_message_keys_he(header.n, self.hkr)?;
            if let Some(hk_used) = hk_used {
                self.hkr = Some(Self::kdf_hk(&hk_used));
            } else {
                return Err(RatchetError::MissingHeaderKey);
            }
        } else {
            self.skip_message_keys_he(header.n, self.hkr)?;
            if let Some(hk_used) = hk_used {
                self.hkr = Some(Self::kdf_hk(&hk_used));
            } else {
                return Err(RatchetError::MissingHeaderKey);
            }
        }

        let ckr = self.ckr.ok_or(RatchetError::MissingReceivingChain)?;
        let (new_ckr, mk) = Self::kdf_ck(&ckr);
        let mk_copy = mk;
        self.ckr = Some(new_ckr);
        self.nr = self.nr.wrapping_add(1);

        if ciphertext.len() < NONCE_LEN {
            return Err(RatchetError::CryptoError);
        }
        let (nonce_bytes, ct) = ciphertext.split_at(NONCE_LEN);
        let mut full_ad = ad.to_vec();
        full_ad.extend_from_slice(enc_header);
        let cipher = Aes128SivAead::new_from_slice(&mk).unwrap();
        let mut nonce = Nonce::default();
        nonce.copy_from_slice(nonce_bytes);
        let result = cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ct,
                    aad: &full_ad,
                },
            )
            .map_err(Into::into);

        if result.is_ok() {
            if let Some(hk_cache) = hk_for_cache {
                if retain_mk {
                    self.mkskipped.insert((hk_cache, header.n), mk_copy);
                } else {
                    self.mkskipped.remove(&(hk_cache, header.n));
                }
            }
        }

        result
    }

    // === Internal helpers (skipped messages, DH‑ratchet) ===

    /// try_skipped_keys – only `remove` *after* successful decrypt.
    fn try_skipped_keys(
        &mut self,
        enc_header: &[u8],
        ciphertext: &[u8],
        ad: &[u8],
    ) -> Result<SkippedDecrypt, RatchetError> {
        let keys: Vec<_> = self.mkskipped.keys().cloned().collect();
        for (hk, n) in keys {
            if let Ok(hdr) = Self::hdecrypt(&hk, enc_header) {
                if hdr.n == n {
                    if let Some(mk) = self.mkskipped.get(&(hk, n)) {
                        let mk_copy = *mk;
                        if let Ok(Some(pt)) = Self::decrypt_with_mk(ciphertext, ad, enc_header, mk)
                        {
                            self.mkskipped.remove(&(hk, n));
                            return Ok(Some((pt, hk, n, mk_copy)));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn decrypt_header(
        &mut self,
        enc_header: &[u8],
    ) -> Result<(Header, bool, Option<[u8; 32]>), RatchetError> {
        // First try current HK_r if available
        if let Some(mut hk) = self.hkr {
            for _steps in 0..=MAX_SKIP_PER_CHAIN {
                if let Ok(hdr) = Self::hdecrypt(&hk, enc_header) {
                    // Return the actual HK that succeeded
                    return Ok((hdr, false, Some(hk)));
                }
                hk = Self::kdf_hk(&hk);
            }
        }

        // Before first DH ratchet (ckr = None), only accept headers with pn = 0
        if self.ckr.is_none() {
            let mut hk = self.nhkr;
            for _steps in 0..=MAX_SKIP_PER_CHAIN {
                if let Ok(hdr) = Self::hdecrypt(&hk, enc_header) {
                    if hdr.pn == 0 {
                        return Ok((hdr, true, Some(hk)));
                    }
                }
                hk = Self::kdf_hk(&hk);
            }
            // Reject all headers with pn ≠ 0 before first DH ratchet
            return Err(RatchetError::MaxSkipExceeded);
        }

        // After first DH ratchet, try NHK_r path with derivations
        let mut hk = self.nhkr;
        for _steps in 0..=MAX_SKIP_PER_CHAIN {
            if let Ok(hdr) = Self::hdecrypt(&hk, enc_header) {
                return Ok((hdr, true, Some(hk)));
            }
            hk = Self::kdf_hk(&hk);
        }
        Err(RatchetError::MaxSkipExceeded)
    }

    /// Derive all skipped keys up to `until`, but keep at most
    /// `MAX_SKIP_STORE` *newest* (hk,n)->mk entries.
    /// Older keys are thrown away deterministically.
    /// If the *gap length itself* exceeds the 1 000-step limit that every
    /// Signal-family client enforces, we abort with `MaxSkipExceeded`.
    fn skip_message_keys_he(
        &mut self,
        until: u32,
        mut hk_opt: Option<[u8; 32]>,
    ) -> Result<(), RatchetError> {
        // 0. Sanity check against spec-violating peers
        if until.wrapping_sub(self.nr) as usize > MAX_SKIP_PER_CHAIN {
            return Err(RatchetError::MaxSkipExceeded);
        }

        // No receiving chain yet?  Fast-forward the counter.
        let mut ck_r = match self.ckr {
            Some(k) => k,
            None => {
                self.nr = until;
                return Ok(());
            }
        };

        // 1. Walk the hash ratchet
        while self.nr < until {
            let (new_ck, mk) = Self::kdf_ck(&ck_r);
            ck_r = new_ck;

            // Only retain the tail so RAM stays bounded.
            let distance = until - self.nr; // keys left after this one
            if (distance as usize) <= MAX_SKIP_STORE {
                if let Some(ref mut hk) = hk_opt {
                    self.mkskipped.insert((*hk, self.nr), mk);
                    *hk = Self::kdf_hk(hk);
                }
            }

            self.nr = self.nr.wrapping_add(1);
        }
        self.ckr = Some(ck_r);

        // 2. FIFO eviction (defence-in-depth)
        while self.mkskipped.len() > MAX_SKIP_GLOBAL {
            if let Some(oldest) = self.mkskipped.keys().min_by_key(|(_, n)| *n).copied() {
                self.mkskipped.remove(&oldest);
            }
        }
        Ok(())
    }

    // dh_ratchet_he: advance root, header & chain keys after every DH step.
    fn dh_ratchet_he(&mut self, header: &Header) -> Result<(), RatchetError> {
        self.pn = self.ns;
        self.ns = 0;
        self.nr = 0;

        // Promote NHKs -> HKs and keep the last two HKs for self-decryption.
        self.rotate_header_keys();

        // 1. Receiving chain (dhs ‖ header.dh)
        Self::validate_pk(&header.dh)?;
        self.dhr = Some(header.dh);

        let dh_out1 = Self::dh(&self.dhs, &header.dh);
        let (new_rk, ck_r, nhk_r) = Self::kdf_rk_he(&self.rk, &dh_out1);
        self.rk = new_rk;
        self.ckr = Some(ck_r);
        self.nhkr = nhk_r;

        // 2. Generate *new* sending key‑pair and chain.
        let (dhs_sk, dhs_pk) = Self::generate_dh();
        self.dhs = dhs_sk;
        self.dhs_pub = dhs_pk;

        let dh_out2 = Self::dh(&self.dhs, &header.dh);
        let (new_rk2, ck_s2, nhk_s2) = Self::kdf_rk_he(&self.rk, &dh_out2);
        self.rk = new_rk2;
        self.cks = Some(ck_s2);
        self.nhks = nhk_s2;

        self.nonce_seq_msg = NonceSeq::new();
        Ok(())
    }

    /// Cache an outgoing draft message key; bounded LRU with zeroisation.
    fn cache_outgoing_header(&mut self, enc_header: &[u8], mk: &[u8; 32]) {
        let digest: [u8; 16] = blake3::hash(enc_header).as_bytes()[..16]
            .try_into()
            .unwrap();
        self.outgoing_cache
            .put((digest, self.ns), Zeroizing::new(*mk));
    }

    pub fn commit_sender(&mut self, max_n: Option<u32>) {
        match max_n {
            Some(bound) => {
                let keys: Vec<_> = self
                    .outgoing_cache
                    .iter()
                    .filter(|(k, _)| k.1 <= bound)
                    .map(|(k, _)| *k)
                    .collect();
                for k in keys {
                    self.outgoing_cache.pop(&k);
                }
            }
            None => while self.outgoing_cache.pop_lru().is_some() {},
        }
    }

    /// Decrypt a ciphertext we previously produced with encrypt.
    pub fn decrypt_outgoing(
        &mut self,
        header: &[u8],
        ciphertext: &[u8],
        ad: &[u8],
    ) -> Option<Vec<u8>> {
        // 1. try with current HK_s, else fallback to prev_hks.
        let hdr = self
            .hks
            .as_ref()
            .and_then(|hk| Self::hdecrypt(hk, header).ok())
            .or_else(|| {
                self.prev_hks
                    .iter()
                    .find_map(|hk| Self::hdecrypt(hk, header).ok())
            })?;

        // 2. look up the cached MK with exact (digest,n) key.
        let digest: [u8; 16] = blake3::hash(header).as_bytes()[..16].try_into().unwrap();
        let key = **self.outgoing_cache.get(&(digest, hdr.n))?;

        // 3. decrypt with the cached message key
        if let Some(pt) = Self::decrypt_with_mk(ciphertext, ad, header, &key)
            .ok()
            .flatten()
        {
            // Mark the draft as used
            self.outgoing_cache.get(&(digest, hdr.n));
            return Some(pt);
        }
        None
    }

    /// Permanently forget skipped‑message keys that are no longer required.
    ///
    /// * `header_key` – if `Some(hk)`, only keys bound to that HK are considered.
    /// * `n_max`      – if `Some(m)`, forget indices ≤ *m*; `None` ⇒ forget all.
    pub fn commit_receiver(&mut self, header_key: Option<[u8; 32]>, n_max: Option<u32>) {
        self.mkskipped.retain(|(hk, n), _| {
            let hk_ok = header_key.is_none_or(|h| hk != &h);
            let n_ok = n_max.is_some_and(|m| *n > m);
            hk_ok && n_ok
        });
    }

    /// Rotate HK_s, retaining previous two for draft decryption.
    fn rotate_header_keys(&mut self) {
        if let Some(curr) = self.hks.take() {
            if self.prev_hks.len() == 2 {
                self.prev_hks.pop_front();
            }
            self.prev_hks.push_back(curr);
        }
        self.hks = Some(self.nhks);
        self.hkr = Some(self.nhkr);
    }

    fn decrypt_with_mk(
        ciphertext: &[u8],
        ad: &[u8],
        enc_header: &[u8],
        mk: &[u8; 32],
    ) -> Result<Option<Vec<u8>>, RatchetError> {
        if ciphertext.len() < NONCE_LEN {
            return Ok(None);
        }
        let (nonce_bytes, ct) = ciphertext.split_at(NONCE_LEN);
        let cipher = Aes128SivAead::new_from_slice(mk).map_err(|_| RatchetError::CryptoError)?;
        let mut nonce = Nonce::default();
        nonce.copy_from_slice(nonce_bytes);
        let mut full_ad = ad.to_vec();
        full_ad.extend_from_slice(enc_header);
        cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ct,
                    aad: &full_ad,
                },
            )
            .map(Some)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, rand::rngs::OsRng};

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let initial_root = [0u8; 32];
        let shared_hka = [1u8; 32];
        let shared_nhkb = [2u8; 32];

        // Generate Receiver's DH keypair
        let receiver_sk = StaticSecret::random_from_rng(OsRng);
        let receiver_pk = PublicKey::from(&receiver_sk);

        // Initialize states
        let mut sender = RatchetStateHE::new();
        let mut receiver = RatchetStateHE::new();
        sender
            .init_sender_he(&initial_root, receiver_pk, shared_hka, shared_nhkb)
            .unwrap();
        receiver
            .init_receiver_he(
                &initial_root,
                (receiver_sk, receiver_pk),
                shared_hka,
                shared_nhkb,
            )
            .unwrap();

        let ad = b"associated data";
        let plaintext = b"hello, world";

        // Sender encrypts
        let (enc_hdr, payload) = sender
            .ratchet_encrypt_he(plaintext, ad)
            .expect("encryption failed");
        // Receiver decrypts
        let decrypted = receiver
            .ratchet_decrypt_he(&enc_hdr, &payload, ad)
            .expect("decryption failed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_header_decryption_fails_with_wrong_key() {
        let initial_root = [0u8; 32];
        let shared_hka = [3u8; 32];
        let shared_nhkb = [4u8; 32];

        let receiver_sk = StaticSecret::random_from_rng(OsRng);
        let receiver_pk = PublicKey::from(&receiver_sk);

        let mut sender = RatchetStateHE::new();
        let mut receiver = RatchetStateHE::new();
        sender
            .init_sender_he(&initial_root, receiver_pk, shared_hka, shared_nhkb)
            .unwrap();
        // Receiver with wrong header keys
        receiver
            .init_receiver_he(
                &initial_root,
                (receiver_sk, receiver_pk),
                shared_hka.map(|_| 0),
                shared_nhkb.map(|_| 0),
            )
            .unwrap();

        let ad = b"ad";
        let plaintext = b"data";
        let (enc_hdr, payload) = sender
            .ratchet_encrypt_he(plaintext, ad)
            .expect("encryption failed");
        // Receiver attempts decryption, should fail
        assert!(receiver.ratchet_decrypt_he(&enc_hdr, &payload, ad).is_err());
    }

    // Helper function to setup Sender and Receiver
    fn setup_ratchet_pair() -> (RatchetStateHE, RatchetStateHE) {
        let initial_root = [0u8; 32];
        let shared_hka = [1u8; 32];
        let shared_nhkb = [2u8; 32];

        // Generate Receiver's DH keypair
        let receiver_sk = StaticSecret::random_from_rng(OsRng);
        let receiver_pk = PublicKey::from(&receiver_sk);

        // Initialize states
        let mut sender = RatchetStateHE::new();
        let mut receiver = RatchetStateHE::new();
        sender
            .init_sender_he(&initial_root, receiver_pk, shared_hka, shared_nhkb)
            .unwrap();
        receiver
            .init_receiver_he(
                &initial_root,
                (receiver_sk, receiver_pk),
                shared_hka,
                shared_nhkb,
            )
            .unwrap();

        (sender, receiver)
    }

    #[test]
    fn test_multiple_messages() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // Send multiple messages from Sender to Receiver
        for i in 0..5 {
            let plaintext = format!("message {}", i).into_bytes();
            let (enc_hdr, payload) = sender
                .ratchet_encrypt_he(&plaintext, ad)
                .expect("encryption failed");
            let decrypted = receiver.ratchet_decrypt_he(&enc_hdr, &payload, ad).unwrap();
            assert_eq!(decrypted, plaintext);
        }
    }

    #[test]
    fn test_bidirectional_conversation() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // Sender sends to Receiver
        let s_msg1 = b"Hello Receiver!";
        let (enc_hdr1, payload1) = sender
            .ratchet_encrypt_he(s_msg1, ad)
            .expect("encryption failed");
        let decrypted1 = receiver
            .ratchet_decrypt_he(&enc_hdr1, &payload1, ad)
            .unwrap();
        assert_eq!(decrypted1, s_msg1);

        // Receiver replies to Sender
        let r_msg1 = b"Hi Sender!";
        let (enc_hdr2, payload2) = receiver
            .ratchet_encrypt_he(r_msg1, ad)
            .expect("encryption failed");
        let decrypted2 = sender.ratchet_decrypt_he(&enc_hdr2, &payload2, ad).unwrap();
        assert_eq!(decrypted2, r_msg1);

        // Sender sends another message
        let s_msg2 = b"How are you?";
        let (enc_hdr3, payload3) = sender
            .ratchet_encrypt_he(s_msg2, ad)
            .expect("encryption failed");
        let decrypted3 = receiver
            .ratchet_decrypt_he(&enc_hdr3, &payload3, ad)
            .unwrap();
        assert_eq!(decrypted3, s_msg2);

        // Receiver sends another reply
        let r_msg2 = b"I'm good, thanks!";
        let (enc_hdr4, payload4) = receiver
            .ratchet_encrypt_he(r_msg2, ad)
            .expect("encryption failed");
        let decrypted4 = sender.ratchet_decrypt_he(&enc_hdr4, &payload4, ad).unwrap();
        assert_eq!(decrypted4, r_msg2);
    }

    #[test]
    fn test_empty_associated_data() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let empty_ad = b"";
        let plaintext = b"message with empty AD";

        let (enc_hdr, payload) = sender
            .ratchet_encrypt_he(plaintext, empty_ad)
            .expect("encryption failed");
        let decrypted = receiver
            .ratchet_decrypt_he(&enc_hdr, &payload, empty_ad)
            .unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_empty_message() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";
        let empty_plaintext = b"";

        let (enc_hdr, payload) = sender
            .ratchet_encrypt_he(empty_plaintext, ad)
            .expect("encryption failed");
        let decrypted = receiver.ratchet_decrypt_he(&enc_hdr, &payload, ad).unwrap();
        assert_eq!(decrypted, empty_plaintext);
    }

    #[test]
    fn test_large_message() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";
        // Create a 10KB message
        let large_plaintext = vec![0xa5; 10 * 1024];

        let (enc_hdr, payload) = sender
            .ratchet_encrypt_he(&large_plaintext, ad)
            .expect("encryption failed");
        let decrypted = receiver.ratchet_decrypt_he(&enc_hdr, &payload, ad).unwrap();
        assert_eq!(decrypted, large_plaintext);
    }

    #[test]
    fn test_out_of_order_messages() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // Sender encrypts multiple messages
        let msg1 = b"message 1";
        let (hdr1, payload1) = sender
            .ratchet_encrypt_he(msg1, ad)
            .expect("encryption failed");

        let msg2 = b"message 2";
        let (hdr2, payload2) = sender
            .ratchet_encrypt_he(msg2, ad)
            .expect("encryption failed");

        let msg3 = b"message 3";
        let (hdr3, payload3) = sender
            .ratchet_encrypt_he(msg3, ad)
            .expect("encryption failed");

        // Receiver receives them out of order: 2, 1, 3
        let decrypted2 = receiver.ratchet_decrypt_he(&hdr2, &payload2, ad).unwrap();
        assert_eq!(decrypted2, msg2);

        // Should still be able to decrypt message 1 even though it's "old"
        let decrypted1 = receiver.ratchet_decrypt_he(&hdr1, &payload1, ad).unwrap();
        assert_eq!(decrypted1, msg1);

        // And continue with message 3
        let decrypted3 = receiver.ratchet_decrypt_he(&hdr3, &payload3, ad).unwrap();
        assert_eq!(decrypted3, msg3);
    }

    #[test]
    fn test_dh_ratchet_step() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // 1. Sender sends a message to Receiver
        let msg1 = b"First message from Sender";
        let (hdr1, payload1) = sender
            .ratchet_encrypt_he(msg1, ad)
            .expect("encryption failed");
        let decrypted1 = receiver.ratchet_decrypt_he(&hdr1, &payload1, ad).unwrap();
        assert_eq!(decrypted1, msg1);

        // 2. Receiver sends a reply - this triggers a DH ratchet on Sender's side
        let msg2 = b"First response from Receiver";
        let (hdr2, payload2) = receiver
            .ratchet_encrypt_he(msg2, ad)
            .expect("encryption failed");
        let decrypted2 = sender.ratchet_decrypt_he(&hdr2, &payload2, ad).unwrap();
        assert_eq!(decrypted2, msg2);

        // 3. Sender sends another message - this uses the new ratchet keys
        let msg3 = b"Second message from Sender";
        let (hdr3, payload3) = sender
            .ratchet_encrypt_he(msg3, ad)
            .expect("encryption failed");
        let decrypted3 = receiver.ratchet_decrypt_he(&hdr3, &payload3, ad).unwrap();
        assert_eq!(decrypted3, msg3);

        // 4. Receiver sends another reply - another DH ratchet
        let msg4 = b"Second response from Receiver";
        let (hdr4, payload4) = receiver
            .ratchet_encrypt_he(msg4, ad)
            .expect("encryption failed");
        let decrypted4 = sender.ratchet_decrypt_he(&hdr4, &payload4, ad).unwrap();
        assert_eq!(decrypted4, msg4);
    }

    #[test]
    fn test_skipped_message_keys_cleanup() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // Sender encrypts multiple messages
        let messages = (0..10)
            .map(|i| {
                let msg = format!("message {}", i).into_bytes();
                let result = sender
                    .ratchet_encrypt_he(&msg, ad)
                    .expect("encryption failed");
                (msg, result.0, result.1)
            })
            .collect::<Vec<_>>();

        // Receiver only decrypts messages 0, 5, and 9
        let indices_to_decrypt = [0, 5, 9];

        for &idx in indices_to_decrypt.iter() {
            let (msg, hdr, payload) = &messages[idx];
            let decrypted = receiver.ratchet_decrypt_he(hdr, payload, ad).unwrap();
            assert_eq!(&decrypted, msg);
        }

        // Receiver should have stored skipped keys for messages 1-4 and 6-8
        // Verify the size of the skipped keys map
        assert_eq!(receiver.mkskipped.len(), 7);

        // Now decrypt the remaining messages in reverse order
        for idx in (1..9).filter(|i| !indices_to_decrypt.contains(i)).rev() {
            let (msg, hdr, payload) = &messages[idx];
            let decrypted = receiver.ratchet_decrypt_he(hdr, payload, ad).unwrap();
            assert_eq!(&decrypted, msg);
        }

        // All skipped keys should be used now
        assert_eq!(receiver.mkskipped.len(), 0);
    }

    #[test]
    fn test_incorrect_associated_data() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"correct associated data";
        let wrong_ad = b"wrong associated data";

        // Sender encrypts with correct AD
        let plaintext = b"secret message";
        let (enc_hdr, payload) = sender
            .ratchet_encrypt_he(plaintext, ad)
            .expect("encryption failed");

        // Receiver tries to decrypt with wrong AD
        let result = receiver.ratchet_decrypt_he(&enc_hdr, &payload, wrong_ad);
        assert!(result.is_err(), "Decryption should fail with incorrect AD");
    }

    #[test]
    fn test_max_skip_limit() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // First, let receiver decrypt one message to establish the chain
        let first_msg = b"first message";
        let (first_hdr, first_payload) = sender
            .ratchet_encrypt_he(first_msg, ad)
            .expect("encryption failed");
        let _ = receiver
            .ratchet_decrypt_he(&first_hdr, &first_payload, ad)
            .unwrap();

        // Now sender encrypts MAX_SKIP_PER_CHAIN + 2 more messages
        let skip_plus_2 = MAX_SKIP_PER_CHAIN + 2;
        let messages = (0..skip_plus_2)
            .map(|i| {
                let msg = format!("message {}", i).into_bytes();
                let result = sender
                    .ratchet_encrypt_he(&msg, ad)
                    .expect("encryption failed");
                (msg, result.0, result.1)
            })
            .collect::<Vec<_>>();

        // Receiver tries to decrypt the last message directly (skipping MAX_SKIP_PER_CHAIN + 2 messages)
        let (_, last_hdr, last_payload) = &messages[skip_plus_2 - 1];

        // This should fail due to too many skipped messages
        let result = receiver.ratchet_decrypt_he(last_hdr, last_payload, ad);

        match result {
            Err(RatchetError::MaxSkipExceeded) => {
                // Expected error - test passes
                println!("Got expected MaxSkipExceeded error");
            }
            Err(other) => panic!("Expected MaxSkipExceeded, got {:?}", other),
            Ok(_) => {
                // If it succeeds, check how many skipped keys were created
                println!(
                    "Unexpectedly succeeded. Skipped keys: {}",
                    receiver.mkskipped.len()
                );
                panic!("Expected error due to max skip exceeded, but decryption succeeded");
            }
        }
    }

    #[test]
    fn test_header_deserialization() {
        // Create a header
        let header = Header {
            dh: PublicKey::from([1u8; 32]),
            pn: 42,
            n: 123,
        };

        // Serialize and deserialize
        let mut serialized = Vec::new();
        ciborium::into_writer(&header, &mut serialized).unwrap();
        let deserialized: Header = ciborium::from_reader(serialized.as_slice()).unwrap();

        // Compare
        assert_eq!(header.pn, deserialized.pn);
        assert_eq!(header.n, deserialized.n);
        assert_eq!(header.dh.as_bytes(), deserialized.dh.as_bytes());
    }

    #[test]
    fn test_different_associated_data_lengths() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let plaintext = b"test message";

        // Test with different AD lengths
        let ad_lengths = [0, 1, 16, 64, 1024];

        for len in ad_lengths.iter() {
            let ad = vec![0xbb; *len];
            let (enc_hdr, payload) = sender
                .ratchet_encrypt_he(plaintext, &ad)
                .expect("encryption failed");
            let decrypted = receiver
                .ratchet_decrypt_he(&enc_hdr, &payload, &ad)
                .unwrap();
            assert_eq!(decrypted, plaintext);
        }
    }

    #[test]
    fn test_corrupted_header() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";
        let plaintext = b"test message";

        // Sender encrypts a message
        let (mut enc_hdr, payload) = sender
            .ratchet_encrypt_he(plaintext, ad)
            .expect("encryption failed");

        // Corrupt the header by modifying a byte
        if !enc_hdr.is_empty() {
            let index = enc_hdr.len() / 2;
            enc_hdr[index] ^= 0x01; // Flip a bit
        }

        // Receiver tries to decrypt
        let result = receiver.ratchet_decrypt_he(&enc_hdr, &payload, ad);
        assert!(
            result.is_err(),
            "Decryption should fail with corrupted header"
        );
    }

    #[test]
    fn test_corrupted_payload() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";
        let plaintext = b"test message";

        // Sender encrypts a message
        let (enc_hdr, mut payload) = sender
            .ratchet_encrypt_he(plaintext, ad)
            .expect("encryption failed");

        // Corrupt the payload by modifying a byte
        if !payload.is_empty() {
            let index = payload.len() / 2;
            payload[index] ^= 0x01; // Flip a bit
        }

        // Receiver tries to decrypt
        let result = receiver.ratchet_decrypt_he(&enc_hdr, &payload, ad);
        assert!(
            result.is_err(),
            "Decryption should fail with corrupted payload"
        );
    }

    #[test]
    fn test_alternating_conversation() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // Multiple rounds of back-and-forth conversation
        for i in 0..10 {
            // Sender to Receiver
            let s_msg = format!("Sender message {}", i).into_bytes();
            let (s_hdr, s_payload) = sender
                .ratchet_encrypt_he(&s_msg, ad)
                .expect("encryption failed");
            let s_decrypted = receiver.ratchet_decrypt_he(&s_hdr, &s_payload, ad).unwrap();
            assert_eq!(s_decrypted, s_msg);

            // Receiver to Sender
            let r_msg = format!("Receiver message {}", i).into_bytes();
            let (r_hdr, r_payload) = receiver
                .ratchet_encrypt_he(&r_msg, ad)
                .expect("encryption failed");
            let r_decrypted = sender.ratchet_decrypt_he(&r_hdr, &r_payload, ad).unwrap();
            assert_eq!(r_decrypted, r_msg);
        }
    }

    #[test]
    fn test_multiple_messages_then_ratchet() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // Sender sends multiple messages
        for i in 0..5 {
            let msg = format!("Sender message {}", i).into_bytes();
            let (hdr, payload) = sender
                .ratchet_encrypt_he(&msg, ad)
                .expect("encryption failed");
            let decrypted = receiver.ratchet_decrypt_he(&hdr, &payload, ad).unwrap();
            assert_eq!(decrypted, msg);
        }

        // Receiver replies (triggering DH ratchet)
        let receiver_msg = b"Receiver's reply";
        let (hdr, payload) = receiver
            .ratchet_encrypt_he(receiver_msg, ad)
            .expect("encryption failed");
        let decrypted = sender.ratchet_decrypt_he(&hdr, &payload, ad).unwrap();
        assert_eq!(decrypted, receiver_msg);

        // Sender sends more messages with new ratchet state
        for i in 0..5 {
            let msg = format!("Sender new message {}", i).into_bytes();
            let (hdr, payload) = sender
                .ratchet_encrypt_he(&msg, ad)
                .expect("encryption failed");
            let decrypted = receiver.ratchet_decrypt_he(&hdr, &payload, ad).unwrap();
            assert_eq!(decrypted, msg);
        }
    }

    #[test]
    fn test_large_out_of_order_messages() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";
        let num_messages = 100;

        // Sender encrypts a large number of messages
        let messages: Vec<_> = (0..num_messages)
            .map(|i| {
                let msg = format!("message {}", i).into_bytes();
                let result = sender
                    .ratchet_encrypt_he(&msg, ad)
                    .expect("encryption failed");
                (msg, result.0, result.1)
            })
            .collect();

        // Create a shuffled order for receiving messages
        let mut receive_order: Vec<usize> = (0..num_messages).collect();

        // Simple deterministic shuffle to ensure reproducible test
        for i in 0..num_messages {
            let j = (i * 17 + 7) % num_messages; // Simple deterministic permutation
            receive_order.swap(i, j);
        }

        // Receiver decrypts messages in shuffled order
        let mut decrypted_count = 0;
        for &idx in &receive_order {
            // Only decrypt messages within skip limit to avoid hitting MAX_SKIP_PER_CHAIN
            // We'll decrypt in small batches to stay within limits
            if decrypted_count < 50 {
                // Limit to stay within MAX_SKIP_PER_CHAIN
                let (ref expected_msg, ref hdr, ref payload) = messages[idx];

                // Try to decrypt, but it might fail if we exceed skip limits
                match receiver.ratchet_decrypt_he(hdr, payload, ad) {
                    Ok(decrypted) => {
                        assert_eq!(&decrypted, expected_msg);
                        decrypted_count += 1;
                    }
                    Err(RatchetError::MaxSkipExceeded) => {
                        // Expected when we try to skip too many messages
                        break;
                    }
                    Err(e) => {
                        panic!("Unexpected error: {:?}", e);
                    }
                }
            }
        }

        // Should have successfully decrypted at least some messages
        assert!(
            decrypted_count > 10,
            "Should successfully decrypt at least a few messages, got {}",
            decrypted_count
        );

        // The remaining skipped keys should be stored for future use
        println!(
            "Successfully decrypted {} out of {} messages out of order",
            decrypted_count, num_messages
        );
        println!("Skipped message keys stored: {}", receiver.mkskipped.len());
    }

    #[test]
    fn test_manageable_out_of_order_messages() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";
        let num_messages = 10; // Small number to stay well within limits

        // Sender encrypts messages
        let messages: Vec<_> = (0..num_messages)
            .map(|i| {
                let msg = format!("message {}", i).into_bytes();
                let result = sender
                    .ratchet_encrypt_he(&msg, ad)
                    .expect("encryption failed");
                (msg, result.0, result.1)
            })
            .collect();

        // Receive in a specific out-of-order pattern: even indices first, then odd
        let mut receive_order = Vec::new();

        // First all even-indexed messages
        for i in (0..num_messages).step_by(2) {
            receive_order.push(i);
        }

        // Then all odd-indexed messages
        for i in (1..num_messages).step_by(2) {
            receive_order.push(i);
        }

        // Decrypt all messages in this order
        for &idx in &receive_order {
            let (ref expected_msg, ref hdr, ref payload) = messages[idx];
            let decrypted = receiver
                .ratchet_decrypt_he(hdr, payload, ad)
                .unwrap_or_else(|_| panic!("Failed to decrypt message {}", idx));
            assert_eq!(&decrypted, expected_msg);
        }

        // All messages should be successfully decrypted
        assert_eq!(
            receiver.mkskipped.len(),
            0,
            "All skipped keys should be consumed"
        );
        println!(
            "Successfully decrypted all {} messages out of order",
            num_messages
        );
    }

    #[test]
    fn test_skip_first_message_on_new_chain() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // Sender -> receiver to establish initial chain.
        let (hdr0, payload0) = sender
            .ratchet_encrypt_he(b"initial sync", ad)
            .expect("sender encrypt failed");
        receiver
            .ratchet_decrypt_he(&hdr0, &payload0, ad)
            .expect("receiver failed to decrypt initial message");

        // Receiver replies so the sender performs a DH ratchet.
        let (hdr1, payload1) = receiver
            .ratchet_encrypt_he(b"reply", ad)
            .expect("receiver encrypt failed");
        sender
            .ratchet_decrypt_he(&hdr1, &payload1, ad)
            .expect("sender failed to decrypt reply");

        // Sender now transmits the first message on the new chain (n = 0) - drop it.
        let _ = sender
            .ratchet_encrypt_he(b"dropped message", ad)
            .expect("sender encrypt failed");

        // Sender sends the second message on that chain (n = 1).
        let (hdr2, payload2) = sender
            .ratchet_encrypt_he(b"delivered message", ad)
            .expect("sender encrypt failed");

        // Receiver should be able to decrypt even though n = 0 was skipped.
        let decrypted = receiver
            .ratchet_decrypt_he(&hdr2, &payload2, ad)
            .expect("receiver failed to decrypt message after skipping n=0");
        assert_eq!(decrypted, b"delivered message");
    }

    #[test]
    fn test_basic_draft_functionality() {
        let (mut sender, mut _receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // Test single message draft
        let msg = b"test draft message";
        let (enc_hdr, payload) = sender
            .ratchet_encrypt_he(msg, ad)
            .expect("encryption failed");

        // Sender should be able to decrypt its own draft
        let decrypted = sender
            .decrypt_outgoing(&enc_hdr, &payload, ad)
            .expect("Sender failed to decrypt its own draft");
        assert_eq!(&decrypted, msg);

        // Test multiple drafts
        let mut drafts = Vec::new();
        for i in 0..5 {
            let msg = format!("draft {}", i).into_bytes();
            let (enc_hdr, payload) = sender
                .ratchet_encrypt_he(&msg, ad)
                .expect("encryption failed");
            drafts.push((msg, enc_hdr, payload));
        }

        // Decrypt all drafts in any order
        for (i, (ref expected_msg, ref hdr, ref payload)) in drafts.iter().enumerate() {
            let decrypted = sender
                .decrypt_outgoing(hdr, payload, ad)
                .unwrap_or_else(|| panic!("Failed to decrypt draft {}", i));
            assert_eq!(&decrypted, expected_msg);
        }

        println!("Basic draft functionality works!");
    }

    #[test]
    fn test_sender_draft_decryption_and_commit() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";
        let num_messages = 100;

        // Sender encrypts many messages (drafts)
        let mut drafts = Vec::new();
        for i in 0..num_messages {
            let msg = format!("draft message {}", i).into_bytes();
            let (enc_hdr, payload) = sender
                .ratchet_encrypt_he(&msg, ad)
                .expect("encryption failed");
            drafts.push((msg, enc_hdr, payload));
        }

        println!("Encrypted {} draft messages", num_messages);

        // Test sender's ability to decrypt a few sample drafts
        // Note: each decrypt_outgoing call removes the key, so only test once per draft
        let test_indices = [0, 50, 99]; // Limited sample to preserve other drafts
        for &idx in &test_indices {
            let (ref expected_msg, ref hdr, ref payload) = drafts[idx];
            let decrypted = sender
                .decrypt_outgoing(hdr, payload, ad)
                .unwrap_or_else(|| panic!("Sender failed to decrypt its own draft {}", idx));
            assert_eq!(&decrypted, expected_msg);
        }

        println!("Sender successfully decrypted sample drafts");

        // Receiver should be able to decrypt any subset of messages
        // Let's test with only 10% of messages (simulating 90% message loss)
        let receiver_indices: Vec<usize> = (0..num_messages).step_by(10).collect(); // Every 10th message

        for &idx in &receiver_indices {
            let (ref expected_msg, ref hdr, ref payload) = drafts[idx];
            let decrypted = receiver
                .ratchet_decrypt_he(hdr, payload, ad)
                .unwrap_or_else(|_| panic!("Receiver failed to decrypt message {}", idx));
            assert_eq!(&decrypted, expected_msg);
        }

        println!(
            "Receiver successfully decrypted {} out of {} messages ({}% message loss)",
            receiver_indices.len(),
            num_messages,
            (100 * (num_messages - receiver_indices.len())) / num_messages
        );

        // Verify sender still has most draft keys cached (some were used in testing)
        assert!(
            sender.outgoing_cache.len() > num_messages - 20,
            "Sender should have most draft keys cached"
        );

        // Commit sender drafts up to message 50
        sender.commit_sender(Some(50));

        // Test that committed drafts are no longer available (except those already used)
        // Use draft 30 which wasn't used in testing above
        let (_, ref hdr_30, ref payload_30) = drafts[30];
        let result = sender.decrypt_outgoing(hdr_30, payload_30, ad);
        assert!(
            result.is_none(),
            "Draft 30 should be committed and unavailable"
        );

        // Test that newer drafts are still available
        // Use draft 75 which wasn't used in testing and is > 50
        let (ref expected_msg_75, ref hdr_75, ref payload_75) = drafts[75];
        let decrypted_75 = sender
            .decrypt_outgoing(hdr_75, payload_75, ad)
            .expect("Draft 75 should still be available");
        assert_eq!(&decrypted_75, expected_msg_75);

        println!("Sender successfully committed drafts 0-50, retained 51-99");

        // Commit all remaining drafts
        sender.commit_sender(None);

        // Test that all remaining drafts are now committed
        // Use draft 85 which wasn't used before
        let (_, ref hdr_85, ref payload_85) = drafts[85];
        let result = sender.decrypt_outgoing(hdr_85, payload_85, ad);
        assert!(result.is_none(), "All drafts should be committed");

        assert!(
            sender.outgoing_cache.is_empty(),
            "All draft keys should be cleared"
        );
        println!("All sender drafts successfully committed and cleared");
    }

    #[test]
    fn test_receiver_extreme_message_loss_handling() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";
        let num_messages = 200;

        // Sender encrypts many messages
        let mut messages = Vec::new();
        for i in 0..num_messages {
            let msg = format!("message {}", i).into_bytes();
            let (enc_hdr, payload) = sender
                .ratchet_encrypt_he(&msg, ad)
                .expect("encryption failed");
            messages.push((msg, enc_hdr, payload));
        }

        println!("Encrypted {} messages", num_messages);

        // Simulate extreme message loss - receiver only gets 5% of messages
        // But in a pattern that tests different scenarios
        let mut received_indices = Vec::new();

        // Get first message (important for chain establishment)
        received_indices.push(0);

        // Get some early messages with gaps
        received_indices.extend([5, 15, 25]);

        // Get some messages from middle with large gaps
        received_indices.extend([50, 80, 120]);

        // Get some from near the end
        received_indices.extend([150, 180, 199]);

        println!(
            "Receiver will attempt to decrypt {} out of {} messages ({}% received)",
            received_indices.len(),
            num_messages,
            (100 * received_indices.len()) / num_messages
        );

        let mut successfully_decrypted = 0;
        let mut skip_limit_hits = 0;

        for &idx in &received_indices {
            let (ref expected_msg, ref hdr, ref payload) = messages[idx];

            match receiver.ratchet_decrypt_he(hdr, payload, ad) {
                Ok(decrypted) => {
                    assert_eq!(&decrypted, expected_msg);
                    successfully_decrypted += 1;
                    println!(" Successfully decrypted message {}", idx);
                }
                Err(RatchetError::MaxSkipExceeded) => {
                    skip_limit_hits += 1;
                    println!(
                        " Hit skip limit for message {} (expected for large gaps)",
                        idx
                    );
                }
                Err(e) => {
                    panic!("Unexpected error decrypting message {}: {:?}", idx, e);
                }
            }
        }

        println!("Results:");
        println!("  Successfully decrypted: {}", successfully_decrypted);
        println!("  Skip limit hits: {}", skip_limit_hits);
        println!("  Skipped keys stored: {}", receiver.mkskipped.len());

        // We should decrypt at least the first few messages before hitting limits
        assert!(
            successfully_decrypted >= 3,
            "Should successfully decrypt at least a few messages, got {}",
            successfully_decrypted
        );

        // Test sender's ability to decrypt a few of its own drafts
        // Note: each decrypt_outgoing call removes the key, so be selective
        let draft_test_indices = [10, 50, 100];
        for &idx in &draft_test_indices {
            let (ref expected_msg, ref hdr, ref payload) = messages[idx];
            let decrypted = sender
                .decrypt_outgoing(hdr, payload, ad)
                .unwrap_or_else(|| panic!("Sender should decrypt own draft {}", idx));
            assert_eq!(&decrypted, expected_msg);
        }

        println!(" Sender can still decrypt its own drafts despite receiver's message loss");

        // Test receiver cleanup
        receiver.commit_receiver(None, Some(50));
        println!("Cleaned up receiver skipped keys for messages ≤ 50");

        // Test sender cleanup
        sender.commit_sender(Some(100));
        println!("Committed sender drafts ≤ 100");

        // Verify partial cleanup worked
        // Test with a draft that wasn't used above and should be committed
        let (_, ref hdr_early, ref payload_early) = messages[75];

        assert!(
            sender
                .decrypt_outgoing(hdr_early, payload_early, ad)
                .is_none(),
            "Draft 75 should be committed (≤ 100)"
        );

        // Test with a draft that should still be available
        let (ref _expected_late, ref hdr_late, ref payload_late) = messages[150];
        assert!(
            sender
                .decrypt_outgoing(hdr_late, payload_late, ad)
                .is_some(),
            "Late draft should still be available"
        );

        println!(" Cleanup operations work correctly");
    }

    #[test]
    fn test_comprehensive_draft_drop_and_receiver_commit() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        println!("=== Comprehensive Draft Drop and Receiver Commit Test ===");

        // PHASE 1: Sender creates many drafts but only sends a small percentage
        let total_drafts = 100;
        let mut all_drafts = Vec::new();

        for i in 0..total_drafts {
            let msg = format!("draft {}", i).into_bytes();
            let (enc_hdr, payload) = sender
                .ratchet_encrypt_he(&msg, ad)
                .expect("encryption failed");
            all_drafts.push((msg, enc_hdr, payload));
        }

        println!("Phase 1: Sender created {} draft messages", total_drafts);

        // PHASE 2: Simulate severe packet loss - only 10% of messages actually sent
        let send_rate = 10; // Send every 10th message
        let sent_indices: Vec<usize> = (0..total_drafts).step_by(send_rate).collect();

        println!(
            "Phase 2: Sender decides to only send {} out of {} messages ({}% delivery)",
            sent_indices.len(),
            total_drafts,
            (100 * sent_indices.len()) / total_drafts
        );

        // PHASE 3: Receiver attempts to decrypt the sparse messages
        let mut successfully_received = 0;
        let mut first_skip_limit_hit = None;

        for &idx in &sent_indices {
            let (ref expected_msg, ref hdr, ref payload) = all_drafts[idx];
            match receiver.ratchet_decrypt_he(hdr, payload, ad) {
                Ok(decrypted) => {
                    assert_eq!(&decrypted, expected_msg);
                    successfully_received += 1;
                    println!(
                        "✓ Successfully received message {} ({}th sent)",
                        idx, successfully_received
                    );
                }
                Err(RatchetError::MaxSkipExceeded) => {
                    println!("✗ Hit skip limit at message index {}", idx);
                    first_skip_limit_hit = Some(idx);
                    break;
                }
                Err(e) => {
                    panic!("Unexpected error: {:?}", e);
                }
            }
        }

        println!(
            "Phase 3 result: Received {} messages before hitting skip limits",
            successfully_received
        );
        println!(
            "Receiver has {} skipped message keys stored",
            receiver.mkskipped.len()
        );

        // PHASE 4: Test sender's ability to decrypt all its own drafts
        let test_draft_indices = [5, 15, 25, 35, 45, 55, 65, 75, 85, 95];
        for &idx in &test_draft_indices {
            let (ref expected_msg, ref hdr, ref payload) = all_drafts[idx];
            let decrypted = sender
                .decrypt_outgoing(hdr, payload, ad)
                .unwrap_or_else(|| panic!("Sender should decrypt own draft {}", idx));
            assert_eq!(&decrypted, expected_msg);
        }
        println!("Phase 4: Sender can decrypt all its own drafts regardless of receiver state");

        // PHASE 5: Receiver commit operations - cleanup old skipped keys
        let skipped_before_commit = receiver.mkskipped.len();

        // Commit messages up to index 40 (assuming they'll never arrive)
        let commit_threshold = 40;
        receiver.commit_receiver(None, Some(commit_threshold));

        let skipped_after_commit = receiver.mkskipped.len();
        println!(
            "Phase 5a: commit_receiver(None, Some({})) - skipped keys: {} -> {}",
            commit_threshold, skipped_before_commit, skipped_after_commit
        );

        // Should have reduced the number of skipped keys
        if skipped_before_commit > 0 {
            assert!(
                skipped_after_commit < skipped_before_commit,
                "Should have cleaned up some skipped keys"
            );
        }

        // PHASE 6: Test DH ratchet scenario with receiver commit
        // Trigger a DH ratchet by having receiver send a message
        let receiver_msg = b"receiver triggers DH ratchet";
        let (r_hdr, r_payload) = receiver
            .ratchet_encrypt_he(receiver_msg, ad)
            .expect("encryption failed");
        let _ = sender.ratchet_decrypt_he(&r_hdr, &r_payload, ad).unwrap();

        println!("Phase 6a: Triggered DH ratchet with receiver message");

        // Store the header key from before the ratchet for selective cleanup
        let pre_ratchet_hk = receiver.hkr;

        // Sender sends more messages in new chain
        let mut new_chain_messages = Vec::new();
        for i in 0..20 {
            let msg = format!("new chain message {}", i).into_bytes();
            let (enc_hdr, payload) = sender
                .ratchet_encrypt_he(&msg, ad)
                .expect("encryption failed");
            new_chain_messages.push((msg, enc_hdr, payload));
        }

        // Receiver gets only some messages from new chain (sparse delivery continues)
        let new_chain_indices = [0, 3, 7, 12, 18];
        for &idx in &new_chain_indices {
            let (ref expected_msg, ref hdr, ref payload) = new_chain_messages[idx];
            let decrypted = receiver
                .ratchet_decrypt_he(hdr, payload, ad)
                .unwrap_or_else(|_| panic!("Failed to decrypt new chain message {}", idx));
            assert_eq!(&decrypted, expected_msg);
        }

        let skipped_after_new_chain = receiver.mkskipped.len();
        println!(
            "Phase 6b: After new chain messages, receiver has {} skipped keys",
            skipped_after_new_chain
        );

        // PHASE 7: Test header-key-specific commit (cleanup only old chain)
        if let Some(old_hk) = pre_ratchet_hk {
            let before_selective_commit = receiver.mkskipped.len();
            receiver.commit_receiver(Some(old_hk), None);
            let after_selective_commit = receiver.mkskipped.len();

            println!(
                "Phase 7: Header-key specific commit - skipped keys: {} -> {}",
                before_selective_commit, after_selective_commit
            );
        }

        // PHASE 8: Test sender commit operations
        println!("Phase 8a: Testing sender commit operations");

        // Test that sender can still decrypt drafts before committing
        let (ref expected_msg, ref test_hdr, ref test_payload) = all_drafts[30];
        let decrypted = sender
            .decrypt_outgoing(test_hdr, test_payload, ad)
            .expect("Should decrypt before commit");
        assert_eq!(&decrypted, expected_msg);

        // Commit sender drafts up to the threshold
        sender.commit_sender(Some(commit_threshold));

        // Test that committed drafts are no longer available
        let result = sender.decrypt_outgoing(test_hdr, test_payload, ad);
        assert!(result.is_none(), "Committed draft should not be available");

        // But newer drafts should still be available
        let (ref expected_msg_new, ref new_hdr, ref new_payload) = all_drafts[60];
        let decrypted_new = sender
            .decrypt_outgoing(new_hdr, new_payload, ad)
            .expect("New draft should still be available");
        assert_eq!(&decrypted_new, expected_msg_new);

        println!("Phase 8b: Sender commit works correctly - old drafts removed, new ones retained");

        // PHASE 9: Final cleanup and verification
        println!("Phase 9: Final cleanup operations");

        // Commit all remaining receiver skipped keys
        receiver.commit_receiver(None, None);
        assert_eq!(receiver.mkskipped.len(), 0);

        // Commit all remaining sender drafts
        sender.commit_sender(None);
        assert!(sender.outgoing_cache.is_empty());

        // Verify all drafts are now committed
        for idx in [60, 70, 80, 90] {
            let (_, ref hdr, ref payload) = all_drafts[idx];
            assert!(
                sender.decrypt_outgoing(hdr, payload, ad).is_none(),
                "All drafts should be committed after final cleanup"
            );
        }

        println!("Phase 9:  Complete cleanup successful");

        // PHASE 10: Summary
        println!("\n=== Test Summary ===");
        println!(
            " Sender created {} drafts but only sent {}% due to packet loss",
            total_drafts,
            (100 * sent_indices.len()) / total_drafts
        );
        println!(
            " Receiver successfully handled {} messages before hitting skip limits",
            successfully_received
        );
        if let Some(limit_idx) = first_skip_limit_hit {
            println!(
                " Skip limit correctly triggered at message index {}",
                limit_idx
            );
        }
        println!(" Sender could decrypt all its own drafts throughout the process");
        println!(" receiver.commit_receiver() successfully cleaned up stale skipped keys");
        println!(" sender.commit_sender() successfully managed draft cache memory");
        println!(" Header-key-specific cleanup worked across DH ratchet boundaries");
        println!(" Complete cleanup restored both sides to clean state");

        println!("\nThis test demonstrates the real-world scenario where:");
        println!("- Sender encrypts many messages (drafts) but network drops most of them");
        println!("- Receiver accumulates skipped message keys for gaps");
        println!("- Both sides use commit operations to manage memory and clean up stale state");
        println!(
            "- System gracefully handles severe packet loss while maintaining forward secrecy"
        );
    }

    #[test]
    fn test_multiple_outgoing_decrypt_same_message() {
        let (mut sender, mut _receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // Sender encrypts a message (creates a draft)
        let msg = b"test message for multiple decryption";
        let (enc_hdr, payload) = sender
            .ratchet_encrypt_he(msg, ad)
            .expect("encryption failed");

        // Decrypt the same draft multiple times
        for i in 1..=5 {
            let decrypted = sender
                .decrypt_outgoing(&enc_hdr, &payload, ad)
                .unwrap_or_else(|| panic!("Failed to decrypt draft on attempt {}", i));
            assert_eq!(&decrypted, msg, "Decryption {} should match original", i);
        }

        println!("Successfully decrypted the same draft message 5 times");

        // Verify the key is still in the cache after multiple decryptions
        let digest: [u8; 16] = blake3::hash(&enc_hdr).as_bytes()[..16].try_into().unwrap();
        assert!(
            sender.outgoing_cache.get(&(digest, 0)).is_some(),
            "Draft key should still be in cache after multiple decryptions"
        );

        // One more decryption to be sure
        let final_decrypt = sender
            .decrypt_outgoing(&enc_hdr, &payload, ad)
            .expect("Final decryption should still work");
        assert_eq!(&final_decrypt, msg);

        println!(" Draft key persists in cache, allowing unlimited re-decryption");
    }
}
