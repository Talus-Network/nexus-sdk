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

// Less-or-equal comparators
/// a <= b for UInt64 via bit-lex compare (no slack witnesses).
fn enforce_le_uint64<F: PrimeField>(a: &UInt64<F>, b: &UInt64<F>) -> Result<(), SynthesisError> {
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
fn pack_bits_le_to_fp<F: PrimeField>(bits: &[Boolean<F>]) -> FpVar<F> {
    let mut acc = FpVar::<F>::zero();
    let mut coeff = F::from(1u64);
    for b in bits.iter() {
        acc += &FpVar::<F>::from(b.clone()) * coeff;
        coeff.double_in_place();
    }
    acc
}

/// Enforce that 32 bytes equal the digest limbs exposed as field elements.
fn enforce_digest_eq_limbs<F: PrimeField>(
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

fn pack_bytes_le_to_fp<F: PrimeField>(bytes: &[UInt8<F>]) -> FpVar<F> {
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
            .enumerate()
            .map(|(i, b)| UInt8::new_witness(ark_relations::ns!(cs, "effect_byte"), || Ok(b)))
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
    EffectsCircuit {
        publics: [public],
        witnesses: [witness],
        _pd: PhantomData,
    }
}

#[derive(Clone, Debug)]
pub struct TxPolicyPublic<F: PrimeField> {
    pub tx_digest_limbs: Vec<F>,
    pub allowed_cmd_tags: Vec<F>,
    pub move_call_tag: F,
    pub allowed_pkg_limbs: Vec<[F; 32 / DIGEST_LIMB_BYTES]>,
    pub allowed_target_hash_limbs: Vec<[F; 32 / DIGEST_LIMB_BYTES]>,
    pub max_cmds: usize,
    pub max_id_len: usize,
}

#[derive(Clone)]
pub struct TxPolicyWitness<F: PrimeField> {
    pub tx_bytes: Vec<UInt8<F>>,
    pub tag_offsets: Vec<usize>,
    pub pkg_offsets: Vec<usize>,
    pub mod_offsets: Vec<usize>,
    pub mod_lens: Vec<u32>,
    pub fun_offsets: Vec<usize>,
    pub fun_lens: Vec<u32>,
    pub cmd_len: u32,
}

pub fn enforce_tx_policy<F: PrimeField, D: Digest256Gadget<F>>(
    cs: &ConstraintSystemRef<F>,
    pubcfg: &TxPolicyPublic<F>,
    wit: &TxPolicyWitness<F>,
) -> Result<(), SynthesisError> {
    // tx_bytes → tx_digest
    let h = D::hash(&wit.tx_bytes)?;
    let pub_tx_limbs = pubcfg
        .tx_digest_limbs
        .iter()
        .map(|&x| FpVar::<F>::new_input(ark_relations::ns!(cs, "tx_pi"), || Ok(x)))
        .collect::<Result<Vec<_>, _>>()?;
    enforce_digest_eq_limbs::<F>(&h, &pub_tx_limbs)?;

    // cmd_len ≤ max_cmds and Σ present = cmd_len
    let cmd_len_var =
        UInt64::<F>::new_witness(ark_relations::ns!(cs, "cmd_len"), || Ok(wit.cmd_len as u64))?;
    enforce_le_uint64(&cmd_len_var, &UInt64::constant(pubcfg.max_cmds as u64))?;

    let present_flags = (0..pubcfg.max_cmds)
        .map(|j| {
            Boolean::new_witness(ark_relations::ns!(cs, "present_flag"), || {
                Ok((j as u32) < wit.cmd_len)
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut sum_present = FpVar::<F>::zero();
    for p in present_flags.iter() {
        sum_present += &FpVar::<F>::from(p.clone());
    }
    let cmd_len_bits = cmd_len_var.to_bits_le()?;
    let cmd_len_fe = pack_bits_le_to_fp(&cmd_len_bits);
    sum_present.enforce_equal(&cmd_len_fe)?;

    // allowed sets
    let allowed_tags_vars = pubcfg
        .allowed_cmd_tags
        .iter()
        .map(|&t| FpVar::<F>::new_input(ark_relations::ns!(cs, "allowed_tag"), || Ok(t)))
        .collect::<Result<Vec<_>, _>>()?;
    let move_call_tag = FpVar::<F>::new_input(ark_relations::ns!(cs, "move_call_tag"), || {
        Ok(pubcfg.move_call_tag)
    })?;
    let allowed_pkg_vars: Vec<Vec<FpVar<F>>> = pubcfg
        .allowed_pkg_limbs
        .iter()
        .map(|limbs| {
            limbs
                .iter()
                .map(|&x| FpVar::<F>::new_input(ark_relations::ns!(cs, "pkg_limb"), || Ok(x)))
                .collect()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let allowed_target_vars: Vec<Vec<FpVar<F>>> = pubcfg
        .allowed_target_hash_limbs
        .iter()
        .map(|limbs| {
            limbs
                .iter()
                .map(|&x| FpVar::<F>::new_input(ark_relations::ns!(cs, "target_limb"), || Ok(x)))
                .collect()
        })
        .collect::<Result<Vec<_>, _>>()?;

    for j in 0..pubcfg.max_cmds {
        // Tag
        let tag_off = *wit.tag_offsets.get(j).unwrap_or(&0usize);
        if tag_off >= wit.tx_bytes.len() {
            continue;
        }
        let tag_bits = wit.tx_bytes[tag_off].to_bits_le()?;
        let tag_fe = pack_bits_le_to_fp(&tag_bits);

        // tag ∈ allowed set under present mask
        let mut prod = FpVar::<F>::one();
        for a in allowed_tags_vars.iter() {
            prod *= &(&tag_fe - a);
        }
        let present_fe: FpVar<F> = present_flags[j].clone().into();
        ((FpVar::<F>::one() - present_fe.clone()) * &prod).enforce_equal(&FpVar::<F>::zero())?;

        // MoveCall?
        let is_mc = eq_fe_as_bool(tag_fe.clone(), move_call_tag.clone())?;
        let must_check_mc = present_flags[j].clone() & is_mc;

        // Package allow-list
        let pkg_off = *wit.pkg_offsets.get(j).unwrap_or(&0usize);
        let pkg_bytes: [UInt8<F>; 32] = core::array::from_fn(|k| {
            if pkg_off + k < wit.tx_bytes.len() {
                wit.tx_bytes[pkg_off + k].clone()
            } else {
                UInt8::constant(0u8)
            }
        });
        if !allowed_pkg_vars.is_empty() {
            let pkg_limb0 = pack_bytes_le_to_fp::<F>(&pkg_bytes[0..DIGEST_LIMB_BYTES]);
            let pkg_limb1 =
                pack_bytes_le_to_fp::<F>(&pkg_bytes[DIGEST_LIMB_BYTES..2 * DIGEST_LIMB_BYTES]);
            let mut prod_pkgs = FpVar::<F>::one();
            for limbs in allowed_pkg_vars.iter() {
                let diff = (pkg_limb0.clone() - limbs[0].clone()).square()?
                    + (pkg_limb1.clone() - limbs[1].clone()).square()?;
                prod_pkgs *= diff;
            }
            let mask: FpVar<F> = must_check_mc.clone().into();
            ((FpVar::<F>::one() - mask) * &prod_pkgs).enforce_equal(&FpVar::<F>::zero())?;
        }

        // Package::Module::Function allow-list
        if !allowed_target_vars.is_empty() {
            let mod_off = *wit.mod_offsets.get(j).unwrap_or(&0usize);
            let fun_off = *wit.fun_offsets.get(j).unwrap_or(&0usize);
            let mlen = *wit.mod_lens.get(j).unwrap_or(&0u32) as usize;
            let flen = *wit.fun_lens.get(j).unwrap_or(&0u32) as usize;

            // BCS length words (4 bytes LE) just before the strings
            let mod_len_bytes = take_le4(&wit.tx_bytes, mod_off.saturating_sub(4));
            let fun_len_bytes = take_le4(&wit.tx_bytes, fun_off.saturating_sub(4));
            let mlen_fe = FpVar::<F>::new_witness(ark_relations::ns!(cs, "mod_len"), || {
                Ok(F::from(mlen as u64))
            })?;
            let flen_fe = FpVar::<F>::new_witness(ark_relations::ns!(cs, "fun_len"), || {
                Ok(F::from(flen as u64))
            })?;
            pack_bytes_le_to_fp::<F>(&mod_len_bytes).enforce_equal(&mlen_fe)?;
            pack_bytes_le_to_fp::<F>(&fun_len_bytes).enforce_equal(&flen_fe)?;

            // Build padded module/function strings
            let mut mod_pad: Vec<UInt8<F>> = Vec::with_capacity(pubcfg.max_id_len);
            for k in 0..pubcfg.max_id_len {
                let b = if k < mlen && (mod_off + k) < wit.tx_bytes.len() {
                    wit.tx_bytes[mod_off + k].clone()
                } else {
                    UInt8::constant(0u8)
                };
                mod_pad.push(b);
            }
            let mut fun_pad: Vec<UInt8<F>> = Vec::with_capacity(pubcfg.max_id_len);
            for k in 0..pubcfg.max_id_len {
                let b = if k < flen && (fun_off + k) < wit.tx_bytes.len() {
                    wit.tx_bytes[fun_off + k].clone()
                } else {
                    UInt8::constant(0u8)
                };
                fun_pad.push(b);
            }

            // to_hash = pkg || 0x00 || le32(mlen) || mod_pad || 0x01 || le32(flen) || fun_pad
            let mut to_hash: Vec<UInt8<F>> =
                Vec::with_capacity(32 + 1 + 4 + pubcfg.max_id_len + 1 + 4 + pubcfg.max_id_len);
            to_hash.extend_from_slice(&pkg_bytes);
            to_hash.push(UInt8::constant(0u8));
            to_hash.extend_from_slice(&mod_len_bytes);
            to_hash.extend(mod_pad.iter().cloned());
            to_hash.push(UInt8::constant(1u8));
            to_hash.extend_from_slice(&fun_len_bytes);
            to_hash.extend(fun_pad.iter().cloned());

            let tgt_hash = D::hash(&to_hash)?; // 32 bytes
            let tgt_limb0 = pack_bytes_le_to_fp::<F>(&tgt_hash[0..DIGEST_LIMB_BYTES]);
            let tgt_limb1 =
                pack_bytes_le_to_fp::<F>(&tgt_hash[DIGEST_LIMB_BYTES..2 * DIGEST_LIMB_BYTES]);

            let mut prod_targets = FpVar::<F>::one();
            for limbs in allowed_target_vars.iter() {
                let diff = (tgt_limb0.clone() - limbs[0].clone()).square()?
                    + (tgt_limb1.clone() - limbs[1].clone()).square()?;
                prod_targets *= diff;
            }
            let mask: FpVar<F> = must_check_mc.into();
            ((FpVar::<F>::one() - mask) * &prod_targets).enforce_equal(&FpVar::<F>::zero())?;
        }
    }
    Ok(())
}

fn take_le4<F: PrimeField>(bytes: &[UInt8<F>], off: usize) -> [UInt8<F>; 4] {
    [0, 1, 2, 3].map(|i| {
        if off + i < bytes.len() {
            bytes[off + i].clone()
        } else {
            UInt8::constant(0u8)
        }
    })
}

/// Helper to check if two field elements are equal and return as Boolean
fn eq_fe_as_bool<F: PrimeField>(a: FpVar<F>, b: FpVar<F>) -> Result<Boolean<F>, SynthesisError> {
    let diff = &a - &b;
    let is_zero = diff.is_zero()?;
    Ok(is_zero)
}

/// Composite: Effects + Tx Policy
pub struct EffectsAndTxCircuit<F: PrimeField, D: Digest256Gadget<F>, const N: usize> {
    pub effects_publics: [EffectsItemPublic<F>; N],
    pub effects_witnesses: [EffectsItemWitness<F>; N],
    pub tx_pub: Option<TxPolicyPublic<F>>, // None => skip tx policy
    pub tx_wit: Option<TxPolicyWitness<F>>,
    _pd: PhantomData<D>,
}

impl<F: PrimeField, D: Digest256Gadget<F>, const N: usize> ConstraintSynthesizer<F>
    for EffectsAndTxCircuit<F, D, N>
{
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        let eff = EffectsCircuit::<F, D, N> {
            publics: self.effects_publics,
            witnesses: self.effects_witnesses,
            _pd: PhantomData,
        };
        eff.generate_constraints(cs.clone())?;

        if let (Some(pubp), Some(witp)) = (self.tx_pub, self.tx_wit) {
            enforce_tx_policy::<F, D>(&cs, &pubp, &witp)?;
        }
        Ok(())
    }
}
