use {
    super::gas_proof::{
        enforce_digest_eq_limbs,
        enforce_le_uint64,
        pack_bits_le_to_fp,
        pack_bytes_le_to_fp,
        Digest256Gadget,
        DIGEST_LIMB_BYTES,
    },
    ark_ff::PrimeField,
    ark_r1cs_std::{
        alloc::AllocVar,
        boolean::Boolean,
        eq::EqGadget,
        fields::{fp::FpVar, FieldVar},
        prelude::*,
        uint64::UInt64,
        uint8::UInt8,
    },
    ark_relations::r1cs::{ConstraintSystemRef, SynthesisError},
    ark_std::vec::Vec,
};

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
