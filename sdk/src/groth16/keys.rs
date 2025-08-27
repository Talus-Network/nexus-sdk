use {
    crate::groth16::{curve::PairingEngine, error::SetupError},
    ark_groth16::{Groth16, PreparedVerifyingKey, ProvingKey, VerifyingKey},
    ark_relations::r1cs::ConstraintSynthesizer,
    ark_snark::SNARK,
    rand::{CryptoRng, RngCore},
};

/// Circuit-specific proving and verification material.
#[derive(Clone)]
pub struct CircuitKeys<E: PairingEngine> {
    pub pk: ProvingKey<E>,
    pub vk: VerifyingKey<E>,
    pub pvk: PreparedVerifyingKey<E>,
}
impl<E: PairingEngine> CircuitKeys<E> {
    pub fn new(pk: ProvingKey<E>, vk: VerifyingKey<E>) -> Self {
        let pvk = PreparedVerifyingKey::from(vk.clone());
        Self { pk, vk, pvk }
    }
}

/// Setup origin
pub enum Setup<E: PairingEngine> {
    /// Dev-only, single-party keygen (dont use in production)
    DevTrusted(CircuitKeys<E>),
    /// Produced via your PoT + Phase-2 pipeline.
    External(CircuitKeys<E>),
}
impl<E: PairingEngine> Setup<E> {
    /// Development-only, one-shot Groth16 setup (no ceremony)
    pub fn dev_trusted<C, R>(circuit: C, mut rng: R) -> Result<Self, SetupError>
    where
        C: ConstraintSynthesizer<<E as ark_ec::pairing::Pairing>::ScalarField>,
        R: RngCore + CryptoRng,
    {
        let (pk, vk) = Groth16::<E>::circuit_specific_setup(circuit, &mut rng)
            .map_err(|_| SetupError::Synthesis)?;
        Ok(Self::DevTrusted(CircuitKeys::new(pk, vk)))
    }

    pub fn keys(&self) -> &CircuitKeys<E> {
        match self {
            Setup::DevTrusted(k) | Setup::External(k) => k,
        }
    }

    pub fn into_keys(self) -> CircuitKeys<E> {
        match self {
            Setup::DevTrusted(k) | Setup::External(k) => k,
        }
    }
}
