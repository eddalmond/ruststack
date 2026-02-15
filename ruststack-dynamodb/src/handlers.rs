//! DynamoDB HTTP request handlers

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use bytes::Bytes;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::storage::{
    AttributeDefinition, AttributeType, AttributeValue, DynamoDBError, DynamoDBStorage,
    GlobalSecondaryIndex, Item, KeySchemaElement, KeyType, LocalSecondaryIndex, Projection,
    ProjectionType, ProvisionedThroughput, ReturnValues,
};

/// Shared state for DynamoDB handlers
pub struct DynamoDBState {
    pub storage: Arc<DynamoDBStorage>,
}

/// Handle a DynamoDB request
pub async fn handle_request(
    State(state): State<Arc<DynamoDBState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Get the target action from x-amz-target header
    let target = match headers.get("x-amz-target") {
        Some(t) => match t.to_str() {
            Ok(s) => s,
            Err(_) => return error_response("SerializationException", "Invalid target header"),
        },
        None => return error_response("MissingAction", "Missing x-amz-target header"),
    };

    // Parse action from target (format: DynamoDB_20120810.ActionName)
    let action = target.split('.').last().unwrap_or(target);

    // Parse request body as JSON
    let body_json: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return error_response("SerializationException", &format!("Invalid JSON: {}", e)),
    };

    // Route to appropriate handler
    let result = match action {
        "CreateTable" => handle_create_table(&state.storage, &body_json),
        "DeleteTable" => handle_delete_table(&state.storage, &body_json),
        "DescribeTable" => handle_describe_table(&state.storage, &body_json),
        "ListTables" => handle_list_tables(&state.storage, &body_json),
        "PutItem" => handle_put_item(&state.storage, &body_json),
        "GetItem" => handle_get_item(&state.storage, &body_json),
        "DeleteItem" => handle_delete_item(&state.storage, &body_json),
        "UpdateItem" => handle_update_item(&state.storage, &body_json),
        "Query" => handle_query(&state.storage, &body_json),
        "Scan" => handle_scan(&state.storage, &body_json),
        "BatchGetItem" => handle_batch_get_item(&state.storage, &body_json),
        "BatchWriteItem" => handle_batch_write_item(&state.storage, &body_json),
        _ => Err(DynamoDBError::ValidationError(format!(
            "Unknown action: {}",
            action
        ))),
    };

    match result {
        Ok(response_body) => json_response(StatusCode::OK, response_body),
        Err(e) => dynamodb_error_response(&e),
    }
}

// === Table Operations ===

fn handle_create_table(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let table_name = body["TableName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing TableName".to_string()))?;

    // Parse key schema
    let key_schema: Vec<KeySchemaElement> = body["KeySchema"]
        .as_array()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing KeySchema".to_string()))?
        .iter()
        .map(|k| parse_key_schema_element(k))
        .collect::<Result<Vec<_>, _>>()?;

    // Parse attribute definitions
    let attribute_definitions: Vec<AttributeDefinition> = body["AttributeDefinitions"]
        .as_array()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing AttributeDefinitions".to_string()))?
        .iter()
        .map(|a| parse_attribute_definition(a))
        .collect::<Result<Vec<_>, _>>()?;

    // Parse provisioned throughput (or use defaults for on-demand)
    let provisioned_throughput = parse_provisioned_throughput(body.get("ProvisionedThroughput"));

    // Parse GSIs
    let global_secondary_indexes = body
        .get("GlobalSecondaryIndexes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|gsi| parse_gsi(gsi))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    // Parse LSIs
    let local_secondary_indexes = body
        .get("LocalSecondaryIndexes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|lsi| parse_lsi(lsi))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    let description = storage.create_table(
        table_name,
        key_schema,
        attribute_definitions,
        provisioned_throughput,
        global_secondary_indexes,
        local_secondary_indexes,
    )?;

    Ok(json!({
        "TableDescription": description
    }))
}

fn handle_delete_table(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let table_name = body["TableName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing TableName".to_string()))?;

    let description = storage.delete_table(table_name)?;

    Ok(json!({
        "TableDescription": description
    }))
}

fn handle_describe_table(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let table_name = body["TableName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing TableName".to_string()))?;

    let description = storage.describe_table(table_name)?;

    Ok(json!({
        "Table": description
    }))
}

fn handle_list_tables(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let table_names = storage.list_tables();

    // Handle pagination (simplified)
    let limit = body
        .get("Limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let exclusive_start = body.get("ExclusiveStartTableName").and_then(|v| v.as_str());

    let mut names: Vec<String> = table_names;
    names.sort();

    // Skip to start
    if let Some(start) = exclusive_start {
        if let Some(pos) = names.iter().position(|n| n == start) {
            names = names.into_iter().skip(pos + 1).collect();
        }
    }

    // Apply limit
    let last_evaluated = if let Some(lim) = limit {
        if names.len() > lim {
            let last = names.get(lim - 1).cloned();
            names.truncate(lim);
            last
        } else {
            None
        }
    } else {
        None
    };

    let mut result = json!({
        "TableNames": names
    });

    if let Some(last) = last_evaluated {
        result["LastEvaluatedTableName"] = json!(last);
    }

    Ok(result)
}

// === Item Operations ===

fn handle_put_item(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let table_name = body["TableName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing TableName".to_string()))?;

    let item = parse_item(&body["Item"])?;

    let condition_expression = body.get("ConditionExpression").and_then(|v| v.as_str());
    let expression_attribute_names = parse_expression_attribute_names(body);
    let expression_attribute_values = parse_expression_attribute_values(body)?;

    let old_item = storage.put_item(
        table_name,
        item,
        condition_expression,
        expression_attribute_names.as_ref(),
        expression_attribute_values.as_ref(),
    )?;

    // Check if ReturnValues is requested
    let return_values = body["ReturnValues"].as_str().unwrap_or("NONE");

    match return_values {
        "ALL_OLD" => {
            if let Some(old) = old_item {
                Ok(json!({ "Attributes": old }))
            } else {
                Ok(json!({}))
            }
        }
        _ => Ok(json!({})),
    }
}

fn handle_get_item(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let table_name = body["TableName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing TableName".to_string()))?;

    let key = parse_item(&body["Key"])?;
    let projection_expression = body.get("ProjectionExpression").and_then(|v| v.as_str());
    let expression_attribute_names = parse_expression_attribute_names(body);

    let item = storage.get_item(
        table_name,
        key,
        projection_expression,
        expression_attribute_names.as_ref(),
    )?;

    match item {
        Some(i) => Ok(json!({ "Item": i })),
        None => Ok(json!({})),
    }
}

fn handle_delete_item(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let table_name = body["TableName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing TableName".to_string()))?;

    let key = parse_item(&body["Key"])?;
    let return_values = body["ReturnValues"].as_str().unwrap_or("NONE");

    let condition_expression = body.get("ConditionExpression").and_then(|v| v.as_str());
    let expression_attribute_names = parse_expression_attribute_names(body);
    let expression_attribute_values = parse_expression_attribute_values(body)?;

    let old_item = storage.delete_item(
        table_name,
        key,
        condition_expression,
        expression_attribute_names.as_ref(),
        expression_attribute_values.as_ref(),
    )?;

    match return_values {
        "ALL_OLD" => {
            if let Some(old) = old_item {
                Ok(json!({ "Attributes": old }))
            } else {
                Ok(json!({}))
            }
        }
        _ => Ok(json!({})),
    }
}

fn handle_update_item(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let table_name = body["TableName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing TableName".to_string()))?;

    let key = parse_item(&body["Key"])?;

    let update_expression = body["UpdateExpression"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing UpdateExpression".to_string()))?;

    let condition_expression = body.get("ConditionExpression").and_then(|v| v.as_str());
    let expression_attribute_names = parse_expression_attribute_names(body);
    let expression_attribute_values = parse_expression_attribute_values(body)?;
    let return_values = ReturnValues::from_str(body["ReturnValues"].as_str().unwrap_or("NONE"));

    let result = storage.update_item(
        table_name,
        key,
        update_expression,
        condition_expression,
        expression_attribute_names.as_ref(),
        expression_attribute_values.as_ref(),
        return_values,
    )?;

    match result {
        Some(attrs) => Ok(json!({ "Attributes": attrs })),
        None => Ok(json!({})),
    }
}

fn handle_query(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let table_name = body["TableName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing TableName".to_string()))?;

    let index_name = body.get("IndexName").and_then(|v| v.as_str());
    let key_condition_expression = body["KeyConditionExpression"].as_str().unwrap_or("");
    let filter_expression = body.get("FilterExpression").and_then(|v| v.as_str());
    let expression_attribute_names = parse_expression_attribute_names(body);
    let expression_attribute_values = parse_expression_attribute_values(body)?;

    let scan_index_forward = body
        .get("ScanIndexForward")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let limit = body
        .get("Limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    let exclusive_start_key = body
        .get("ExclusiveStartKey")
        .filter(|v| !v.is_null())
        .map(|v| parse_item(v))
        .transpose()?;

    let result = storage.query(
        table_name,
        index_name,
        key_condition_expression,
        filter_expression,
        expression_attribute_names.as_ref(),
        expression_attribute_values.as_ref(),
        scan_index_forward,
        limit,
        exclusive_start_key.as_ref(),
    )?;

    let mut response = json!({
        "Items": result.items,
        "Count": result.count,
        "ScannedCount": result.scanned_count
    });

    if let Some(last_key) = result.last_evaluated_key {
        response["LastEvaluatedKey"] = json!(last_key);
    }

    Ok(response)
}

fn handle_scan(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let table_name = body["TableName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing TableName".to_string()))?;

    let index_name = body.get("IndexName").and_then(|v| v.as_str());
    let filter_expression = body.get("FilterExpression").and_then(|v| v.as_str());
    let expression_attribute_names = parse_expression_attribute_names(body);
    let expression_attribute_values = parse_expression_attribute_values(body)?;

    let limit = body
        .get("Limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    let exclusive_start_key = body
        .get("ExclusiveStartKey")
        .filter(|v| !v.is_null())
        .map(|v| parse_item(v))
        .transpose()?;

    let result = storage.scan(
        table_name,
        index_name,
        filter_expression,
        expression_attribute_names.as_ref(),
        expression_attribute_values.as_ref(),
        limit,
        exclusive_start_key.as_ref(),
    )?;

    let mut response = json!({
        "Items": result.items,
        "Count": result.count,
        "ScannedCount": result.scanned_count
    });

    if let Some(last_key) = result.last_evaluated_key {
        response["LastEvaluatedKey"] = json!(last_key);
    }

    Ok(response)
}

// === Batch Operations ===

fn handle_batch_get_item(storage: &DynamoDBStorage, body: &Value) -> Result<Value, DynamoDBError> {
    let request_items = body
        .get("RequestItems")
        .and_then(|v| v.as_object())
        .ok_or_else(|| DynamoDBError::ValidationError("Missing RequestItems".to_string()))?;

    let mut responses: HashMap<String, Vec<Item>> = HashMap::new();
    let mut unprocessed: HashMap<String, Value> = HashMap::new();

    for (table_name, request) in request_items {
        let keys = request
            .get("Keys")
            .and_then(|v| v.as_array())
            .ok_or_else(|| DynamoDBError::ValidationError("Missing Keys".to_string()))?;

        let mut table_items = Vec::new();

        for key_value in keys {
            let key = parse_item(key_value)?;
            if let Some(item) = storage.get_item(table_name, key, None, None)? {
                table_items.push(item);
            }
        }

        responses.insert(table_name.clone(), table_items);
    }

    Ok(json!({
        "Responses": responses,
        "UnprocessedKeys": unprocessed
    }))
}

fn handle_batch_write_item(
    storage: &DynamoDBStorage,
    body: &Value,
) -> Result<Value, DynamoDBError> {
    let request_items = body
        .get("RequestItems")
        .and_then(|v| v.as_object())
        .ok_or_else(|| DynamoDBError::ValidationError("Missing RequestItems".to_string()))?;

    let mut unprocessed: HashMap<String, Value> = HashMap::new();

    for (table_name, requests) in request_items {
        let operations = requests
            .as_array()
            .ok_or_else(|| DynamoDBError::ValidationError("Invalid request format".to_string()))?;

        for op in operations {
            if let Some(put_request) = op.get("PutRequest") {
                let item = parse_item(&put_request["Item"])?;
                storage.put_item(table_name, item, None, None, None)?;
            } else if let Some(delete_request) = op.get("DeleteRequest") {
                let key = parse_item(&delete_request["Key"])?;
                storage.delete_item(table_name, key, None, None, None)?;
            }
        }
    }

    Ok(json!({
        "UnprocessedItems": unprocessed
    }))
}

// === Helper Functions ===

fn parse_key_schema_element(value: &Value) -> Result<KeySchemaElement, DynamoDBError> {
    Ok(KeySchemaElement {
        attribute_name: value["AttributeName"]
            .as_str()
            .ok_or_else(|| {
                DynamoDBError::ValidationError("Missing AttributeName in KeySchema".to_string())
            })?
            .to_string(),
        key_type: match value["KeyType"].as_str() {
            Some("HASH") => KeyType::HASH,
            Some("RANGE") => KeyType::RANGE,
            _ => {
                return Err(DynamoDBError::ValidationError(
                    "Invalid KeyType".to_string(),
                ))
            }
        },
    })
}

fn parse_attribute_definition(value: &Value) -> Result<AttributeDefinition, DynamoDBError> {
    Ok(AttributeDefinition {
        attribute_name: value["AttributeName"]
            .as_str()
            .ok_or_else(|| DynamoDBError::ValidationError("Missing AttributeName".to_string()))?
            .to_string(),
        attribute_type: match value["AttributeType"].as_str() {
            Some("S") => AttributeType::S,
            Some("N") => AttributeType::N,
            Some("B") => AttributeType::B,
            _ => {
                return Err(DynamoDBError::ValidationError(
                    "Invalid AttributeType".to_string(),
                ))
            }
        },
    })
}

fn parse_provisioned_throughput(value: Option<&Value>) -> ProvisionedThroughput {
    value
        .map(|pt| ProvisionedThroughput {
            read_capacity_units: pt["ReadCapacityUnits"].as_i64().unwrap_or(5),
            write_capacity_units: pt["WriteCapacityUnits"].as_i64().unwrap_or(5),
        })
        .unwrap_or(ProvisionedThroughput {
            read_capacity_units: 5,
            write_capacity_units: 5,
        })
}

fn parse_projection(value: &Value) -> Result<Projection, DynamoDBError> {
    let projection_type = match value["ProjectionType"].as_str() {
        Some("ALL") => ProjectionType::ALL,
        Some("KEYS_ONLY") => ProjectionType::KEYS_ONLY,
        Some("INCLUDE") => ProjectionType::INCLUDE,
        _ => ProjectionType::ALL,
    };

    let non_key_attributes = value
        .get("NonKeyAttributes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    Ok(Projection {
        projection_type,
        non_key_attributes,
    })
}

fn parse_gsi(value: &Value) -> Result<GlobalSecondaryIndex, DynamoDBError> {
    let index_name = value["IndexName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing IndexName in GSI".to_string()))?
        .to_string();

    let key_schema: Vec<KeySchemaElement> = value["KeySchema"]
        .as_array()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing KeySchema in GSI".to_string()))?
        .iter()
        .map(|k| parse_key_schema_element(k))
        .collect::<Result<Vec<_>, _>>()?;

    let projection = parse_projection(&value["Projection"])?;

    let provisioned_throughput = value
        .get("ProvisionedThroughput")
        .map(|pt| parse_provisioned_throughput(Some(pt)));

    Ok(GlobalSecondaryIndex {
        index_name,
        key_schema,
        projection,
        provisioned_throughput,
    })
}

fn parse_lsi(value: &Value) -> Result<LocalSecondaryIndex, DynamoDBError> {
    let index_name = value["IndexName"]
        .as_str()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing IndexName in LSI".to_string()))?
        .to_string();

    let key_schema: Vec<KeySchemaElement> = value["KeySchema"]
        .as_array()
        .ok_or_else(|| DynamoDBError::ValidationError("Missing KeySchema in LSI".to_string()))?
        .iter()
        .map(|k| parse_key_schema_element(k))
        .collect::<Result<Vec<_>, _>>()?;

    let projection = parse_projection(&value["Projection"])?;

    Ok(LocalSecondaryIndex {
        index_name,
        key_schema,
        projection,
    })
}

fn parse_expression_attribute_names(body: &Value) -> Option<HashMap<String, String>> {
    body.get("ExpressionAttributeNames")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
}

fn parse_expression_attribute_values(
    body: &Value,
) -> Result<Option<HashMap<String, AttributeValue>>, DynamoDBError> {
    body.get("ExpressionAttributeValues")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| parse_attribute_value(v).map(|av| (k.clone(), av)))
                .collect::<Result<HashMap<_, _>, _>>()
        })
        .transpose()
}

/// Parse a DynamoDB item from JSON
fn parse_item(value: &Value) -> Result<Item, DynamoDBError> {
    let obj = value
        .as_object()
        .ok_or_else(|| DynamoDBError::ValidationError("Item must be an object".to_string()))?;

    let mut item = Item::new();
    for (k, v) in obj {
        item.insert(k.clone(), parse_attribute_value(v)?);
    }
    Ok(item)
}

/// Parse a single attribute value
fn parse_attribute_value(value: &Value) -> Result<AttributeValue, DynamoDBError> {
    if let Some(s) = value.get("S") {
        return Ok(AttributeValue::S {
            S: s.as_str().unwrap_or("").to_string(),
        });
    }
    if let Some(n) = value.get("N") {
        return Ok(AttributeValue::N {
            N: n.as_str().unwrap_or("0").to_string(),
        });
    }
    if let Some(b) = value.get("B") {
        return Ok(AttributeValue::B {
            B: b.as_str().unwrap_or("").to_string(),
        });
    }
    if let Some(b) = value.get("BOOL") {
        return Ok(AttributeValue::BOOL {
            BOOL: b.as_bool().unwrap_or(false),
        });
    }
    if let Some(n) = value.get("NULL") {
        return Ok(AttributeValue::NULL {
            NULL: n.as_bool().unwrap_or(true),
        });
    }
    if let Some(l) = value.get("L") {
        let arr = l
            .as_array()
            .ok_or_else(|| DynamoDBError::ValidationError("L must be an array".to_string()))?;
        let values: Result<Vec<_>, _> = arr.iter().map(parse_attribute_value).collect();
        return Ok(AttributeValue::L { L: values? });
    }
    if let Some(m) = value.get("M") {
        let obj = m
            .as_object()
            .ok_or_else(|| DynamoDBError::ValidationError("M must be an object".to_string()))?;
        let mut map = HashMap::new();
        for (k, v) in obj {
            map.insert(k.clone(), parse_attribute_value(v)?);
        }
        return Ok(AttributeValue::M { M: map });
    }
    if let Some(ss) = value.get("SS") {
        let arr = ss
            .as_array()
            .ok_or_else(|| DynamoDBError::ValidationError("SS must be an array".to_string()))?;
        let values: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        return Ok(AttributeValue::SS { SS: values });
    }
    if let Some(ns) = value.get("NS") {
        let arr = ns
            .as_array()
            .ok_or_else(|| DynamoDBError::ValidationError("NS must be an array".to_string()))?;
        let values: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        return Ok(AttributeValue::NS { NS: values });
    }
    if let Some(bs) = value.get("BS") {
        let arr = bs
            .as_array()
            .ok_or_else(|| DynamoDBError::ValidationError("BS must be an array".to_string()))?;
        let values: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        return Ok(AttributeValue::BS { BS: values });
    }

    Err(DynamoDBError::ValidationError(format!(
        "Unknown attribute type: {:?}",
        value
    )))
}

/// Create a JSON response
fn json_response(status: StatusCode, body: Value) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/x-amz-json-1.0")
        .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
        .unwrap()
}

/// Create an error response
fn error_response(error_type: &str, message: &str) -> Response {
    let body = json!({
        "__type": format!("com.amazonaws.dynamodb.v20120810#{}", error_type),
        "message": message
    });

    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(header::CONTENT_TYPE, "application/x-amz-json-1.0")
        .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
        .unwrap()
}

/// Map DynamoDBError to HTTP response
fn dynamodb_error_response(error: &DynamoDBError) -> Response {
    let (error_type, status) = match error {
        DynamoDBError::ResourceNotFound(_) => {
            ("ResourceNotFoundException", StatusCode::BAD_REQUEST)
        }
        DynamoDBError::ResourceInUse(_) => ("ResourceInUseException", StatusCode::BAD_REQUEST),
        DynamoDBError::ValidationError(_) => ("ValidationException", StatusCode::BAD_REQUEST),
        DynamoDBError::ConditionalCheckFailed => {
            ("ConditionalCheckFailedException", StatusCode::BAD_REQUEST)
        }
        DynamoDBError::ProvisionedThroughputExceeded => (
            "ProvisionedThroughputExceededException",
            StatusCode::BAD_REQUEST,
        ),
        DynamoDBError::Internal(_) => ("InternalServerError", StatusCode::INTERNAL_SERVER_ERROR),
        DynamoDBError::Expression(_) => ("ValidationException", StatusCode::BAD_REQUEST),
    };

    let body = json!({
        "__type": format!("com.amazonaws.dynamodb.v20120810#{}", error_type),
        "message": error.to_string()
    });

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/x-amz-json-1.0")
        .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
        .unwrap()
}
