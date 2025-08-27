use ark_ec::pairing::Pairing;

/// Type alias for the pairing engine used across the Groth16 module.
pub trait PairingEngine: Pairing + 'static {}
impl<T: Pairing + 'static> PairingEngine for T {}

pub type DefaultCurve = ark_bls12_381::Bls12_381;
