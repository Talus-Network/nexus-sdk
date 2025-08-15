use {
    crate::groth16::curve::PairingEngine,
    ark_groth16::{Groth16, PreparedVerifyingKey, ProvingKey, VerifyingKey},
    ark_relations::r1cs::ConstraintSynthesizer,
    ark_snark::SNARK,
    rand::{CryptoRng, RngCore},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("synthesis error")]
    Synthesis,
}

/// Circuit-specific proving/verification keys
#[derive(Clone)]
pub struct CircuitKeys<E: PairingEngine> {
    pub pk: ProvingKey<E>,
    pub vk: VerifyingKey<E>,
    pub pvk: PreparedVerifyingKey<E>,
}

impl<E: PairingEngine> CircuitKeys<E> {
    pub fn new(pk: ProvingKey<E>, vk: VerifyingKey<E>) -> Self {
        let pvk = ark_groth16::prepare_verifying_key(&vk);
        Self { pk, vk, pvk }
    }
}

/// Setup front-end.
#[derive(Clone)]
pub enum Setup<E: PairingEngine> {
    DevTrusted(CircuitKeys<E>),
    External(CircuitKeys<E>),
}

impl<E: PairingEngine> Setup<E> {
    /// Development-only single-party setup. Do not use in production.
    pub fn dev_trusted<C, R>(circuit: C, mut rng: R) -> Result<Self, SetupError>
    where
        C: ConstraintSynthesizer<E::ScalarField>,
        R: RngCore + CryptoRng,
    {
        let (pk, vk) = Groth16::<E>::circuit_specific_setup(circuit, &mut rng)
            .map_err(|_| SetupError::Synthesis)?;
        Ok(Self::DevTrusted(CircuitKeys::new(pk, vk)))
    }

    /// Provide already-derived circuit keys.
    pub fn external(keys: CircuitKeys<E>) -> Self {
        Self::External(keys)
    }

    pub fn keys(&self) -> &CircuitKeys<E> {
        match self {
            Setup::DevTrusted(k) | Setup::External(k) => k,
        }
    }

    /// Take ownership of circuit keys.
    pub fn into_keys(self) -> CircuitKeys<E> {
        match self {
            Setup::DevTrusted(k) | Setup::External(k) => k,
        }
    }
}
