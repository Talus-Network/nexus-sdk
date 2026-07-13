//! Walrus blob storage integration for WASM.
//!
//! Provides HTTP-based upload to Walrus publisher for remote port data storage.

use {
    js_sys::Promise,
    serde::Deserialize,
    wasm_bindgen::prelude::*,
    wasm_bindgen_futures::JsFuture,
};

/// Response from Walrus publisher PUT /v1/blobs
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageInfo {
    newly_created: Option<NewlyCreated>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewlyCreated {
    blob_object: BlobObject,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlobObject {
    blob_id: String,
}

/// Upload JSON data to Walrus and return the blob ID.
///
/// Uses the Walrus publisher API: PUT {publisher_url}/v1/blobs?epochs={epochs}
/// with the JSON content as body.
///
/// # Arguments
/// * `publisher_url` - Walrus publisher URL (e.g. https://publisher.walrus-testnet.walrus.space)
/// * `data_json` - JSON string of the data to upload
/// * `save_for_epochs` - Number of epochs to store the data (1-53)
///
/// # Returns
/// A Promise that resolves to the blob ID string, or rejects with an error message.
#[wasm_bindgen]
pub fn upload_json_to_walrus(publisher_url: &str, data_json: &str, save_for_epochs: u8) -> Promise {
    let url = format!("{}/v1/blobs?epochs={}", publisher_url, save_for_epochs);

    let init = web_sys::RequestInit::new();
    init.set_method("PUT");
    init.set_mode(web_sys::RequestMode::Cors);
    let body = JsValue::from_str(data_json);
    init.set_body(&body);

    let request =
        web_sys::Request::new_with_str_and_init(&url, &init).expect("Failed to create request");

    request
        .headers()
        .set("Content-Type", "application/json")
        .expect("Failed to set Content-Type header");

    let window = web_sys::window().expect("No window");
    let fetch_promise = window.fetch_with_request(&request);

    wasm_bindgen_futures::future_to_promise(async move {
        let resp_value = JsFuture::from(fetch_promise).await.map_err(|e| {
            JsValue::from(format!(
                "Walrus upload request failed: {}",
                js_sys::Reflect::get(&e, &JsValue::from_str("message"))
                    .ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))
        })?;

        let resp: web_sys::Response = resp_value
            .dyn_into()
            .map_err(|_| JsValue::from("Fetch returned non-Response"))?;

        if !resp.ok() {
            let status = resp.status();
            let text = JsFuture::from(
                resp.text()
                    .map_err(|_| JsValue::from("Failed to get response text"))?,
            )
            .await
            .map_err(|_| JsValue::from("Failed to read response body"))?;
            let text_str = text.as_string().unwrap_or_default();
            return Err(JsValue::from(format!(
                "Walrus API error {}: {}",
                status, text_str
            )));
        }

        let json = JsFuture::from(
            resp.json()
                .map_err(|_| JsValue::from("Failed to get JSON"))?,
        )
        .await
        .map_err(|_| JsValue::from("Failed to parse JSON response"))?;

        let info: StorageInfo = serde_wasm_bindgen::from_value(json)
            .map_err(|e| JsValue::from(format!("Failed to parse Walrus response: {}", e)))?;

        let blob_id = info
            .newly_created
            .ok_or_else(|| JsValue::from("Walrus response missing newlyCreated"))?
            .blob_object
            .blob_id;

        Ok(JsValue::from(blob_id))
    })
}
