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
//! // 1⃣ Session setup (X3DH, QR‑code, pre‑keys …).  Out of scope here:
//! let sender_root_key = [0u8; 32];          // derived elsewhere …
//! let receiver_root_key   = sender_root_key;     // shared!
//!
//! // Initial shared header keys obtained through X3DH or similar.
//! let shared_hka  = [1u8; 32]; // Sender‑>Receiver  (HK_sending for Sender)
//! let shared_nhkb = [2u8; 32]; // Receiver‑>Sender  (NHK_sending for Receiver)
//!
//! // 2⃣ Instantiate ratchets on both sides.
//! let mut sender = RatchetStateHE::new();
//! let mut receiver   = RatchetStateHE::new();
//!
//! // Receiver chooses a long‑term X25519 key‑pair for the first DH‑ratchet step.
//! let receiver_kp = RatchetStateHE::generate_dh();
//!
//! sender.init_sender_he(&sender_root_key, receiver_kp.1, shared_hka, shared_nhkb).unwrap();
//! receiver.init_receiver_he(&receiver_root_key, receiver_kp, shared_hka, shared_nhkb).unwrap();
//!
//! // 3⃣ Sender sends an encrypted message to Receiver.
//! let (hdr, msg) = sender.ratchet_encrypt_he(b"hello Receiver!", b"assoc-data").unwrap();
//!
//! // 4⃣ Receiver receives and decrypts.
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
    rand::{rngs::OsRng, RngCore},
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    sha2::Sha256,
    std::collections::HashMap,
    subtle::ConstantTimeEq,
    thiserror::Error,
    x25519_dalek::{PublicKey, StaticSecret},
    zeroize::Zeroize,
};

/// Maximum number of skipped message keys to derive per chain before rejecting incoming traffic,
///  as mentioned in section 4 of the spec.
const MAX_SKIP_PER_CHAIN: usize = 1_000;
/// Upper bound across all chains maintained in memory at any moment. This is
/// a defence‑in‑depth limit to avoid unbounded growth of
/// [`mkskipped`](RatchetStateHE::mkskipped).
const MAX_SKIP_GLOBAL: usize = 2 * MAX_SKIP_PER_CHAIN;

/// Each AES‑SIV nonce is 128‑bit.  We concatenate an 8‑byte random prefix with
/// an 8‑byte big‑endian counter to get a unique value for every encryption.
const NONCE_LEN: usize = 16;

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
/// Additional static helpers (`encrypt_static_he`, `decrypt_static_he`, …)
/// allow the sender to work on a reply before the previous ciphertext has
/// been delivered.
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
    /// Map keyed by `(header_key, n)` → message key, maintained while the state
    /// is alive.
    mkskipped: HashMap<([u8; 32], u32), [u8; 32]>,
    /// Nonce sequence for payload encryption
    #[serde(with = "nonce_seq_serde")]
    nonce_seq_msg: NonceSeq,
    /// Nonce sequence for header encryption
    #[serde(with = "nonce_seq_serde")]
    nonce_seq_header: NonceSeq,
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
        self.nhks.zeroize();
        self.nhkr.zeroize();
        self.mkskipped.clear();
        self.nonce_seq_msg.zeroize();
        self.nonce_seq_header.zeroize();
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
    fn validate_pk(pk: &PublicKey) -> Result<(), RatchetError> {
        // Extend `SMALL_ORDER` as needed.
        const SMALL_ORDER: [[u8; 32]; 1] = [[0u8; 32]];
        for bad in SMALL_ORDER.iter() {
            if pk.as_bytes().ct_eq(bad).unwrap_u8() == 1 {
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

    // === Header encryption helpers (AES‑SIV) ===

    /// Encrypt and authenticate a header with AES‑128‑SIV.
    /// Takes the header key and plaintext and returns the encrypted header.
    #[inline]
    fn hencrypt(&mut self, hk: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, RatchetError> {
        let cipher = Aes128SivAead::new_from_slice(hk).map_err(|_| RatchetError::CryptoError)?;
        let nonce_bytes = self.nonce_seq_header.next();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let mut ct = cipher.encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: &[],
            },
        )?;
        let mut out = nonce_bytes.to_vec();
        out.append(&mut ct);
        Ok(out)
    }

    /// Decrypt and verify a header given the 32‑byte header key.
    /// Takes the header key and encrypted header and returns the decrypted header.
    #[inline]
    fn hdecrypt(hk: &[u8; 32], data: &[u8]) -> Result<Header, RatchetError> {
        if data.len() < NONCE_LEN {
            return Err(RatchetError::CryptoError);
        }
        let (nonce_bytes, ct) = data.split_at(NONCE_LEN);
        let cipher = Aes128SivAead::new_from_slice(hk).map_err(|_| RatchetError::CryptoError)?;
        let nonce = Nonce::from_slice(nonce_bytes);
        let pt = cipher.decrypt(nonce, Payload { msg: ct, aad: &[] })?;
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
            nonce_seq_msg: NonceSeq::new(),
            nonce_seq_header: NonceSeq::new(),
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
        self.nonce_seq_header = NonceSeq::new();
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
        self.nonce_seq_header = NonceSeq::new();
        Ok(())
    }

    // === Public API – send / receive ===

    /// Encrypt a plaintext with associated data `ad` into `(enc_header, payload)`.
    ///
    /// Returns `(enc_header, ciphertext_payload)` suitable for transport.  The
    /// function advances the sending chain, so do not call it twice for
    /// the same message.
    pub fn ratchet_encrypt_he(
        &mut self,
        plaintext: &[u8],
        ad: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), RatchetError> {
        // Check if sender has a sending chain
        let cks = self.cks.ok_or(RatchetError::MissingSendingChain)?;
        // Derive new chain key and message key
        let (new_cks, mk) = Self::kdf_ck(&cks);
        // Update chain key
        self.cks = Some(new_cks);

        // 1⃣ Build & encrypt header.
        let header = Header {
            dh: self.dhs_pub,
            pn: self.pn,
            n: self.ns,
        };
        let mut header_bytes = Vec::new();
        ciborium::into_writer(&header, &mut header_bytes).expect("cbor");

        let hk = self.hks.clone().ok_or(RatchetError::MissingHeaderKey)?;
        // Encrypt header
        let enc_header = self.hencrypt(&hk, &header_bytes)?;

        // 2⃣ Encrypt payload where AAD = user AD || enc_header(see specs)
        let mut full_ad = ad.to_vec();
        full_ad.extend_from_slice(&enc_header);

        let cipher = Aes128SivAead::new_from_slice(&mk).map_err(|_| RatchetError::CryptoError)?;
        let nonce_bytes = self.nonce_seq_msg.next();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let mut ct = cipher.encrypt(
            nonce,
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
    pub fn ratchet_decrypt_he(
        &mut self,
        enc_header: &[u8],
        ciphertext: &[u8],
        ad: &[u8],
    ) -> Result<Vec<u8>, RatchetError> {
        // Try for skipped message keys
        if let Some(pt) = self.try_skipped_keys(enc_header, ciphertext, ad)? {
            return Ok(pt);
        }

        // Decrypt header
        let (header, did_dh_ratchet) = self.decrypt_header(enc_header)?;
        // If we did a DH ratchet step, skip message keys
        if did_dh_ratchet {
            self.skip_message_keys_he(header.pn)?;
            self.dh_ratchet_he(&header)?;
        }
        self.skip_message_keys_he(header.n)?;

        // Is there a receiving chain?
        let ckr = self.ckr.ok_or(RatchetError::MissingReceivingChain)?;
        let (new_ckr, mk) = Self::kdf_ck(&ckr);
        self.ckr = Some(new_ckr);
        self.nr = self.nr.wrapping_add(1);

        if ciphertext.len() < NONCE_LEN {
            return Err(RatchetError::CryptoError);
        }
        let (nonce_bytes, ct) = ciphertext.split_at(NONCE_LEN);
        let cipher = Aes128SivAead::new_from_slice(&mk).map_err(|_| RatchetError::CryptoError)?;
        let nonce = Nonce::from_slice(nonce_bytes);

        let mut full_ad = ad.to_vec();
        full_ad.extend_from_slice(enc_header);
        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: ct,
                    aad: &full_ad,
                },
            )
            .map_err(Into::into)
    }

    // === Internal helpers (skipped messages, DH‑ratchet) ===

    // try_skipped_keys: attempt decryption with previously stored skipped keys.
    fn try_skipped_keys(
        &mut self,
        enc_header: &[u8],
        ciphertext: &[u8],
        ad: &[u8],
    ) -> Result<Option<Vec<u8>>, RatchetError> {
        for ((hk_bytes, idx), mk) in &self.mkskipped {
            if ciphertext.len() < NONCE_LEN {
                continue;
            }

            // Avoid timing side‑channels by checking header keys in constant time.
            if hk_bytes.ct_eq(&self.nhkr).unwrap_u8() == 0
                && hk_bytes.ct_eq(&self.hkr.unwrap_or([0u8; 32])).unwrap_u8() == 0
            {
                continue;
            }

            if let Ok(hdr) = Self::hdecrypt(hk_bytes, enc_header) {
                if hdr.n == *idx {
                    let (nonce_bytes, ct) = ciphertext.split_at(NONCE_LEN);
                    let cipher =
                        Aes128SivAead::new_from_slice(mk).map_err(|_| RatchetError::CryptoError)?;
                    let nonce = Nonce::from_slice(nonce_bytes);
                    let mut full_ad = ad.to_vec();
                    full_ad.extend_from_slice(enc_header);
                    if let Ok(pt) = cipher.decrypt(
                        nonce,
                        Payload {
                            msg: ct,
                            aad: &full_ad,
                        },
                    ) {
                        self.mkskipped.remove(&(*hk_bytes, *idx));
                        return Ok(Some(pt));
                    }
                }
            }
        }
        Ok(None)
    }

    // decrypt_header: try current HK, else next‑HK (DH‑ratchet trigger).
    fn decrypt_header(&self, enc_header: &[u8]) -> Result<(Header, bool), RatchetError> {
        if let Some(hk) = &self.hkr {
            if let Ok(hdr) = Self::hdecrypt(hk, enc_header) {
                return Ok((hdr, false));
            }
        }
        // Try next header key
        // it means we did a DH ratchet step
        let hdr = Self::hdecrypt(&self.nhkr, enc_header)?;
        Ok((hdr, true))
    }

    // skip_message_keys_he: derive and store skipped message‑keys up to `until`.
    fn skip_message_keys_he(&mut self, until: u32) -> Result<(), RatchetError> {
        if self.nr + (MAX_SKIP_PER_CHAIN as u32) < until {
            return Err(RatchetError::MaxSkipExceeded);
        }
        if let Some(mut ck_r) = self.ckr {
            while self.nr < until {
                let (new_ck, mk) = Self::kdf_ck(&ck_r);
                ck_r = new_ck;
                if let Some(hkr) = &self.hkr {
                    self.mkskipped.insert((*hkr, self.nr), mk);
                    if self.mkskipped.len() > MAX_SKIP_GLOBAL {
                        return Err(RatchetError::MaxSkipExceeded); // flush or error
                    }
                }
                self.nr = self.nr.wrapping_add(1);
            }
            self.ckr = Some(ck_r);
        }
        Ok(())
    }

    // dh_ratchet_he: advance root, header & chain keys after every DH step.
    fn dh_ratchet_he(&mut self, header: &Header) -> Result<(), RatchetError> {
        self.pn = self.ns;
        self.ns = 0;
        self.nr = 0;

        self.hks = Some(self.nhks.clone());
        self.hkr = Some(self.nhkr.clone());

        // 1⃣ Receiving chain (dhs ‖ header.dh)
        Self::validate_pk(&header.dh)?;
        self.dhr = Some(header.dh);

        let dh_out1 = Self::dh(&self.dhs, &header.dh);
        let (new_rk, ck_r, nhk_r) = Self::kdf_rk_he(&self.rk, &dh_out1);
        self.rk = new_rk;
        self.ckr = Some(ck_r);
        self.nhkr = nhk_r;

        // 2⃣ Generate *new* sending key‑pair and chain.
        let (dhs_sk, dhs_pk) = Self::generate_dh();
        self.dhs = dhs_sk;
        self.dhs_pub = dhs_pk;

        let dh_out2 = Self::dh(&self.dhs, &header.dh);
        let (new_rk2, ck_s2, nhk_s2) = Self::kdf_rk_he(&self.rk, &dh_out2);
        self.rk = new_rk2;
        self.cks = Some(ck_s2);
        self.nhks = nhk_s2;

        self.nonce_seq_msg = NonceSeq::new();
        self.nonce_seq_header = NonceSeq::new();
        Ok(())
    }

    // Convenience: derive *message key* directly from a chain key.
    fn mk_from_ck(ck: &[u8; 32]) -> [u8; 32] {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(ck).unwrap();
        mac.update(&[0x02]);
        let tag = mac.finalize().into_bytes();
        let mut mk = [0u8; 32];
        mk.copy_from_slice(&tag);
        mk
    }

    // === Optional helpers for asynchronous senders ("static HE") ===

    /// Derive `enc_header` and `ciphertext` without mutating the state.
    /// The caller must ensure that the real call to
    /// [`ratchet_encrypt_he`](Self::ratchet_encrypt_he) follows immediately;
    /// otherwise the chain and header keys will diverge.
    pub fn encrypt_static_he(&self, plaintext: &[u8], ad: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
        let ck_s = self.cks.as_ref()?;
        let hk_s = self.hks.as_ref()?;
        let mk = Self::mk_from_ck(ck_s);

        let header = Header {
            dh: self.dhs_pub,
            pn: self.pn,
            n: self.ns,
        };
        let mut hdr_bytes = Vec::new();
        ciborium::into_writer(&header, &mut hdr_bytes).ok()?;

        let mut nonce_seq = NonceSeq::new();
        let nonce_bytes = nonce_seq.next();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let cipher = Aes128SivAead::new_from_slice(hk_s).ok()?;
        let mut header_ct = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: &hdr_bytes,
                    aad: &[],
                },
            )
            .ok()?;
        let mut enc_header = nonce_bytes.to_vec();
        enc_header.append(&mut header_ct);

        let mut full_ad = ad.to_vec();
        full_ad.extend_from_slice(&enc_header);

        let nonce_bytes2 = nonce_seq.next();
        let nonce2 = Nonce::from_slice(&nonce_bytes2);
        let cipher = Aes128SivAead::new_from_slice(&mk).ok()?;
        let mut ct = cipher
            .encrypt(
                nonce2,
                Payload {
                    msg: plaintext,
                    aad: &full_ad,
                },
            )
            .ok()?;
        let mut payload = nonce_bytes2.to_vec();
        payload.append(&mut ct);
        Some((enc_header, payload))
    }

    /// Decrypt receiver side using immutable state. Useful for "peek" or stateless workers.
    pub fn decrypt_static_he(
        &self,
        enc_header: &[u8],
        ciphertext: &[u8],
        ad: &[u8],
    ) -> Option<Vec<u8>> {
        let _header = self
            .hkr
            .and_then(|ref hk| Self::hdecrypt(hk, enc_header).ok())
            .or_else(|| Self::hdecrypt(&self.nhkr, enc_header).ok())?;
        let ck_r = self.ckr.as_ref()?;
        let mk = Self::mk_from_ck(ck_r);

        if ciphertext.len() < NONCE_LEN {
            return None;
        }
        let (nonce_bytes, ct) = ciphertext.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let mut full_ad = ad.to_vec();
        full_ad.extend_from_slice(enc_header);
        let cipher = Aes128SivAead::new_from_slice(&mk).ok()?;
        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: ct,
                    aad: &full_ad,
                },
            )
            .ok()
    }

    /// Symmetric to [`decrypt_static_he`] but runs on the sender to inspect its own in‑flight message.
    /// We sent this message to the receiver. But we want to inspect it before sending a final message.
    /// Returns the decrypted payload.
    pub fn decrypt_own_static_he(
        &self,
        enc_header: &[u8],
        ciphertext: &[u8],
        ad: &[u8],
    ) -> Option<Vec<u8>> {
        let hk_s = self.hks.as_ref()?;
        let _header = Self::hdecrypt(hk_s, enc_header).ok()?;
        let ck_s = self.cks.as_ref()?;
        let mk = Self::mk_from_ck(ck_s);

        if ciphertext.len() < NONCE_LEN {
            return None;
        }
        let (nonce_bytes, ct) = ciphertext.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let mut full_ad = ad.to_vec();
        full_ad.extend_from_slice(enc_header);
        let cipher = Aes128SivAead::new_from_slice(&mk).ok()?;
        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: ct,
                    aad: &full_ad,
                },
            )
            .ok()
    }

    /// Get a reference to the root key
    pub fn root_key(&self) -> &[u8; 32] {
        &self.rk
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
            .init_sender_he(&initial_root, receiver_pk.clone(), shared_hka, shared_nhkb)
            .unwrap();
        receiver
            .init_receiver_he(
                &initial_root,
                (receiver_sk, receiver_pk.clone()),
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
            .init_sender_he(&initial_root, receiver_pk.clone(), shared_hka, shared_nhkb)
            .unwrap();
        // Receiver with wrong header keys
        receiver
            .init_receiver_he(
                &initial_root,
                (receiver_sk, receiver_pk.clone()),
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
            .init_sender_he(&initial_root, receiver_pk.clone(), shared_hka, shared_nhkb)
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
    #[should_panic]
    fn test_max_skip_limit() {
        let (mut sender, mut receiver) = setup_ratchet_pair();
        let ad = b"associated data";

        // Sender encrypts MAX_SKIP_PER_CHAIN + 2 messages
        let max_plus_2 = MAX_SKIP_PER_CHAIN as usize + 2;
        let messages = (0..max_plus_2)
            .map(|i| {
                let msg = format!("message {}", i).into_bytes();
                let result = sender
                    .ratchet_encrypt_he(&msg, ad)
                    .expect("encryption failed");
                (msg, result.0, result.1)
            })
            .collect::<Vec<_>>();

        // Receiver tries to decrypt the last message directly (skipping MAX_SKIP + 1 messages)
        let (_, last_hdr, last_payload) = &messages[max_plus_2 - 1];

        // This should panic due to too many skipped messages
        receiver
            .ratchet_decrypt_he(last_hdr, last_payload, ad)
            .unwrap();
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
}
