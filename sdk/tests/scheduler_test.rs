#![cfg(feature = "test_utils")]

use {
    nexus_sdk::{
        sui,
        types::{
            deserialize_sui_option_u64,
            deserialize_sui_u64,
            PolicySymbol,
            TaskState,
            TypeName,
        },
    },
    serde::Deserialize,
    serde_json::json,
};

#[test]
fn policy_symbol_supports_pos0_fallback_shape() {
    let value = json!({
        "variant": "Witness",
        "pos0": { "name": "0x1::module::Type" },
    });

    let symbol: PolicySymbol = serde_json::from_value(value).expect("deserialize policy symbol");
    assert_eq!(
        symbol,
        PolicySymbol::Witness(TypeName::new("0x1::module::Type"))
    );
}

#[test]
fn task_state_supports_variant_object_and_errors_on_unknown_variant() {
    let paused: TaskState = serde_json::from_value(json!({ "variant": "Paused" }))
        .expect("deserialize TaskState from object");
    assert_eq!(paused, TaskState::Paused);

    let err = serde_json::from_value::<TaskState>(json!({ "variant": "Bogus" }))
        .expect_err("unknown TaskState should error");
    let msg = err.to_string();
    assert!(
        msg.contains("unknown variant") || msg.contains("Bogus"),
        "unexpected error: {msg}"
    );
}

#[test]
fn task_state_roundtrips_through_bcs() {
    for state in [TaskState::Active, TaskState::Paused, TaskState::Canceled] {
        let bytes = bcs::to_bytes(&state).expect("bcs serialize TaskState");
        let decoded: TaskState = bcs::from_bytes(&bytes).expect("bcs deserialize TaskState");
        assert_eq!(decoded, state);
    }
}

#[test]
fn task_state_serializes_as_strings_in_json() {
    assert_eq!(
        serde_json::to_value(TaskState::Active).expect("serialize"),
        json!("Active")
    );
    assert_eq!(
        serde_json::to_value(TaskState::Paused).expect("serialize"),
        json!("Paused")
    );
    assert_eq!(
        serde_json::to_value(TaskState::Canceled).expect("serialize"),
        json!("Canceled")
    );
}

#[test]
fn deserialize_sui_u64_accepts_numbers_and_rejects_other_json_types() {
    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(deserialize_with = "deserialize_sui_u64")]
        value: u64,
    }

    let parsed: Wrapper =
        serde_json::from_value(json!({ "value": 42 })).expect("number value should parse");
    assert_eq!(parsed.value, 42);

    assert!(serde_json::from_value::<Wrapper>(json!({ "value": true })).is_err());
}

#[test]
fn deserialize_sui_option_u64_uses_standard_layout_for_bcs() {
    #[derive(serde::Serialize)]
    struct Raw {
        value: Option<u64>,
    }

    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(deserialize_with = "deserialize_sui_option_u64")]
        value: Option<u64>,
    }

    let bytes = bcs::to_bytes(&Raw { value: Some(7) }).expect("bcs serialize");
    let parsed: Wrapper = bcs::from_bytes(&bytes).expect("bcs deserialize");
    assert_eq!(parsed.value, Some(7));
}

#[test]
fn policy_symbol_uid_pos0_fallback_parses_addresses() {
    let addr = sui::types::Address::from_static("0x123");
    let value = json!({
        "variant": "Uid",
        "pos0": addr,
    });

    let parsed: PolicySymbol = serde_json::from_value(value).expect("deserialize PolicySymbol");
    assert_eq!(parsed, PolicySymbol::Uid(addr));
}
