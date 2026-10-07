use {
    ed25519_dalek::{Signer as _, SigningKey},
    nexus_sdk::{
        fqn,
        move_bindings::interface::meta_schema::MetaSchema,
        signed_http::v3::{
            error::SignedHttpError,
            wire::{
                sha256,
                sign_request,
                verify_response,
                ResponseHeadersRef,
                CANONICAL_TOOL_RESPONSE_CONTENT_TYPE,
                HEADER_SIGNATURE_VERSION,
                HEADER_TOOL_SIGNATURE,
            },
        },
        types::{NexusData, NexusValue, OffchainToolOutput},
        walrus::{WalrusClient, WalrusContentDigestMismatch, WalrusReader},
        ToolFqn,
    },
    nexus_toolkit::{runtime::routes_for_with_config_, NexusTool, ToolkitRuntimeConfig},
    reqwest::{header::HeaderMap, Client, StatusCode},
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::json,
    std::{
        sync::{Arc, Mutex, OnceLock},
        time::Duration,
    },
    tokio::{net::TcpListener, task::JoinHandle},
};

const BLOB_ID: &[u8] = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const TOOL_KEY: [u8; 32] = [9; 32];
const NONCE: [u8; 32] = [4; 32];

#[derive(Deserialize, JsonSchema)]
struct Input {
    case: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Output {
    Ok {
        #[schemars(with = "String")]
        inline: NexusData,
        #[schemars(with = "String")]
        stored: NexusData,
        #[schemars(with = "Vec<String>")]
        batch: NexusData,
        #[serde(skip)]
        case: String,
    },
}

struct TypedOutputTool;

impl NexusTool for TypedOutputTool {
    type Input = Input;
    type Output = Output;

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.typed_output@1")
    }

    fn description() -> &'static str {
        "Returns explicit Nexus data through the standard runtime."
    }

    async fn new() -> Self {
        Self
    }

    async fn health(&self) -> anyhow::Result<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, Input { case }: Input) -> Output {
        let stored = NexusData::walrus_data(BLOB_ID, [1; 32]).unwrap();
        let batch = if case == "singleton" {
            stored.values().unwrap()
        } else {
            vec![
                NexusValue::inline_data(br#""first""#).unwrap(),
                NexusValue::walrus_data(BLOB_ID, [2; 32]).unwrap(),
                NexusValue::walrus_data(BLOB_ID, [3; 32]).unwrap(),
            ]
        };
        Output::Ok {
            inline: NexusData::inline_data(br#""inline result""#).unwrap(),
            stored,
            batch: NexusData::from_values(batch, true).unwrap(),
            case,
        }
    }

    fn encode_output(output: Output) -> anyhow::Result<OffchainToolOutput> {
        let Output::Ok {
            inline,
            stored,
            batch,
            case,
        } = output;
        let mut output = OffchainToolOutput::from_ports(
            b"ok".to_vec(),
            [
                ("inline".into(), inline),
                ("stored".into(), stored),
                ("batch".into(), batch),
            ],
        )?;

        // Corrupt the encoded result to exercise runtime validation even when
        // a custom encoder bypasses the SDK constructors.
        match case.as_str() {
            "valid" | "singleton" => {}
            "blob_id" => {
                output.ports[1].values[0] = NexusValue::WalrusData {
                    blob_id: b"invalid".to_vec(),
                    content_digest: vec![1; 32],
                };
            }
            "short_digest" | "long_digest" => {
                output.ports[1].values[0] = NexusValue::WalrusData {
                    blob_id: BLOB_ID.to_vec(),
                    content_digest: vec![1; if case == "short_digest" { 31 } else { 33 }],
                };
            }
            "variant" => output.tag = b"unknown".to_vec(),
            "port_name" => output.ports[1].port_name = b"unknown".to_vec(),
            "port_order" => output.ports.swap(0, 1),
            "missing_port" => {
                output.ports.pop();
            }
            "extra_port" => output.ports.push(output.ports[0].clone()),
            "empty_one" => output.ports[1].values.clear(),
            "multiple_one" => {
                let extra = output.ports[0].values[0].clone();
                output.ports[1].values.push(extra);
            }
            "empty_many" => output.ports[2].values.clear(),
            "object_in_data" => {
                output.ports[2].values[1] =
                    NexusValue::object(nexus_sdk::sui::types::Address::from_static("0x1"));
            }
            _ => panic!("unknown test case: {case}"),
        }
        Ok(output)
    }
}

// The test operator configures storage; endpoints are never tool inputs.
static UPLOAD_CLIENT: OnceLock<WalrusClient> = OnceLock::new();

fn generated_output_bytes(seed: &str) -> Vec<u8> {
    let value = format!("{seed}:{}", "large result 🦭 ".repeat(12_000));
    format!("  {}\n", serde_json::to_string(&value).unwrap()).into_bytes()
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum UploadedOutput {
    Ok {
        #[schemars(with = "String")]
        stored: NexusData,
    },
}

struct UploadingTool;

impl NexusTool for UploadingTool {
    type Input = Input;
    type Output = UploadedOutput;

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.uploaded_output@1")
    }

    fn description() -> &'static str {
        "Uploads generated bytes and returns the SDK reference."
    }

    async fn new() -> Self {
        Self
    }

    async fn health(&self) -> anyhow::Result<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, Input { case }: Input) -> UploadedOutput {
        let value = UPLOAD_CLIENT
            .get()
            .expect("the operator configured storage")
            .upload_value(generated_output_bytes(&case), 2)
            .await
            .expect("the SDK uploads and verifies the generated bytes");
        UploadedOutput::Ok {
            stored: NexusData::from_values(vec![value], false).unwrap(),
        }
    }

    fn encode_output(output: UploadedOutput) -> anyhow::Result<OffchainToolOutput> {
        let UploadedOutput::Ok { stored } = output;
        OffchainToolOutput::from_ports(b"ok".to_vec(), [("stored".into(), stored)])
    }
}

struct ToolServer {
    url: String,
    client: Client,
    schema: MetaSchema,
    task: JoinHandle<()>,
}

impl ToolServer {
    async fn new<T: NexusTool<Input = Input>>() -> Self {
        Self::with_body_limit::<T>(None).await
    }

    async fn with_body_limit<T: NexusTool<Input = Input>>(limit: Option<u64>) -> Self {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let config = ToolkitRuntimeConfig::from_json_str(
            &json!({
                "invoke_max_body_bytes": limit,
                "signed_http": {
                    "mode": "required",
                    "allowed_leaders": {
                        "version": 1,
                        "leaders": [{
                            "leader_id": "leader",
                            "keys": [{
                                "kid": 0,
                                "public_key": hex::encode(leader.verifying_key().to_bytes()),
                            }],
                        }],
                    },
                    "tools": {
                        (T::fqn().to_string()): {
                            "response_signing_key": hex::encode(TOOL_KEY),
                        },
                    },
                },
            })
            .to_string(),
        )
        .unwrap();
        // Use the same routes as bootstrap!, with isolated configuration and
        // an ephemeral listener so tests need no shared environment or port.
        let routes = routes_for_with_config_::<T>(Arc::new(config));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(warp::serve(routes).incoming(listener).run());
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .no_proxy()
            .build()
            .unwrap();
        let metadata: serde_json::Value = client
            .get(format!("{url}/meta"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let schema = MetaSchema::from_offchain_json_schemas(
            &serde_json::to_vec(&metadata["input_schema"]).unwrap(),
            &serde_json::to_vec(&metadata["output_schema"]).unwrap(),
        )
        .unwrap();
        Self {
            url,
            client,
            schema,
            task,
        }
    }

    async fn invoke(&self, case: &str) -> ToolResponse {
        let input = NexusData::inline_data(serde_json::to_vec(case).unwrap()).unwrap();
        let body = json!({"ports": [{
            "port_name": "case",
            "value": input.to_json_value().unwrap(),
        }]});
        let inputs = self.schema.resolved_inputs_from_json(&body).unwrap();
        let input_hash = self.schema.resolved_inputs_sha256(&inputs).unwrap();
        let leader = SigningKey::from_bytes(&[7; 32]);
        let mut request = self.client.post(format!("{}/invoke", self.url)).json(&body);
        for (name, value) in sign_request("leader", 0, input_hash, NONCE, &leader).to_pairs() {
            request = request.header(name, value);
        }
        let response = request.send().await.unwrap();
        ToolResponse {
            status: response.status(),
            headers: response.headers().clone(),
            body: response.bytes().await.unwrap().to_vec(),
            leader_signature: leader.sign(&input_hash).to_bytes(),
        }
    }
}

impl Drop for ToolServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct ToolResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
    leader_signature: [u8; 64],
}

impl ToolResponse {
    fn verify(&self, body: &[u8]) -> Result<[u8; 64], SignedHttpError> {
        verify_response(
            ResponseHeadersRef::from_getter(|name| self.headers.get(name)?.to_str().ok()),
            &self.leader_signature,
            &NONCE,
            body,
            SigningKey::from_bytes(&TOOL_KEY).verifying_key().to_bytes(),
        )
    }
}

#[tokio::test]
async fn invocation_body_boundary_uses_the_shared_execution_limit() {
    let server = ToolServer::new::<TypedOutputTool>().await;
    let limit = nexus_sdk::execution_limits::MAX_INVOKE_BODY_BYTES as usize;
    for (size, status) in [
        (limit, StatusCode::UNAUTHORIZED),
        (limit + 1, StatusCode::PAYLOAD_TOO_LARGE),
    ] {
        // An admitted body reaches authentication; an oversized body is
        // rejected by the transport before authentication or tool invocation.
        let response = server
            .client
            .post(format!("{}/invoke", server.url))
            .body(vec![b' '; size])
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert!(!response.headers().contains_key(HEADER_TOOL_SIGNATURE));
    }
}

#[tokio::test]
async fn configured_body_limits_control_admission_without_changing_tool_execution() {
    let envelope = nexus_sdk::execution_limits::MAX_INVOKE_BODY_BYTES;
    for limit in [1024, 10 * 1024 * 1024, envelope + 1] {
        let server = ToolServer::with_body_limit::<TypedOutputTool>(Some(limit)).await;
        for (size, expected) in [
            (limit, StatusCode::UNAUTHORIZED),
            (limit + 1, StatusCode::PAYLOAD_TOO_LARGE),
        ] {
            let response = server
                .client
                .post(format!("{}/invoke", server.url))
                .body(vec![b' '; size as usize])
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
        let response = server.invoke("valid").await;
        assert_eq!(response.status, StatusCode::OK);
        response.verify(&response.body).unwrap();
    }
}

#[tokio::test]
async fn signed_outputs_preserve_inline_and_walrus_values_with_schema_cardinality() {
    let server = ToolServer::new::<TypedOutputTool>().await;
    for case in ["valid", "singleton"] {
        let response = server.invoke(case).await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.headers["content-type"],
            CANONICAL_TOOL_RESPONSE_CONTENT_TYPE
        );
        response.verify(&response.body).unwrap();
        let output: OffchainToolOutput = bcs::from_bytes(&response.body).unwrap();
        assert_eq!(bcs::to_bytes(&output).unwrap(), response.body);
        assert_eq!(output.tag, b"ok");
        let expected_batch = if case == "singleton" {
            vec![NexusValue::walrus_data(BLOB_ID, [1; 32]).unwrap()]
        } else {
            vec![
                NexusValue::inline_data(br#""first""#).unwrap(),
                NexusValue::walrus_data(BLOB_ID, [2; 32]).unwrap(),
                NexusValue::walrus_data(BLOB_ID, [3; 32]).unwrap(),
            ]
        };
        assert_eq!(
            server.schema.canonical_output_ports(&output).unwrap(),
            vec![
                (
                    b"inline".to_vec(),
                    NexusData::inline_data(br#""inline result""#).unwrap()
                ),
                (
                    b"stored".to_vec(),
                    NexusData::walrus_data(BLOB_ID, [1; 32]).unwrap()
                ),
                (
                    b"batch".to_vec(),
                    NexusData::from_values(expected_batch, true).unwrap()
                ),
            ],
        );

        let replay = server.invoke(case).await;
        assert_eq!(replay.status, response.status);
        assert_eq!(replay.body, response.body);
        assert_eq!(
            replay.headers[HEADER_TOOL_SIGNATURE],
            response.headers[HEADER_TOOL_SIGNATURE]
        );
        replay.verify(&replay.body).unwrap();
    }
}

#[tokio::test]
async fn tool_uploads_generated_bytes_and_signs_a_reference_that_reads_back_exactly() {
    let mut walrus = mockito::Server::new_async().await;
    let blob_id = "AQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let uploaded = Arc::new(Mutex::new(None::<Vec<u8>>));
    let publisher_bytes = Arc::clone(&uploaded);
    // Only storage HTTP is a fixture. Reads serve the bytes actually uploaded.
    let put = walrus
        .mock("PUT", "/v1/blobs?epochs=2")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body_from_request(move |request| {
            *publisher_bytes.lock().unwrap() = Some(request.body().unwrap().clone());
            serde_json::to_vec(&json!({"newlyCreated": {"blobObject": {
                "id": "0x1", "blobId": blob_id, "storage": {"endEpoch": 200}
            }}}))
            .unwrap()
        })
        .expect(1)
        .create_async()
        .await;
    let aggregator_bytes = Arc::clone(&uploaded);
    let get = walrus
        .mock("GET", format!("/v1/blobs/{blob_id}").as_str())
        .with_status(200)
        .with_body_from_request(move |_| {
            aggregator_bytes
                .lock()
                .unwrap()
                .as_ref()
                .expect("the blob was uploaded before readback")
                .clone()
        })
        .expect(3)
        .create_async()
        .await;
    let uploader = WalrusClient::builder()
        .with_client(
            Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
        )
        .with_publisher_url(&walrus.url())
        .with_aggregator_url(&walrus.url())
        .build();
    assert!(UPLOAD_CLIENT.set(uploader).is_ok());

    let seed = "generated by the tool";
    let expected_bytes = generated_output_bytes(seed);
    assert!(NexusData::inline_data(expected_bytes.clone()).is_err());
    let server = ToolServer::new::<UploadingTool>().await;
    let response = server.invoke(seed).await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.headers["content-type"],
        CANONICAL_TOOL_RESPONSE_CONTENT_TYPE
    );
    response.verify(&response.body).unwrap();
    let output: OffchainToolOutput = bcs::from_bytes(&response.body).unwrap();
    assert_eq!(output.tag, b"ok");
    let ports = server.schema.canonical_output_ports(&output).unwrap();
    assert_eq!(ports.len(), 1);
    assert_eq!(ports[0].0, b"stored");
    let values = ports[0].1.values().unwrap();
    let [NexusValue::WalrusData {
        blob_id: returned_blob_id,
        content_digest,
    }] = values.as_slice()
    else {
        panic!("the signed output must contain the uploaded reference");
    };
    assert_eq!(returned_blob_id, blob_id.as_bytes());
    assert_eq!(
        content_digest.as_slice(),
        sha256(&expected_bytes).as_slice()
    );
    assert_eq!(uploaded.lock().unwrap().as_ref(), Some(&expected_bytes));

    let reader = WalrusReader::new(&walrus.url(), expected_bytes.len()).unwrap();
    let returned_blob_id = std::str::from_utf8(returned_blob_id).unwrap();
    let readback = reader
        .read_verified(returned_blob_id, content_digest, expected_bytes.len())
        .await
        .unwrap();
    assert_eq!(readback, expected_bytes);

    // Changing stored bytes after signing must fail digest verification.
    uploaded.lock().unwrap().as_mut().unwrap()[0] ^= 1;
    let error = reader
        .read_verified(returned_blob_id, content_digest, expected_bytes.len())
        .await
        .unwrap_err();
    assert!(error.is::<WalrusContentDigestMismatch>());
    put.assert_async().await;
    get.assert_async().await;
}

#[tokio::test]
async fn changing_any_walrus_blob_id_or_digest_invalidates_the_response_signature() {
    let server = ToolServer::new::<TypedOutputTool>().await;
    let response = server.invoke("valid").await;
    assert_eq!(response.status, StatusCode::OK);
    response.verify(&response.body).unwrap();
    let output: OffchainToolOutput = bcs::from_bytes(&response.body).unwrap();

    for (port_index, value_index) in [(1, 0), (2, 1), (2, 2)] {
        for change_blob_id in [true, false] {
            let mut tampered = output.clone();
            let NexusValue::WalrusData {
                blob_id,
                content_digest,
            } = &mut tampered.ports[port_index].values[value_index]
            else {
                panic!("fixture must contain a Walrus reference");
            };
            if change_blob_id {
                blob_id[0] = b'B';
            } else {
                content_digest[0] ^= 1;
            }
            assert!(server.schema.conforms_raw_output(&tampered));
            let body = bcs::to_bytes(&tampered).unwrap();
            assert_ne!(body, response.body);
            assert!(matches!(
                response.verify(&body),
                Err(SignedHttpError::InvalidSignature)
            ));
        }
    }
}

#[tokio::test]
async fn malformed_typed_outputs_are_rejected_without_a_signature() {
    let server = ToolServer::new::<TypedOutputTool>().await;
    for case in [
        "blob_id",
        "short_digest",
        "long_digest",
        "variant",
        "port_name",
        "port_order",
        "missing_port",
        "extra_port",
        "empty_one",
        "multiple_one",
        "empty_many",
        "object_in_data",
    ] {
        let response = server.invoke(case).await;
        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR, "{case}");
        assert_eq!(
            response.headers["content-type"], "application/json",
            "{case}"
        );
        assert!(
            !response.headers.contains_key(HEADER_TOOL_SIGNATURE),
            "{case}"
        );
        assert!(
            !response.headers.contains_key(HEADER_SIGNATURE_VERSION),
            "{case}"
        );
        let error: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(error["error"], "output_serialization_error", "{case}");
    }
}
