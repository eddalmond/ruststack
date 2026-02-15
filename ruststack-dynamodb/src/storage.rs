//! DynamoDB table storage

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use crate::expression::{
    apply_update, evaluate_condition, evaluate_key_conditions, parse_condition,
    parse_key_condition, parse_update_expression, ExpressionContext, ExpressionError,
};

/// DynamoDB errors
#[derive(Debug, Error)]
pub enum DynamoDBError {
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Resource in use: {0}")]
    ResourceInUse(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Conditional check failed")]
    ConditionalCheckFailed,

    #[error("Provisioned throughput exceeded")]
    ProvisionedThroughputExceeded,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Expression error: {0}")]
    Expression(#[from] ExpressionError),
}

/// Key schema element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeySchemaElement {
    #[serde(rename = "AttributeName")]
    pub attribute_name: String,
    #[serde(rename = "KeyType")]
    pub key_type: KeyType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum KeyType {
    HASH,
    RANGE,
}

/// Attribute definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDefinition {
    #[serde(rename = "AttributeName")]
    pub attribute_name: String,
    #[serde(rename = "AttributeType")]
    pub attribute_type: AttributeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttributeType {
    S, // String
    N, // Number
    B, // Binary
}

/// Provisioned throughput settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionedThroughput {
    #[serde(rename = "ReadCapacityUnits")]
    pub read_capacity_units: i64,
    #[serde(rename = "WriteCapacityUnits")]
    pub write_capacity_units: i64,
}

/// Global Secondary Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSecondaryIndex {
    #[serde(rename = "IndexName")]
    pub index_name: String,
    #[serde(rename = "KeySchema")]
    pub key_schema: Vec<KeySchemaElement>,
    #[serde(rename = "Projection")]
    pub projection: Projection,
    #[serde(
        rename = "ProvisionedThroughput",
        skip_serializing_if = "Option::is_none"
    )]
    pub provisioned_throughput: Option<ProvisionedThroughput>,
}

/// Local Secondary Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSecondaryIndex {
    #[serde(rename = "IndexName")]
    pub index_name: String,
    #[serde(rename = "KeySchema")]
    pub key_schema: Vec<KeySchemaElement>,
    #[serde(rename = "Projection")]
    pub projection: Projection,
}

/// Projection for secondary indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projection {
    #[serde(rename = "ProjectionType")]
    pub projection_type: ProjectionType,
    #[serde(rename = "NonKeyAttributes", skip_serializing_if = "Option::is_none")]
    pub non_key_attributes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(non_camel_case_types)]
pub enum ProjectionType {
    ALL,
    KEYS_ONLY,
    INCLUDE,
}

/// GSI description for responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSecondaryIndexDescription {
    #[serde(rename = "IndexName")]
    pub index_name: String,
    #[serde(rename = "KeySchema")]
    pub key_schema: Vec<KeySchemaElement>,
    #[serde(rename = "Projection")]
    pub projection: Projection,
    #[serde(rename = "IndexStatus")]
    pub index_status: IndexStatus,
    #[serde(rename = "IndexArn")]
    pub index_arn: String,
    #[serde(rename = "ItemCount")]
    pub item_count: i64,
    #[serde(rename = "IndexSizeBytes")]
    pub index_size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IndexStatus {
    CREATING,
    ACTIVE,
    DELETING,
    UPDATING,
}

/// Table description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDescription {
    #[serde(rename = "TableName")]
    pub table_name: String,
    #[serde(rename = "TableArn")]
    pub table_arn: String,
    #[serde(rename = "TableStatus")]
    pub table_status: TableStatus,
    #[serde(rename = "KeySchema")]
    pub key_schema: Vec<KeySchemaElement>,
    #[serde(rename = "AttributeDefinitions")]
    pub attribute_definitions: Vec<AttributeDefinition>,
    #[serde(rename = "ProvisionedThroughput")]
    pub provisioned_throughput: ProvisionedThroughput,
    #[serde(rename = "CreationDateTime")]
    pub creation_date_time: f64,
    #[serde(rename = "ItemCount")]
    pub item_count: i64,
    #[serde(rename = "TableSizeBytes")]
    pub table_size_bytes: i64,
    #[serde(
        rename = "GlobalSecondaryIndexes",
        skip_serializing_if = "Option::is_none"
    )]
    pub global_secondary_indexes: Option<Vec<GlobalSecondaryIndexDescription>>,
    #[serde(
        rename = "LocalSecondaryIndexes",
        skip_serializing_if = "Option::is_none"
    )]
    pub local_secondary_indexes: Option<Vec<LocalSecondaryIndex>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TableStatus {
    CREATING,
    ACTIVE,
    DELETING,
    UPDATING,
}

/// An item in DynamoDB (simplified attribute value)
pub type Item = HashMap<String, AttributeValue>;

/// Simplified attribute value representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
#[allow(non_snake_case)]
pub enum AttributeValue {
    S { S: String },
    N { N: String },
    B { B: String },
    BOOL { BOOL: bool },
    NULL { NULL: bool },
    L { L: Vec<AttributeValue> },
    M { M: HashMap<String, AttributeValue> },
    SS { SS: Vec<String> },
    NS { NS: Vec<String> },
    BS { BS: Vec<String> },
}

impl AttributeValue {
    pub fn string(s: impl Into<String>) -> Self {
        AttributeValue::S { S: s.into() }
    }

    pub fn number(n: impl Into<String>) -> Self {
        AttributeValue::N { N: n.into() }
    }

    /// Get the string value if this is an S type
    pub fn as_string(&self) -> Option<&str> {
        match self {
            AttributeValue::S { S } => Some(S),
            _ => None,
        }
    }

    /// Get the number string if this is an N type
    pub fn as_number(&self) -> Option<&str> {
        match self {
            AttributeValue::N { N } => Some(N),
            _ => None,
        }
    }
}

/// Secondary index storage
#[allow(dead_code)]
struct SecondaryIndex {
    key_schema: Vec<KeySchemaElement>,
    projection: Projection,
    /// Maps index key to primary keys
    items: DashMap<String, Vec<String>>,
}

impl SecondaryIndex {
    fn new(key_schema: Vec<KeySchemaElement>, projection: Projection) -> Self {
        Self {
            key_schema,
            projection,
            items: DashMap::new(),
        }
    }

    /// Get the index key for an item
    fn get_index_key(&self, item: &Item) -> Option<String> {
        let mut key_parts = Vec::new();

        for key_elem in &self.key_schema {
            let attr = item.get(&key_elem.attribute_name)?;

            let key_str = match attr {
                AttributeValue::S { S } => S.clone(),
                AttributeValue::N { N } => N.clone(),
                AttributeValue::B { B } => B.clone(),
                _ => return None,
            };
            key_parts.push(key_str);
        }

        Some(key_parts.join("#"))
    }

    /// Add an item to the index
    fn add_item(&self, item: &Item, primary_key: &str) {
        if let Some(index_key) = self.get_index_key(item) {
            self.items
                .entry(index_key)
                .or_default()
                .push(primary_key.to_string());
        }
    }

    /// Remove an item from the index
    fn remove_item(&self, item: &Item, primary_key: &str) {
        if let Some(index_key) = self.get_index_key(item) {
            if let Some(mut keys) = self.items.get_mut(&index_key) {
                keys.retain(|k| k != primary_key);
            }
        }
    }
}

/// A table with its items
struct Table {
    description: TableDescription,
    items: DashMap<String, Item>,
    global_secondary_indexes: HashMap<String, SecondaryIndex>,
    local_secondary_indexes: HashMap<String, SecondaryIndex>,
}

impl Table {
    fn new(
        description: TableDescription,
        gsis: Vec<GlobalSecondaryIndex>,
        lsis: Vec<LocalSecondaryIndex>,
    ) -> Self {
        let mut global_secondary_indexes = HashMap::new();
        for gsi in gsis {
            global_secondary_indexes.insert(
                gsi.index_name.clone(),
                SecondaryIndex::new(gsi.key_schema, gsi.projection),
            );
        }

        let mut local_secondary_indexes = HashMap::new();
        for lsi in lsis {
            local_secondary_indexes.insert(
                lsi.index_name.clone(),
                SecondaryIndex::new(lsi.key_schema, lsi.projection),
            );
        }

        Self {
            description,
            items: DashMap::new(),
            global_secondary_indexes,
            local_secondary_indexes,
        }
    }

    /// Get the primary key for an item
    fn get_key(&self, item: &Item) -> Result<String, DynamoDBError> {
        let mut key_parts = Vec::new();

        for key_elem in &self.description.key_schema {
            let attr = item.get(&key_elem.attribute_name).ok_or_else(|| {
                DynamoDBError::ValidationError(format!(
                    "Missing key attribute: {}",
                    key_elem.attribute_name
                ))
            })?;

            // Convert attribute to string key component
            let key_str = match attr {
                AttributeValue::S { S } => S.clone(),
                AttributeValue::N { N } => N.clone(),
                AttributeValue::B { B } => B.clone(),
                _ => {
                    return Err(DynamoDBError::ValidationError(format!(
                        "Invalid key attribute type for {}",
                        key_elem.attribute_name
                    )))
                }
            };
            key_parts.push(key_str);
        }

        Ok(key_parts.join("#"))
    }

    /// Extract key attributes from an item
    fn extract_key(&self, item: &Item) -> Item {
        let mut key = Item::new();
        for key_elem in &self.description.key_schema {
            if let Some(attr) = item.get(&key_elem.attribute_name) {
                key.insert(key_elem.attribute_name.clone(), attr.clone());
            }
        }
        key
    }

    /// Get the hash key attribute name
    #[allow(dead_code)]
    fn hash_key_name(&self) -> &str {
        self.description
            .key_schema
            .iter()
            .find(|k| k.key_type == KeyType::HASH)
            .map(|k| k.attribute_name.as_str())
            .unwrap_or("")
    }

    /// Get the range key attribute name (if exists)
    fn range_key_name(&self) -> Option<&str> {
        self.description
            .key_schema
            .iter()
            .find(|k| k.key_type == KeyType::RANGE)
            .map(|k| k.attribute_name.as_str())
    }

    /// Update secondary indexes when an item changes
    fn update_indexes(&self, old_item: Option<&Item>, new_item: Option<&Item>, primary_key: &str) {
        // Remove old item from indexes
        if let Some(old) = old_item {
            for index in self.global_secondary_indexes.values() {
                index.remove_item(old, primary_key);
            }
            for index in self.local_secondary_indexes.values() {
                index.remove_item(old, primary_key);
            }
        }

        // Add new item to indexes
        if let Some(new) = new_item {
            for index in self.global_secondary_indexes.values() {
                index.add_item(new, primary_key);
            }
            for index in self.local_secondary_indexes.values() {
                index.add_item(new, primary_key);
            }
        }
    }
}

/// In-memory DynamoDB storage
pub struct DynamoDBStorage {
    tables: DashMap<String, Table>,
}

impl Default for DynamoDBStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamoDBStorage {
    pub fn new() -> Self {
        Self {
            tables: DashMap::new(),
        }
    }

    /// Create a new table
    pub fn create_table(
        &self,
        table_name: &str,
        key_schema: Vec<KeySchemaElement>,
        attribute_definitions: Vec<AttributeDefinition>,
        provisioned_throughput: ProvisionedThroughput,
        global_secondary_indexes: Option<Vec<GlobalSecondaryIndex>>,
        local_secondary_indexes: Option<Vec<LocalSecondaryIndex>>,
    ) -> Result<TableDescription, DynamoDBError> {
        if self.tables.contains_key(table_name) {
            return Err(DynamoDBError::ResourceInUse(table_name.to_string()));
        }

        let gsis = global_secondary_indexes.clone().unwrap_or_default();
        let lsis = local_secondary_indexes.clone().unwrap_or_default();

        // Build GSI descriptions for response
        let gsi_descriptions: Option<Vec<GlobalSecondaryIndexDescription>> =
            global_secondary_indexes.map(|indexes| {
                indexes
                    .into_iter()
                    .map(|gsi| GlobalSecondaryIndexDescription {
                        index_name: gsi.index_name.clone(),
                        key_schema: gsi.key_schema,
                        projection: gsi.projection,
                        index_status: IndexStatus::ACTIVE,
                        index_arn: format!(
                            "arn:aws:dynamodb:us-east-1:000000000000:table/{}/index/{}",
                            table_name, gsi.index_name
                        ),
                        item_count: 0,
                        index_size_bytes: 0,
                    })
                    .collect()
            });

        let description = TableDescription {
            table_name: table_name.to_string(),
            table_arn: format!(
                "arn:aws:dynamodb:us-east-1:000000000000:table/{}",
                table_name
            ),
            table_status: TableStatus::ACTIVE,
            key_schema,
            attribute_definitions,
            provisioned_throughput,
            creation_date_time: chrono::Utc::now().timestamp() as f64,
            item_count: 0,
            table_size_bytes: 0,
            global_secondary_indexes: gsi_descriptions,
            local_secondary_indexes,
        };

        let table = Table::new(description.clone(), gsis, lsis);
        self.tables.insert(table_name.to_string(), table);

        Ok(description)
    }

    /// Delete a table
    pub fn delete_table(&self, table_name: &str) -> Result<TableDescription, DynamoDBError> {
        let (_, table) = self
            .tables
            .remove(table_name)
            .ok_or_else(|| DynamoDBError::ResourceNotFound(table_name.to_string()))?;

        let mut desc = table.description;
        desc.table_status = TableStatus::DELETING;
        Ok(desc)
    }

    /// Describe a table
    pub fn describe_table(&self, table_name: &str) -> Result<TableDescription, DynamoDBError> {
        let table = self
            .tables
            .get(table_name)
            .ok_or_else(|| DynamoDBError::ResourceNotFound(table_name.to_string()))?;

        let mut desc = table.description.clone();
        desc.item_count = table.items.len() as i64;

        // Update GSI item counts
        if let Some(ref mut gsis) = desc.global_secondary_indexes {
            for gsi in gsis {
                if let Some(index) = table.global_secondary_indexes.get(&gsi.index_name) {
                    gsi.item_count = index.items.len() as i64;
                }
            }
        }

        Ok(desc)
    }

    /// List all tables
    pub fn list_tables(&self) -> Vec<String> {
        self.tables.iter().map(|r| r.key().clone()).collect()
    }

    /// Put an item (with optional condition expression)
    pub fn put_item(
        &self,
        table_name: &str,
        item: Item,
        condition_expression: Option<&str>,
        expression_attribute_names: Option<&HashMap<String, String>>,
        expression_attribute_values: Option<&HashMap<String, AttributeValue>>,
    ) -> Result<Option<Item>, DynamoDBError> {
        let table = self
            .tables
            .get(table_name)
            .ok_or_else(|| DynamoDBError::ResourceNotFound(table_name.to_string()))?;

        let key = table.get_key(&item)?;

        // Check condition expression if provided
        if let Some(cond_expr) = condition_expression {
            let ctx =
                ExpressionContext::new(expression_attribute_names, expression_attribute_values);
            let condition = parse_condition(cond_expr)?;

            if let Some(cond) = condition {
                // Get existing item for condition check
                let existing = table.items.get(&key);
                let empty_item = Item::new();
                let item_for_check = existing.as_ref().map(|r| r.value()).unwrap_or(&empty_item);

                if !evaluate_condition(&cond, item_for_check, &ctx)? {
                    return Err(DynamoDBError::ConditionalCheckFailed);
                }
            }
        }

        // Get old item for index update
        let old_item = table.items.get(&key).map(|r| r.value().clone());

        // Update indexes
        table.update_indexes(old_item.as_ref(), Some(&item), &key);

        // Insert the item
        let old = table.items.insert(key, item);

        Ok(old)
    }

    /// Get an item by key
    pub fn get_item(
        &self,
        table_name: &str,
        key: Item,
        projection_expression: Option<&str>,
        _expression_attribute_names: Option<&HashMap<String, String>>,
    ) -> Result<Option<Item>, DynamoDBError> {
        let table = self
            .tables
            .get(table_name)
            .ok_or_else(|| DynamoDBError::ResourceNotFound(table_name.to_string()))?;

        let key_str = table.get_key(&key)?;
        let item = table.items.get(&key_str).map(|r| r.clone());

        // Apply projection if specified
        if let (Some(_item), Some(_proj)) = (&item, projection_expression) {
            // TODO: Implement projection expression
            // For now, return full item
        }

        Ok(item)
    }

    /// Delete an item
    pub fn delete_item(
        &self,
        table_name: &str,
        key: Item,
        condition_expression: Option<&str>,
        expression_attribute_names: Option<&HashMap<String, String>>,
        expression_attribute_values: Option<&HashMap<String, AttributeValue>>,
    ) -> Result<Option<Item>, DynamoDBError> {
        let table = self
            .tables
            .get(table_name)
            .ok_or_else(|| DynamoDBError::ResourceNotFound(table_name.to_string()))?;

        let key_str = table.get_key(&key)?;

        // Check condition expression if provided
        if let Some(cond_expr) = condition_expression {
            let ctx =
                ExpressionContext::new(expression_attribute_names, expression_attribute_values);
            let condition = parse_condition(cond_expr)?;

            if let Some(cond) = condition {
                let existing = table.items.get(&key_str);
                let empty_item = Item::new();
                let item_for_check = existing.as_ref().map(|r| r.value()).unwrap_or(&empty_item);

                if !evaluate_condition(&cond, item_for_check, &ctx)? {
                    return Err(DynamoDBError::ConditionalCheckFailed);
                }
            }
        }

        // Get and remove the item
        let old_item = table.items.remove(&key_str).map(|(_, v)| v);

        // Update indexes
        if let Some(ref old) = old_item {
            table.update_indexes(Some(old), None, &key_str);
        }

        Ok(old_item)
    }

    /// Update an item
    #[allow(clippy::too_many_arguments)]
    pub fn update_item(
        &self,
        table_name: &str,
        key: Item,
        update_expression: &str,
        condition_expression: Option<&str>,
        expression_attribute_names: Option<&HashMap<String, String>>,
        expression_attribute_values: Option<&HashMap<String, AttributeValue>>,
        return_values: ReturnValues,
    ) -> Result<Option<Item>, DynamoDBError> {
        let table = self
            .tables
            .get(table_name)
            .ok_or_else(|| DynamoDBError::ResourceNotFound(table_name.to_string()))?;

        let key_str = table.get_key(&key)?;
        let ctx = ExpressionContext::new(expression_attribute_names, expression_attribute_values);

        // Get existing item or create with key
        let mut item = table
            .items
            .get(&key_str)
            .map(|r| r.clone())
            .unwrap_or_else(|| key.clone());

        let old_item = item.clone();

        // Check condition expression if provided
        if let Some(cond_expr) = condition_expression {
            let condition = parse_condition(cond_expr)?;

            if let Some(cond) = condition {
                if !evaluate_condition(&cond, &item, &ctx)? {
                    return Err(DynamoDBError::ConditionalCheckFailed);
                }
            }
        }

        // Parse and apply update expression
        let update = parse_update_expression(update_expression)?;
        apply_update(&mut item, &update, &ctx)?;

        // Update indexes
        let had_old = table.items.contains_key(&key_str);
        if had_old {
            table.update_indexes(Some(&old_item), Some(&item), &key_str);
        } else {
            table.update_indexes(None, Some(&item), &key_str);
        }

        // Store the updated item
        table.items.insert(key_str, item.clone());

        // Return appropriate values
        match return_values {
            ReturnValues::None => Ok(None),
            ReturnValues::AllOld => {
                if had_old {
                    Ok(Some(old_item))
                } else {
                    Ok(None)
                }
            }
            ReturnValues::UpdatedOld => {
                // Return only attributes that were updated
                // Simplified: return all old for now
                if had_old {
                    Ok(Some(old_item))
                } else {
                    Ok(None)
                }
            }
            ReturnValues::AllNew => Ok(Some(item)),
            ReturnValues::UpdatedNew => {
                // Return only updated attributes
                // Simplified: return all new for now
                Ok(Some(item))
            }
        }
    }

    /// Query items
    #[allow(clippy::too_many_arguments)]
    pub fn query(
        &self,
        table_name: &str,
        index_name: Option<&str>,
        key_condition_expression: &str,
        filter_expression: Option<&str>,
        expression_attribute_names: Option<&HashMap<String, String>>,
        expression_attribute_values: Option<&HashMap<String, AttributeValue>>,
        scan_index_forward: bool,
        limit: Option<usize>,
        exclusive_start_key: Option<&Item>,
    ) -> Result<QueryResult, DynamoDBError> {
        let table = self
            .tables
            .get(table_name)
            .ok_or_else(|| DynamoDBError::ResourceNotFound(table_name.to_string()))?;

        let ctx = ExpressionContext::new(expression_attribute_names, expression_attribute_values);

        // Parse key condition
        let key_conditions = parse_key_condition(key_condition_expression)?;

        // Parse filter expression
        let filter = filter_expression
            .map(parse_condition)
            .transpose()?
            .flatten();

        // Get items from index or table
        let items: Vec<Item> = if let Some(idx_name) = index_name {
            // Query GSI or LSI
            let index = table
                .global_secondary_indexes
                .get(idx_name)
                .or_else(|| table.local_secondary_indexes.get(idx_name))
                .ok_or_else(|| {
                    DynamoDBError::ValidationError(format!("Index not found: {}", idx_name))
                })?;

            // Find matching items through index
            let mut result = Vec::new();
            for entry in index.items.iter() {
                for primary_key in entry.value() {
                    if let Some(item) = table.items.get(primary_key) {
                        result.push(item.clone());
                    }
                }
            }
            result
        } else {
            // Query main table
            table.items.iter().map(|r| r.value().clone()).collect()
        };

        // Filter by key conditions
        let mut filtered: Vec<Item> = items
            .into_iter()
            .filter(|item| evaluate_key_conditions(&key_conditions, item, &ctx).unwrap_or(false))
            .collect();

        // Apply filter expression
        if let Some(ref filter_cond) = filter {
            filtered.retain(|item| evaluate_condition(filter_cond, item, &ctx).unwrap_or(false));
        }

        // Sort by sort key
        if let Some(range_key) = table.range_key_name() {
            filtered.sort_by(|a, b| {
                let av = a.get(range_key);
                let bv = b.get(range_key);
                compare_attribute_values_opt(av, bv)
            });

            if !scan_index_forward {
                filtered.reverse();
            }
        }

        // Handle pagination
        let scanned_count = filtered.len();

        // Find start position
        let start_pos = if let Some(start_key) = exclusive_start_key {
            let start_key_str = table.get_key(start_key)?;
            filtered
                .iter()
                .position(|item| table.get_key(item).ok().as_ref() == Some(&start_key_str))
                .map(|p| p + 1)
                .unwrap_or(0)
        } else {
            0
        };

        filtered = filtered.into_iter().skip(start_pos).collect();

        // Apply limit
        let (items, last_evaluated_key) = if let Some(lim) = limit {
            if filtered.len() > lim {
                let last = filtered.get(lim - 1).cloned();
                filtered.truncate(lim);
                (filtered, last.map(|item| table.extract_key(&item)))
            } else {
                (filtered, None)
            }
        } else {
            (filtered, None)
        };

        Ok(QueryResult {
            count: items.len(),
            items,
            scanned_count,
            last_evaluated_key,
        })
    }

    /// Scan all items in a table
    #[allow(clippy::too_many_arguments)]
    pub fn scan(
        &self,
        table_name: &str,
        index_name: Option<&str>,
        filter_expression: Option<&str>,
        expression_attribute_names: Option<&HashMap<String, String>>,
        expression_attribute_values: Option<&HashMap<String, AttributeValue>>,
        limit: Option<usize>,
        exclusive_start_key: Option<&Item>,
    ) -> Result<QueryResult, DynamoDBError> {
        let table = self
            .tables
            .get(table_name)
            .ok_or_else(|| DynamoDBError::ResourceNotFound(table_name.to_string()))?;

        let ctx = ExpressionContext::new(expression_attribute_names, expression_attribute_values);

        // Parse filter expression
        let filter = filter_expression
            .map(parse_condition)
            .transpose()?
            .flatten();

        // Get all items
        let mut items: Vec<Item> = if let Some(idx_name) = index_name {
            // Scan GSI or LSI
            let index = table
                .global_secondary_indexes
                .get(idx_name)
                .or_else(|| table.local_secondary_indexes.get(idx_name))
                .ok_or_else(|| {
                    DynamoDBError::ValidationError(format!("Index not found: {}", idx_name))
                })?;

            let mut result = Vec::new();
            for entry in index.items.iter() {
                for primary_key in entry.value() {
                    if let Some(item) = table.items.get(primary_key) {
                        result.push(item.clone());
                    }
                }
            }
            result
        } else {
            table.items.iter().map(|r| r.value().clone()).collect()
        };

        let scanned_count = items.len();

        // Apply filter expression
        if let Some(ref filter_cond) = filter {
            items.retain(|item| evaluate_condition(filter_cond, item, &ctx).unwrap_or(false));
        }

        // Handle pagination
        let start_pos = if let Some(start_key) = exclusive_start_key {
            let start_key_str = table.get_key(start_key)?;
            items
                .iter()
                .position(|item| table.get_key(item).ok().as_ref() == Some(&start_key_str))
                .map(|p| p + 1)
                .unwrap_or(0)
        } else {
            0
        };

        items = items.into_iter().skip(start_pos).collect();

        // Apply limit
        let _count = items.len();
        let (items, last_evaluated_key) = if let Some(lim) = limit {
            if items.len() > lim {
                let last = items.get(lim - 1).cloned();
                items.truncate(lim);
                (items, last.map(|item| table.extract_key(&item)))
            } else {
                (items, None)
            }
        } else {
            (items, None)
        };

        Ok(QueryResult {
            count: items.len(),
            items,
            scanned_count,
            last_evaluated_key,
        })
    }
}

/// Return values option for UpdateItem
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReturnValues {
    None,
    AllOld,
    UpdatedOld,
    AllNew,
    UpdatedNew,
}

impl ReturnValues {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "ALL_OLD" => ReturnValues::AllOld,
            "UPDATED_OLD" => ReturnValues::UpdatedOld,
            "ALL_NEW" => ReturnValues::AllNew,
            "UPDATED_NEW" => ReturnValues::UpdatedNew,
            _ => ReturnValues::None,
        }
    }
}

/// Query/Scan result
#[derive(Debug)]
pub struct QueryResult {
    pub items: Vec<Item>,
    pub count: usize,
    pub scanned_count: usize,
    pub last_evaluated_key: Option<Item>,
}

/// Helper to compare optional attribute values
fn compare_attribute_values_opt(
    a: Option<&AttributeValue>,
    b: Option<&AttributeValue>,
) -> std::cmp::Ordering {
    match (a, b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(av), Some(bv)) => compare_attribute_values(av, bv),
    }
}

fn compare_attribute_values(a: &AttributeValue, b: &AttributeValue) -> std::cmp::Ordering {
    match (a, b) {
        (AttributeValue::S { S: s1 }, AttributeValue::S { S: s2 }) => s1.cmp(s2),
        (AttributeValue::N { N: n1 }, AttributeValue::N { N: n2 }) => {
            let num1: f64 = n1.parse().unwrap_or(0.0);
            let num2: f64 = n2.parse().unwrap_or(0.0);
            num1.partial_cmp(&num2).unwrap_or(std::cmp::Ordering::Equal)
        }
        (AttributeValue::B { B: b1 }, AttributeValue::B { B: b2 }) => b1.cmp(b2),
        _ => std::cmp::Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_storage() -> DynamoDBStorage {
        let storage = DynamoDBStorage::new();

        storage
            .create_table(
                "TestTable",
                vec![KeySchemaElement {
                    attribute_name: "pk".to_string(),
                    key_type: KeyType::HASH,
                }],
                vec![AttributeDefinition {
                    attribute_name: "pk".to_string(),
                    attribute_type: AttributeType::S,
                }],
                ProvisionedThroughput {
                    read_capacity_units: 5,
                    write_capacity_units: 5,
                },
                None,
                None,
            )
            .unwrap();

        storage
    }

    fn create_composite_key_storage() -> DynamoDBStorage {
        let storage = DynamoDBStorage::new();

        storage
            .create_table(
                "TestTable",
                vec![
                    KeySchemaElement {
                        attribute_name: "pk".to_string(),
                        key_type: KeyType::HASH,
                    },
                    KeySchemaElement {
                        attribute_name: "sk".to_string(),
                        key_type: KeyType::RANGE,
                    },
                ],
                vec![
                    AttributeDefinition {
                        attribute_name: "pk".to_string(),
                        attribute_type: AttributeType::S,
                    },
                    AttributeDefinition {
                        attribute_name: "sk".to_string(),
                        attribute_type: AttributeType::S,
                    },
                ],
                ProvisionedThroughput {
                    read_capacity_units: 5,
                    write_capacity_units: 5,
                },
                None,
                None,
            )
            .unwrap();

        storage
    }

    #[test]
    fn test_create_and_describe_table() {
        let storage = create_test_storage();

        let desc = storage.describe_table("TestTable").unwrap();
        assert_eq!(desc.table_name, "TestTable");
        assert_eq!(desc.table_status, TableStatus::ACTIVE);
    }

    #[test]
    fn test_list_tables() {
        let storage = create_test_storage();

        let tables = storage.list_tables();
        assert_eq!(tables.len(), 1);
        assert!(tables.contains(&"TestTable".to_string()));
    }

    #[test]
    fn test_put_and_get_item() {
        let storage = create_test_storage();

        let mut item = Item::new();
        item.insert("pk".to_string(), AttributeValue::string("item1"));
        item.insert("data".to_string(), AttributeValue::string("test data"));

        storage
            .put_item("TestTable", item.clone(), None, None, None)
            .unwrap();

        let mut key = Item::new();
        key.insert("pk".to_string(), AttributeValue::string("item1"));

        let result = storage.get_item("TestTable", key, None, None).unwrap();
        assert!(result.is_some());

        let retrieved = result.unwrap();
        assert_eq!(
            retrieved.get("data").unwrap().as_string().unwrap(),
            "test data"
        );
    }

    #[test]
    fn test_delete_item() {
        let storage = create_test_storage();

        let mut item = Item::new();
        item.insert("pk".to_string(), AttributeValue::string("item1"));
        storage
            .put_item("TestTable", item, None, None, None)
            .unwrap();

        let mut key = Item::new();
        key.insert("pk".to_string(), AttributeValue::string("item1"));

        let deleted = storage
            .delete_item("TestTable", key.clone(), None, None, None)
            .unwrap();
        assert!(deleted.is_some());

        let result = storage.get_item("TestTable", key, None, None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_item() {
        let storage = create_test_storage();

        // First put an item
        let mut item = Item::new();
        item.insert("pk".to_string(), AttributeValue::string("item1"));
        item.insert("count".to_string(), AttributeValue::number("5"));
        storage
            .put_item("TestTable", item, None, None, None)
            .unwrap();

        // Update it
        let mut key = Item::new();
        key.insert("pk".to_string(), AttributeValue::string("item1"));

        let mut values = HashMap::new();
        values.insert(":inc".to_string(), AttributeValue::number("1"));

        let result = storage
            .update_item(
                "TestTable",
                key.clone(),
                "SET count = count + :inc",
                None,
                None,
                Some(&values),
                ReturnValues::AllNew,
            )
            .unwrap();

        assert!(result.is_some());
        let updated = result.unwrap();
        assert_eq!(updated.get("count").unwrap().as_number().unwrap(), "6");
    }

    #[test]
    fn test_conditional_put_fails() {
        let storage = create_test_storage();

        // Put an item
        let mut item = Item::new();
        item.insert("pk".to_string(), AttributeValue::string("item1"));
        storage
            .put_item("TestTable", item.clone(), None, None, None)
            .unwrap();

        // Try to put again with condition that it doesn't exist
        let result = storage.put_item(
            "TestTable",
            item,
            Some("attribute_not_exists(pk)"),
            None,
            None,
        );

        assert!(matches!(result, Err(DynamoDBError::ConditionalCheckFailed)));
    }

    #[test]
    fn test_query_with_filter() {
        let storage = create_composite_key_storage();

        // Put some items
        for i in 0..5 {
            let mut item = Item::new();
            item.insert("pk".to_string(), AttributeValue::string("user1"));
            item.insert(
                "sk".to_string(),
                AttributeValue::string(format!("order#{}", i)),
            );
            item.insert(
                "amount".to_string(),
                AttributeValue::number(format!("{}", i * 10)),
            );
            storage
                .put_item("TestTable", item, None, None, None)
                .unwrap();
        }

        // Query with filter
        let mut values = HashMap::new();
        values.insert(":pk".to_string(), AttributeValue::string("user1"));
        values.insert(":min".to_string(), AttributeValue::number("20"));

        let result = storage
            .query(
                "TestTable",
                None,
                "pk = :pk",
                Some("amount >= :min"),
                None,
                Some(&values),
                true,
                None,
                None,
            )
            .unwrap();

        // Should have items with amount >= 20 (items 2, 3, 4)
        assert_eq!(result.items.len(), 3);
    }

    #[test]
    fn test_scan_with_filter() {
        let storage = create_test_storage();

        for i in 0..5 {
            let mut item = Item::new();
            item.insert(
                "pk".to_string(),
                AttributeValue::string(format!("item{}", i)),
            );
            item.insert(
                "status".to_string(),
                AttributeValue::string(if i % 2 == 0 { "active" } else { "inactive" }),
            );
            storage
                .put_item("TestTable", item, None, None, None)
                .unwrap();
        }

        let mut values = HashMap::new();
        values.insert(":status".to_string(), AttributeValue::string("active"));

        let result = storage
            .scan(
                "TestTable",
                None,
                Some("status = :status"),
                None,
                Some(&values),
                None,
                None,
            )
            .unwrap();

        // Should have 3 active items (0, 2, 4)
        assert_eq!(result.items.len(), 3);
    }

    #[test]
    fn test_delete_table() {
        let storage = create_test_storage();

        let desc = storage.delete_table("TestTable").unwrap();
        assert_eq!(desc.table_status, TableStatus::DELETING);

        let result = storage.describe_table("TestTable");
        assert!(result.is_err());
    }

    #[test]
    fn test_gsi_creation_and_query() {
        let storage = DynamoDBStorage::new();

        // Create table with GSI
        storage
            .create_table(
                "TestTable",
                vec![KeySchemaElement {
                    attribute_name: "pk".to_string(),
                    key_type: KeyType::HASH,
                }],
                vec![
                    AttributeDefinition {
                        attribute_name: "pk".to_string(),
                        attribute_type: AttributeType::S,
                    },
                    AttributeDefinition {
                        attribute_name: "gsi_pk".to_string(),
                        attribute_type: AttributeType::S,
                    },
                ],
                ProvisionedThroughput {
                    read_capacity_units: 5,
                    write_capacity_units: 5,
                },
                Some(vec![GlobalSecondaryIndex {
                    index_name: "gsi-index".to_string(),
                    key_schema: vec![KeySchemaElement {
                        attribute_name: "gsi_pk".to_string(),
                        key_type: KeyType::HASH,
                    }],
                    projection: Projection {
                        projection_type: ProjectionType::ALL,
                        non_key_attributes: None,
                    },
                    provisioned_throughput: None,
                }]),
                None,
            )
            .unwrap();

        // Add items
        for i in 0..3 {
            let mut item = Item::new();
            item.insert(
                "pk".to_string(),
                AttributeValue::string(format!("item{}", i)),
            );
            item.insert("gsi_pk".to_string(), AttributeValue::string("category1"));
            item.insert(
                "data".to_string(),
                AttributeValue::string(format!("data{}", i)),
            );
            storage
                .put_item("TestTable", item, None, None, None)
                .unwrap();
        }

        // Query GSI
        let mut values = HashMap::new();
        values.insert(":gsi_pk".to_string(), AttributeValue::string("category1"));

        let result = storage
            .query(
                "TestTable",
                Some("gsi-index"),
                "gsi_pk = :gsi_pk",
                None,
                None,
                Some(&values),
                true,
                None,
                None,
            )
            .unwrap();

        assert_eq!(result.items.len(), 3);
    }
}
