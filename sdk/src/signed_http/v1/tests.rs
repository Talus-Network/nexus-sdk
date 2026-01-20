use {super::*, ed25519_dalek::SigningKey};

fn sk_from_byte(byte: u8) -> SigningKey {
    SigningKey::from_bytes(&[byte; 32])
}

#[derive(Clone)]
struct FixedClockV1 {
    now_ms: u64,
}

impl ClockV1 for FixedClockV1 {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

fn headers_ref_for_encoded(headers: &EncodedSignatureHeadersV1) -> SignatureHeadersRef<'_> {
    SignatureHeadersRef::new(
        Some(SIG_VERSION_V1),
        Some(&headers.sig_input_b64),
        Some(&headers.sig_b64),
    )
}

#[test]
fn sign_and_verify_request_roundtrip() {
    let leader_sk = sk_from_byte(7);
    let leader_pk = leader_sk.verifying_key().to_bytes();

    let allowed = AllowedLeadersV1::try_from(AllowedLeadersFileV1 {
        version: 1,
        leaders: vec![AllowedLeaderFileV1 {
            leader_id: "0x1111".to_string(),
            keys: vec![AllowedLeaderKeyFileV1 {
                kid: 0,
                public_key: hex::encode(leader_pk),
            }],
        }],
    })
    .unwrap();

    let body = br#"{"hello":"world"}"#;
    let claims = InvokeRequestClaimsV1 {
        leader_id: "0x1111".to_string(),
        leader_kid: 0,
        tool_id: "demo::tool::1.0.0".to_string(),
        iat_ms: 1000,
        exp_ms: 2000,
        nonce: "abc".to_string(),
        method: "POST".to_string(),
        path: "/invoke".to_string(),
        query: "".to_string(),
        body_sha256: sha256_hex(body),
    };

    let (sig_input, sig) = sign_invoke_request_v1(&claims, &leader_sk).unwrap();
    let decoded = DecodedSignatureV1 {
        sig_input,
        signature: sig,
    };

    let opts = VerifyOptions {
        now_ms: 1500,
        max_clock_skew_ms: 0,
        max_validity_ms: 10_000,
    };

    let verified = verify_invoke_request_v1(
        decoded,
        HttpRequestMeta {
            method: "POST",
            path: "/invoke",
            query: "",
        },
        body,
        "demo::tool::1.0.0",
        &allowed,
        &opts,
    )
    .unwrap();

    assert_eq!(verified.claims.leader_id, "0x1111");
    assert_eq!(verified.leader_public_key, leader_pk);
}

#[test]
fn verify_fails_on_body_hash_mismatch() {
    let leader_sk = sk_from_byte(7);
    let leader_pk = leader_sk.verifying_key().to_bytes();

    let allowed = AllowedLeadersV1::try_from(AllowedLeadersFileV1 {
        version: 1,
        leaders: vec![AllowedLeaderFileV1 {
            leader_id: "0x1111".to_string(),
            keys: vec![AllowedLeaderKeyFileV1 {
                kid: 0,
                public_key: hex::encode(leader_pk),
            }],
        }],
    })
    .unwrap();

    let body = br#"{"hello":"world"}"#;
    let claims = InvokeRequestClaimsV1 {
        leader_id: "0x1111".to_string(),
        leader_kid: 0,
        tool_id: "demo::tool::1.0.0".to_string(),
        iat_ms: 1000,
        exp_ms: 2000,
        nonce: "abc".to_string(),
        method: "POST".to_string(),
        path: "/invoke".to_string(),
        query: "".to_string(),
        body_sha256: sha256_hex(br#"{"different":"body"}"#),
    };

    let (sig_input, sig) = sign_invoke_request_v1(&claims, &leader_sk).unwrap();
    let decoded = DecodedSignatureV1 {
        sig_input,
        signature: sig,
    };

    let opts = VerifyOptions {
        now_ms: 1500,
        max_clock_skew_ms: 0,
        max_validity_ms: 10_000,
    };

    let err = verify_invoke_request_v1(
        decoded,
        HttpRequestMeta {
            method: "POST",
            path: "/invoke",
            query: "",
        },
        body,
        "demo::tool::1.0.0",
        &allowed,
        &opts,
    )
    .unwrap_err();

    assert!(matches!(err, SignedHttpError::BodyHashMismatch));
}

#[test]
fn engine_end_to_end_roundtrip_happy_path() {
    let leader_id = "0x1111";
    let tool_id = "demo::tool::1.0.0";

    let leader_sk = sk_from_byte(7);
    let leader_pk = leader_sk.verifying_key().to_bytes();

    let tool_sk = sk_from_byte(9);
    let tool_pk = tool_sk.verifying_key().to_bytes();

    let allowed = AllowedLeadersV1::try_from(AllowedLeadersFileV1 {
        version: 1,
        leaders: vec![AllowedLeaderFileV1 {
            leader_id: leader_id.to_string(),
            keys: vec![AllowedLeaderKeyFileV1 {
                kid: 0,
                public_key: hex::encode(leader_pk),
            }],
        }],
    })
    .unwrap();

    let engine = SignedHttpEngineV1::with_clock(
        SignedHttpPolicyV1 {
            max_clock_skew_ms: 0,
            max_validity_ms: 10_000,
        },
        FixedClockV1 { now_ms: 1_500 },
    );

    let invoker = engine.invoker(leader_id.to_string(), 0, leader_sk);
    let responder =
        engine.responder_with_in_memory_replay(tool_id.to_string(), 0, tool_sk, allowed);

    let req_body = br#"{"hello":"world"}"#.to_vec();
    let http = HttpRequestMeta {
        method: "POST",
        path: "/invoke",
        query: "",
    };

    let outbound = invoker
        .begin_invoke_with_nonce(
            tool_id.to_string(),
            http.clone(),
            &req_body,
            "abc".to_string(),
        )
        .unwrap();

    let decision = responder
        .authenticate_invoke(
            http,
            &req_body,
            headers_ref_for_encoded(outbound.request_headers()),
        )
        .unwrap();

    let inbound = match decision {
        ResponderDecisionV1::Proceed(session) => session,
        _ => panic!("expected Proceed"),
    };

    let ctx = inbound.auth_context();
    assert_eq!(ctx.invoker_id, leader_id);
    assert_eq!(ctx.invoker_kid, 0);
    assert_eq!(ctx.responder_id, tool_id);
    assert_eq!(ctx.method, "POST");
    assert_eq!(ctx.path, "/invoke");
    assert_eq!(ctx.query, "");

    let resp_body = br#"{"ok":true}"#.to_vec();
    let signed = inbound.finish(200, resp_body.clone()).unwrap();

    let verified = outbound
        .verify_response(
            signed.status,
            headers_ref_for_encoded(&signed.headers),
            &signed.body,
            &StaticResponderKeyV1 {
                responder_id: tool_id.to_string(),
                responder_kid: 0,
                public_key: tool_pk,
            },
        )
        .unwrap();

    assert_eq!(verified.responder_id, tool_id);
    assert_eq!(verified.responder_kid, 0);
    assert_eq!(verified.nonce, "abc");
    assert_eq!(verified.status, 200);
}

#[test]
fn engine_verify_response_fails_without_tool_key_then_succeeds() {
    struct EmptyToolKeys;
    impl ResponderKeyResolver for EmptyToolKeys {
        fn responder_public_key(
            &self,
            _responder_id: &str,
            _responder_kid: u64,
        ) -> Option<[u8; 32]> {
            None
        }
    }

    let tool_id = "demo::tool::1.0.0";

    let leader_sk = sk_from_byte(7);
    let leader_pk = leader_sk.verifying_key().to_bytes();

    let tool_sk = sk_from_byte(9);
    let tool_pk = tool_sk.verifying_key().to_bytes();

    let allowed = AllowedLeadersV1::try_from(AllowedLeadersFileV1 {
        version: 1,
        leaders: vec![AllowedLeaderFileV1 {
            leader_id: "0x1111".to_string(),
            keys: vec![AllowedLeaderKeyFileV1 {
                kid: 0,
                public_key: hex::encode(leader_pk),
            }],
        }],
    })
    .unwrap();

    let engine = SignedHttpEngineV1::with_clock(
        SignedHttpPolicyV1 {
            max_clock_skew_ms: 0,
            max_validity_ms: 10_000,
        },
        FixedClockV1 { now_ms: 1_500 },
    );

    let invoker = engine.invoker("0x1111".to_string(), 0, leader_sk);
    let responder =
        engine.responder_with_in_memory_replay(tool_id.to_string(), 0, tool_sk, allowed);

    let req_body = br#"{"hello":"world"}"#.to_vec();
    let http = HttpRequestMeta {
        method: "POST",
        path: "/invoke",
        query: "",
    };
    let outbound = invoker
        .begin_invoke_with_nonce(
            tool_id.to_string(),
            http.clone(),
            &req_body,
            "abc".to_string(),
        )
        .unwrap();

    let decision = responder
        .authenticate_invoke(
            http,
            &req_body,
            headers_ref_for_encoded(outbound.request_headers()),
        )
        .unwrap();
    let inbound = match decision {
        ResponderDecisionV1::Proceed(session) => session,
        _ => panic!("expected Proceed"),
    };

    let signed = inbound.finish(200, br#"{"ok":true}"#.to_vec()).unwrap();

    let err = outbound
        .verify_response(
            signed.status,
            headers_ref_for_encoded(&signed.headers),
            &signed.body,
            &EmptyToolKeys,
        )
        .unwrap_err();
    assert!(matches!(
        err,
        SignedHttpError::UnknownToolKey {
            tool_id: _,
            tool_kid: 0
        }
    ));

    outbound
        .verify_response(
            signed.status,
            headers_ref_for_encoded(&signed.headers),
            &signed.body,
            &StaticResponderKeyV1 {
                responder_id: tool_id.to_string(),
                responder_kid: 0,
                public_key: tool_pk,
            },
        )
        .unwrap();
}

#[test]
fn engine_replay_in_flight_then_cached_return() {
    let leader_id = "0x1111";
    let tool_id = "demo::tool::1.0.0";

    let leader_sk = sk_from_byte(7);
    let leader_pk = leader_sk.verifying_key().to_bytes();

    let tool_sk = sk_from_byte(9);
    let tool_pk = tool_sk.verifying_key().to_bytes();

    let allowed = AllowedLeadersV1::try_from(AllowedLeadersFileV1 {
        version: 1,
        leaders: vec![AllowedLeaderFileV1 {
            leader_id: leader_id.to_string(),
            keys: vec![AllowedLeaderKeyFileV1 {
                kid: 0,
                public_key: hex::encode(leader_pk),
            }],
        }],
    })
    .unwrap();

    let engine = SignedHttpEngineV1::with_clock(
        SignedHttpPolicyV1 {
            max_clock_skew_ms: 0,
            max_validity_ms: 10_000,
        },
        FixedClockV1 { now_ms: 1_500 },
    );

    let invoker = engine.invoker(leader_id.to_string(), 0, leader_sk);
    let responder =
        engine.responder_with_in_memory_replay(tool_id.to_string(), 0, tool_sk, allowed);

    let http = HttpRequestMeta {
        method: "POST",
        path: "/invoke",
        query: "",
    };
    let req_body = br#"{"hello":"world"}"#.to_vec();
    let outbound = invoker
        .begin_invoke_with_nonce(
            tool_id.to_string(),
            http.clone(),
            &req_body,
            "abc".to_string(),
        )
        .unwrap();
    let req_headers = headers_ref_for_encoded(outbound.request_headers());

    let inbound = match responder
        .authenticate_invoke(http.clone(), &req_body, req_headers)
        .unwrap()
    {
        ResponderDecisionV1::Proceed(session) => session,
        _ => panic!("expected Proceed"),
    };

    match responder
        .authenticate_invoke(http.clone(), &req_body, req_headers)
        .unwrap()
    {
        ResponderDecisionV1::Reject(rej) => {
            assert_eq!(rej.kind, ResponderRejectionKindV1::InFlight);
        }
        _ => panic!("expected InFlight rejection"),
    }

    let signed_first = inbound.finish(200, br#"{"ok":true}"#.to_vec()).unwrap();

    let cached = match responder
        .authenticate_invoke(http, &req_body, req_headers)
        .unwrap()
    {
        ResponderDecisionV1::Return(resp) => resp,
        _ => panic!("expected cached Return"),
    };

    assert_eq!(cached.status, signed_first.status);
    assert_eq!(cached.body, signed_first.body);
    assert_eq!(cached.headers.sig_b64, signed_first.headers.sig_b64);
    assert_eq!(
        cached.headers.sig_input_b64,
        signed_first.headers.sig_input_b64
    );

    outbound
        .verify_response(
            cached.status,
            headers_ref_for_encoded(&cached.headers),
            &cached.body,
            &StaticResponderKeyV1 {
                responder_id: tool_id.to_string(),
                responder_kid: 0,
                public_key: tool_pk,
            },
        )
        .unwrap();
}

#[test]
fn engine_replay_conflict_is_rejected() {
    let leader_id = "0x1111";
    let tool_id = "demo::tool::1.0.0";

    let leader_sk = sk_from_byte(7);
    let leader_pk = leader_sk.verifying_key().to_bytes();

    let tool_sk = sk_from_byte(9);

    let allowed = AllowedLeadersV1::try_from(AllowedLeadersFileV1 {
        version: 1,
        leaders: vec![AllowedLeaderFileV1 {
            leader_id: leader_id.to_string(),
            keys: vec![AllowedLeaderKeyFileV1 {
                kid: 0,
                public_key: hex::encode(leader_pk),
            }],
        }],
    })
    .unwrap();

    let engine = SignedHttpEngineV1::with_clock(
        SignedHttpPolicyV1 {
            max_clock_skew_ms: 0,
            max_validity_ms: 10_000,
        },
        FixedClockV1 { now_ms: 1_500 },
    );

    let invoker = engine.invoker(leader_id.to_string(), 0, leader_sk);
    let responder =
        engine.responder_with_in_memory_replay(tool_id.to_string(), 0, tool_sk, allowed);

    let http = HttpRequestMeta {
        method: "POST",
        path: "/invoke",
        query: "",
    };

    let req_body_a = br#"{"hello":"world"}"#.to_vec();
    let outbound_a = invoker
        .begin_invoke_with_nonce(
            tool_id.to_string(),
            http.clone(),
            &req_body_a,
            "abc".to_string(),
        )
        .unwrap();

    let inbound_a = match responder
        .authenticate_invoke(
            http.clone(),
            &req_body_a,
            headers_ref_for_encoded(outbound_a.request_headers()),
        )
        .unwrap()
    {
        ResponderDecisionV1::Proceed(session) => session,
        _ => panic!("expected Proceed"),
    };

    let req_body_b = br#"{"hello":"different"}"#.to_vec();
    let outbound_b = invoker
        .begin_invoke_with_nonce(
            tool_id.to_string(),
            http.clone(),
            &req_body_b,
            "abc".to_string(),
        )
        .unwrap();

    match responder
        .authenticate_invoke(
            http,
            &req_body_b,
            headers_ref_for_encoded(outbound_b.request_headers()),
        )
        .unwrap()
    {
        ResponderDecisionV1::Reject(rej) => {
            assert_eq!(rej.kind, ResponderRejectionKindV1::ReplayConflict);
        }
        _ => panic!("expected ReplayConflict rejection"),
    }

    drop(inbound_a);
}
