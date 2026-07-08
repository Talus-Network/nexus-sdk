#![cfg(feature = "walrus")]

use {
    anyhow::Result,
    mockito::{Server, ServerGuard},
    nexus_sdk::walrus::{BlobObject, BlobStorage, NewlyCreated, StorageInfo, WalrusClient},
    serde::{Deserialize, Serialize},
    std::path::PathBuf,
    tempfile::tempdir,
};

const EPOCHS: u8 = 1;
const TEST_CONTENT: &[u8] = b"Hello, World!";

/// Setup mock server for Walrus testing
async fn setup_mock_server() -> Result<(ServerGuard, WalrusClient)> {
    // Create mock server
    let server = Server::new_async().await;
    let server_url = server.url();

    // Create a client with the base URL set to our mock server
    let client = reqwest::Client::builder().build().unwrap();

    // Create a Walrus client that points to our mock server
    let walrus_client = WalrusClient::builder()
        .with_client(client)
        .with_publisher_url(&server_url)
        .with_aggregator_url(&server_url)
        .build();

    Ok((server, walrus_client))
}

/// Helper to create a temp file with content
async fn create_temp_file(content: &[u8]) -> Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempdir()?;
    let file_path = dir.path().join("test_file.txt");
    tokio::fs::write(&file_path, content).await?;
    Ok((dir, file_path))
}

#[tokio::test]
async fn test_upload_file() -> Result<()> {
    let (mut server, client) = setup_mock_server().await?;

    // Create test file
    let (_dir, file_path) = create_temp_file(TEST_CONTENT).await?;

    // Setup mock response
    let mock_response = StorageInfo {
        newly_created: Some(NewlyCreated {
            blob_object: BlobObject {
                blob_id: "test_blob_id".to_string(),
                id: "test_object_id".to_string(),
                storage: BlobStorage { end_epoch: 100 },
            },
        }),
        already_certified: None,
    };

    let mock = server
        .mock(
            "PUT",
            mockito::Matcher::Regex(format!("/v1/blobs\\?epochs={}", EPOCHS)),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_response)?)
        .create_async()
        .await;

    // Test upload_file
    let storage_info = client.upload_file(&file_path, EPOCHS, None).await?;

    // Verify response
    assert!(storage_info.newly_created.is_some());
    let blob_object = storage_info.newly_created.unwrap().blob_object;
    assert_eq!(blob_object.blob_id, "test_blob_id");
    assert_eq!(blob_object.id, "test_object_id");
    assert_eq!(blob_object.storage.end_epoch, 100);
    assert!(storage_info.already_certified.is_none());

    // Verify the request was made
    mock.assert_async().await;

    Ok(())
}

#[tokio::test]
async fn test_upload_bytes() -> Result<()> {
    let (mut server, client) = setup_mock_server().await?;

    let test_data = b"raw payload".to_vec();

    // Setup mock response
    let mock_response = StorageInfo {
        newly_created: Some(NewlyCreated {
            blob_object: BlobObject {
                blob_id: "bytes_blob_id".to_string(),
                id: "bytes_object_id".to_string(),
                storage: BlobStorage { end_epoch: 200 },
            },
        }),
        already_certified: None,
    };

    let mock = server
        .mock(
            "PUT",
            mockito::Matcher::Regex(format!("/v1/blobs\\?epochs={}", EPOCHS)),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_response)?)
        .create_async()
        .await;

    let storage_info = client.upload_bytes(test_data, EPOCHS, None).await?;

    // Verify response
    assert!(storage_info.newly_created.is_some());
    let blob_object = storage_info.newly_created.unwrap().blob_object;
    assert_eq!(blob_object.blob_id, "bytes_blob_id");
    assert_eq!(blob_object.id, "bytes_object_id");
    assert_eq!(blob_object.storage.end_epoch, 200);
    assert!(storage_info.already_certified.is_none());

    // Verify the request was made
    mock.assert_async().await;

    Ok(())
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct JsonPayload {
    message: String,
    count: u64,
}

#[tokio::test]
async fn test_upload_json() -> Result<()> {
    let (mut server, client) = setup_mock_server().await?;

    let payload = JsonPayload {
        message: "hello".to_string(),
        count: 7,
    };

    let mock_response = StorageInfo {
        newly_created: Some(NewlyCreated {
            blob_object: BlobObject {
                blob_id: "json_blob_id".to_string(),
                id: "json_object_id".to_string(),
                storage: BlobStorage { end_epoch: 300 },
            },
        }),
        already_certified: None,
    };

    let mock = server
        .mock(
            "PUT",
            mockito::Matcher::Regex(format!("/v1/blobs\\?epochs={}", EPOCHS)),
        )
        .match_header("content-type", "application/json")
        .match_body(mockito::Matcher::Exact(serde_json::to_string(&payload)?))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_response)?)
        .create_async()
        .await;

    let storage_info = client.upload_json(&payload, EPOCHS, None).await?;

    let blob_object = storage_info
        .newly_created
        .expect("upload_json should return a created blob")
        .blob_object;
    assert_eq!(blob_object.blob_id, "json_blob_id");
    assert_eq!(blob_object.id, "json_object_id");
    assert_eq!(blob_object.storage.end_epoch, 300);

    mock.assert_async().await;

    Ok(())
}

#[tokio::test]
async fn test_download_file() -> Result<()> {
    let (mut server, client) = setup_mock_server().await?;

    // Create a temp directory for output
    let dir = tempdir()?;
    let output_path = dir.path().join("downloaded_file.txt");

    // Setup mock response

    let mock = server
        .mock("GET", "/v1/blobs/test_blob_id")
        .with_status(200)
        .with_body(TEST_CONTENT)
        .create_async()
        .await;

    // Test download_file
    client.download_file("test_blob_id", &output_path).await?;

    // Verify the downloaded content
    let downloaded_content = tokio::fs::read(&output_path).await?;
    assert_eq!(downloaded_content.len(), TEST_CONTENT.len());
    assert_eq!(downloaded_content, TEST_CONTENT);

    // Verify the request was made
    mock.assert_async().await;

    // Keep the directory alive until the end of the test
    drop(dir);

    Ok(())
}

#[tokio::test]
async fn test_read_bytes() -> Result<()> {
    let (mut server, client) = setup_mock_server().await?;

    let test_data = b"downloaded bytes".to_vec();

    let mock = server
        .mock("GET", "/v1/blobs/bytes_blob_id")
        .with_status(200)
        .with_body(test_data.clone())
        .create_async()
        .await;

    let result = client.read_file("bytes_blob_id").await?;

    assert_eq!(result, test_data);

    // Verify the request was made
    mock.assert_async().await;

    Ok(())
}

#[tokio::test]
async fn test_read_json() -> Result<()> {
    let (mut server, client) = setup_mock_server().await?;

    let payload = JsonPayload {
        message: "downloaded".to_string(),
        count: 9,
    };

    let mock = server
        .mock("GET", "/v1/blobs/json_blob_id")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&payload)?)
        .create_async()
        .await;

    let result: JsonPayload = client.read_json("json_blob_id").await?;

    assert_eq!(result, payload);
    mock.assert_async().await;

    Ok(())
}

#[tokio::test]
async fn test_verify_blob() -> Result<()> {
    let (mut server, client) = setup_mock_server().await?;

    // Setup mock response for existing blob
    let mock_exists = server
        .mock("HEAD", "/v1/blobs/existing_blob_id")
        .with_status(200)
        .create_async()
        .await;

    // Setup mock response for non-existing blob
    let mock_not_exists = server
        .mock("HEAD", "/v1/blobs/nonexistent_blob_id")
        .with_status(404)
        .create_async()
        .await;

    // Test verify_blob for existing blob
    let exists = client.verify_blob("existing_blob_id").await?;
    assert!(exists);

    // Test verify_blob for non-existing blob
    let not_exists = client.verify_blob("nonexistent_blob_id").await?;
    assert!(!not_exists);

    // Verify the requests were made
    mock_exists.assert_async().await;
    mock_not_exists.assert_async().await;

    Ok(())
}

#[tokio::test]
async fn test_read_file() -> Result<()> {
    let (mut server, client) = setup_mock_server().await?;

    // Setup mock response
    let mock = server
        .mock("GET", "/v1/blobs/test_blob_id")
        .with_status(200)
        .with_body(TEST_CONTENT)
        .create_async()
        .await;

    // Test read_file
    let content = client.read_file("test_blob_id").await?;

    // Verify the content was read correctly
    assert_eq!(content.len(), TEST_CONTENT.len());
    assert_eq!(content, TEST_CONTENT);

    // Verify the request was made
    mock.assert_async().await;

    Ok(())
}

#[tokio::test]
async fn test_error_handling() -> Result<()> {
    let (mut server, client) = setup_mock_server().await?;

    // Setup mock for server error
    let mock_error = server
        .mock("GET", "/v1/blobs/error_blob_id")
        .with_status(500)
        .with_body("Internal Server Error")
        .create_async()
        .await;

    // Test error handling
    let result = client.read_file("error_blob_id").await;
    assert!(result.is_err());

    // Verify the request was made
    mock_error.assert_async().await;

    Ok(())
}
