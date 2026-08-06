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
        signature_version: Some(SIGNATURE_VERSION_V2),
        leader_id: Some(&headers.leader_id),
        leader_key_id: Some("7"),
        input_hash: Some(&headers.input_hash),
        leader_signature: Some(&headers.leader_signature),
        nonce: Some(&headers.nonce),
    }
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

    // The leader signs only the canonical input hash; the Tool response signature and
    // RegisteredKey stamp bind the deterministic nonce to the result onchain.
}

#[test]
fn invalid_request_signature_is_rejected() {
    let leader = SigningKey::from_bytes(&[7; 32]);
    let headers = sign_request("leader", 7, sha256(b"one"), [1; 32], &leader);
    let mut tampered = headers.clone();
    tampered.input_hash = sign_request("leader", 7, sha256(b"two"), [1; 32], &leader).input_hash;

    assert!(authenticate_request(
        request_ref(&tampered),
        &allowed("leader", 7, leader.verifying_key().to_bytes()),
    )
    .is_err());
}

#[test]
fn response_signs_nonce_leader_signature_and_exact_result_hash() {
    let leader = SigningKey::from_bytes(&[7; 32]);
    let tool = SigningKey::from_bytes(&[9; 32]);
    let request = sign_request("leader", 7, sha256(b"inputs"), [1; 32], &leader);
    let authenticated = authenticate_request(
        request_ref(&request),
        &allowed("leader", 7, leader.verifying_key().to_bytes()),
    )
    .unwrap();
    let result = b"exact BCS TaggedOutput";
    let response = sign_response(
        &authenticated.leader_signature,
        &authenticated.nonce,
        result,
        &tool,
    );
    let signature = verify_response(
        ResponseHeadersRef {
            signature_version: Some(SIGNATURE_VERSION_V2),
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
            result
        )
        .len(),
        b"nexus.signed_http.v2.tool_response".len() + 128
    );
    assert_ne!(signature, [0; 64]);
    assert!(verify_response(
        ResponseHeadersRef {
            signature_version: Some(SIGNATURE_VERSION_V2),
            tool_signature: Some(&response.tool_signature),
        },
        &authenticated.leader_signature,
        &authenticated.nonce,
        b"different result bytes",
        tool.verifying_key().to_bytes(),
    )
    .is_err());
    assert!(verify_response(
        ResponseHeadersRef {
            signature_version: Some(SIGNATURE_VERSION_V2),
            tool_signature: Some(&response.tool_signature),
        },
        &authenticated.leader_signature,
        &[2; 32],
        result,
        tool.verifying_key().to_bytes(),
    )
    .is_err());
}

#[test]
fn old_claim_headers_are_not_accepted() {
    assert!(authenticate_request(
        RequestHeadersRef {
            signature_version: Some("1"),
            ..Default::default()
        },
        &allowed("leader", 7, [1; 32]),
    )
    .is_err());
}

#[test]
fn header_getters_read_the_complete_v2_contract() {
    let request_values = HashMap::from([
        (HEADER_SIGNATURE_VERSION, SIGNATURE_VERSION_V2),
        (HEADER_LEADER_ID, "leader-a"),
        (HEADER_LEADER_KEY_ID, "7"),
        (HEADER_INPUT_HASH, "input"),
        (HEADER_LEADER_SIGNATURE, "signature"),
        (HEADER_NONCE, "nonce"),
    ]);
    let request = RequestHeadersRef::from_getter(|name| request_values.get(name).copied());
    assert_eq!(request.signature_version, Some(SIGNATURE_VERSION_V2));
    assert_eq!(request.leader_id, Some("leader-a"));
    assert_eq!(request.leader_key_id, Some("7"));
    assert_eq!(request.input_hash, Some("input"));
    assert_eq!(request.leader_signature, Some("signature"));
    assert_eq!(request.nonce, Some("nonce"));

    let response_values = HashMap::from([
        (HEADER_SIGNATURE_VERSION, SIGNATURE_VERSION_V2),
        (HEADER_TOOL_SIGNATURE, "tool-signature"),
    ]);
    let response = ResponseHeadersRef::from_getter(|name| response_values.get(name).copied());
    assert_eq!(response.signature_version, Some(SIGNATURE_VERSION_V2));
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
    let invalid_key = allowed("leader", 7, invalid_public_key);
    assert!(matches!(
        authenticate_request(request_ref(&encoded), &invalid_key),
        Err(SignedHttpError::InvalidPublicKey { .. })
    ));

    let other = SigningKey::from_bytes(&[8; 32]);
    let wrong_key = allowed("leader", 7, other.verifying_key().to_bytes());
    assert!(matches!(
        authenticate_request(request_ref(&encoded), &wrong_key),
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
                signature_version: Some(SIGNATURE_VERSION_V2),
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
                signature_version: Some(SIGNATURE_VERSION_V2),
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
                signature_version: Some(SIGNATURE_VERSION_V2),
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

    let bad_version = AllowedLeadersFileV1 {
        version: 2,
        leaders: vec![],
    };
    assert!(AllowedLeaders::try_from(bad_version)
        .unwrap_err()
        .to_string()
        .contains("unsupported version"));

    let duplicate = AllowedLeadersFileV1 {
        version: 1,
        leaders: vec![leader(public_key.clone()), leader(public_key)],
    };
    assert!(AllowedLeaders::try_from(duplicate)
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
