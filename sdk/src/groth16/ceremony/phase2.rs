use {
    crate::groth16::{curve::PairingEngine, error::SetupError, keys::CircuitKeys},
    ark_ec::{pairing::Pairing, CurveGroup},
    ark_ff::{Field, PrimeField},
    ark_groth16::PreparedVerifyingKey,
    ark_serialize::{CanonicalDeserialize, CanonicalSerialize},
    blake2::Blake2b512,
    digest::Digest,
    rand::{CryptoRng, RngCore},
};

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct SameExponentProof<E: PairingEngine> {
    pub a1: E::G1Affine,
    pub a2: E::G2Affine,
    pub s: <E as Pairing>::ScalarField,
}

fn hash_to_field<F: PrimeField>(dst: &'static [u8], bytes: &[u8]) -> F {
    let mut hasher = Blake2b512::new();
    hasher.update(dst);
    hasher.update(bytes);
    let out = hasher.finalize();
    F::from_le_bytes_mod_order(&out)
}
fn serialize_points<E: PairingEngine, G1: CanonicalSerialize, G2: CanonicalSerialize>(
    g1s: &[G1],
    g2s: &[G2],
    extra: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(extra);
    for p in g1s {
        p.serialize_compressed(&mut buf).unwrap();
    }
    for q in g2s {
        q.serialize_compressed(&mut buf).unwrap();
    }
    buf
}
fn nonzero_random<F: PrimeField, R: RngCore + CryptoRng>(rng: &mut R) -> F {
    loop {
        let r = F::rand(rng);
        if !r.is_zero() {
            return r;
        }
    }
}

/// One contribution to the Phase 2 ceremony that randomizes `delta`
/// and rescales dependent queries (`h_query`, `l_query`).
#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct DeltaContribution<E: PairingEngine> {
    pub before_delta_g1: E::G1Affine,
    pub before_delta_g2: E::G2Affine,
    pub after_delta_g1: E::G1Affine,
    pub after_delta_g2: E::G2Affine,
    pub pok: SameExponentProof<E>,
}

impl<E: PairingEngine> DeltaContribution<E> {
    pub fn apply<R: RngCore + CryptoRng>(keys: &mut CircuitKeys<E>, rng: &mut R) -> Self {
        let r = nonzero_random::<E::ScalarField, _>(rng);
        let r_inv = r.inverse().expect("nonzero");

        let before_delta_g1 = keys.pk.delta_g1;
        let before_delta_g2 = keys.vk.delta_g2;

        let k = nonzero_random::<E::ScalarField, _>(rng);
        let a1 = (before_delta_g1 * k).into_affine();
        let a2 = (before_delta_g2 * k).into_affine();

        let after_delta_g1 = (before_delta_g1 * r).into_affine();
        let after_delta_g2 = (before_delta_g2 * r).into_affine();

        // scale h_query and l_query by r^{-1}
        keys.pk.h_query = keys
            .pk
            .h_query
            .iter()
            .map(|h| (*h * r_inv).into_affine())
            .collect();
        keys.pk.l_query = keys
            .pk
            .l_query
            .iter()
            .map(|l| (*l * r_inv).into_affine())
            .collect();

        keys.pk.delta_g1 = after_delta_g1;
        keys.vk.delta_g2 = after_delta_g2;
        keys.pk.vk.delta_g2 = after_delta_g2;
        keys.pvk = PreparedVerifyingKey::from(keys.vk.clone());

        let ctx = serialize_points::<E, _, _>(
            &[before_delta_g1, after_delta_g1, a1],
            &[before_delta_g2, after_delta_g2, a2],
            b"phase2-delta",
        );
        let c = hash_to_field::<E::ScalarField>(b"groth16.phase2.delta", &ctx);
        let s = k + c * r;

        let pok = SameExponentProof { a1, a2, s };
        Self {
            before_delta_g1,
            before_delta_g2,
            after_delta_g1,
            after_delta_g2,
            pok,
        }
    }

    pub fn verify(
        &self,
        before: &CircuitKeys<E>,
        after: &CircuitKeys<E>,
    ) -> Result<(), SetupError> {
        // 1) PoK
        let c_bytes = serialize_points::<E, _, _>(
            &[self.before_delta_g1, self.after_delta_g1, self.pok.a1],
            &[self.before_delta_g2, self.after_delta_g2, self.pok.a2],
            b"phase2-delta",
        );
        let c = hash_to_field::<E::ScalarField>(b"groth16.phase2.delta", &c_bytes);

        let lhs1 = (self.before_delta_g1 * self.pok.s).into_affine();
        let rhs1 = (self.pok.a1 + (self.after_delta_g1 * c)).into_affine();
        if lhs1 != rhs1 {
            return Err(SetupError::InvalidContribution("delta PoK (G1) failed"));
        }
        let lhs2 = (self.before_delta_g2 * self.pok.s).into_affine();
        let rhs2 = (self.pok.a2 + (self.after_delta_g2 * c)).into_affine();
        if lhs2 != rhs2 {
            return Err(SetupError::InvalidContribution("delta PoK (G2) failed"));
        }

        // 2) ratio checks on h_query and l_query
        if before.pk.h_query.len() != after.pk.h_query.len() {
            return Err(SetupError::InvalidContribution("h_query length mismatch"));
        }
        if before.pk.l_query.len() != after.pk.l_query.len() {
            return Err(SetupError::InvalidContribution("l_query length mismatch"));
        }
        for (hb, ha) in before.pk.h_query.iter().zip(after.pk.h_query.iter()) {
            if ark_ec::pairing::PairingOutput::<E>::from(E::pairing(*ha, after.vk.delta_g2))
                != E::pairing(*hb, before.vk.delta_g2)
            {
                return Err(SetupError::InvalidContribution(
                    "h_query ratio check failed",
                ));
            }
        }
        for (lb, la) in before.pk.l_query.iter().zip(after.pk.l_query.iter()) {
            if ark_ec::pairing::PairingOutput::<E>::from(E::pairing(*la, after.vk.delta_g2))
                != E::pairing(*lb, before.vk.delta_g2)
            {
                return Err(SetupError::InvalidContribution(
                    "l_query ratio check failed",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        ark_ff::UniformRand,
        ark_groth16::Groth16,
        ark_relations::r1cs::{ConstraintSynthesizer, LinearCombination, Variable},
        ark_snark::SNARK,
        rand::{rngs::StdRng, SeedableRng},
    };

    #[derive(Clone)]
    struct MulAddCircuit<F: PrimeField> {
        a: F,
        b: F,
        c: F,
        d: F,
    }
    impl<F: PrimeField> ConstraintSynthesizer<F> for MulAddCircuit<F> {
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

    type E = ark_bls12_381::Bls12_381;

    #[test]
    fn delta_update_roundtrip() {
        let mut rng = StdRng::seed_from_u64(42);
        let a = <E as Pairing>::ScalarField::rand(&mut rng);
        let b = <E as Pairing>::ScalarField::rand(&mut rng);
        let c = a * b;
        let d = a + b;

        let circuit = MulAddCircuit::<ark_bls12_381::Fr> { a, b, c, d };
        let (pk0, vk0) = Groth16::<E>::circuit_specific_setup(circuit.clone(), &mut rng).unwrap();
        let mut keys = CircuitKeys::new(pk0, vk0);

        // prove/verify before
        let pr0 = Groth16::<E>::prove(&keys.pk, circuit.clone(), &mut rng).unwrap();
        assert!(Groth16::<E>::verify(&keys.vk, &[c, d], &pr0).unwrap());

        // apply & verify contribution
        let before = keys.clone();
        let contrib = DeltaContribution::<E>::apply(&mut keys, &mut rng);
        contrib.verify(&before, &keys).unwrap();

        // prove/verify after
        let pr1 = Groth16::<E>::prove(&keys.pk, circuit, &mut rng).unwrap();
        assert!(Groth16::<E>::verify(&keys.vk, &[c, d], &pr1).unwrap());
    }
}
