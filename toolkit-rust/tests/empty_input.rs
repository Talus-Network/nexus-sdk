use {
    nexus_sdk::{fqn, types::NexusData, ToolFqn},
    nexus_toolkit::{runtime::routes_for_with_config_, NexusTool, ToolkitRuntimeConfig},
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::json,
    std::sync::Arc,
    warp::http::StatusCode,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input {}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Output {
    Ok {},
}

struct EmptyInputTool;

impl NexusTool for EmptyInputTool {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("domain.author.empty@1")
    }

    fn description() -> &'static str {
        "Returns an empty result without inputs."
    }

    async fn health(&self) -> anyhow::Result<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, _: Self::Input) -> Self::Output {
        Output::Ok {}
    }
}

#[tokio::test]
async fn empty_input_tool_accepts_only_empty_transport() {
    let config = Arc::new(ToolkitRuntimeConfig::from_json_str("{}").unwrap());
    let routes = routes_for_with_config_::<EmptyInputTool>(config);
    let response = warp::test::request()
        .method("POST")
        .path("/invoke")
        .json(&json!({"ports": []}))
        .reply(&routes)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body().as_ref(), b"\x02ok\x00");

    let unexpected = NexusData::inline_data(b"1")
        .unwrap()
        .to_json_value()
        .unwrap();
    for input in [
        json!({}),
        json!({"ports": [{"port_name": "unexpected", "value": unexpected}]}),
    ] {
        let response = warp::test::request()
            .method("POST")
            .path("/invoke")
            .json(&input)
            .reply(&routes)
            .await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
