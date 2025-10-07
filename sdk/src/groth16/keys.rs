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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::groth16::curve::DefaultCurve,
        ark_ec::pairing::Pairing,
        ark_ff::UniformRand,
        ark_groth16::Groth16,
        ark_relations::r1cs::{ConstraintSynthesizer, LinearCombination, Variable},
        ark_snark::SNARK,
        rand::{rngs::StdRng, SeedableRng},
    };

    #[derive(Clone)]
    struct MulAddCircuit<F: ark_ff::PrimeField> {
        a: F,
        b: F,
        c: F,
        d: F,
    }

    impl<F: ark_ff::PrimeField> ConstraintSynthesizer<F> for MulAddCircuit<F> {
        fn generate_constraints(
            self,
            cs: ark_relations::r1cs::ConstraintSystemRef<F>,
        ) -> Result<(), ark_relations::r1cs::SynthesisError> {
            let c_var = cs.new_input_variable(|| Ok(self.c))?;
            let d_var = cs.new_input_variable(|| Ok(self.d))?;
            let a = cs.new_witness_variable(|| Ok(self.a))?;
            let b = cs.new_witness_variable(|| Ok(self.b))?;

            cs.enforce_constraint(
                LinearCombination::from(a),
                LinearCombination::from(b),
                LinearCombination::from(c_var),
            )?;
            cs.enforce_constraint(
                LinearCombination::from(a) + LinearCombination::from(b),
                LinearCombination::from(Variable::One),
                LinearCombination::from(d_var),
            )?;
            Ok(())
        }
    }

    #[test]
    fn dev_trusted_setup_produces_usable_keys() {
        type E = DefaultCurve;
        type Fr = <E as Pairing>::ScalarField;

        let mut rng = StdRng::seed_from_u64(2024);
        let a = Fr::rand(&mut rng);
        let b = Fr::rand(&mut rng);
        let c = a * b;
        let d = a + b;

        let circuit = MulAddCircuit { a, b, c, d };
        let setup = Setup::<E>::dev_trusted(circuit.clone(), &mut rng).expect("dev setup");

        {
            let keys = setup.keys();
            let proof = Groth16::<E>::prove(&keys.pk, circuit.clone(), &mut rng).expect("prove");
            assert!(
                Groth16::<E>::verify(&keys.vk, &[c, d], &proof).expect("verify"),
                "verification with borrowed keys should succeed"
            );
        }

        let keys = setup.into_keys();
        let proof = Groth16::<E>::prove(&keys.pk, circuit.clone(), &mut rng).expect("prove");
        assert!(
            Groth16::<E>::verify(&keys.vk, &[c, d], &proof).expect("verify"),
            "verification with owned keys should succeed"
        );
        assert!(
            Groth16::<E>::verify_with_processed_vk(&keys.pvk, &[c, d], &proof).expect("verify pvk"),
            "prepared verifying key should match verifying key"
        );

        // Mutate keys to ensure API remains usable after ownership transfer.
        let another_proof = Groth16::<E>::prove(&keys.pk, circuit, &mut rng).expect("second proof");
        assert!(
            Groth16::<E>::verify_with_processed_vk(&keys.pvk, &[c, d], &another_proof)
                .expect("verify pvk"),
            "prepared VK stays in sync after reuse"
        );
    }
}
