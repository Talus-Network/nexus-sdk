use {ark_ff::PrimeField, ark_relations::r1cs::ConstraintSynthesizer};

/// A thin wrapper for any generic Groth16 circuit.
pub struct AnyCircuit<F: PrimeField> {
    pub circuit: Box<dyn ConstraintSynthesizer<F>>,
    pub public_inputs: Vec<F>,
}
