mod nexus_tool;

// Re-export types that are used in the trait definition.
pub use {anyhow::Result as AnyResult, nexus_tool::NexusTool, warp::http::StatusCode};
use {
    serde_json::json,
    std::net::SocketAddr,
    warp::{Filter, Rejection, Reply},
};

// Re-export schemars derive macros.
#[cfg(feature = "schemars_derive")]
extern crate schemars_derive;

#[cfg(feature = "schemars_derive")]
pub use schemars_derive::JsonSchema;

#[cfg(feature = "schemars_derive")]
extern crate schemars;

#[cfg(feature = "schemars_derive")]
pub use schemars;

// Re-export serde derive macros.
#[cfg(feature = "serde_derive")]
extern crate serde_derive;

#[cfg(feature = "serde_derive")]
pub use serde_derive::{Deserialize, Serialize};

#[cfg(feature = "serde_derive")]
extern crate serde;

#[cfg(feature = "serde_derive")]
pub use serde;

/// Bootstraps the Nexus Tool with the given address. It starts a Warp server
/// with the endpoints generated by the provided Nexus Tool.
pub async fn bootstrap<T: NexusTool>(addr: impl Into<SocketAddr>) {
    let addr = addr.into();

    // Check that the output type is an enum.
    let output_schema = json!(schemars::schema_for!(T::Output));

    if output_schema["oneOf"].is_null() {
        panic!("The output type must be an enum to generate the correct output schema.");
    }

    let health_route = warp::get()
        .and(warp::path("health"))
        .and_then(health_handler::<T>);

    let meta_route = warp::get()
        .and(warp::path("meta"))
        .map(move || warp::reply::json(&T::meta(addr)));

    let invoke_route = warp::post()
        .and(warp::path("invoke"))
        .and(warp::body::json())
        .and_then(invoke_handler::<T>);

    let routes = health_route.or(meta_route).or(invoke_route);

    warp::serve(routes).run(addr).await
}

async fn health_handler<T: NexusTool>() -> Result<impl Reply, Rejection> {
    let status = T::health()
        .await
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    Ok(warp::reply::with_status("", status))
}

async fn invoke_handler<T: NexusTool>(input: serde_json::Value) -> Result<impl Reply, Rejection> {
    // Deserialize the input payload into [T::Input].
    let input = match serde_json::from_value(input) {
        Ok(input) => input,
        Err(e) => {
            let reply = json!({
                "error": "input_deserialization_error",
                "details": e.to_string(),
            });

            // Reply with 422 if we can't parse the input data.
            return Ok(warp::reply::with_status(
                warp::reply::json(&reply),
                StatusCode::UNPROCESSABLE_ENTITY,
            ));
        }
    };

    // Invoke the tool logic.
    match T::invoke(input).await {
        Ok(output) => Ok(warp::reply::with_status(
            warp::reply::json(&output),
            StatusCode::OK,
        )),
        Err(e) => {
            let reply = json!({
                "error": "tool_invocation_error",
                "details": e.to_string(),
            });

            // Reply with 500 if the tool invocation fails.
            return Ok(warp::reply::with_status(
                warp::reply::json(&reply),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    }
}
