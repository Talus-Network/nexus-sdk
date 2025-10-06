#![allow(dead_code)]

use {
    super::gas::{
        enforce_digest_eq_limbs,
        enforce_le_uint64,
        pack_bits_le_to_fp,
        pack_bytes_le_to_fp,
        Digest256Gadget,
        DIGEST_LIMB_BYTES,
    },
    ark_ff::PrimeField,
    ark_r1cs_std::{
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

const COMMAND_MOVE_CALL: u8 = 0;
const COMMAND_TRANSFER_OBJECTS: u8 = 1;
const COMMAND_SPLIT_COIN: u8 = 2;
const COMMAND_MERGE_COINS: u8 = 3;
const COMMAND_PUBLISH: u8 = 4;
const COMMAND_MAKE_MOVE_VEC: u8 = 5;
const COMMAND_UPGRADE: u8 = 6;

const MOVE_CALL_LITERAL: &[u8] = b"MoveCall";
const TRANSFER_OBJECTS_LITERAL: &[u8] = b"TransferObjects";
const SPLIT_COIN_LITERAL: &[u8] = b"SplitCoin";
const MERGE_COINS_LITERAL: &[u8] = b"MergeCoins";
const PUBLISH_LITERAL: &[u8] = b"Publish";
const MAKE_MOVE_VEC_LITERAL: &[u8] = b"MakeMoveVec";
const UPGRADE_LITERAL: &[u8] = b"Upgrade";

const DIGEST_LIMBS: usize = 32 / DIGEST_LIMB_BYTES;
const LENGTH_PREFIX_BYTES: usize = 4;
const MOVE_CALL_PACKAGE_BYTES: usize = 32;

#[derive(Clone, Debug)]
pub struct TxPolicyPublic<F: PrimeField> {
    pub tx_digest_limbs: [F; DIGEST_LIMBS],
    pub dfa_hash_limbs: [F; DIGEST_LIMBS],
    pub max_actions: usize,
    pub max_symbol_bytes: usize,
    pub max_states: usize,
    pub max_transitions_per_state: usize,
    pub max_id_len: usize,
}

#[derive(Clone)]
pub struct TxPolicyWitness<F: PrimeField> {
    pub tx_bytes: Vec<UInt8<F>>,
    pub command_tag_offsets: Vec<usize>,
    pub move_call_pkg_offsets: Vec<usize>,
    pub move_call_module_offsets: Vec<usize>,
    pub move_call_module_lengths: Vec<u32>,
    pub move_call_function_offsets: Vec<usize>,
    pub move_call_function_lengths: Vec<u32>,
    pub action_count: u32,
    pub dfa: DfaWitness<F>,
}

#[derive(Clone)]
pub struct DfaWitness<F: PrimeField> {
    pub start_state: usize,
    pub states: Vec<DfaStateWitness<F>>,
}

#[derive(Clone)]
pub struct DfaStateWitness<F: PrimeField> {
    pub is_accepting: bool,
    pub transitions: Vec<DfaTransitionWitness<F>>,
}

#[derive(Clone)]
pub struct DfaTransitionWitness<F: PrimeField> {
    pub target: usize,
    pub symbol: Vec<UInt8<F>>,
}

struct DfaTransitionVar<F: PrimeField> {
    target: FpVar<F>,
    symbol_limbs: [FpVar<F>; DIGEST_LIMBS],
}

pub fn enforce_tx_policy<F: PrimeField, D: Digest256Gadget<F>>(
    cs: &ConstraintSystemRef<F>,
    pubcfg: &TxPolicyPublic<F>,
    wit: &TxPolicyWitness<F>,
) -> Result<(), SynthesisError> {
    let action_limit = pubcfg.max_actions;
    assert_eq!(wit.command_tag_offsets.len(), action_limit);
    assert_eq!(wit.move_call_pkg_offsets.len(), action_limit);
    assert_eq!(wit.move_call_module_offsets.len(), action_limit);
    assert_eq!(wit.move_call_module_lengths.len(), action_limit);
    assert_eq!(wit.move_call_function_offsets.len(), action_limit);
    assert_eq!(wit.move_call_function_lengths.len(), action_limit);
    assert!(
        pubcfg.max_symbol_bytes > 0,
        "symbol budget must be positive"
    );
    assert!(!wit.tx_bytes.is_empty(), "transaction bytes required");

    // Public inputs: transaction digest and DFA hash.
    let tx_pi_limbs = pubcfg
        .tx_digest_limbs
        .iter()
        .map(|&x| FpVar::<F>::new_input(ark_relations::ns!(cs, "tx_digest"), || Ok(x)))
        .collect::<Result<Vec<_>, _>>()?;
    let dfa_pi_limbs = pubcfg
        .dfa_hash_limbs
        .iter()
        .map(|&x| FpVar::<F>::new_input(ark_relations::ns!(cs, "dfa_hash"), || Ok(x)))
        .collect::<Result<Vec<_>, _>>()?;

    // Bind transaction bytes to its digest.
    let tx_digest = D::hash(&wit.tx_bytes)?;
    enforce_digest_eq_limbs::<F>(&tx_digest, &tx_pi_limbs)?;

    // Action multiplicity witness: number of commands actually present.
    let action_bound = UInt64::<F>::constant(pubcfg.max_actions as u64);
    let action_count = UInt64::<F>::new_witness(ark_relations::ns!(cs, "action_count"), || {
        Ok(wit.action_count as u64)
    })?;
    enforce_le_uint64(&action_count, &action_bound)?;

    let mut present_flags = Vec::with_capacity(action_limit);
    for j in 0..action_limit {
        present_flags.push(Boolean::new_witness(
            ark_relations::ns!(cs, "present"),
            || Ok((j as u32) < wit.action_count),
        )?);
    }

    let mut present_sum = FpVar::<F>::zero();
    for flag in &present_flags {
        present_sum += &FpVar::<F>::from(flag.clone());
    }
    let count_bits = action_count.to_bits_le()?;
    let count_fe = pack_bits_le_to_fp(&count_bits);
    present_sum.enforce_equal(&count_fe)?;

    // Pre-compute length-prefixed encodings for command variants whose payload is static.
    let symbol_layout = LENGTH_PREFIX_BYTES + pubcfg.max_symbol_bytes;
    let split_symbol = constant_symbol::<F>(SPLIT_COIN_LITERAL, pubcfg.max_symbol_bytes);
    let merge_symbol = constant_symbol::<F>(MERGE_COINS_LITERAL, pubcfg.max_symbol_bytes);
    let publish_symbol = constant_symbol::<F>(PUBLISH_LITERAL, pubcfg.max_symbol_bytes);
    let transfer_symbol = constant_symbol::<F>(TRANSFER_OBJECTS_LITERAL, pubcfg.max_symbol_bytes);
    let make_move_vec_symbol = constant_symbol::<F>(MAKE_MOVE_VEC_LITERAL, pubcfg.max_symbol_bytes);
    let upgrade_symbol = constant_symbol::<F>(UPGRADE_LITERAL, pubcfg.max_symbol_bytes);

    // Extract per-action symbols and digest limbs.
    let mut action_symbol_limbs: Vec<[FpVar<F>; DIGEST_LIMBS]> = Vec::with_capacity(action_limit);
    for j in 0..action_limit {
        let tag_offset = wit.command_tag_offsets[j];
        assert!(tag_offset < wit.tx_bytes.len(), "command tag offset OOB");
        let tag_byte = wit.tx_bytes[tag_offset].clone();
        let tag_bits = tag_byte.to_bits_le()?;
        let tag_fe = pack_bits_le_to_fp(&tag_bits);

        let is_move_call = tag_is::<F>(&tag_fe, COMMAND_MOVE_CALL)?;
        let is_transfer = tag_is::<F>(&tag_fe, COMMAND_TRANSFER_OBJECTS)?;
        let is_split = tag_is::<F>(&tag_fe, COMMAND_SPLIT_COIN)?;
        let is_merge = tag_is::<F>(&tag_fe, COMMAND_MERGE_COINS)?;
        let is_publish = tag_is::<F>(&tag_fe, COMMAND_PUBLISH)?;
        let is_make_move_vec = tag_is::<F>(&tag_fe, COMMAND_MAKE_MOVE_VEC)?;
        let is_upgrade = tag_is::<F>(&tag_fe, COMMAND_UPGRADE)?;

        // Every PTB command must materialise as one symbol tailored to the DFA alphabet.
        let recognized = is_move_call.clone()
            | is_transfer.clone()
            | is_split.clone()
            | is_merge.clone()
            | is_publish.clone()
            | is_make_move_vec.clone()
            | is_upgrade.clone();
        let invalid = present_flags[j].clone() & !recognized;
        invalid.enforce_equal(&Boolean::constant(false))?;

        if wit.move_call_module_lengths[j] as usize > pubcfg.max_id_len
            || wit.move_call_function_lengths[j] as usize > pubcfg.max_id_len
        {
            panic!("identifier exceeds configured bound");
        }

        let move_call_symbol = move_call_symbol(&wit.tx_bytes, wit, j, &is_move_call, pubcfg)?;

        let mut symbol_bytes = vec![UInt8::constant(0u8); symbol_layout];
        apply_symbol_selection(&mut symbol_bytes, &transfer_symbol, &is_transfer)?;
        apply_symbol_selection(&mut symbol_bytes, &split_symbol, &is_split)?;
        apply_symbol_selection(&mut symbol_bytes, &merge_symbol, &is_merge)?;
        apply_symbol_selection(&mut symbol_bytes, &publish_symbol, &is_publish)?;
        apply_symbol_selection(&mut symbol_bytes, &make_move_vec_symbol, &is_make_move_vec)?;
        apply_symbol_selection(&mut symbol_bytes, &upgrade_symbol, &is_upgrade)?;
        apply_symbol_selection(&mut symbol_bytes, &move_call_symbol, &is_move_call)?;

        let symbol_hash = D::hash(&symbol_bytes)?;
        let limbs = digest_to_limbs::<F>(&symbol_hash);
        action_symbol_limbs.push(limbs);
    }

    // Reconstruct DFA structure and commit to its hash.
    assert!(
        !wit.dfa.states.is_empty(),
        "dfa must contain at least one state"
    );
    assert!(
        wit.dfa.states.len() <= pubcfg.max_states,
        "state bound exceeded"
    );
    assert!(
        wit.dfa.start_state < wit.dfa.states.len(),
        "start state out of range"
    );

    let mut dfa_serialization = Vec::new();
    dfa_serialization.extend(le_u32_constants::<F>(wit.dfa.states.len() as u32));
    dfa_serialization.extend(le_u32_constants::<F>(wit.dfa.start_state as u32));

    let mut accept_flags = Vec::with_capacity(wit.dfa.states.len());
    let mut dfa_vars = Vec::with_capacity(wit.dfa.states.len());

    for (_state_idx, state) in wit.dfa.states.iter().enumerate() {
        assert!(
            state.transitions.len() <= pubcfg.max_transitions_per_state,
            "transition bound exceeded"
        );
        accept_flags.push(Boolean::constant(state.is_accepting));
        dfa_serialization.push(UInt8::constant(if state.is_accepting { 1 } else { 0 }));
        dfa_serialization.extend(le_u32_constants::<F>(state.transitions.len() as u32));

        let mut state_vars = Vec::with_capacity(state.transitions.len());
        for transition in &state.transitions {
            assert_eq!(
                transition.symbol.len(),
                symbol_layout,
                "transition symbols must be length-prefixed and padded"
            );
            assert!(
                transition.target < wit.dfa.states.len(),
                "dfa target out of bounds"
            );

            dfa_serialization.extend(le_u32_constants::<F>(transition.target as u32));
            dfa_serialization.extend(transition.symbol.iter().cloned());

            let digest = D::hash(&transition.symbol)?;
            let limbs = digest_to_limbs::<F>(&digest);
            state_vars.push(DfaTransitionVar {
                target: FpVar::<F>::constant(F::from(transition.target as u64)),
                symbol_limbs: limbs,
            });
        }
        dfa_vars.push(state_vars);
    }

    let dfa_digest = D::hash(&dfa_serialization)?;
    enforce_digest_eq_limbs::<F>(&dfa_digest, &dfa_pi_limbs)?;

    // Run the DFA over the observed action stream.
    let mut current_state = FpVar::<F>::constant(F::from(wit.dfa.start_state as u64));
    for (idx, symbol_limbs) in action_symbol_limbs.iter().enumerate() {
        let present = present_flags[idx].clone();
        let present_fe: FpVar<F> = present.clone().into();
        let mut next_state = FpVar::<F>::zero();
        let mut found = FpVar::<F>::zero();

        for (state_idx, transitions) in dfa_vars.iter().enumerate() {
            let state_const = FpVar::<F>::constant(F::from(state_idx as u64));
            let in_state = current_state.is_eq(&state_const)?;
            for transition in transitions {
                let matches = equal_digest_limbs(symbol_limbs, &transition.symbol_limbs)?;
                let active = in_state.clone() & matches.clone() & present.clone();
                let active_fe: FpVar<F> = active.clone().into();
                next_state += transition.target.clone() * &active_fe;
                found += active_fe;
            }
        }

        found.enforce_equal(&present_fe)?;
        let keep = FpVar::<F>::one() - &present_fe;
        let updated = next_state + current_state.clone() * keep;
        current_state = updated;
    }

    let mut accept_indicator = FpVar::<F>::zero();
    for (state_idx, flag) in accept_flags.iter().enumerate() {
        let state_const = FpVar::<F>::constant(F::from(state_idx as u64));
        let here = current_state.is_eq(&state_const)? & flag.clone();
        accept_indicator += FpVar::<F>::from(here);
    }
    accept_indicator.enforce_equal(&FpVar::<F>::one())?;

    Ok(())
}

/// Canonical `MoveCall` symbol: literal tag, package bytes, then module/function identifiers.
fn move_call_symbol<F: PrimeField>(
    tx_bytes: &[UInt8<F>],
    wit: &TxPolicyWitness<F>,
    idx: usize,
    mask: &Boolean<F>,
    pubcfg: &TxPolicyPublic<F>,
) -> Result<Vec<UInt8<F>>, SynthesisError> {
    let pkg_off = wit.move_call_pkg_offsets[idx];
    assert!(
        pkg_off + MOVE_CALL_PACKAGE_BYTES <= tx_bytes.len(),
        "package offset out of bounds"
    );
    let pkg_bytes: Vec<UInt8<F>> = (0..MOVE_CALL_PACKAGE_BYTES)
        .map(|k| tx_bytes[pkg_off + k].clone())
        .collect();

    let module_len = wit.move_call_module_lengths[idx] as usize;
    let module_off = wit.move_call_module_offsets[idx];
    assert!(
        module_off + module_len <= tx_bytes.len(),
        "module slice OOB"
    );
    let module_len_bytes = take_le4(tx_bytes, module_off.saturating_sub(LENGTH_PREFIX_BYTES));
    let module_len_value = pack_bytes_le_to_fp::<F>(&module_len_bytes);
    let module_len_expected = FpVar::<F>::constant(F::from(module_len as u64));
    enforce_masked_zero(module_len_value - module_len_expected, mask)?;
    let module_bytes: Vec<UInt8<F>> = (0..module_len)
        .map(|k| tx_bytes[module_off + k].clone())
        .collect();

    let function_len = wit.move_call_function_lengths[idx] as usize;
    let function_off = wit.move_call_function_offsets[idx];
    assert!(
        function_off + function_len <= tx_bytes.len(),
        "function slice OOB"
    );
    let function_len_bytes = take_le4(tx_bytes, function_off.saturating_sub(LENGTH_PREFIX_BYTES));
    let function_len_value = pack_bytes_le_to_fp::<F>(&function_len_bytes);
    let function_len_expected = FpVar::<F>::constant(F::from(function_len as u64));
    enforce_masked_zero(function_len_value - function_len_expected, mask)?;
    let function_bytes: Vec<UInt8<F>> = (0..function_len)
        .map(|k| tx_bytes[function_off + k].clone())
        .collect();

    let mut payload = Vec::new();
    payload.extend(MOVE_CALL_LITERAL.iter().map(|&b| UInt8::constant(b)));
    payload.extend(pkg_bytes);
    payload.extend(le_u32_constants::<F>(module_len as u32));
    payload.extend(module_bytes);
    payload.extend(le_u32_constants::<F>(function_len as u32));
    payload.extend(function_bytes);

    let payload_len = payload.len();
    assert!(
        payload_len <= pubcfg.max_symbol_bytes,
        "move call symbol exceeds configured bound"
    );
    Ok(encode_symbol_payload(
        payload,
        payload_len,
        pubcfg.max_symbol_bytes,
    ))
}

/// Length-prefix followed by padded literal for commands with no auxiliary data.
fn constant_symbol<F: PrimeField>(literal: &[u8], max_symbol_bytes: usize) -> Vec<UInt8<F>> {
    assert!(
        literal.len() <= max_symbol_bytes,
        "symbol literal exceeds bound"
    );
    let payload = literal
        .iter()
        .map(|&b| UInt8::constant(b))
        .collect::<Vec<_>>();
    encode_symbol_payload(payload, literal.len(), max_symbol_bytes)
}

/// Attach a canonical little-endian length prefix and zero padding to a symbol payload.
fn encode_symbol_payload<F: PrimeField>(
    mut payload: Vec<UInt8<F>>,
    payload_len: usize,
    max_symbol_bytes: usize,
) -> Vec<UInt8<F>> {
    let mut symbol = Vec::with_capacity(LENGTH_PREFIX_BYTES + max_symbol_bytes);
    symbol.extend(le_u32_constants::<F>(payload_len as u32));
    symbol.append(&mut payload);
    while symbol.len() < LENGTH_PREFIX_BYTES + max_symbol_bytes {
        symbol.push(UInt8::constant(0u8));
    }
    symbol
}

fn apply_symbol_selection<F: PrimeField>(
    accumulator: &mut [UInt8<F>],
    candidate: &[UInt8<F>],
    flag: &Boolean<F>,
) -> Result<(), SynthesisError> {
    debug_assert_eq!(accumulator.len(), candidate.len());
    for (slot, cand) in accumulator.iter_mut().zip(candidate.iter()) {
        let selected = UInt8::conditionally_select(flag, cand, slot)?;
        *slot = selected;
    }
    Ok(())
}

fn digest_to_limbs<F: PrimeField>(digest: &[UInt8<F>; 32]) -> [FpVar<F>; DIGEST_LIMBS] {
    let mut limbs = Vec::with_capacity(DIGEST_LIMBS);
    for chunk in digest.chunks(DIGEST_LIMB_BYTES) {
        limbs.push(pack_bytes_le_to_fp::<F>(chunk));
    }
    limbs.try_into().expect("digest limb length mismatch")
}

fn equal_digest_limbs<F: PrimeField>(
    lhs: &[FpVar<F>; DIGEST_LIMBS],
    rhs: &[FpVar<F>; DIGEST_LIMBS],
) -> Result<Boolean<F>, SynthesisError> {
    let mut acc = Boolean::constant(true);
    for (a, b) in lhs.iter().zip(rhs.iter()) {
        acc = acc & a.is_eq(b)?;
    }
    Ok(acc)
}

fn enforce_masked_zero<F: PrimeField>(
    value: FpVar<F>,
    mask: &Boolean<F>,
) -> Result<(), SynthesisError> {
    let masked = value * FpVar::<F>::from(mask.clone());
    masked.enforce_equal(&FpVar::<F>::zero())
}

fn tag_is<F: PrimeField>(tag: &FpVar<F>, value: u8) -> Result<Boolean<F>, SynthesisError> {
    tag.is_eq(&FpVar::<F>::constant(F::from(value as u64)))
}

fn le_u32_constants<F: PrimeField>(value: u32) -> [UInt8<F>; LENGTH_PREFIX_BYTES] {
    core::array::from_fn(|i| {
        let byte = ((value >> (8 * i)) & 0xff) as u8;
        UInt8::constant(byte)
    })
}

fn take_le4<F: PrimeField>(bytes: &[UInt8<F>], off: usize) -> [UInt8<F>; LENGTH_PREFIX_BYTES] {
    core::array::from_fn(|i| {
        if off + i < bytes.len() {
            bytes[off + i].clone()
        } else {
            UInt8::constant(0u8)
        }
    })
}
