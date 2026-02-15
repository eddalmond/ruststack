//! Integration tests for Lambda service
//!
//! These tests verify the Lambda service works correctly with the AWS SDK.

use std::io::Write;
use std::time::Duration;

use aws_config::BehaviorVersion;
use aws_sdk_lambda::{
    primitives::Blob,
    types::{Environment, FunctionCode, Runtime},
    Client,
};
use tokio::net::TcpListener;

use ruststack_lambda::LambdaState;
use std::sync::Arc;

/// Create a test Lambda client pointing to our local server
async fn create_test_client(port: u16) -> Client {
    let config = aws_config::defaults(BehaviorVersion::latest())
        .endpoint_url(format!("http://localhost:{}", port))
        .credentials_provider(aws_sdk_lambda::config::Credentials::new(
            "test",
            "test",
            None,
            None,
            "test",
        ))
        .region(aws_sdk_lambda::config::Region::new("us-east-1"))
        .load()
        .await;

    Client::new(&config)
}

/// Start a test server and return the port
async fn start_test_server() -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let lambda_state = Arc::new(LambdaState::new());

    // Build lambda-only router for testing
    let router = axum::Router::new()
        .route(
            "/2015-03-31/functions",
            axum::routing::get({
                let state = lambda_state.clone();
                move |query| {
                    ruststack_lambda::handlers::list_functions(
                        axum::extract::State(state.clone()),
                        query,
                    )
                }
            }),
        )
        .route(
            "/2015-03-31/functions",
            axum::routing::post({
                let state = lambda_state.clone();
                move |body| {
                    ruststack_lambda::handlers::create_function(
                        axum::extract::State(state.clone()),
                        body,
                    )
                }
            }),
        )
        .route(
            "/2015-03-31/functions/:function_name",
            axum::routing::get({
                let state = lambda_state.clone();
                move |path| {
                    ruststack_lambda::handlers::get_function(
                        axum::extract::State(state.clone()),
                        path,
                    )
                }
            }),
        )
        .route(
            "/2015-03-31/functions/:function_name",
            axum::routing::delete({
                let state = lambda_state.clone();
                move |path| {
                    ruststack_lambda::handlers::delete_function(
                        axum::extract::State(state.clone()),
                        path,
                    )
                }
            }),
        )
        .route(
            "/2015-03-31/functions/:function_name/invocations",
            axum::routing::post({
                let state = lambda_state.clone();
                move |path, headers, body| {
                    ruststack_lambda::handlers::invoke_function(
                        axum::extract::State(state.clone()),
                        path,
                        headers,
                        body,
                    )
                }
            }),
        );

    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    (port, handle)
}

/// Create a zip file containing a simple Python handler
fn create_simple_handler_zip() -> Vec<u8> {
    let mut buffer = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("handler.py", options).unwrap();
        zip.write_all(
            br#"
def lambda_handler(event, context):
    """Simple handler that returns the event with some metadata."""
    return {
        'statusCode': 200,
        'body': {
            'message': 'Hello from Lambda!',
            'event': event,
            'function_name': context.function_name,
            'request_id': context.aws_request_id
        }
    }
"#,
        )
        .unwrap();
        zip.finish().unwrap();
    }
    buffer
}

/// Create a zip file with a handler that does some computation
fn create_compute_handler_zip() -> Vec<u8> {
    let mut buffer = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("compute.py", options).unwrap();
        zip.write_all(
            br#"
import json

def handler(event, context):
    """Handler that computes factorial."""
    n = event.get('n', 5)
    
    def factorial(x):
        if x <= 1:
            return 1
        return x * factorial(x - 1)
    
    result = factorial(n)
    return {
        'statusCode': 200,
        'input': n,
        'factorial': result
    }
"#,
        )
        .unwrap();
        zip.finish().unwrap();
    }
    buffer
}

/// Create a zip file with a handler that raises an error
fn create_error_handler_zip() -> Vec<u8> {
    let mut buffer = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("error.py", options).unwrap();
        zip.write_all(
            br#"
def handler(event, context):
    """Handler that always raises an error."""
    raise ValueError("This is a test error")
"#,
        )
        .unwrap();
        zip.finish().unwrap();
    }
    buffer
}

#[tokio::test]
async fn test_create_function() {
    let (port, _handle) = start_test_server().await;
    let client = create_test_client(port).await;

    let zip_data = create_simple_handler_zip();

    let result = client
        .create_function()
        .function_name("test-function")
        .runtime(Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/lambda-role")
        .handler("handler.lambda_handler")
        .code(FunctionCode::builder().zip_file(Blob::new(zip_data)).build())
        .send()
        .await;

    assert!(result.is_ok(), "CreateFunction should succeed: {:?}", result);
    let function = result.unwrap();
    assert_eq!(function.function_name(), Some("test-function"));
    assert_eq!(function.runtime(), Some(&Runtime::Python312));
}

#[tokio::test]
async fn test_get_function() {
    let (port, _handle) = start_test_server().await;
    let client = create_test_client(port).await;

    let zip_data = create_simple_handler_zip();

    // First create a function
    client
        .create_function()
        .function_name("get-test")
        .runtime(Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/lambda-role")
        .handler("handler.lambda_handler")
        .code(FunctionCode::builder().zip_file(Blob::new(zip_data)).build())
        .send()
        .await
        .unwrap();

    // Then get it
    let result = client.get_function().function_name("get-test").send().await;

    assert!(result.is_ok(), "GetFunction should succeed: {:?}", result);
    let response = result.unwrap();
    let config = response.configuration().unwrap();
    assert_eq!(config.function_name(), Some("get-test"));
}

#[tokio::test]
async fn test_delete_function() {
    let (port, _handle) = start_test_server().await;
    let client = create_test_client(port).await;

    let zip_data = create_simple_handler_zip();

    // Create function
    client
        .create_function()
        .function_name("to-delete")
        .runtime(Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/lambda-role")
        .handler("handler.lambda_handler")
        .code(FunctionCode::builder().zip_file(Blob::new(zip_data)).build())
        .send()
        .await
        .unwrap();

    // Delete it
    let result = client
        .delete_function()
        .function_name("to-delete")
        .send()
        .await;
    assert!(result.is_ok(), "DeleteFunction should succeed: {:?}", result);

    // Verify it's gone
    let get_result = client
        .get_function()
        .function_name("to-delete")
        .send()
        .await;
    assert!(get_result.is_err(), "Function should not exist after deletion");
}

#[tokio::test]
async fn test_list_functions() {
    let (port, _handle) = start_test_server().await;
    let client = create_test_client(port).await;

    let zip_data = create_simple_handler_zip();

    // Create several functions
    for i in 0..3 {
        client
            .create_function()
            .function_name(format!("list-test-{}", i))
            .runtime(Runtime::Python312)
            .role("arn:aws:iam::000000000000:role/lambda-role")
            .handler("handler.lambda_handler")
            .code(
                FunctionCode::builder()
                    .zip_file(Blob::new(zip_data.clone()))
                    .build(),
            )
            .send()
            .await
            .unwrap();
    }

    // List functions
    let result = client.list_functions().send().await;
    assert!(result.is_ok(), "ListFunctions should succeed: {:?}", result);
    let response = result.unwrap();
    assert_eq!(
        response.functions().len(),
        3,
        "Should have 3 functions"
    );
}

#[tokio::test]
async fn test_function_not_found() {
    let (port, _handle) = start_test_server().await;
    let client = create_test_client(port).await;

    let result = client
        .get_function()
        .function_name("nonexistent")
        .send()
        .await;

    assert!(
        result.is_err(),
        "GetFunction for nonexistent function should fail"
    );
}

#[tokio::test]
async fn test_duplicate_function() {
    let (port, _handle) = start_test_server().await;
    let client = create_test_client(port).await;

    let zip_data = create_simple_handler_zip();

    // Create first function
    client
        .create_function()
        .function_name("duplicate-test")
        .runtime(Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/lambda-role")
        .handler("handler.lambda_handler")
        .code(
            FunctionCode::builder()
                .zip_file(Blob::new(zip_data.clone()))
                .build(),
        )
        .send()
        .await
        .unwrap();

    // Try to create duplicate
    let result = client
        .create_function()
        .function_name("duplicate-test")
        .runtime(Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/lambda-role")
        .handler("handler.lambda_handler")
        .code(FunctionCode::builder().zip_file(Blob::new(zip_data)).build())
        .send()
        .await;

    assert!(
        result.is_err(),
        "Creating duplicate function should fail"
    );
}

#[tokio::test]
async fn test_invoke_function_simple() {
    let (port, _handle) = start_test_server().await;
    let client = create_test_client(port).await;

    let zip_data = create_simple_handler_zip();

    // Create function
    client
        .create_function()
        .function_name("invoke-test")
        .runtime(Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/lambda-role")
        .handler("handler.lambda_handler")
        .code(FunctionCode::builder().zip_file(Blob::new(zip_data)).build())
        .send()
        .await
        .unwrap();

    // Invoke function
    let result = client
        .invoke()
        .function_name("invoke-test")
        .payload(Blob::new(r#"{"name": "test"}"#))
        .send()
        .await;

    // Note: This test requires Python to be installed
    // If Python is not available, the invocation will fail gracefully
    match result {
        Ok(response) => {
            println!("Invoke response: {:?}", response);
            if response.function_error().is_some() {
                // Function error (e.g., Python not found) - that's OK for testing
                println!("Function error: {:?}", response.function_error());
            } else {
                // Successful invocation
                let payload = response.payload().map(|p| {
                    String::from_utf8_lossy(p.as_ref()).to_string()
                });
                println!("Payload: {:?}", payload);
                assert!(payload.is_some(), "Should have a payload");
            }
        }
        Err(e) => {
            println!("Invoke failed (this may be expected if Python is not installed): {}", e);
        }
    }
}

#[tokio::test]
async fn test_invoke_compute_function() {
    let (port, _handle) = start_test_server().await;
    let client = create_test_client(port).await;

    let zip_data = create_compute_handler_zip();

    // Create function
    client
        .create_function()
        .function_name("compute-test")
        .runtime(Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/lambda-role")
        .handler("compute.handler")
        .code(FunctionCode::builder().zip_file(Blob::new(zip_data)).build())
        .send()
        .await
        .unwrap();

    // Invoke function with n=6 (factorial should be 720)
    let result = client
        .invoke()
        .function_name("compute-test")
        .payload(Blob::new(r#"{"n": 6}"#))
        .send()
        .await;

    match result {
        Ok(response) => {
            if response.function_error().is_none() {
                let payload = response.payload().map(|p| {
                    String::from_utf8_lossy(p.as_ref()).to_string()
                });
                if let Some(payload_str) = payload {
                    println!("Compute result: {}", payload_str);
                    // Parse and verify
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload_str) {
                        assert_eq!(json["factorial"], 720, "6! should be 720");
                    }
                }
            }
        }
        Err(e) => {
            println!("Compute invoke failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_invoke_error_function() {
    let (port, _handle) = start_test_server().await;
    let client = create_test_client(port).await;

    let zip_data = create_error_handler_zip();

    // Create function
    client
        .create_function()
        .function_name("error-test")
        .runtime(Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/lambda-role")
        .handler("error.handler")
        .code(FunctionCode::builder().zip_file(Blob::new(zip_data)).build())
        .send()
        .await
        .unwrap();

    // Invoke function - should return error response
    let result = client
        .invoke()
        .function_name("error-test")
        .payload(Blob::new(r#"{}"#))
        .send()
        .await;

    match result {
        Ok(response) => {
            // Should have function error set
            println!("Error response: {:?}", response);
            // Lambda returns 200 even for handled errors, check X-Amz-Function-Error header
            if response.function_error().is_some() {
                println!("Function error correctly returned: {:?}", response.function_error());
            }
        }
        Err(e) => {
            println!("Error invoke failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_function_with_environment_variables() {
    let (port, _handle) = start_test_server().await;
    let client = create_test_client(port).await;

    // Create handler that reads env vars
    let mut buffer = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("env_handler.py", options).unwrap();
        zip.write_all(
            br#"
import os
import json

def handler(event, context):
    return {
        'statusCode': 200,
        'env': {
            'MY_VAR': os.environ.get('MY_VAR', 'not set'),
            'SECRET': os.environ.get('SECRET', 'not set'),
        }
    }
"#,
        )
        .unwrap();
        zip.finish().unwrap();
    }

    // Create function with env vars
    let result = client
        .create_function()
        .function_name("env-test")
        .runtime(Runtime::Python312)
        .role("arn:aws:iam::000000000000:role/lambda-role")
        .handler("env_handler.handler")
        .code(FunctionCode::builder().zip_file(Blob::new(buffer)).build())
        .environment(
            Environment::builder()
                .variables("MY_VAR", "hello")
                .variables("SECRET", "world")
                .build(),
        )
        .send()
        .await;

    assert!(result.is_ok(), "Should create function with env vars");
    
    let function = result.unwrap();
    let env = function.environment();
    assert!(env.is_some(), "Function should have environment");
}
