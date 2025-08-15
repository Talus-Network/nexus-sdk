use {
    crate::groth16::{curve::PairingEngine, srs::CircuitKeys},
    ark_groth16::{Groth16, Proof},
    ark_snark::SNARK,
    rand::{CryptoRng, RngCore},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ProofError {
    #[error("prover synthesis error")]
    Prover,
    #[error("verification failed")]
    Verify,
}

/// A proof plus the exact public inputs it was produced/verified against.
#[derive(Clone)]
pub struct ProofBundle<E: PairingEngine> {
    pub proof: Proof<E>,
    pub public_inputs: Vec<E::ScalarField>,
}

pub struct Prover<E: PairingEngine> {
    _m: core::marker::PhantomData<E>,
}
impl<E: PairingEngine> Default for Prover<E> {
    fn default() -> Self {
        Self {
            _m: Default::default(),
        }
    }
}

impl<E: PairingEngine> Prover<E> {
    pub fn prove<C, R>(
        circuit: C,
        public_inputs: &[E::ScalarField],
        keys: &CircuitKeys<E>,
        mut rng: R,
    ) -> Result<ProofBundle<E>, ProofError>
    where
        C: ark_relations::r1cs::ConstraintSynthesizer<E::ScalarField>,
        R: RngCore + CryptoRng,
    {
        let proof =
            Groth16::<E>::prove(&keys.pk, circuit, &mut rng).map_err(|_| ProofError::Prover)?;
        Ok(ProofBundle {
            proof,
            public_inputs: public_inputs.to_vec(),
        })
    }
}

pub struct Verifier<E: PairingEngine> {
    _m: core::marker::PhantomData<E>,
}
impl<E: PairingEngine> Default for Verifier<E> {
    fn default() -> Self {
        Self {
            _m: Default::default(),
        }
    }
}

impl<E: PairingEngine> Verifier<E> {
    pub fn verify(bundle: &ProofBundle<E>, keys: &CircuitKeys<E>) -> Result<(), ProofError> {
        let ok =
            Groth16::<E>::verify_with_processed_vk(&keys.pvk, &bundle.public_inputs, &bundle.proof)
                .map_err(|_| ProofError::Verify)?;
        if ok {
            Ok(())
        } else {
            Err(ProofError::Verify)
        }
    }
}
