use {super::wire::*, ed25519_dalek::SigningKey};

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
    let headers = sign_request("leader", 7, input_hash, "nonce-a", &leader);
    let authenticated = authenticate_request(
        request_ref(&headers),
        &allowed("leader", 7, leader.verifying_key().to_bytes()),
    )
    .unwrap();

    assert_eq!(authenticated.input_hash, input_hash);
    assert_eq!(authenticated.nonce, "nonce-a");

    let same_signature = sign_request("other", 99, input_hash, "nonce-b", &leader);
    assert_eq!(same_signature.leader_signature, headers.leader_signature);

    // Identity and nonce deliberately remain outside the onchain proof. HTTPS, not this
    // signature, protects them and the request body while in transit.
}

#[test]
fn invalid_request_signature_is_rejected() {
    let leader = SigningKey::from_bytes(&[7; 32]);
    let headers = sign_request("leader", 7, sha256(b"one"), "nonce", &leader);
    let mut tampered = headers.clone();
    tampered.input_hash = sign_request("leader", 7, sha256(b"two"), "nonce", &leader).input_hash;

    assert!(authenticate_request(
        request_ref(&tampered),
        &allowed("leader", 7, leader.verifying_key().to_bytes()),
    )
    .is_err());
}

#[test]
fn response_signs_leader_signature_and_exact_result_hash() {
    let leader = SigningKey::from_bytes(&[7; 32]);
    let tool = SigningKey::from_bytes(&[9; 32]);
    let request = sign_request("leader", 7, sha256(b"inputs"), "nonce", &leader);
    let authenticated = authenticate_request(
        request_ref(&request),
        &allowed("leader", 7, leader.verifying_key().to_bytes()),
    )
    .unwrap();
    let result = b"exact BCS TaggedOutput";
    let response = sign_response(&authenticated.leader_signature, result, &tool);
    let signature = verify_response(
        ResponseHeadersRef {
            signature_version: Some(SIGNATURE_VERSION_V2),
            tool_signature: Some(&response.tool_signature),
        },
        &authenticated.leader_signature,
        result,
        tool.verifying_key().to_bytes(),
    )
    .unwrap();

    assert_eq!(
        tool_signature_message(&authenticated.leader_signature, result).len(),
        96
    );
    assert_ne!(signature, [0; 64]);
    assert!(verify_response(
        ResponseHeadersRef {
            signature_version: Some(SIGNATURE_VERSION_V2),
            tool_signature: Some(&response.tool_signature),
        },
        &authenticated.leader_signature,
        b"different result bytes",
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
