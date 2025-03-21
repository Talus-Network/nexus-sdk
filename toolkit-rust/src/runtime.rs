use {
    crate::NexusTool,
    reqwest::Url,
    serde_json::json,
    warp::{
        filters::{host::Authority, path::FullPath},
        http::StatusCode,
        Filter,
        Rejection,
        Reply,
    },
};

/// Macro to bootstrap the runtime for a set of tools. The macro generates the
/// necessary routes for each tool and serves them on the provided address.
///
/// # Examples
///
/// ### One tool running on `127.0.0.1:8080`
///
/// ```ignore
/// use nexus_toolkit::bootstrap;
///
/// #[tokio::main]
/// async fn main() {
///     bootstrap!(YourTool);
/// }
/// ```
///
/// ### Multiple tools running on `127.0.0.1:8080`
///
/// ```ignore
/// use nexus_toolkit::bootstrap;
///
/// #[tokio::main]
/// async fn main() {
///     bootstrap!([YourTool, AnotherTool]);
/// }
/// ```
///
/// ### One tool running on the provided address
///
/// ```ignore
/// use nexus_toolkit::bootstrap;
///
/// #[tokio::main]
/// async fn main() {
///     bootstrap!(([127, 0, 0, 1], 8081), YourTool);
/// }
/// ```
///
/// ### Multiple tools running on the provided address
///
/// ```ignore
/// use nexus_toolkit::bootstrap;
///
/// #[tokio::main]
/// async fn main() {
///     bootstrap!(([127, 0, 0, 1], 8081), [YourTool, AnotherTool]);
/// }
/// ```
#[macro_export]
macro_rules! bootstrap {
    ($addr:expr, [$tool:ty $(, $next_tool:ty)* $(,)?]) => {{
        use {
            $crate::warp::{http::StatusCode, Filter},
        };

        // Create routes for each Tool in the bundle.
        let routes = $crate::routes_for_::<$tool>();
        $(let routes = routes.or($crate::routes_for_::<$next_tool>());)*

        // Add a default health route in case there is none in the root.
        let default_health_route = $crate::warp::get()
            .and($crate::warp::path("health"))
            .map(|| $crate::warp::reply::with_status("", StatusCode::OK));

        let routes = routes.or(default_health_route);

        // Serve the routes.
        $crate::warp::serve(routes).run($addr).await
    }};
    // Default address.
    ([$($tool:ty),+ $(,)?]) => {
        bootstrap!(([127, 0, 0, 1], 8080), [$($tool, )*]);
    };
    // Only 1 tool.
    ($addr:expr, $tool:ty) => {
        bootstrap!($addr, [$tool]);
    };
    // Only 1 tool with default address.
    ($tool:ty) => {
        bootstrap!(([127, 0, 0, 1], 8080), [$tool]);
    };
}

/// This function generates the necessary routes for a given [NexusTool].
///
/// **This is an internal function used by [bootstrap!] macro and should not be
/// used directly.**
#[doc(hidden)]
pub fn routes_for_<T: NexusTool>() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    // Force output schema to be an enum.
    let output_schema = json!(schemars::schema_for!(T::Output));

    if output_schema["oneOf"].is_null() {
        panic!("The output type must be an enum to generate the correct output schema.");
    }

    let base_path = T::path()
        .split("/")
        .filter(|s| !s.is_empty())
        .fold(warp::any().boxed(), |filter, segment| {
            filter.and(warp::path(segment.to_string())).boxed()
        });

    let health_route = warp::get()
        .and(base_path.clone())
        .and(warp::path("health"))
        .and_then(health_handler::<T>);

    // Meta path is tool base URL path and `/meta`.
    let meta_route = warp::get()
        .and(base_path.clone())
        .and(warp::path("meta"))
        .and(warp::filters::host::optional())
        .and(warp::path::full())
        .and_then(meta_handler::<T>);

    // Invoke path is tool base URL path and `/invoke`.
    let invoke_route = warp::post()
        .and(base_path)
        .and(warp::path("invoke"))
        .and(warp::body::json())
        .and_then(invoke_handler::<T>);

    health_route.or(meta_route).or(invoke_route)
}

async fn health_handler<T: NexusTool>() -> Result<impl Reply, Rejection> {
    let tool = T::new().await;

    let status = tool
        .health()
        .await
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    Ok(warp::reply::with_status("", status))
}

async fn meta_handler<T: NexusTool>(
    host: Option<Authority>,
    path: FullPath,
) -> Result<impl Reply, Rejection> {
    // If the host is malformed or not present, return a 400.
    let host = match host {
        Some(host) => host,
        None => {
            let reply = json!({
                "error": "host_header_required",
                "details": "Host header is required.",
            });

            return Ok(warp::reply::with_status(
                warp::reply::json(&reply),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    // Stripping 'meta' suffix from the path will give us the base path.
    let base_path = match path.as_str().strip_suffix("meta") {
        Some(base_path) => base_path,
        None => {
            // This is probably never reached as we create the endpoints
            // ourselves.
            let reply = json!({
                "error": "invalid_path",
                "details": "Meta path must end with '/meta'.",
            });

            return Ok(warp::reply::with_status(
                warp::reply::json(&reply),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    // Assume `http` for localhost, otherwise use `https`.
    //
    // TODO: This could probably be improved.
    let scheme = if host.host() == "localhost" {
        "http"
    } else {
        "https"
    };

    let url = match Url::parse(&format!("{scheme}://{host}{base_path}")) {
        Ok(url) => url,
        Err(e) => {
            let reply = json!({
                "error": "url_parsing_error",
                "details": e.to_string(),
            });

            return Ok(warp::reply::with_status(
                warp::reply::json(&reply),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&T::meta(url)),
        StatusCode::OK,
    ))
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

    let tool = T::new().await;

    // Invoke the tool logic.
    let output = tool.invoke(input).await;

    Ok(warp::reply::with_status(
        warp::reply::json(&output),
        StatusCode::OK,
    ))
}
