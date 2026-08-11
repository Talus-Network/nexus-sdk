use {
    super::{error::SignedHttpError, wire::*},
    base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _},
    ed25519_dalek::SigningKey,
    std::collections::HashMap,
};

fn allowed(leader_id: &str, key_id: u64, public_key: [u8; 32]) -> AllowedLeaders {
    AllowedLeaders::try_from(AllowedLeadersFileV1 {
        version: 1,
        leaders: vec![AllowedLeaderFileV1 {
            leader_id: leader_id.to_string(),
            keys: vec![AllowedLeaderKeyFileV1 {
                kid: key_id,
                public_key: hex::encode(public_key),
            }],
        }],
    })
    .unwrap()
}

fn request_ref(headers: &EncodedRequestHeaders) -> RequestHeadersRef<'_> {
    RequestHeadersRef {
        signature_version: Some(SIGNATURE_VERSION_V3),
        leader_id: Some(&headers.leader_id),
        leader_key_id: Some("7"),
        input_hash: Some(&headers.input_hash),
        leader_signature: Some(&headers.leader_signature),
        nonce: Some(&headers.nonce),
    }
}

#[test]
fn direct_header_pairs_have_no_commitment_version() {
    let leader = SigningKey::from_bytes(&[7; 32]);
    let request = sign_request("leader", 7, [3; 32], [4; 32], &leader);
    assert_eq!(
        request
            .to_pairs()
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
        vec![
            HEADER_SIGNATURE_VERSION,
            HEADER_LEADER_ID,
            HEADER_LEADER_KEY_ID,
            HEADER_INPUT_HASH,
            HEADER_LEADER_SIGNATURE,
            HEADER_NONCE,
        ]
    );

    let response = sign_response(
        &URL_SAFE_NO_PAD
            .decode(&request.leader_signature)
            .unwrap()
            .try_into()
            .unwrap(),
        &[4; 32],
        b"result",
        &SigningKey::from_bytes(&[9; 32]),
    );
    assert_eq!(
        response
            .to_pairs()
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
        vec![HEADER_SIGNATURE_VERSION, HEADER_TOOL_SIGNATURE]
    );
}

#[test]
fn request_signs_only_the_input_hash() {
    let leader = SigningKey::from_bytes(&[7; 32]);
    let input_hash = sha256(b"canonical inputs");
    let nonce = [1; 32];
    let headers = sign_request("leader", 7, input_hash, nonce, &leader);
    let authenticated = authenticate_request(
        request_ref(&headers),
        &allowed("leader", 7, leader.verifying_key().to_bytes()),
    )
    .unwrap();

    assert_eq!(authenticated.input_hash, input_hash);
    assert_eq!(authenticated.nonce, nonce);

    let same_signature = sign_request("other", 99, input_hash, [2; 32], &leader);
    assert_eq!(same_signature.leader_signature, headers.leader_signature);
}

#[cfg(feature = "types")]
#[test]
fn ordered_output_bcs_matches_move_output_hash_golden() {
    use crate::{
        commitments::output_sha256,
        move_bindings::primitives::data::NexusValue,
        types::{OffchainToolOutput, OffchainToolOutputPort},
    };

    let output = OffchainToolOutput {
        tag: b"ok".to_vec(),
        ports: vec![OffchainToolOutputPort {
            port_name: b"result".to_vec(),
            values: vec![NexusValue::InlineData {
                bytes: b"x".to_vec(),
            }],
        }],
    };
    let bytes = bcs::to_bytes(&output).expect("off-chain Tool output should serialize");

    assert_eq!(
        bytes,
        vec![
            0x02, b'o', b'k', 0x01, 0x06, b'r', b'e', b's', b'u', b'l', b't', 0x01, 0x01, 0x01,
            b'x',
        ]
    );
    assert_eq!(
        hex::encode(output_sha256(&bytes)),
        "53c01b8ce70ee1095349bb6b0b70f5698b8c7e491df7406df6dbe66f7b3c64aa"
    );
}

#[cfg(feature = "types")]
#[test]
fn port_rename_and_same_typed_reorder_change_authenticated_response_bytes() {
    use crate::{
        commitments::output_sha256,
        move_bindings::primitives::data::NexusValue,
        types::{OffchainToolOutput, OffchainToolOutputPort},
    };

    let output = OffchainToolOutput {
        tag: b"ok".to_vec(),
        ports: vec![
            OffchainToolOutputPort {
                port_name: b"first".to_vec(),
                values: vec![NexusValue::InlineData {
                    bytes: b"one".to_vec(),
                }],
            },
            OffchainToolOutputPort {
                port_name: b"second".to_vec(),
                values: vec![NexusValue::InlineData {
                    bytes: b"two".to_vec(),
                }],
            },
        ],
    };
    let mut renamed = output.clone();
    renamed.ports[0].port_name = b"renamed".to_vec();
    let mut reordered = output.clone();
    reordered.ports.swap(0, 1);
    let body = bcs::to_bytes(&output).unwrap();
    let renamed_body = bcs::to_bytes(&renamed).unwrap();
    let reordered_body = bcs::to_bytes(&reordered).unwrap();

    assert_ne!(body, renamed_body);
    assert_ne!(body, reordered_body);
    assert_ne!(output_sha256(&body), output_sha256(&renamed_body));
    assert_ne!(output_sha256(&body), output_sha256(&reordered_body));

    let leader_signature = [3; 64];
    let nonce = [4; 32];
    let tool = SigningKey::from_bytes(&[9; 32]);
    let signed = sign_response(&leader_signature, &nonce, &body, &tool);
    for tampered_body in [&renamed_body, &reordered_body] {
        assert!(verify_response(
            ResponseHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V3),
                tool_signature: Some(&signed.tool_signature),
            },
            &leader_signature,
            &nonce,
            tampered_body,
            tool.verifying_key().to_bytes(),
        )
        .is_err());
    }
}

#[test]
fn invalid_request_signature_is_rejected() {
    let leader = SigningKey::from_bytes(&[7; 32]);
    let headers = sign_request("leader", 7, sha256(b"one"), [1; 32], &leader);
    let mut tampered = headers.clone();
    tampered.input_hash = sign_request("leader", 7, sha256(b"two"), [1; 32], &leader).input_hash;

    assert!(matches!(
        authenticate_request(
            request_ref(&tampered),
            &allowed("leader", 7, leader.verifying_key().to_bytes()),
        ),
        Err(SignedHttpError::InvalidSignature)
    ));
}

#[test]
fn response_uses_the_registered_key_direct_message() {
    let leader = SigningKey::from_bytes(&[7; 32]);
    let tool = SigningKey::from_bytes(&[9; 32]);
    let request = sign_request("leader", 7, sha256(b"inputs"), [1; 32], &leader);
    let authenticated = authenticate_request(
        request_ref(&request),
        &allowed("leader", 7, leader.verifying_key().to_bytes()),
    )
    .unwrap();
    let result = b"exact ordered BCS output";
    let response = sign_response(
        &authenticated.leader_signature,
        &authenticated.nonce,
        result,
        &tool,
    );
    let signature = verify_response(
        ResponseHeadersRef {
            signature_version: Some(SIGNATURE_VERSION_V3),
            tool_signature: Some(&response.tool_signature),
        },
        &authenticated.leader_signature,
        &authenticated.nonce,
        result,
        tool.verifying_key().to_bytes(),
    )
    .unwrap();

    assert_eq!(
        tool_signature_message(
            &authenticated.leader_signature,
            &authenticated.nonce,
            result,
        ),
        crate::nexus::registered_key::tool_signature_message(
            &authenticated.leader_signature,
            &authenticated.nonce,
            result,
        )
    );
    assert_ne!(signature, [0; 64]);

    for (nonce, result) in [
        (authenticated.nonce, b"different result bytes".as_slice()),
        ([2; 32], result.as_slice()),
    ] {
        assert!(verify_response(
            ResponseHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V3),
                tool_signature: Some(&response.tool_signature),
            },
            &authenticated.leader_signature,
            &nonce,
            result,
            tool.verifying_key().to_bytes(),
        )
        .is_err());
    }
}

#[test]
fn header_getters_read_the_complete_direct_contract() {
    let request_values = HashMap::from([
        (HEADER_SIGNATURE_VERSION, SIGNATURE_VERSION_V3),
        (HEADER_LEADER_ID, "leader-a"),
        (HEADER_LEADER_KEY_ID, "7"),
        (HEADER_INPUT_HASH, "input"),
        (HEADER_LEADER_SIGNATURE, "signature"),
        (HEADER_NONCE, "nonce"),
    ]);
    let request = RequestHeadersRef::from_getter(|name| request_values.get(name).copied());
    assert_eq!(request.signature_version, Some(SIGNATURE_VERSION_V3));
    assert_eq!(request.leader_id, Some("leader-a"));
    assert_eq!(request.leader_key_id, Some("7"));
    assert_eq!(request.input_hash, Some("input"));
    assert_eq!(request.leader_signature, Some("signature"));
    assert_eq!(request.nonce, Some("nonce"));

    let response_values = HashMap::from([
        (HEADER_SIGNATURE_VERSION, SIGNATURE_VERSION_V3),
        (HEADER_TOOL_SIGNATURE, "tool-signature"),
    ]);
    let response = ResponseHeadersRef::from_getter(|name| response_values.get(name).copied());
    assert_eq!(response.signature_version, Some(SIGNATURE_VERSION_V3));
    assert_eq!(response.tool_signature, Some("tool-signature"));
}

#[test]
fn request_authentication_reports_malformed_headers_and_unknown_keys() {
    let leader = SigningKey::from_bytes(&[7; 32]);
    let encoded = sign_request("leader", 7, sha256(b"inputs"), [1; 32], &leader);
    let keys = allowed("leader", 7, leader.verifying_key().to_bytes());

    let mut headers = request_ref(&encoded);
    headers.leader_id = None;
    assert!(matches!(
        authenticate_request(headers, &keys),
        Err(SignedHttpError::MissingHeader(HEADER_LEADER_ID))
    ));

    let mut headers = request_ref(&encoded);
    headers.leader_key_id = Some("not-an-integer");
    assert!(matches!(
        authenticate_request(headers, &keys),
        Err(SignedHttpError::InvalidInteger {
            header: HEADER_LEADER_KEY_ID,
            ..
        })
    ));

    let mut headers = request_ref(&encoded);
    headers.input_hash = Some("***");
    assert!(matches!(
        authenticate_request(headers, &keys),
        Err(SignedHttpError::InvalidBase64 {
            header: HEADER_INPUT_HASH,
            ..
        })
    ));

    let short_hash = URL_SAFE_NO_PAD.encode([1u8; 31]);
    let mut headers = request_ref(&encoded);
    headers.input_hash = Some(&short_hash);
    assert!(matches!(
        authenticate_request(headers, &keys),
        Err(SignedHttpError::InvalidLength {
            header: HEADER_INPUT_HASH,
            actual: 31,
            expected: 32,
        })
    ));

    let short_nonce = URL_SAFE_NO_PAD.encode([1u8; 31]);
    let mut headers = request_ref(&encoded);
    headers.nonce = Some(&short_nonce);
    assert!(matches!(
        authenticate_request(headers, &keys),
        Err(SignedHttpError::InvalidLength {
            header: HEADER_NONCE,
            actual: 31,
            expected: 32,
        })
    ));

    let unknown = allowed("other", 7, leader.verifying_key().to_bytes());
    assert!(matches!(
        authenticate_request(request_ref(&encoded), &unknown),
        Err(SignedHttpError::UnknownLeaderKey { .. })
    ));
}

#[test]
fn request_authentication_rejects_invalid_public_key_and_signature() {
    let leader = SigningKey::from_bytes(&[7; 32]);
    let encoded = sign_request("leader", 7, sha256(b"inputs"), [1; 32], &leader);
    let invalid_public_key = (0..=u8::MAX)
        .map(|byte| [byte; 32])
        .find(|bytes| ed25519_dalek::VerifyingKey::from_bytes(bytes).is_err())
        .expect("at least one repeated-byte encoding is not an Ed25519 point");
    assert!(matches!(
        authenticate_request(
            request_ref(&encoded),
            &allowed("leader", 7, invalid_public_key)
        ),
        Err(SignedHttpError::InvalidPublicKey { .. })
    ));

    let other = SigningKey::from_bytes(&[8; 32]);
    assert!(matches!(
        authenticate_request(
            request_ref(&encoded),
            &allowed("leader", 7, other.verifying_key().to_bytes()),
        ),
        Err(SignedHttpError::InvalidSignature)
    ));
}

#[test]
fn response_verification_rejects_missing_malformed_and_wrong_signatures() {
    let leader_signature = [3u8; 64];
    let nonce = [4u8; 32];
    let tool = SigningKey::from_bytes(&[9; 32]);
    let encoded = sign_response(&leader_signature, &nonce, b"result", &tool);

    assert!(matches!(
        verify_response(
            ResponseHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V3),
                tool_signature: None,
            },
            &leader_signature,
            &nonce,
            b"result",
            tool.verifying_key().to_bytes(),
        ),
        Err(SignedHttpError::MissingHeader(HEADER_TOOL_SIGNATURE))
    ));

    assert!(matches!(
        verify_response(
            ResponseHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V3),
                tool_signature: Some("bad base64***"),
            },
            &leader_signature,
            &nonce,
            b"result",
            tool.verifying_key().to_bytes(),
        ),
        Err(SignedHttpError::InvalidBase64 {
            header: HEADER_TOOL_SIGNATURE,
            ..
        })
    ));

    let other = SigningKey::from_bytes(&[10; 32]);
    assert!(matches!(
        verify_response(
            ResponseHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V3),
                tool_signature: Some(&encoded.tool_signature),
            },
            &leader_signature,
            &nonce,
            b"result",
            other.verifying_key().to_bytes(),
        ),
        Err(SignedHttpError::InvalidSignature)
    ));
}

#[test]
fn allowed_leader_files_reject_bad_versions_duplicates_and_keys() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let public_key = hex::encode(key.verifying_key().to_bytes());
    let leader = |encoded_key: String| AllowedLeaderFileV1 {
        leader_id: "leader".to_string(),
        keys: vec![AllowedLeaderKeyFileV1 {
            kid: 7,
            public_key: encoded_key,
        }],
    };

    assert!(AllowedLeaders::try_from(AllowedLeadersFileV1 {
        version: 2,
        leaders: vec![],
    })
    .unwrap_err()
    .to_string()
    .contains("unsupported version"));

    assert!(AllowedLeaders::try_from(AllowedLeadersFileV1 {
        version: 1,
        leaders: vec![leader(public_key.clone()), leader(public_key)],
    })
    .unwrap_err()
    .to_string()
    .contains("duplicate leader key"));

    for invalid in ["not-hex".to_string(), hex::encode([1u8; 31])] {
        assert!(AllowedLeaders::try_from(AllowedLeadersFileV1 {
            version: 1,
            leaders: vec![leader(invalid)],
        })
        .unwrap_err()
        .to_string()
        .contains("invalid Ed25519 public key"));
    }
}

#[test]
fn v3_rejects_v2_signature_headers() {
    let keys = allowed("leader", 7, [1; 32]);
    assert!(matches!(
        authenticate_request(
            RequestHeadersRef {
                signature_version: Some(crate::signed_http::v2::wire::SIGNATURE_VERSION_V2),
                ..Default::default()
            },
            &keys,
        ),
        Err(SignedHttpError::UnsupportedVersion(_))
    ));
}

#[test]
fn canonical_response_content_type_is_sdk_owned() {
    assert_eq!(
        CANONICAL_TOOL_RESPONSE_CONTENT_TYPE,
        "application/vnd.nexus.canonical-tool-response+bcs"
    );
}
