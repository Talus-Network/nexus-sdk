use {
    super::wire::*,
    base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _},
    ed25519_dalek::{Signer as _, SigningKey, VerifyingKey},
};

const TOOL_ID: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42,
];
const NONCE: [u8; 32] = [
    32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55,
    56, 57, 58, 59, 60, 61, 62, 63,
];
const INPUT_HASH: [u8; 32] = [
    64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87,
    88, 89, 90, 91, 92, 93, 94, 95,
];
const SIGNING_SEED: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31,
];
const PUBLIC_KEY: [u8; 32] = [
    3, 161, 7, 191, 243, 206, 16, 190, 29, 112, 221, 24, 231, 75, 192, 153, 103, 228, 214, 48, 155,
    165, 13, 95, 29, 220, 134, 100, 18, 85, 49, 184,
];
const SIGNATURE: [u8; 64] = [
    109, 66, 245, 108, 146, 243, 159, 161, 73, 210, 33, 153, 197, 66, 74, 144, 112, 219, 25, 2,
    226, 205, 168, 164, 53, 0, 250, 44, 234, 242, 145, 186, 54, 98, 220, 32, 86, 219, 240, 96, 249,
    115, 172, 163, 85, 179, 62, 96, 82, 188, 115, 166, 80, 15, 169, 169, 193, 61, 58, 203, 240, 13,
    120, 6,
];

struct TestKeys([u8; 32]);

impl LeaderKeyResolver for TestKeys {
    fn leader_public_key(&self, leader_id: &str, leader_key_id: u64) -> Option<[u8; 32]> {
        (leader_id == "vector-leader" && leader_key_id == 7).then_some(self.0)
    }
}

#[test]
fn known_answer_transcript_and_request_headers_match_independent_vector() {
    let signing_key = SigningKey::from_bytes(&SIGNING_SEED);
    let request = sign_request("vector-leader", 7, TOOL_ID, NONCE, INPUT_HASH, &signing_key);
    let expected_message = hex::decode("6e657875732e7369676e65645f687474702e76342e6c65616465725f72657175657374000000000000000000000000000000000000000000000000000000000000002a202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f").unwrap();
    assert_eq!(
        leader_request_message(&TOOL_ID, &NONCE, &INPUT_HASH),
        expected_message
    );
    assert_eq!(signing_key.verifying_key().to_bytes(), PUBLIC_KEY);
    assert_eq!(URL_SAFE_NO_PAD.decode(&request.tool_id).unwrap(), TOOL_ID);
    assert_eq!(
        request.tool_id,
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACo"
    );
    assert_eq!(
        URL_SAFE_NO_PAD.decode(&request.leader_signature).unwrap(),
        SIGNATURE
    );
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
            HEADER_TOOL_ID,
            HEADER_INPUT_HASH,
            HEADER_LEADER_SIGNATURE,
            HEADER_NONCE,
        ]
    );

    let key_id = request.leader_key_id.to_string();
    let headers = RequestHeadersRef {
        signature_version: Some(SIGNATURE_VERSION_V4),
        leader_id: Some(&request.leader_id),
        leader_key_id: Some(&key_id),
        tool_id: Some(&request.tool_id),
        input_hash: Some(&request.input_hash),
        leader_signature: Some(&request.leader_signature),
        nonce: Some(&request.nonce),
    };
    let authenticated = authenticate_request(headers, &TestKeys(PUBLIC_KEY)).unwrap();
    assert_eq!(authenticated.tool_id, TOOL_ID);
    assert_eq!(authenticated.input_hash, INPUT_HASH);
    assert_eq!(authenticated.nonce, NONCE);
    assert_eq!(authenticated.leader_signature, SIGNATURE);
}

#[test]
fn changing_tool_id_nonce_or_input_hash_invalidates_the_original_signature() {
    let public_key = VerifyingKey::from_bytes(&PUBLIC_KEY).unwrap();
    let original_message = leader_request_message(&TOOL_ID, &NONCE, &INPUT_HASH);
    public_key
        .verify_strict(
            &original_message,
            &ed25519_dalek::Signature::from_bytes(&SIGNATURE),
        )
        .unwrap();

    let mut other_tool_id = TOOL_ID;
    other_tool_id[31] ^= 1;
    let other_nonce = [0x55; 32];
    let other_input_hash = [0x66; 32];
    for message in [
        leader_request_message(&other_tool_id, &NONCE, &INPUT_HASH),
        leader_request_message(&TOOL_ID, &other_nonce, &INPUT_HASH),
        leader_request_message(&TOOL_ID, &NONCE, &other_input_hash),
    ] {
        assert!(public_key
            .verify_strict(&message, &ed25519_dalek::Signature::from_bytes(&SIGNATURE))
            .is_err());
    }
}

#[test]
fn v3_and_missing_request_versions_are_rejected_without_fallback() {
    let keys = TestKeys(PUBLIC_KEY);
    let v3_headers = RequestHeadersRef {
        signature_version: Some("3"),
        ..RequestHeadersRef::default()
    };
    assert!(matches!(
        authenticate_request(v3_headers, &keys),
        Err(super::error::SignedHttpError::UnsupportedVersion(version)) if version == "3"
    ));

    assert!(matches!(
        authenticate_request(RequestHeadersRef::default(), &keys),
        Err(super::error::SignedHttpError::MissingHeader(
            HEADER_SIGNATURE_VERSION
        ))
    ));
}

#[test]
fn malformed_tool_id_length_is_rejected_before_signature_verification() {
    let encoded_tool_id = URL_SAFE_NO_PAD.encode([0; 31]);
    let headers = RequestHeadersRef {
        signature_version: Some(SIGNATURE_VERSION_V4),
        leader_id: Some("vector-leader"),
        leader_key_id: Some("7"),
        tool_id: Some(&encoded_tool_id),
        input_hash: Some(&URL_SAFE_NO_PAD.encode(INPUT_HASH)),
        leader_signature: Some(&URL_SAFE_NO_PAD.encode(SIGNATURE)),
        nonce: Some(&URL_SAFE_NO_PAD.encode(NONCE)),
    };
    assert!(matches!(
        authenticate_request(headers, &TestKeys(PUBLIC_KEY)),
        Err(super::error::SignedHttpError::InvalidLength {
            header: HEADER_TOOL_ID,
            actual: 31,
            expected: 32,
        })
    ));
}

#[test]
fn response_signature_uses_existing_transcript_and_requires_v4_header() {
    let leader_signature = SIGNATURE;
    let result_bytes = b"canonical response bytes";
    let tool_key = SigningKey::from_bytes(&[2; 32]);
    let response = sign_response(&leader_signature, &NONCE, result_bytes, &tool_key);
    let pairs = response.to_pairs();
    assert_eq!(
        pairs[0],
        (HEADER_SIGNATURE_VERSION, SIGNATURE_VERSION_V4.to_string())
    );

    let signature_header = &response.tool_signature;
    let headers = ResponseHeadersRef {
        signature_version: Some(SIGNATURE_VERSION_V4),
        tool_signature: Some(signature_header),
    };
    let message =
        crate::commitments::tool_signature_message(&leader_signature, &NONCE, result_bytes);
    let expected_signature = tool_key.sign(&message).to_bytes();
    assert_eq!(
        verify_response(
            headers,
            &leader_signature,
            &NONCE,
            result_bytes,
            tool_key.verifying_key().to_bytes(),
        )
        .unwrap(),
        expected_signature
    );

    assert!(matches!(
        verify_response(
            ResponseHeadersRef {
                signature_version: Some("3"),
                tool_signature: Some(signature_header),
            },
            &leader_signature,
            &NONCE,
            result_bytes,
            tool_key.verifying_key().to_bytes(),
        ),
        Err(super::error::SignedHttpError::UnsupportedVersion(version)) if version == "3"
    ));
}
