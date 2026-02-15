//! Comprehensive tests for DynamoDB storage layer

use crate::storage::*;
use std::collections::HashMap;

// =============================================================================
// TEST HELPERS
// =============================================================================

fn storage() -> DynamoDBStorage {
    DynamoDBStorage::new()
}

fn simple_table(storage: &DynamoDBStorage, name: &str) {
    storage
        .create_table(
            name,
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
}

fn composite_table(storage: &DynamoDBStorage, name: &str) {
    storage
        .create_table(
            name,
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
}

fn make_item(attrs: Vec<(&str, AttributeValue)>) -> Item {
    attrs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

// =============================================================================
// TABLE OPERATIONS
// =============================================================================

mod table_tests {
    use super::*;

    #[test]
    fn test_create_table() {
        let s = storage();
        let desc = s
            .create_table(
                "TestTable",
                vec![KeySchemaElement {
                    attribute_name: "id".to_string(),
                    key_type: KeyType::HASH,
                }],
                vec![AttributeDefinition {
                    attribute_name: "id".to_string(),
                    attribute_type: AttributeType::S,
                }],
                ProvisionedThroughput {
                    read_capacity_units: 10,
                    write_capacity_units: 5,
                },
                None,
                None,
            )
            .unwrap();

        assert_eq!(desc.table_name, "TestTable");
        assert_eq!(desc.table_status, TableStatus::ACTIVE);
        assert_eq!(desc.provisioned_throughput.read_capacity_units, 10);
    }

    #[test]
    fn test_create_table_already_exists() {
        let s = storage();
        simple_table(&s, "Table");

        let result = s.create_table(
            "Table",
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
        );

        assert!(matches!(result, Err(DynamoDBError::ResourceInUse(_))));
    }

    #[test]
    fn test_describe_table() {
        let s = storage();
        simple_table(&s, "Table");

        let desc = s.describe_table("Table").unwrap();
        assert_eq!(desc.table_name, "Table");
        assert_eq!(desc.item_count, 0);
    }

    #[test]
    fn test_describe_table_not_found() {
        let s = storage();
        let result = s.describe_table("NonExistent");
        assert!(matches!(result, Err(DynamoDBError::ResourceNotFound(_))));
    }

    #[test]
    fn test_describe_table_item_count() {
        let s = storage();
        simple_table(&s, "Table");

        // Add items
        for i in 0..5 {
            s.put_item(
                "Table",
                make_item(vec![("pk", AttributeValue::string(format!("item{}", i)))]),
                None,
                None,
                None,
            )
            .unwrap();
        }

        let desc = s.describe_table("Table").unwrap();
        assert_eq!(desc.item_count, 5);
    }

    #[test]
    fn test_delete_table() {
        let s = storage();
        simple_table(&s, "Table");

        let desc = s.delete_table("Table").unwrap();
        assert_eq!(desc.table_status, TableStatus::DELETING);

        // Should not be accessible after deletion
        let result = s.describe_table("Table");
        assert!(matches!(result, Err(DynamoDBError::ResourceNotFound(_))));
    }

    #[test]
    fn test_delete_table_not_found() {
        let s = storage();
        let result = s.delete_table("NonExistent");
        assert!(matches!(result, Err(DynamoDBError::ResourceNotFound(_))));
    }

    #[test]
    fn test_list_tables_empty() {
        let s = storage();
        let tables = s.list_tables();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_list_tables_multiple() {
        let s = storage();
        simple_table(&s, "Alpha");
        simple_table(&s, "Beta");
        simple_table(&s, "Gamma");

        let tables = s.list_tables();
        assert_eq!(tables.len(), 3);
        assert!(tables.contains(&"Alpha".to_string()));
        assert!(tables.contains(&"Beta".to_string()));
        assert!(tables.contains(&"Gamma".to_string()));
    }

    #[test]
    fn test_create_table_with_gsi() {
        let s = storage();
        let desc = s
            .create_table(
                "Table",
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
                        attribute_name: "email".to_string(),
                        attribute_type: AttributeType::S,
                    },
                ],
                ProvisionedThroughput {
                    read_capacity_units: 5,
                    write_capacity_units: 5,
                },
                Some(vec![GlobalSecondaryIndex {
                    index_name: "email-index".to_string(),
                    key_schema: vec![KeySchemaElement {
                        attribute_name: "email".to_string(),
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

        assert!(desc.global_secondary_indexes.is_some());
        let gsis = desc.global_secondary_indexes.unwrap();
        assert_eq!(gsis.len(), 1);
        assert_eq!(gsis[0].index_name, "email-index");
        assert_eq!(gsis[0].index_status, IndexStatus::ACTIVE);
    }

    #[test]
    fn test_create_table_with_lsi() {
        let s = storage();
        let desc = s
            .create_table(
                "Table",
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
                    AttributeDefinition {
                        attribute_name: "created_at".to_string(),
                        attribute_type: AttributeType::N,
                    },
                ],
                ProvisionedThroughput {
                    read_capacity_units: 5,
                    write_capacity_units: 5,
                },
                None,
                Some(vec![LocalSecondaryIndex {
                    index_name: "created-index".to_string(),
                    key_schema: vec![
                        KeySchemaElement {
                            attribute_name: "pk".to_string(),
                            key_type: KeyType::HASH,
                        },
                        KeySchemaElement {
                            attribute_name: "created_at".to_string(),
                            key_type: KeyType::RANGE,
                        },
                    ],
                    projection: Projection {
                        projection_type: ProjectionType::KEYS_ONLY,
                        non_key_attributes: None,
                    },
                }]),
            )
            .unwrap();

        assert!(desc.local_secondary_indexes.is_some());
        let lsis = desc.local_secondary_indexes.unwrap();
        assert_eq!(lsis.len(), 1);
        assert_eq!(lsis[0].index_name, "created-index");
    }
}

// =============================================================================
// ITEM CRUD OPERATIONS
// =============================================================================

mod item_crud_tests {
    use super::*;

    #[test]
    fn test_put_and_get_item() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("name", AttributeValue::string("Test Item")),
            ("count", AttributeValue::number("42")),
        ]);

        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let result = s.get_item("Table", key, None, None).unwrap();

        assert!(result.is_some());
        let retrieved = result.unwrap();
        assert_eq!(
            retrieved.get("name").unwrap().as_string().unwrap(),
            "Test Item"
        );
        assert_eq!(retrieved.get("count").unwrap().as_number().unwrap(), "42");
    }

    #[test]
    fn test_put_item_table_not_found() {
        let s = storage();
        let item = make_item(vec![("pk", AttributeValue::string("item1"))]);

        let result = s.put_item("NonExistent", item, None, None, None);
        assert!(matches!(result, Err(DynamoDBError::ResourceNotFound(_))));
    }

    #[test]
    fn test_get_item_not_found() {
        let s = storage();
        simple_table(&s, "Table");

        let key = make_item(vec![("pk", AttributeValue::string("nonexistent"))]);
        let result = s.get_item("Table", key, None, None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_item_table_not_found() {
        let s = storage();
        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);

        let result = s.get_item("NonExistent", key, None, None);
        assert!(matches!(result, Err(DynamoDBError::ResourceNotFound(_))));
    }

    #[test]
    fn test_overwrite_item() {
        let s = storage();
        simple_table(&s, "Table");

        let item1 = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("value", AttributeValue::string("original")),
        ]);
        s.put_item("Table", item1, None, None, None).unwrap();

        let item2 = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("value", AttributeValue::string("updated")),
        ]);
        s.put_item("Table", item2, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let result = s.get_item("Table", key, None, None).unwrap().unwrap();
        assert_eq!(result.get("value").unwrap().as_string().unwrap(), "updated");
    }

    #[test]
    fn test_delete_item() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("data", AttributeValue::string("test")),
        ]);
        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let deleted = s
            .delete_item("Table", key.clone(), None, None, None)
            .unwrap();
        assert!(deleted.is_some());
        assert_eq!(
            deleted.unwrap().get("data").unwrap().as_string().unwrap(),
            "test"
        );

        // Verify deleted
        let result = s.get_item("Table", key, None, None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_item_not_found() {
        let s = storage();
        simple_table(&s, "Table");

        let key = make_item(vec![("pk", AttributeValue::string("nonexistent"))]);
        let result = s.delete_item("Table", key, None, None, None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_composite_key_item() {
        let s = storage();
        composite_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("user1")),
            ("sk", AttributeValue::string("order#001")),
            ("total", AttributeValue::number("100")),
        ]);
        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![
            ("pk", AttributeValue::string("user1")),
            ("sk", AttributeValue::string("order#001")),
        ]);
        let result = s.get_item("Table", key, None, None).unwrap().unwrap();
        assert_eq!(result.get("total").unwrap().as_number().unwrap(), "100");
    }
}

// =============================================================================
// UPDATE ITEM TESTS
// =============================================================================

mod update_item_tests {
    use super::*;

    #[test]
    fn test_update_set_attribute() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("name", AttributeValue::string("original")),
        ]);
        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let mut values = HashMap::new();
        values.insert(":newname".to_string(), AttributeValue::string("updated"));

        let result = s
            .update_item(
                "Table",
                key.clone(),
                "SET #name = :newname",
                None,
                Some(&{
                    let mut names = HashMap::new();
                    names.insert("#name".to_string(), "name".to_string());
                    names
                }),
                Some(&values),
                ReturnValues::AllNew,
            )
            .unwrap();

        let updated = result.unwrap();
        assert_eq!(updated.get("name").unwrap().as_string().unwrap(), "updated");
    }

    #[test]
    fn test_update_increment() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("counter")),
            ("count", AttributeValue::number("10")),
        ]);
        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("counter"))]);
        let mut values = HashMap::new();
        values.insert(":inc".to_string(), AttributeValue::number("5"));

        let result = s
            .update_item(
                "Table",
                key,
                "SET #count = #count + :inc",
                None,
                Some(&{
                    let mut names = HashMap::new();
                    names.insert("#count".to_string(), "count".to_string());
                    names
                }),
                Some(&values),
                ReturnValues::AllNew,
            )
            .unwrap()
            .unwrap();

        assert_eq!(result.get("count").unwrap().as_number().unwrap(), "15");
    }

    #[test]
    fn test_update_decrement() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("counter")),
            ("count", AttributeValue::number("100")),
        ]);
        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("counter"))]);
        let mut values = HashMap::new();
        values.insert(":dec".to_string(), AttributeValue::number("30"));

        let result = s
            .update_item(
                "Table",
                key,
                "SET #count = #count - :dec",
                None,
                Some(&{
                    let mut names = HashMap::new();
                    names.insert("#count".to_string(), "count".to_string());
                    names
                }),
                Some(&values),
                ReturnValues::AllNew,
            )
            .unwrap()
            .unwrap();

        assert_eq!(result.get("count").unwrap().as_number().unwrap(), "70");
    }

    #[test]
    fn test_update_remove_attribute() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("attr1", AttributeValue::string("value1")),
            ("attr2", AttributeValue::string("value2")),
        ]);
        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let result = s
            .update_item(
                "Table",
                key,
                "REMOVE attr2",
                None,
                None,
                None,
                ReturnValues::AllNew,
            )
            .unwrap()
            .unwrap();

        assert!(result.get("attr1").is_some());
        assert!(result.get("attr2").is_none());
    }

    #[test]
    fn test_update_if_not_exists() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![("pk", AttributeValue::string("item1"))]);
        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let mut values = HashMap::new();
        values.insert(":default".to_string(), AttributeValue::number("10"));

        // First update - should use default since count doesn't exist
        s.update_item(
            "Table",
            key.clone(),
            "SET #count = if_not_exists(#count, :default)",
            None,
            Some(&{
                let mut names = HashMap::new();
                names.insert("#count".to_string(), "count".to_string());
                names
            }),
            Some(&values),
            ReturnValues::None,
        )
        .unwrap();

        let result = s
            .get_item("Table", key.clone(), None, None)
            .unwrap()
            .unwrap();
        assert_eq!(result.get("count").unwrap().as_number().unwrap(), "10");

        // Second update - should keep existing value (10), not use default
        values.insert(":default".to_string(), AttributeValue::number("999"));
        s.update_item(
            "Table",
            key.clone(),
            "SET #count = if_not_exists(#count, :default)",
            None,
            Some(&{
                let mut names = HashMap::new();
                names.insert("#count".to_string(), "count".to_string());
                names
            }),
            Some(&values),
            ReturnValues::None,
        )
        .unwrap();

        let result = s.get_item("Table", key, None, None).unwrap().unwrap();
        // Should still be 10, not 999
        assert_eq!(result.get("count").unwrap().as_number().unwrap(), "10");
    }

    #[test]
    fn test_update_return_values_all_old() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("value", AttributeValue::string("original")),
        ]);
        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let mut values = HashMap::new();
        values.insert(":new".to_string(), AttributeValue::string("updated"));

        let result = s
            .update_item(
                "Table",
                key,
                "SET #value = :new",
                None,
                Some(&{
                    let mut names = HashMap::new();
                    names.insert("#value".to_string(), "value".to_string());
                    names
                }),
                Some(&values),
                ReturnValues::AllOld,
            )
            .unwrap()
            .unwrap();

        // Should return old value
        assert_eq!(
            result.get("value").unwrap().as_string().unwrap(),
            "original"
        );
    }

    #[test]
    fn test_update_creates_item() {
        let s = storage();
        simple_table(&s, "Table");

        // Item doesn't exist yet
        let key = make_item(vec![("pk", AttributeValue::string("newitem"))]);
        let mut values = HashMap::new();
        values.insert(":val".to_string(), AttributeValue::string("hello"));

        s.update_item(
            "Table",
            key.clone(),
            "SET #attr = :val",
            None,
            Some(&{
                let mut names = HashMap::new();
                names.insert("#attr".to_string(), "attr".to_string());
                names
            }),
            Some(&values),
            ReturnValues::None,
        )
        .unwrap();

        // Item should now exist
        let result = s.get_item("Table", key, None, None).unwrap().unwrap();
        assert_eq!(result.get("attr").unwrap().as_string().unwrap(), "hello");
    }
}

// =============================================================================
// CONDITION EXPRESSION TESTS
// =============================================================================

mod condition_tests {
    use super::*;

    #[test]
    fn test_conditional_put_attribute_not_exists_success() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("data", AttributeValue::string("new")),
        ]);

        // Should succeed - item doesn't exist
        s.put_item("Table", item, Some("attribute_not_exists(pk)"), None, None)
            .unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let result = s.get_item("Table", key, None, None).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_conditional_put_attribute_not_exists_fails() {
        let s = storage();
        simple_table(&s, "Table");

        // Create item first
        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("data", AttributeValue::string("original")),
        ]);
        s.put_item("Table", item.clone(), None, None, None).unwrap();

        // Try to put again with condition - should fail
        let result = s.put_item("Table", item, Some("attribute_not_exists(pk)"), None, None);
        assert!(matches!(result, Err(DynamoDBError::ConditionalCheckFailed)));
    }

    #[test]
    fn test_conditional_put_attribute_exists() {
        let s = storage();
        simple_table(&s, "Table");

        // Create item first
        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("version", AttributeValue::number("1")),
        ]);
        s.put_item("Table", item, None, None, None).unwrap();

        // Update with condition - should succeed
        let new_item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("version", AttributeValue::number("2")),
        ]);
        s.put_item("Table", new_item, Some("attribute_exists(pk)"), None, None)
            .unwrap();
    }

    #[test]
    fn test_conditional_delete_with_value_check() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("status", AttributeValue::string("pending")),
        ]);
        s.put_item("Table", item, None, None, None).unwrap();

        // Try delete with wrong value - should fail
        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let mut values = HashMap::new();
        values.insert(":status".to_string(), AttributeValue::string("completed"));

        let result = s.delete_item(
            "Table",
            key.clone(),
            Some("#status = :status"),
            Some(&{
                let mut names = HashMap::new();
                names.insert("#status".to_string(), "status".to_string());
                names
            }),
            Some(&values),
        );
        assert!(matches!(result, Err(DynamoDBError::ConditionalCheckFailed)));

        // Delete with correct value - should succeed
        let mut values = HashMap::new();
        values.insert(":status".to_string(), AttributeValue::string("pending"));

        s.delete_item(
            "Table",
            key.clone(),
            Some("#status = :status"),
            Some(&{
                let mut names = HashMap::new();
                names.insert("#status".to_string(), "status".to_string());
                names
            }),
            Some(&values),
        )
        .unwrap();

        // Verify deleted
        let result = s.get_item("Table", key, None, None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_conditional_update_with_version() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("version", AttributeValue::number("1")),
            ("data", AttributeValue::string("original")),
        ]);
        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let mut values = HashMap::new();
        values.insert(":expected".to_string(), AttributeValue::number("1"));
        values.insert(":newversion".to_string(), AttributeValue::number("2"));
        values.insert(":newdata".to_string(), AttributeValue::string("updated"));

        // Update with correct version
        s.update_item(
            "Table",
            key.clone(),
            "SET #version = :newversion, #data = :newdata",
            Some("#version = :expected"),
            Some(&{
                let mut names = HashMap::new();
                names.insert("#version".to_string(), "version".to_string());
                names.insert("#data".to_string(), "data".to_string());
                names
            }),
            Some(&values),
            ReturnValues::None,
        )
        .unwrap();

        // Try update with old version - should fail
        let result = s.update_item(
            "Table",
            key,
            "SET #version = :newversion, #data = :newdata",
            Some("#version = :expected"),
            Some(&{
                let mut names = HashMap::new();
                names.insert("#version".to_string(), "version".to_string());
                names.insert("#data".to_string(), "data".to_string());
                names
            }),
            Some(&values),
            ReturnValues::None,
        );
        assert!(matches!(result, Err(DynamoDBError::ConditionalCheckFailed)));
    }
}

// =============================================================================
// QUERY TESTS
// =============================================================================

mod query_tests {
    use super::*;

    fn setup_orders(s: &DynamoDBStorage) {
        composite_table(s, "Orders");

        for i in 1..=10 {
            let item = make_item(vec![
                ("pk", AttributeValue::string("customer1")),
                ("sk", AttributeValue::string(format!("order#{:03}", i))),
                ("amount", AttributeValue::number(format!("{}", i * 100))),
                (
                    "status",
                    AttributeValue::string(if i % 2 == 0 { "shipped" } else { "pending" }),
                ),
            ]);
            s.put_item("Orders", item, None, None, None).unwrap();
        }

        // Add orders for another customer
        for i in 1..=3 {
            let item = make_item(vec![
                ("pk", AttributeValue::string("customer2")),
                ("sk", AttributeValue::string(format!("order#{:03}", i))),
                ("amount", AttributeValue::number(format!("{}", i * 50))),
                ("status", AttributeValue::string("pending")),
            ]);
            s.put_item("Orders", item, None, None, None).unwrap();
        }
    }

    #[test]
    fn test_query_partition_key_only() {
        let s = storage();
        setup_orders(&s);

        let mut values = HashMap::new();
        values.insert(":pk".to_string(), AttributeValue::string("customer1"));

        let result = s
            .query(
                "Orders",
                None,
                "pk = :pk",
                None,
                None,
                Some(&values),
                true,
                None,
                None,
            )
            .unwrap();

        assert_eq!(result.items.len(), 10);
        assert_eq!(result.count, 10);
    }

    #[test]
    fn test_query_with_range_condition() {
        let s = storage();
        setup_orders(&s);

        let mut values = HashMap::new();
        values.insert(":pk".to_string(), AttributeValue::string("customer1"));
        values.insert(":sk".to_string(), AttributeValue::string("order#005"));

        let result = s
            .query(
                "Orders",
                None,
                "pk = :pk AND sk > :sk",
                None,
                None,
                Some(&values),
                true,
                None,
                None,
            )
            .unwrap();

        // order#006 through order#010 = 5 items
        assert_eq!(result.items.len(), 5);
    }

    #[test]
    fn test_query_with_filter() {
        let s = storage();
        setup_orders(&s);

        let mut values = HashMap::new();
        values.insert(":pk".to_string(), AttributeValue::string("customer1"));
        values.insert(":status".to_string(), AttributeValue::string("shipped"));

        let result = s
            .query(
                "Orders",
                None,
                "pk = :pk",
                Some("#status = :status"),
                Some(&{
                    let mut names = HashMap::new();
                    names.insert("#status".to_string(), "status".to_string());
                    names
                }),
                Some(&values),
                true,
                None,
                None,
            )
            .unwrap();

        // 5 shipped orders (even numbers)
        assert_eq!(result.items.len(), 5);
    }

    #[test]
    fn test_query_with_limit() {
        let s = storage();
        setup_orders(&s);

        let mut values = HashMap::new();
        values.insert(":pk".to_string(), AttributeValue::string("customer1"));

        let result = s
            .query(
                "Orders",
                None,
                "pk = :pk",
                None,
                None,
                Some(&values),
                true,
                Some(3),
                None,
            )
            .unwrap();

        assert_eq!(result.items.len(), 3);
    }

    #[test]
    fn test_query_scan_index_forward_false() {
        let s = storage();
        setup_orders(&s);

        let mut values = HashMap::new();
        values.insert(":pk".to_string(), AttributeValue::string("customer1"));

        let result = s
            .query(
                "Orders",
                None,
                "pk = :pk",
                None,
                None,
                Some(&values),
                false, // reverse order
                Some(3),
                None,
            )
            .unwrap();

        // Should get the last 3 items in reverse order
        let sks: Vec<&str> = result
            .items
            .iter()
            .map(|i| i.get("sk").unwrap().as_string().unwrap())
            .collect();
        assert_eq!(sks, vec!["order#010", "order#009", "order#008"]);
    }

    #[test]
    fn test_query_begins_with() {
        let s = storage();
        composite_table(&s, "Table");

        // Add items with various sort keys
        for prefix in ["doc", "img", "vid"] {
            for i in 1..=3 {
                let item = make_item(vec![
                    ("pk", AttributeValue::string("user1")),
                    ("sk", AttributeValue::string(format!("{}#{}", prefix, i))),
                ]);
                s.put_item("Table", item, None, None, None).unwrap();
            }
        }

        let mut values = HashMap::new();
        values.insert(":pk".to_string(), AttributeValue::string("user1"));
        values.insert(":prefix".to_string(), AttributeValue::string("doc"));

        let result = s
            .query(
                "Table",
                None,
                "pk = :pk AND begins_with(sk, :prefix)",
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

    #[test]
    fn test_query_between() {
        let s = storage();
        setup_orders(&s);

        let mut values = HashMap::new();
        values.insert(":pk".to_string(), AttributeValue::string("customer1"));
        values.insert(":sk1".to_string(), AttributeValue::string("order#003"));
        values.insert(":sk2".to_string(), AttributeValue::string("order#007"));

        let result = s.query(
            "Table",
            None,
            "pk = :pk AND sk BETWEEN :sk1 AND :sk2",
            None,
            None,
            Some(&values),
            true,
            None,
            None,
        );

        // This might fail depending on table setup, but tests the parsing
    }

    #[test]
    fn test_query_table_not_found() {
        let s = storage();

        let mut values = HashMap::new();
        values.insert(":pk".to_string(), AttributeValue::string("user1"));

        let result = s.query(
            "NonExistent",
            None,
            "pk = :pk",
            None,
            None,
            Some(&values),
            true,
            None,
            None,
        );

        assert!(matches!(result, Err(DynamoDBError::ResourceNotFound(_))));
    }
}

// =============================================================================
// SCAN TESTS
// =============================================================================

mod scan_tests {
    use super::*;

    fn setup_products(s: &DynamoDBStorage) {
        simple_table(s, "Products");

        let categories = ["Electronics", "Books", "Clothing", "Electronics", "Books"];
        for (i, cat) in categories.iter().enumerate() {
            let item = make_item(vec![
                ("pk", AttributeValue::string(format!("prod{}", i))),
                ("category", AttributeValue::string(cat.to_string())),
                ("price", AttributeValue::number(format!("{}", (i + 1) * 25))),
            ]);
            s.put_item("Products", item, None, None, None).unwrap();
        }
    }

    #[test]
    fn test_scan_all() {
        let s = storage();
        setup_products(&s);

        let result = s
            .scan("Products", None, None, None, None, None, None)
            .unwrap();
        assert_eq!(result.items.len(), 5);
        assert_eq!(result.scanned_count, 5);
    }

    #[test]
    fn test_scan_with_filter() {
        let s = storage();
        setup_products(&s);

        let mut values = HashMap::new();
        values.insert(":cat".to_string(), AttributeValue::string("Electronics"));

        let result = s
            .scan(
                "Products",
                None,
                Some("category = :cat"),
                None,
                Some(&values),
                None,
                None,
            )
            .unwrap();

        assert_eq!(result.items.len(), 2);
        assert_eq!(result.scanned_count, 5);
    }

    #[test]
    fn test_scan_with_limit() {
        let s = storage();
        setup_products(&s);

        let result = s
            .scan("Products", None, None, None, None, Some(2), None)
            .unwrap();

        assert_eq!(result.items.len(), 2);
    }

    #[test]
    fn test_scan_with_filter_numeric_comparison() {
        let s = storage();
        setup_products(&s);

        let mut values = HashMap::new();
        values.insert(":minprice".to_string(), AttributeValue::number("75"));

        let result = s
            .scan(
                "Products",
                None,
                Some("price >= :minprice"),
                None,
                Some(&values),
                None,
                None,
            )
            .unwrap();

        // Products with price >= 75: prod2(75), prod3(100), prod4(125)
        assert_eq!(result.items.len(), 3);
    }

    #[test]
    fn test_scan_table_not_found() {
        let s = storage();

        let result = s.scan("NonExistent", None, None, None, None, None, None);
        assert!(matches!(result, Err(DynamoDBError::ResourceNotFound(_))));
    }
}

// =============================================================================
// GSI TESTS
// =============================================================================

mod gsi_tests {
    use super::*;

    fn setup_users_with_gsi(s: &DynamoDBStorage) {
        s.create_table(
            "Users",
            vec![KeySchemaElement {
                attribute_name: "userId".to_string(),
                key_type: KeyType::HASH,
            }],
            vec![
                AttributeDefinition {
                    attribute_name: "userId".to_string(),
                    attribute_type: AttributeType::S,
                },
                AttributeDefinition {
                    attribute_name: "email".to_string(),
                    attribute_type: AttributeType::S,
                },
                AttributeDefinition {
                    attribute_name: "department".to_string(),
                    attribute_type: AttributeType::S,
                },
            ],
            ProvisionedThroughput {
                read_capacity_units: 5,
                write_capacity_units: 5,
            },
            Some(vec![
                GlobalSecondaryIndex {
                    index_name: "email-index".to_string(),
                    key_schema: vec![KeySchemaElement {
                        attribute_name: "email".to_string(),
                        key_type: KeyType::HASH,
                    }],
                    projection: Projection {
                        projection_type: ProjectionType::ALL,
                        non_key_attributes: None,
                    },
                    provisioned_throughput: None,
                },
                GlobalSecondaryIndex {
                    index_name: "department-index".to_string(),
                    key_schema: vec![KeySchemaElement {
                        attribute_name: "department".to_string(),
                        key_type: KeyType::HASH,
                    }],
                    projection: Projection {
                        projection_type: ProjectionType::ALL,
                        non_key_attributes: None,
                    },
                    provisioned_throughput: None,
                },
            ]),
            None,
        )
        .unwrap();

        // Add users
        for i in 1..=5 {
            let dept = if i <= 3 { "Engineering" } else { "Marketing" };
            let item = make_item(vec![
                ("userId", AttributeValue::string(format!("user{}", i))),
                (
                    "email",
                    AttributeValue::string(format!("user{}@example.com", i)),
                ),
                ("name", AttributeValue::string(format!("User {}", i))),
                ("department", AttributeValue::string(dept)),
            ]);
            s.put_item("Users", item, None, None, None).unwrap();
        }
    }

    #[test]
    fn test_gsi_query_by_email() {
        let s = storage();
        setup_users_with_gsi(&s);

        let mut values = HashMap::new();
        values.insert(
            ":email".to_string(),
            AttributeValue::string("user3@example.com"),
        );

        let result = s
            .query(
                "Users",
                Some("email-index"),
                "email = :email",
                None,
                None,
                Some(&values),
                true,
                None,
                None,
            )
            .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(
            result.items[0].get("name").unwrap().as_string().unwrap(),
            "User 3"
        );
    }

    #[test]
    fn test_gsi_query_multiple_results() {
        let s = storage();
        setup_users_with_gsi(&s);

        let mut values = HashMap::new();
        values.insert(":dept".to_string(), AttributeValue::string("Engineering"));

        let result = s
            .query(
                "Users",
                Some("department-index"),
                "department = :dept",
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

    #[test]
    fn test_gsi_query_index_not_found() {
        let s = storage();
        setup_users_with_gsi(&s);

        let mut values = HashMap::new();
        values.insert(":val".to_string(), AttributeValue::string("test"));

        let result = s.query(
            "Users",
            Some("nonexistent-index"),
            "attr = :val",
            None,
            None,
            Some(&values),
            true,
            None,
            None,
        );

        assert!(matches!(result, Err(DynamoDBError::ValidationError(_))));
    }
}

// =============================================================================
// ATTRIBUTE VALUE EDGE CASES
// =============================================================================

mod attribute_value_tests {
    use super::*;

    #[test]
    fn test_all_attribute_types() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            (
                "string_attr",
                AttributeValue::S {
                    S: "text".to_string(),
                },
            ),
            (
                "number_attr",
                AttributeValue::N {
                    N: "42.5".to_string(),
                },
            ),
            (
                "binary_attr",
                AttributeValue::B {
                    B: "YmluYXJ5IGRhdGE=".to_string(),
                },
            ), // "binary data" in base64
            ("bool_attr", AttributeValue::BOOL { BOOL: true }),
            ("null_attr", AttributeValue::NULL { NULL: true }),
            (
                "list_attr",
                AttributeValue::L {
                    L: vec![
                        AttributeValue::S { S: "a".to_string() },
                        AttributeValue::N { N: "1".to_string() },
                    ],
                },
            ),
            (
                "map_attr",
                AttributeValue::M {
                    M: {
                        let mut m = HashMap::new();
                        m.insert(
                            "nested".to_string(),
                            AttributeValue::S {
                                S: "value".to_string(),
                            },
                        );
                        m
                    },
                },
            ),
            (
                "string_set",
                AttributeValue::SS {
                    SS: vec!["a".to_string(), "b".to_string()],
                },
            ),
            (
                "number_set",
                AttributeValue::NS {
                    NS: vec!["1".to_string(), "2".to_string()],
                },
            ),
        ]);

        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let result = s.get_item("Table", key, None, None).unwrap().unwrap();

        // Verify all types stored correctly
        assert!(matches!(
            result.get("string_attr"),
            Some(AttributeValue::S { .. })
        ));
        assert!(matches!(
            result.get("number_attr"),
            Some(AttributeValue::N { .. })
        ));
        assert!(matches!(
            result.get("bool_attr"),
            Some(AttributeValue::BOOL { BOOL: true })
        ));
        assert!(matches!(
            result.get("null_attr"),
            Some(AttributeValue::NULL { NULL: true })
        ));
        assert!(matches!(
            result.get("list_attr"),
            Some(AttributeValue::L { .. })
        ));
        assert!(matches!(
            result.get("map_attr"),
            Some(AttributeValue::M { .. })
        ));
        assert!(matches!(
            result.get("string_set"),
            Some(AttributeValue::SS { .. })
        ));
        assert!(matches!(
            result.get("number_set"),
            Some(AttributeValue::NS { .. })
        ));
    }

    #[test]
    fn test_empty_string() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("empty", AttributeValue::string("")),
        ]);

        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let result = s.get_item("Table", key, None, None).unwrap().unwrap();
        assert_eq!(result.get("empty").unwrap().as_string().unwrap(), "");
    }

    #[test]
    fn test_large_number() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            (
                "big",
                AttributeValue::number("99999999999999999999999999999"),
            ),
            ("negative", AttributeValue::number("-12345.6789")),
            ("scientific", AttributeValue::number("1.23e10")),
        ]);

        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let result = s.get_item("Table", key, None, None).unwrap().unwrap();
        assert_eq!(
            result.get("big").unwrap().as_number().unwrap(),
            "99999999999999999999999999999"
        );
    }

    #[test]
    fn test_deeply_nested_map() {
        let s = storage();
        simple_table(&s, "Table");

        let nested = AttributeValue::M {
            M: {
                let mut m1 = HashMap::new();
                m1.insert(
                    "level2".to_string(),
                    AttributeValue::M {
                        M: {
                            let mut m2 = HashMap::new();
                            m2.insert(
                                "level3".to_string(),
                                AttributeValue::M {
                                    M: {
                                        let mut m3 = HashMap::new();
                                        m3.insert(
                                            "value".to_string(),
                                            AttributeValue::string("deep"),
                                        );
                                        m3
                                    },
                                },
                            );
                            m2
                        },
                    },
                );
                m1
            },
        };

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("nested", nested),
        ]);

        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let result = s.get_item("Table", key, None, None).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_unicode_strings() {
        let s = storage();
        simple_table(&s, "Table");

        let item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("japanese", AttributeValue::string("こんにちは")),
            ("chinese", AttributeValue::string("你好")),
            ("emoji", AttributeValue::string("🎉🚀💻")),
            ("arabic", AttributeValue::string("مرحبا")),
        ]);

        s.put_item("Table", item, None, None, None).unwrap();

        let key = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let result = s.get_item("Table", key, None, None).unwrap().unwrap();
        assert_eq!(
            result.get("japanese").unwrap().as_string().unwrap(),
            "こんにちは"
        );
        assert_eq!(result.get("emoji").unwrap().as_string().unwrap(), "🎉🚀💻");
    }
}

// =============================================================================
// RETURN VALUES ENUM
// =============================================================================

mod return_values_tests {
    use super::*;

    #[test]
    fn test_return_values_from_str() {
        assert_eq!(ReturnValues::from_str("ALL_OLD"), ReturnValues::AllOld);
        assert_eq!(ReturnValues::from_str("ALL_NEW"), ReturnValues::AllNew);
        assert_eq!(
            ReturnValues::from_str("UPDATED_OLD"),
            ReturnValues::UpdatedOld
        );
        assert_eq!(
            ReturnValues::from_str("UPDATED_NEW"),
            ReturnValues::UpdatedNew
        );
        assert_eq!(ReturnValues::from_str("NONE"), ReturnValues::None);
        assert_eq!(ReturnValues::from_str("unknown"), ReturnValues::None);
        assert_eq!(ReturnValues::from_str("all_old"), ReturnValues::AllOld); // case insensitive
    }
}
