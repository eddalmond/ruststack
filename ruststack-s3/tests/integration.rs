//! S3 integration tests using aws-sdk-s3

use aws_sdk_s3::config::BehaviorVersion;
use aws_sdk_s3::Client;
use ruststack_s3::{storage::EphemeralStorage, S3State};
use std::sync::Arc;

fn create_test_router() -> axum::Router {
    let storage = Arc::new(EphemeralStorage::new());
    let state = Arc::new(S3State { storage });

    axum::Router::new()
        .route("/", axum::routing::get(ruststack_s3::handlers::handle_root))
        .route(
            "/:bucket",
            axum::routing::any(ruststack_s3::handlers::handle_bucket),
        )
        .route(
            "/:bucket/:key",
            axum::routing::any(ruststack_s3::handlers::handle_object),
        )
        .with_state(state)
}

async fn create_test_client(endpoint: &str) -> Client {
    let shared_config = aws_config::defaults(BehaviorVersion::latest())
        .endpoint_url(endpoint)
        .region(aws_config::Region::new("us-east-2"))
        .credentials_provider(aws_sdk_s3::config::Credentials::new(
            "test", "test", None, None, "test",
        ))
        .load()
        .await;

    let s3_config = aws_sdk_s3::config::Builder::from(&shared_config)
        .force_path_style(true)
        .build();

    aws_sdk_s3::Client::from_conf(s3_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    async fn start_test_server() -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let endpoint = format!("http://{}", addr);

        let router = create_test_router();

        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        (endpoint, handle)
    }

    async fn create_bucket_via_http(endpoint: &str, bucket_name: &str) {
        let http = reqwest::Client::new();
        let url = format!("{}/{}", endpoint, bucket_name);
        let _ = http.put(url).send().await;
    }

    #[tokio::test]
    async fn test_list_buckets() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        let list_result = client.list_buckets().send().await;
        assert!(list_result.is_ok());

        let response = list_result.unwrap();
        assert_eq!(response.buckets().len(), 0);
    }

    #[tokio::test]
    async fn test_create_and_list_bucket() {
        let (endpoint, _handle) = start_test_server().await;

        create_bucket_via_http(&endpoint, "bucket1").await;

        let client = create_test_client(&endpoint).await;

        let list_result = client.list_buckets().send().await;
        assert!(list_result.is_ok());
        assert_eq!(list_result.unwrap().buckets().len(), 1);
    }

    #[tokio::test]
    async fn test_object_operations() {
        let (endpoint, _handle) = start_test_server().await;

        create_bucket_via_http(&endpoint, "bucket2").await;

        let client = create_test_client(&endpoint).await;

        let content = "Hello, S3!";
        let put_result = client
            .put_object()
            .bucket("bucket2")
            .key("test-key.txt")
            .body(content.as_bytes().to_vec().into())
            .send()
            .await;

        assert!(put_result.is_ok(), "PutObject failed: {:?}", put_result);

        let get_result = client
            .get_object()
            .bucket("bucket2")
            .key("test-key.txt")
            .send()
            .await;

        assert!(get_result.is_ok(), "GetObject failed: {:?}", get_result);

        let body = get_result.unwrap().body;
        let data = body.collect().await.unwrap().to_vec();
        assert_eq!(String::from_utf8_lossy(&data), content);
    }

    #[tokio::test]
    async fn test_delete_object() {
        let (endpoint, _handle) = start_test_server().await;

        create_bucket_via_http(&endpoint, "bucket3").await;

        let client = create_test_client(&endpoint).await;

        client
            .put_object()
            .bucket("bucket3")
            .key("to-delete.txt")
            .body(b"content".to_vec().into())
            .send()
            .await
            .unwrap();

        let delete_result = client
            .delete_object()
            .bucket("bucket3")
            .key("to-delete.txt")
            .send()
            .await;

        assert!(
            delete_result.is_ok(),
            "DeleteObject failed: {:?}",
            delete_result
        );

        let get_result = client
            .get_object()
            .bucket("bucket3")
            .key("to-delete.txt")
            .send()
            .await;

        assert!(get_result.is_err(), "Object should be gone after deletion");
    }
}
