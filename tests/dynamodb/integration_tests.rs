//! DynamoDB integration tests using aws-sdk-dynamodb
//!
//! These tests verify the DynamoDB implementation against the official AWS SDK.

use aws_sdk_dynamodb::{
    config::BehaviorVersion,
    types::{
        AttributeDefinition, AttributeValue, GlobalSecondaryIndex, KeySchemaElement, KeyType,
        Projection, ProjectionType, ProvisionedThroughput, ScalarAttributeType,
    },
    Client,
};
use axum::{body::Body, extract::State, http::Request, Router};
use ruststack_dynamodb::{handlers::handle_request, DynamoDBState, DynamoDBStorage};
use std::sync::Arc;
use tower::ServiceExt;

/// Create a test DynamoDB router
fn create_test_router() -> Router {
    let storage = Arc::new(DynamoDBStorage::new());
    let state = Arc::new(DynamoDBState { storage });

    Router::new()
        .route("/", axum::routing::post(handle_request))
        .with_state(state)
}

/// Create an AWS SDK client pointing to our test server
async fn create_test_client(endpoint: &str) -> Client {
    let config = aws_config::defaults(BehaviorVersion::v2023_11_09())
        .endpoint_url(endpoint)
        .region(aws_config::Region::new("us-east-1"))
        .credentials_provider(aws_credential_types::Credentials::new(
            "test", "test", None, None, "test",
        ))
        .load()
        .await;

    Client::new(&config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// Helper to start a test server
    async fn start_test_server() -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let endpoint = format!("http://{}", addr);

        let router = create_test_router();

        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        // Give server a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        (endpoint, handle)
    }

    #[tokio::test]
    async fn test_create_and_describe_table() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        // Create table
        let result = client
            .create_table()
            .table_name("TestTable")
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("pk")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("pk")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();

        assert!(result.table_description().is_some());
        let desc = result.table_description().unwrap();
        assert_eq!(desc.table_name(), Some("TestTable"));

        // Describe table
        let describe_result = client
            .describe_table()
            .table_name("TestTable")
            .send()
            .await
            .unwrap();

        assert!(describe_result.table().is_some());
        let table = describe_result.table().unwrap();
        assert_eq!(table.table_name(), Some("TestTable"));
    }

    #[tokio::test]
    async fn test_put_and_get_item() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        // Create table first
        client
            .create_table()
            .table_name("Items")
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("id")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("id")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();

        // Put item
        client
            .put_item()
            .table_name("Items")
            .item("id", AttributeValue::S("item1".to_string()))
            .item("name", AttributeValue::S("Test Item".to_string()))
            .item("price", AttributeValue::N("99.99".to_string()))
            .send()
            .await
            .unwrap();

        // Get item
        let result = client
            .get_item()
            .table_name("Items")
            .key("id", AttributeValue::S("item1".to_string()))
            .send()
            .await
            .unwrap();

        assert!(result.item().is_some());
        let item = result.item().unwrap();
        assert_eq!(item.get("name").unwrap().as_s().unwrap(), "Test Item");
        assert_eq!(item.get("price").unwrap().as_n().unwrap(), "99.99");
    }

    #[tokio::test]
    async fn test_update_item() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        // Create table
        client
            .create_table()
            .table_name("Counters")
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("name")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("name")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();

        // Put initial item
        client
            .put_item()
            .table_name("Counters")
            .item("name", AttributeValue::S("views".to_string()))
            .item("count", AttributeValue::N("100".to_string()))
            .send()
            .await
            .unwrap();

        // Update with increment
        let result = client
            .update_item()
            .table_name("Counters")
            .key("name", AttributeValue::S("views".to_string()))
            .update_expression("SET #count = #count + :inc")
            .expression_attribute_names("#count", "count")
            .expression_attribute_values(":inc", AttributeValue::N("1".to_string()))
            .return_values(aws_sdk_dynamodb::types::ReturnValue::AllNew)
            .send()
            .await
            .unwrap();

        let attrs = result.attributes().unwrap();
        assert_eq!(attrs.get("count").unwrap().as_n().unwrap(), "101");
    }

    #[tokio::test]
    async fn test_conditional_put_fails() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        // Create table
        client
            .create_table()
            .table_name("Unique")
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("id")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("id")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();

        // Put item
        client
            .put_item()
            .table_name("Unique")
            .item("id", AttributeValue::S("item1".to_string()))
            .send()
            .await
            .unwrap();

        // Try to put same item with condition that it doesn't exist
        let result = client
            .put_item()
            .table_name("Unique")
            .item("id", AttributeValue::S("item1".to_string()))
            .condition_expression("attribute_not_exists(id)")
            .send()
            .await;

        // Should fail with ConditionalCheckFailedException
        assert!(result.is_err());
        let err = result.unwrap_err();
        let service_err = err.into_service_error();
        assert!(service_err.is_conditional_check_failed_exception());
    }

    #[tokio::test]
    async fn test_query_with_composite_key() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        // Create table with composite key
        client
            .create_table()
            .table_name("Orders")
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("customerId")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("orderId")
                    .key_type(KeyType::Range)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("customerId")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("orderId")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();

        // Add orders
        for i in 1..=5 {
            client
                .put_item()
                .table_name("Orders")
                .item("customerId", AttributeValue::S("cust1".to_string()))
                .item("orderId", AttributeValue::S(format!("order{:03}", i)))
                .item("amount", AttributeValue::N(format!("{}", i * 100)))
                .send()
                .await
                .unwrap();
        }

        // Query all orders for customer
        let result = client
            .query()
            .table_name("Orders")
            .key_condition_expression("customerId = :cid")
            .expression_attribute_values(":cid", AttributeValue::S("cust1".to_string()))
            .send()
            .await
            .unwrap();

        assert_eq!(result.count(), 5);

        // Query with range condition
        let result = client
            .query()
            .table_name("Orders")
            .key_condition_expression("customerId = :cid AND orderId > :oid")
            .expression_attribute_values(":cid", AttributeValue::S("cust1".to_string()))
            .expression_attribute_values(":oid", AttributeValue::S("order002".to_string()))
            .send()
            .await
            .unwrap();

        assert_eq!(result.count(), 3); // order003, order004, order005
    }

    #[tokio::test]
    async fn test_scan_with_filter() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        // Create table
        client
            .create_table()
            .table_name("Products")
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("id")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("id")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();

        // Add products
        let categories = [
            "Electronics",
            "Books",
            "Electronics",
            "Clothing",
            "Electronics",
        ];
        for (i, cat) in categories.iter().enumerate() {
            client
                .put_item()
                .table_name("Products")
                .item("id", AttributeValue::S(format!("prod{}", i)))
                .item("category", AttributeValue::S(cat.to_string()))
                .item("price", AttributeValue::N(format!("{}", (i + 1) * 10)))
                .send()
                .await
                .unwrap();
        }

        // Scan with filter
        let result = client
            .scan()
            .table_name("Products")
            .filter_expression("category = :cat")
            .expression_attribute_values(":cat", AttributeValue::S("Electronics".to_string()))
            .send()
            .await
            .unwrap();

        assert_eq!(result.count(), 3);
    }

    #[tokio::test]
    async fn test_delete_item() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        // Create table
        client
            .create_table()
            .table_name("ToDelete")
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("id")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("id")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();

        // Put item
        client
            .put_item()
            .table_name("ToDelete")
            .item("id", AttributeValue::S("item1".to_string()))
            .item("data", AttributeValue::S("test".to_string()))
            .send()
            .await
            .unwrap();

        // Delete with return values
        let result = client
            .delete_item()
            .table_name("ToDelete")
            .key("id", AttributeValue::S("item1".to_string()))
            .return_values(aws_sdk_dynamodb::types::ReturnValue::AllOld)
            .send()
            .await
            .unwrap();

        assert!(result.attributes().is_some());
        let attrs = result.attributes().unwrap();
        assert_eq!(attrs.get("data").unwrap().as_s().unwrap(), "test");

        // Verify deleted
        let get_result = client
            .get_item()
            .table_name("ToDelete")
            .key("id", AttributeValue::S("item1".to_string()))
            .send()
            .await
            .unwrap();

        assert!(get_result.item().is_none());
    }

    #[tokio::test]
    async fn test_list_tables() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        // Create multiple tables
        for i in 1..=3 {
            client
                .create_table()
                .table_name(format!("Table{}", i))
                .key_schema(
                    KeySchemaElement::builder()
                        .attribute_name("id")
                        .key_type(KeyType::Hash)
                        .build()
                        .unwrap(),
                )
                .attribute_definitions(
                    AttributeDefinition::builder()
                        .attribute_name("id")
                        .attribute_type(ScalarAttributeType::S)
                        .build()
                        .unwrap(),
                )
                .provisioned_throughput(
                    ProvisionedThroughput::builder()
                        .read_capacity_units(5)
                        .write_capacity_units(5)
                        .build()
                        .unwrap(),
                )
                .send()
                .await
                .unwrap();
        }

        // List tables
        let result = client.list_tables().send().await.unwrap();
        let names = result.table_names();
        assert_eq!(names.len(), 3);
    }

    #[tokio::test]
    async fn test_gsi_query() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        // Create table with GSI
        client
            .create_table()
            .table_name("Users")
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("userId")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("userId")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("email")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .global_secondary_indexes(
                GlobalSecondaryIndex::builder()
                    .index_name("email-index")
                    .key_schema(
                        KeySchemaElement::builder()
                            .attribute_name("email")
                            .key_type(KeyType::Hash)
                            .build()
                            .unwrap(),
                    )
                    .projection(
                        Projection::builder()
                            .projection_type(ProjectionType::All)
                            .build(),
                    )
                    .provisioned_throughput(
                        ProvisionedThroughput::builder()
                            .read_capacity_units(5)
                            .write_capacity_units(5)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();

        // Add users
        for i in 1..=3 {
            client
                .put_item()
                .table_name("Users")
                .item("userId", AttributeValue::S(format!("user{}", i)))
                .item("email", AttributeValue::S(format!("user{}@example.com", i)))
                .item("name", AttributeValue::S(format!("User {}", i)))
                .send()
                .await
                .unwrap();
        }

        // Query by email using GSI
        let result = client
            .query()
            .table_name("Users")
            .index_name("email-index")
            .key_condition_expression("email = :email")
            .expression_attribute_values(
                ":email",
                AttributeValue::S("user2@example.com".to_string()),
            )
            .send()
            .await
            .unwrap();

        assert_eq!(result.count(), 1);
        let items = result.items();
        assert_eq!(items[0].get("name").unwrap().as_s().unwrap(), "User 2");
    }

    #[tokio::test]
    async fn test_resource_not_found() {
        let (endpoint, _handle) = start_test_server().await;
        let client = create_test_client(&endpoint).await;

        // Try to get item from non-existent table
        let result = client
            .get_item()
            .table_name("NonExistent")
            .key("id", AttributeValue::S("item1".to_string()))
            .send()
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        let service_err = err.into_service_error();
        assert!(service_err.is_resource_not_found_exception());
    }
}
