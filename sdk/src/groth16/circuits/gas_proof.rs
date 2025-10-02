use {
    ark_ff::PrimeField,
    ark_r1cs_std::{
        alloc::AllocVar,
        boolean::Boolean,
        eq::EqGadget,
        fields::fp::FpVar,
        prelude::*,
        uint64::UInt64,
        uint8::UInt8,
    },
    ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError},
    ark_std::{marker::PhantomData, vec::Vec},
};

/// Expose 32-byte digests as 2 field limbs of 16 bytes each.
pub const DIGEST_LIMB_BYTES: usize = 16;

/// Pluggable 256-bit digest gadget.
pub trait Digest256Gadget<F: PrimeField> {
    fn hash(bytes: &[UInt8<F>]) -> Result<[UInt8<F>; 32], SynthesisError>;
    const NAME: &'static str;
}

#[cfg(feature = "sha256-dev")]
pub mod sha256_dev {
    use {super::*, ark_r1cs_std::sha256::constraints::Sha256Gadget};

    pub struct Sha256Dev;
    impl<F: PrimeField> Digest256Gadget<F> for Sha256Dev {
        const NAME: &'static str = "sha256";

        fn hash(bytes: &[UInt8<F>]) -> Result<[UInt8<F>; 32], SynthesisError> {
            let out = Sha256Gadget::evaluate(bytes)?;
            let mut arr = [UInt8::constant(0u8); 32];
            for (i, b) in out.into_iter().enumerate() {
                arr[i] = b;
            }
            Ok(arr)
        }
    }
}

/// A u64 field bound to a specific offset inside a byte blob.
#[derive(Clone)]
pub struct FieldAtOffset<F: PrimeField> {
    pub value: UInt64<F>,
    /// First of the 8 LE bytes for this u64.
    pub offset: usize,
}

impl<F: PrimeField> FieldAtOffset<F> {
    fn constrain_matches_bytes(&self, effects_bytes: &[UInt8<F>]) -> Result<(), SynthesisError> {
        let v_bytes = self.value.to_bytes_le()?[..8].to_vec();
        let start = self.offset;
        let end = start + 8;
        assert!(end <= effects_bytes.len(), "u64 offset OOB");
        let slice = &effects_bytes[start..end];
        for (vb, eb) in v_bytes.into_iter().zip(slice.iter()) {
            vb.enforce_equal(eb)?;
        }
        Ok(())
    }
}

/// One transaction item.
#[derive(Clone)]
pub struct EffectsItemWitness<F: PrimeField> {
    pub effects_bytes: Vec<UInt8<F>>,
    pub tx_digest_bytes: [UInt8<F>; 32],
    pub gas_comp_at: FieldAtOffset<F>,
    pub gas_storage_at: FieldAtOffset<F>,
    pub gas_rebate_at: FieldAtOffset<F>,
    pub tx_digest_offset: usize,
}

/// Public inputs for one item.
#[derive(Clone, Debug)]
pub struct EffectsItemPublic<F: PrimeField> {
    pub tx_digest_limbs: Vec<F>,      // 2 limbs × 16 bytes
    pub effects_digest_limbs: Vec<F>, // 2 limbs × 16 bytes
    pub claimed_total_gas_u64: u64,
    pub tolerance_bps_u16: u16, // 0..=10_000
}

/// Batch circuit.
pub struct EffectsCircuit<F: PrimeField, D: Digest256Gadget<F>, const N: usize> {
    pub publics: [EffectsItemPublic<F>; N],
    pub witnesses: [EffectsItemWitness<F>; N],
    _pd: PhantomData<D>,
}

impl<F: PrimeField, D: Digest256Gadget<F>, const N: usize> EffectsCircuit<F, D, N> {
    pub fn new(publics: [EffectsItemPublic<F>; N], witnesses: [EffectsItemWitness<F>; N]) -> Self {
        Self {
            publics,
            witnesses,
            _pd: PhantomData,
        }
    }
}

impl<F: PrimeField, D: Digest256Gadget<F>, const N: usize> ConstraintSynthesizer<F>
    for EffectsCircuit<F, D, N>
{
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        let ten_thousand = F::from(10_000u64);

        for i in 0..N {
            // public inputs
            let pub_tx_limbs = self.publics[i]
                .tx_digest_limbs
                .iter()
                .map(|&x| FpVar::<F>::new_input(ark_relations::ns!(cs, "tx_limb"), || Ok(x)))
                .collect::<Result<Vec<_>, _>>()?;

            let pub_eff_limbs = self.publics[i]
                .effects_digest_limbs
                .iter()
                .map(|&x| FpVar::<F>::new_input(ark_relations::ns!(cs, "eff_limb"), || Ok(x)))
                .collect::<Result<Vec<_>, _>>()?;

            let claimed_total =
                UInt64::<F>::new_input(ark_relations::ns!(cs, "claimed_total"), || {
                    Ok(self.publics[i].claimed_total_gas_u64)
                })?;

            let tol_bps_fe = FpVar::<F>::new_input(ark_relations::ns!(cs, "tol_bps"), || {
                Ok(F::from(self.publics[i].tolerance_bps_u16 as u64))
            })?;

            // witnesses
            let e_bytes = self.witnesses[i].effects_bytes.clone();

            // effects hash == public limbs
            let h = D::hash(&e_bytes)?; // 32 bytes
            enforce_digest_eq_limbs::<F>(&h, &pub_eff_limbs)?;

            // bind tx_digest bytes inside effects bytes at offset
            let tx_b = self.witnesses[i].tx_digest_bytes.clone();
            let off = self.witnesses[i].tx_digest_offset;
            assert!(off + 32 <= e_bytes.len(), "tx_digest offset OOB");
            for j in 0..32 {
                tx_b[j].enforce_equal(&e_bytes[off + j])?;
            }
            // and match public tx_digest limbs
            enforce_digest_eq_limbs::<F>(&tx_b, &pub_tx_limbs)?;

            // gas fields: byte binding at offsets
            self.witnesses[i]
                .gas_comp_at
                .constrain_matches_bytes(&e_bytes)?;
            self.witnesses[i]
                .gas_storage_at
                .constrain_matches_bytes(&e_bytes)?;
            self.witnesses[i]
                .gas_rebate_at
                .constrain_matches_bytes(&e_bytes)?;

            // rebate <= storage
            enforce_le_uint64(
                &self.witnesses[i].gas_rebate_at.value,
                &self.witnesses[i].gas_storage_at.value,
            )?;

            // total = comp + storage - rebate
            let comp = &self.witnesses[i].gas_comp_at.value;
            let stor = &self.witnesses[i].gas_storage_at.value;
            let reb = &self.witnesses[i].gas_rebate_at.value;

            let comp_bits = comp.to_bits_le()?;
            let stor_bits = stor.to_bits_le()?;
            let comp_fe = pack_bits_le_to_fp(&comp_bits);
            let stor_fe = pack_bits_le_to_fp(&stor_bits);
            let sum_fe: FpVar<F> = comp_fe + stor_fe;
            let reb_bits = reb.to_bits_le()?;
            let reb_fe = pack_bits_le_to_fp(&reb_bits);
            let total_fe: FpVar<F> = sum_fe - reb_fe;
            let total_bits = total_fe.to_bits_le()?; // booleanized
            let total64 = pack_le_bits_to_uint64(&total_bits[..64])?;

            // symmetric ±tolerance in basis points (no division)
            // 10_000·total ≤ (10_000 + tol)·claimed
            let total64_bits = total64.to_bits_le()?;
            let total64_fe = pack_bits_le_to_fp(&total64_bits);
            let lhs1 = total64_fe.clone() * ten_thousand;
            let claimed_bits = claimed_total.to_bits_le()?;
            let claimed_fe = pack_bits_le_to_fp(&claimed_bits);
            let rhs1 = claimed_fe.clone() * (&FpVar::<F>::constant(ten_thousand) + &tol_bps_fe);
            enforce_le_fe_bits(lhs1, rhs1, 80)?; // values fit comfortably < 2^80

            // 10_000·claimed ≤ (10_000 + tol)·total
            let lhs2 = claimed_fe * ten_thousand;
            let rhs2 = total64_fe * (&FpVar::<F>::constant(ten_thousand) + &tol_bps_fe);
            enforce_le_fe_bits(lhs2, rhs2, 80)?;
        }
        Ok(())
    }
}

/// a <= b for UInt64 via bit-lex compare (no slack witnesses).
pub(super) fn enforce_le_uint64<F: PrimeField>(
    a: &UInt64<F>,
    b: &UInt64<F>,
) -> Result<(), SynthesisError> {
    let a_bits = a.to_bits_le()?;
    let b_bits = b.to_bits_le()?;
    enforce_le_bits(&a_bits, &b_bits)
}

/// lhs <= rhs, where both are < 2^bit_len (range via booleanization).
fn enforce_le_fe_bits<F: PrimeField>(
    lhs: FpVar<F>,
    rhs: FpVar<F>,
    bit_len: usize,
) -> Result<(), SynthesisError> {
    let mut lhs_bits = lhs.to_bits_le()?;
    let mut rhs_bits = rhs.to_bits_le()?;
    lhs_bits.truncate(bit_len);
    rhs_bits.truncate(bit_len);
    enforce_le_bits(&lhs_bits, &rhs_bits)
}

/// Lexicographic compare over big-endian order (we get LE bits, so iterate reversed).
fn enforce_le_bits<F: PrimeField>(
    a_le: &[Boolean<F>],
    b_le: &[Boolean<F>],
) -> Result<(), SynthesisError> {
    let n = core::cmp::max(a_le.len(), b_le.len());
    let mut lt = Boolean::constant(false);
    let mut eq = Boolean::constant(true);

    for idx in (0..n).rev() {
        let a = a_le.get(idx).cloned().unwrap_or(Boolean::FALSE);
        let b = b_le.get(idx).cloned().unwrap_or(Boolean::FALSE);

        let not_a = !&a;
        let a_less_b = not_a & &b; // (!a) & b
        let a_less_b_and_eq = a_less_b & &eq;
        lt = lt | a_less_b_and_eq;

        let xnor = !(&a ^ &b);
        eq = eq & xnor;
    }
    let le = lt | eq;
    le.enforce_equal(&Boolean::TRUE)
}

/// Pack little-endian bits (<= 64) into a `UInt64`.
fn pack_le_bits_to_uint64<F: PrimeField>(bits: &[Boolean<F>]) -> Result<UInt64<F>, SynthesisError> {
    assert!(bits.len() <= 64);
    let mut bytes = Vec::with_capacity(8);
    for i in 0..8 {
        let start = i * 8;
        let b = if start + 8 <= bits.len() {
            UInt8::from_bits_le(&bits[start..start + 8])
        } else {
            let mut seg = bits[start..].to_vec();
            for _ in 0..(start + 8 - bits.len()) {
                seg.push(Boolean::FALSE);
            }
            UInt8::from_bits_le(&seg)
        };
        bytes.push(b);
    }
    Ok(UInt64::from_bits_le(
        &bytes
            .into_iter()
            .flat_map(|b| b.to_bits_le().unwrap())
            .collect::<Vec<_>>(),
    ))
}

/// Pack little-endian bits into an `FpVar`.
pub(super) fn pack_bits_le_to_fp<F: PrimeField>(bits: &[Boolean<F>]) -> FpVar<F> {
    let mut acc = FpVar::<F>::zero();
    let mut coeff = F::from(1u64);
    for b in bits.iter() {
        acc += &FpVar::<F>::from(b.clone()) * coeff;
        coeff.double_in_place();
    }
    acc
}

/// Enforce that 32 bytes equal the digest limbs exposed as field elements.
pub(super) fn enforce_digest_eq_limbs<F: PrimeField>(
    bytes32: &[UInt8<F>; 32],
    limbs: &[FpVar<F>],
) -> Result<(), SynthesisError> {
    debug_assert_eq!(limbs.len(), 32 / DIGEST_LIMB_BYTES);
    let mut i = 0usize;
    for limb in limbs.iter() {
        let seg = &bytes32[i..i + DIGEST_LIMB_BYTES];
        let limb_from_bytes = pack_bytes_le_to_fp::<F>(seg);
        limb_from_bytes.enforce_equal(limb)?;
        i += DIGEST_LIMB_BYTES;
    }
    Ok(())
}

pub(super) fn pack_bytes_le_to_fp<F: PrimeField>(bytes: &[UInt8<F>]) -> FpVar<F> {
    let mut acc = FpVar::<F>::zero();
    let mut coeff = F::from(1u64);
    for b in bytes.iter() {
        let b_bits = b.to_bits_le().unwrap();
        let b_fe = pack_bits_le_to_fp(&b_bits);
        acc += &b_fe * coeff;
        for _ in 0..8 {
            coeff.double_in_place();
        } // ×256 each step
    }
    acc
}

// Witness constructor
impl<F: PrimeField> EffectsItemWitness<F> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        effects_bytes: Vec<u8>,
        tx_digest_bytes: [u8; 32],
        gas_comp: u64,
        gas_storage: u64,
        gas_rebate: u64,
        tx_digest_offset: usize,
        comp_off: usize,
        stor_off: usize,
        reb_off: usize,
        cs: ConstraintSystemRef<F>,
    ) -> Result<Self, SynthesisError> {
        let effects_bytes = effects_bytes
            .into_iter()
            .map(|b| UInt8::new_witness(ark_relations::ns!(cs, "effect_byte"), || Ok(b)))
            .collect::<Result<Vec<_>, _>>()?;
        let tx_b: [UInt8<F>; 32] = core::array::from_fn(|i| {
            UInt8::new_witness(ark_relations::ns!(cs, "tx_digest_byte"), || {
                Ok(tx_digest_bytes[i])
            })
            .unwrap()
        });
        let gas_comp_at = FieldAtOffset {
            value: UInt64::new_witness(ark_relations::ns!(cs, "gas_comp"), || Ok(gas_comp))?,
            offset: comp_off,
        };
        let gas_storage_at = FieldAtOffset {
            value: UInt64::new_witness(ark_relations::ns!(cs, "gas_storage"), || Ok(gas_storage))?,
            offset: stor_off,
        };
        let gas_rebate_at = FieldAtOffset {
            value: UInt64::new_witness(ark_relations::ns!(cs, "gas_rebate"), || Ok(gas_rebate))?,
            offset: reb_off,
        };
        Ok(Self {
            effects_bytes,
            tx_digest_bytes: tx_b,
            gas_comp_at,
            gas_storage_at,
            gas_rebate_at,
            tx_digest_offset,
        })
    }
}

/// Utility: pack 32 digest bytes into 2 field limbs of 16 bytes each (LE).
pub fn pack_digest_to_limbs<F: PrimeField>(digest: [u8; 32]) -> Vec<F> {
    let mut out = Vec::with_capacity(32 / DIGEST_LIMB_BYTES);
    for i in 0..(32 / DIGEST_LIMB_BYTES) {
        let chunk = &digest[i * DIGEST_LIMB_BYTES..(i + 1) * DIGEST_LIMB_BYTES];
        let mut acc = F::from(0u64);
        let mut coeff = F::from(1u64);
        for b in chunk.iter() {
            acc += coeff * F::from(*b as u64);
            for _ in 0..8 {
                coeff.double_in_place();
            } // advance multiplier by ×256
        }
        out.push(acc);
    }
    out
}

/// Convenience constructor for N=1.
#[allow(dead_code)]
pub fn single_item_circuit<F: PrimeField, D: Digest256Gadget<F>>(
    public: EffectsItemPublic<F>,
    witness: EffectsItemWitness<F>,
) -> EffectsCircuit<F, D, 1> {
    EffectsCircuit::new([public], [witness])
}
