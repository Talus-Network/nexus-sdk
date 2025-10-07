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

/// A u64 field bound to a specific offset inside a byte blob.
#[derive(Clone)]
pub struct FieldAtOffset<F: PrimeField> {
    pub value: UInt64<F>,
    /// First of the 8 LE bytes for this u64.
    pub offset: usize,
}

impl<F: PrimeField> FieldAtOffset<F> {
    fn constrain_matches_bytes(&self, bytes: &[UInt8<F>]) -> Result<(), SynthesisError> {
        let start = self.offset;
        let end = start + 8;
        assert!(end <= bytes.len(), "u64 offset OOB");

        let le_bytes = self.value.to_bytes_le()?[..8].to_vec();
        for (bit, actual) in le_bytes.into_iter().zip(&bytes[start..end]) {
            bit.enforce_equal(actual)?;
        }
        Ok(())
    }
}

/// Witness for one (checkpoint, tx) tuple.
#[derive(Clone)]
pub struct CheckpointItemWitness<F: PrimeField> {
    pub summary_bytes: Vec<UInt8<F>>,
    pub contents_bytes: Vec<UInt8<F>>,
    pub effects_bytes: Vec<UInt8<F>>,
    pub content_digest_offset_in_summary: usize,
    pub tx_digest_offset_in_contents: usize,
    pub effects_digest_offset_in_contents: usize,
    pub gas_computation_at: FieldAtOffset<F>,
    pub gas_storage_at: FieldAtOffset<F>,
    pub gas_rebate_at: FieldAtOffset<F>,
}

/// Public inputs for one tuple.
#[derive(Clone, Debug)]
pub struct CheckpointItemPublic<F: PrimeField> {
    pub checkpoint_digest_limbs: Vec<F>,
    pub tx_digest_limbs: Vec<F>,
    pub claimed_total_gas_u64: u64,
    pub tolerance_bps_u16: u16,
}

/// Batch circuit tying checkpoint → contents → effects → gas.
pub struct CheckpointGasCircuit<F: PrimeField, D: Digest256Gadget<F>, const N: usize> {
    pub publics: [CheckpointItemPublic<F>; N],
    pub witnesses: [CheckpointItemWitness<F>; N],
    _pd: PhantomData<D>,
}

#[warn(dead_code)]
impl<F: PrimeField, D: Digest256Gadget<F>, const N: usize> CheckpointGasCircuit<F, D, N> {
    pub fn new(
        publics: [CheckpointItemPublic<F>; N],
        witnesses: [CheckpointItemWitness<F>; N],
    ) -> Self {
        Self {
            publics,
            witnesses,
            _pd: PhantomData,
        }
    }
}

impl<F: PrimeField, D: Digest256Gadget<F>, const N: usize> ConstraintSynthesizer<F>
    for CheckpointGasCircuit<F, D, N>
{
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        let basis = FpVar::<F>::constant(F::from(10_000u64));

        for i in 0..N {
            // Public inputs
            debug_assert_eq!(
                self.publics[i].checkpoint_digest_limbs.len(),
                32 / DIGEST_LIMB_BYTES
            );
            debug_assert_eq!(
                self.publics[i].tx_digest_limbs.len(),
                32 / DIGEST_LIMB_BYTES
            );

            let checkpoint_limbs = self.publics[i]
                .checkpoint_digest_limbs
                .iter()
                .map(|&x| FpVar::<F>::new_input(ark_relations::ns!(cs, "checkpoint_pi"), || Ok(x)))
                .collect::<Result<Vec<_>, _>>()?;
            let tx_limbs = self.publics[i]
                .tx_digest_limbs
                .iter()
                .map(|&x| FpVar::<F>::new_input(ark_relations::ns!(cs, "tx_pi"), || Ok(x)))
                .collect::<Result<Vec<_>, _>>()?;

            let claimed_total = UInt64::<F>::new_input(ark_relations::ns!(cs, "claimed"), || {
                Ok(self.publics[i].claimed_total_gas_u64)
            })?;
            let tol_bps = FpVar::<F>::new_input(ark_relations::ns!(cs, "tolerance"), || {
                Ok(F::from(self.publics[i].tolerance_bps_u16 as u64))
            })?;

            // Witness blobs
            let summary = self.witnesses[i].summary_bytes.clone();
            let contents = self.witnesses[i].contents_bytes.clone();
            let effects = self.witnesses[i].effects_bytes.clone();

            let summary_digest = D::hash(&summary)?;
            enforce_digest_eq_limbs::<F>(&summary_digest, &checkpoint_limbs)?;

            let contents_digest = D::hash(&contents)?;
            let summary_off = self.witnesses[i].content_digest_offset_in_summary;
            assert!(
                summary_off + 32 <= summary.len(),
                "content digest offset OOB"
            );
            for j in 0..32 {
                summary[summary_off + j].enforce_equal(&contents_digest[j])?;
            }

            let tx_off = self.witnesses[i].tx_digest_offset_in_contents;
            assert!(tx_off + 32 <= contents.len(), "tx digest offset OOB");
            let tx_bytes: [UInt8<F>; 32] = core::array::from_fn(|k| contents[tx_off + k].clone());
            enforce_digest_eq_limbs::<F>(&tx_bytes, &tx_limbs)?;

            let effects_off = self.witnesses[i].effects_digest_offset_in_contents;
            assert!(
                effects_off + 32 <= contents.len(),
                "effects digest offset OOB"
            );
            assert!(
                effects_off == tx_off + 32,
                "contents layout must store tx and effects digests consecutively"
            );
            let effects_digest_in_contents: [UInt8<F>; 32] =
                core::array::from_fn(|k| contents[effects_off + k].clone());
            let effects_digest = D::hash(&effects)?;
            for (lhs, rhs) in effects_digest_in_contents.iter().zip(effects_digest.iter()) {
                lhs.enforce_equal(rhs)?;
            }

            self.witnesses[i]
                .gas_computation_at
                .constrain_matches_bytes(&effects)?;
            self.witnesses[i]
                .gas_storage_at
                .constrain_matches_bytes(&effects)?;
            self.witnesses[i]
                .gas_rebate_at
                .constrain_matches_bytes(&effects)?;

            enforce_le_uint64(
                &self.witnesses[i].gas_rebate_at.value,
                &self.witnesses[i].gas_storage_at.value,
            )?;

            let comp_bits = self.witnesses[i].gas_computation_at.value.to_bits_le()?;
            let stor_bits = self.witnesses[i].gas_storage_at.value.to_bits_le()?;
            let comp_fe = pack_bits_le_to_fp(&comp_bits);
            let stor_fe = pack_bits_le_to_fp(&stor_bits);
            let sum_fe = comp_fe + stor_fe;

            let rebate_bits = self.witnesses[i].gas_rebate_at.value.to_bits_le()?;
            let rebate_fe = pack_bits_le_to_fp(&rebate_bits);
            let total_fe = sum_fe - rebate_fe;

            let total_bits = total_fe.to_bits_le()?;
            let total64 = pack_le_bits_to_uint64(&total_bits[..64])?;
            let total64_bits = total64.to_bits_le()?;
            let total64_fe = pack_bits_le_to_fp(&total64_bits);

            let claimed_bits = claimed_total.to_bits_le()?;
            let claimed_fe = pack_bits_le_to_fp(&claimed_bits);
            let window = &basis + &tol_bps;

            let lhs1 = total64_fe.clone() * basis.clone();
            let rhs1 = claimed_fe.clone() * window.clone();
            enforce_le_fe_bits(lhs1, rhs1, 80)?;

            let lhs2 = claimed_fe * basis.clone();
            let rhs2 = total64_fe * window;
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
        let a_less_b = not_a & &b;
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
    let mut padded = bits.to_vec();
    padded.resize(64, Boolean::FALSE);
    Ok(UInt64::from_bits_le(&padded))
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
    let mut idx = 0usize;
    for limb in limbs.iter() {
        let seg = &bytes32[idx..idx + DIGEST_LIMB_BYTES];
        let limb_from_bytes = pack_bytes_le_to_fp::<F>(seg);
        limb_from_bytes.enforce_equal(limb)?;
        idx += DIGEST_LIMB_BYTES;
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
        }
    }
    acc
}

impl<F: PrimeField> CheckpointItemWitness<F> {
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::type_complexity)]
    pub fn new(
        summary_bytes: Vec<u8>,
        contents_bytes: Vec<u8>,
        effects_bytes: Vec<u8>,
        content_digest_offset_in_summary: usize,
        tx_digest_offset_in_contents: usize,
        effects_digest_offset_in_contents: usize,
        gas_comp: u64,
        gas_storage: u64,
        gas_rebate: u64,
        comp_off: usize,
        storage_off: usize,
        rebate_off: usize,
        cs: ConstraintSystemRef<F>,
    ) -> Result<Self, SynthesisError> {
        let summary_bytes = summary_bytes
            .into_iter()
            .map(|b| UInt8::new_witness(ark_relations::ns!(cs, "summary_byte"), || Ok(b)))
            .collect::<Result<Vec<_>, _>>()?;
        let contents_bytes = contents_bytes
            .into_iter()
            .map(|b| UInt8::new_witness(ark_relations::ns!(cs, "contents_byte"), || Ok(b)))
            .collect::<Result<Vec<_>, _>>()?;
        let effects_bytes = effects_bytes
            .into_iter()
            .map(|b| UInt8::new_witness(ark_relations::ns!(cs, "effects_byte"), || Ok(b)))
            .collect::<Result<Vec<_>, _>>()?;

        let gas_computation_at = FieldAtOffset {
            value: UInt64::new_witness(ark_relations::ns!(cs, "gas_comp"), || Ok(gas_comp))?,
            offset: comp_off,
        };
        let gas_storage_at = FieldAtOffset {
            value: UInt64::new_witness(ark_relations::ns!(cs, "gas_storage"), || Ok(gas_storage))?,
            offset: storage_off,
        };
        let gas_rebate_at = FieldAtOffset {
            value: UInt64::new_witness(ark_relations::ns!(cs, "gas_rebate"), || Ok(gas_rebate))?,
            offset: rebate_off,
        };

        Ok(Self {
            summary_bytes,
            contents_bytes,
            effects_bytes,
            content_digest_offset_in_summary,
            tx_digest_offset_in_contents,
            effects_digest_offset_in_contents,
            gas_computation_at,
            gas_storage_at,
            gas_rebate_at,
        })
    }
}
