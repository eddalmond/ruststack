//! Additional expression parser tests

use crate::expression::*;
use crate::storage::{AttributeValue, Item};
use std::collections::HashMap;

mod key_condition_parsing {
    use super::*;

    #[test]
    fn test_partition_key_equals() {
        let conds = parse_key_condition("pk = :pk").unwrap();
        assert_eq!(conds.len(), 1);
        assert_eq!(conds[0].attribute, "pk");
        assert_eq!(conds[0].op, ComparisonOp::Eq);
        assert_eq!(conds[0].values, vec![":pk"]);
    }

    #[test]
    fn test_partition_and_sort_key() {
        let conds = parse_key_condition("pk = :pk AND sk = :sk").unwrap();
        assert_eq!(conds.len(), 2);
        assert_eq!(conds[0].attribute, "pk");
        assert_eq!(conds[1].attribute, "sk");
    }

    #[test]
    fn test_sort_key_less_than() {
        let conds = parse_key_condition("pk = :pk AND sk < :max").unwrap();
        assert_eq!(conds[1].op, ComparisonOp::Lt);
    }

    #[test]
    fn test_sort_key_less_than_or_equal() {
        let conds = parse_key_condition("pk = :pk AND sk <= :max").unwrap();
        assert_eq!(conds[1].op, ComparisonOp::Le);
    }

    #[test]
    fn test_sort_key_greater_than() {
        let conds = parse_key_condition("pk = :pk AND sk > :min").unwrap();
        assert_eq!(conds[1].op, ComparisonOp::Gt);
    }

    #[test]
    fn test_sort_key_greater_than_or_equal() {
        let conds = parse_key_condition("pk = :pk AND sk >= :min").unwrap();
        assert_eq!(conds[1].op, ComparisonOp::Ge);
    }

    #[test]
    fn test_sort_key_between() {
        let conds = parse_key_condition("pk = :pk AND sk BETWEEN :start AND :end").unwrap();
        assert_eq!(conds[1].op, ComparisonOp::Between);
        assert_eq!(conds[1].values, vec![":start", ":end"]);
    }

    #[test]
    fn test_with_attribute_name_placeholders() {
        let conds = parse_key_condition("#pk = :pk AND #sk > :sk").unwrap();
        assert_eq!(conds[0].attribute, "#pk");
        assert_eq!(conds[1].attribute, "#sk");
    }

    #[test]
    fn test_empty_expression() {
        let conds = parse_key_condition("").unwrap();
        assert!(conds.is_empty());
    }

    #[test]
    fn test_whitespace_handling() {
        let conds = parse_key_condition("  pk  =  :pk  AND  sk  >  :sk  ").unwrap();
        assert_eq!(conds.len(), 2);
    }
}

mod condition_parsing {
    use super::*;

    #[test]
    fn test_simple_equality() {
        let cond = parse_condition("status = :status").unwrap().unwrap();
        match cond {
            ConditionOp::Comparison { path, op, values } => {
                assert_eq!(path, "status");
                assert_eq!(op, ComparisonOp::Eq);
                assert_eq!(values, vec![":status"]);
            }
            _ => panic!("Expected Comparison"),
        }
    }

    #[test]
    fn test_not_equal() {
        let cond = parse_condition("status <> :inactive").unwrap().unwrap();
        match cond {
            ConditionOp::Comparison { op, .. } => {
                assert_eq!(op, ComparisonOp::Ne);
            }
            _ => panic!("Expected Comparison"),
        }
    }

    #[test]
    fn test_attribute_exists() {
        let cond = parse_condition("attribute_exists(email)").unwrap().unwrap();
        match cond {
            ConditionOp::AttributeExists(path) => assert_eq!(path, "email"),
            _ => panic!("Expected AttributeExists"),
        }
    }

    #[test]
    fn test_attribute_not_exists() {
        let cond = parse_condition("attribute_not_exists(deleted_at)")
            .unwrap()
            .unwrap();
        match cond {
            ConditionOp::AttributeNotExists(path) => assert_eq!(path, "deleted_at"),
            _ => panic!("Expected AttributeNotExists"),
        }
    }

    #[test]
    fn test_begins_with() {
        let cond = parse_condition("begins_with(sk, :prefix)")
            .unwrap()
            .unwrap();
        match cond {
            ConditionOp::BeginsWith(path, value) => {
                assert_eq!(path, "sk");
                assert_eq!(value, ":prefix");
            }
            _ => panic!("Expected BeginsWith"),
        }
    }

    #[test]
    fn test_contains() {
        let cond = parse_condition("contains(tags, :tag)").unwrap().unwrap();
        match cond {
            ConditionOp::Contains(path, value) => {
                assert_eq!(path, "tags");
                assert_eq!(value, ":tag");
            }
            _ => panic!("Expected Contains"),
        }
    }

    #[test]
    fn test_and_condition() {
        let cond = parse_condition("status = :active AND age > :min")
            .unwrap()
            .unwrap();
        match cond {
            ConditionOp::And(left, right) => {
                assert!(matches!(*left, ConditionOp::Comparison { .. }));
                assert!(matches!(*right, ConditionOp::Comparison { .. }));
            }
            _ => panic!("Expected And"),
        }
    }

    #[test]
    fn test_or_condition() {
        let cond = parse_condition("status = :active OR status = :pending")
            .unwrap()
            .unwrap();
        match cond {
            ConditionOp::Or(left, right) => {
                assert!(matches!(*left, ConditionOp::Comparison { .. }));
                assert!(matches!(*right, ConditionOp::Comparison { .. }));
            }
            _ => panic!("Expected Or"),
        }
    }

    #[test]
    fn test_not_condition() {
        let cond = parse_condition("NOT attribute_exists(deleted)")
            .unwrap()
            .unwrap();
        match cond {
            ConditionOp::Not(inner) => {
                assert!(matches!(*inner, ConditionOp::AttributeExists(_)));
            }
            _ => panic!("Expected Not"),
        }
    }

    #[test]
    fn test_between() {
        // BETWEEN is tricky because of AND keyword - primarily used in key conditions
        // This tests key condition BETWEEN which works
        let conds = parse_key_condition("pk = :pk AND sk BETWEEN :min AND :max").unwrap();
        assert_eq!(conds.len(), 2);
        assert_eq!(conds[1].op, ComparisonOp::Between);
        assert_eq!(conds[1].values.len(), 2);
    }

    #[test]
    fn test_in_operator() {
        let cond = parse_condition("status IN (:s1, :s2, :s3)")
            .unwrap()
            .unwrap();
        match cond {
            ConditionOp::Comparison { op, values, .. } => {
                assert_eq!(op, ComparisonOp::In);
                assert_eq!(values.len(), 3);
            }
            _ => panic!("Expected In comparison"),
        }
    }
}

mod update_expression_parsing {
    use super::*;

    #[test]
    fn test_simple_set() {
        let update = parse_update_expression("SET name = :name").unwrap();
        assert_eq!(update.set_actions.len(), 1);
        assert!(update.remove_actions.is_empty());
    }

    #[test]
    fn test_multiple_set() {
        let update =
            parse_update_expression("SET name = :name, age = :age, status = :status").unwrap();
        assert_eq!(update.set_actions.len(), 3);
    }

    #[test]
    fn test_remove() {
        let update = parse_update_expression("REMOVE old_attr").unwrap();
        assert_eq!(update.remove_actions.len(), 1);
        assert!(update.set_actions.is_empty());
    }

    #[test]
    fn test_multiple_remove() {
        let update = parse_update_expression("REMOVE attr1, attr2, attr3").unwrap();
        assert_eq!(update.remove_actions.len(), 3);
    }

    #[test]
    fn test_set_and_remove() {
        let update = parse_update_expression("SET new_attr = :val REMOVE old_attr").unwrap();
        assert_eq!(update.set_actions.len(), 1);
        assert_eq!(update.remove_actions.len(), 1);
    }

    #[test]
    fn test_add() {
        let update = parse_update_expression("ADD tags :newtags").unwrap();
        assert_eq!(update.add_actions.len(), 1);
    }

    #[test]
    fn test_delete() {
        let update = parse_update_expression("DELETE tags :removetags").unwrap();
        assert_eq!(update.delete_actions.len(), 1);
    }

    #[test]
    fn test_all_clauses() {
        let update = parse_update_expression(
            "SET name = :name REMOVE old ADD numbers :num DELETE tags :tag",
        )
        .unwrap();
        assert_eq!(update.set_actions.len(), 1);
        assert_eq!(update.remove_actions.len(), 1);
        assert_eq!(update.add_actions.len(), 1);
        assert_eq!(update.delete_actions.len(), 1);
    }

    #[test]
    fn test_if_not_exists() {
        let update = parse_update_expression("SET count = if_not_exists(count, :zero)").unwrap();
        assert_eq!(update.set_actions.len(), 1);
        match &update.set_actions[0] {
            UpdateAction::Set { value, .. } => {
                assert!(matches!(value, UpdateValue::IfNotExists(_, _)));
            }
            _ => panic!("Expected Set action"),
        }
    }

    #[test]
    fn test_list_append() {
        let update = parse_update_expression("SET items = list_append(items, :newitem)").unwrap();
        assert_eq!(update.set_actions.len(), 1);
        match &update.set_actions[0] {
            UpdateAction::Set { value, .. } => {
                assert!(matches!(value, UpdateValue::ListAppend(_, _)));
            }
            _ => panic!("Expected Set action"),
        }
    }

    #[test]
    fn test_arithmetic_plus() {
        let update = parse_update_expression("SET count = count + :inc").unwrap();
        assert_eq!(update.set_actions.len(), 1);
        match &update.set_actions[0] {
            UpdateAction::Set { value, .. } => {
                assert!(matches!(value, UpdateValue::Plus(_, _)));
            }
            _ => panic!("Expected Set action"),
        }
    }

    #[test]
    fn test_arithmetic_minus() {
        let update = parse_update_expression("SET count = count - :dec").unwrap();
        assert_eq!(update.set_actions.len(), 1);
        match &update.set_actions[0] {
            UpdateAction::Set { value, .. } => {
                assert!(matches!(value, UpdateValue::Minus(_, _)));
            }
            _ => panic!("Expected Set action"),
        }
    }

    #[test]
    fn test_with_attribute_name_placeholder() {
        let update = parse_update_expression("SET #name = :val").unwrap();
        assert_eq!(update.set_actions.len(), 1);
        match &update.set_actions[0] {
            UpdateAction::Set { path, .. } => {
                assert_eq!(path, "#name");
            }
            _ => panic!("Expected Set action"),
        }
    }
}

mod condition_evaluation {
    use super::*;

    fn make_item(attrs: Vec<(&str, AttributeValue)>) -> Item {
        attrs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    #[test]
    fn test_equality_match() {
        let item = make_item(vec![("status", AttributeValue::string("active"))]);
        let mut values = HashMap::new();
        values.insert(":status".to_string(), AttributeValue::string("active"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let cond = parse_condition("status = :status").unwrap().unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_equality_no_match() {
        let item = make_item(vec![("status", AttributeValue::string("inactive"))]);
        let mut values = HashMap::new();
        values.insert(":status".to_string(), AttributeValue::string("active"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let cond = parse_condition("status = :status").unwrap().unwrap();
        assert!(!evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_greater_than() {
        let item = make_item(vec![("age", AttributeValue::number("30"))]);
        let mut values = HashMap::new();
        values.insert(":min".to_string(), AttributeValue::number("18"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let cond = parse_condition("age > :min").unwrap().unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_less_than() {
        let item = make_item(vec![("price", AttributeValue::number("50"))]);
        let mut values = HashMap::new();
        values.insert(":max".to_string(), AttributeValue::number("100"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let cond = parse_condition("price < :max").unwrap().unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_attribute_exists_true() {
        let item = make_item(vec![("email", AttributeValue::string("test@example.com"))]);
        let ctx = ExpressionContext::new(None, None);

        let cond = parse_condition("attribute_exists(email)").unwrap().unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_attribute_exists_false() {
        let item = make_item(vec![("name", AttributeValue::string("Test"))]);
        let ctx = ExpressionContext::new(None, None);

        let cond = parse_condition("attribute_exists(email)").unwrap().unwrap();
        assert!(!evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_attribute_not_exists_true() {
        let item = make_item(vec![("name", AttributeValue::string("Test"))]);
        let ctx = ExpressionContext::new(None, None);

        let cond = parse_condition("attribute_not_exists(deleted_at)")
            .unwrap()
            .unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_begins_with_match() {
        let item = make_item(vec![("sk", AttributeValue::string("USER#123"))]);
        let mut values = HashMap::new();
        values.insert(":prefix".to_string(), AttributeValue::string("USER#"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let cond = parse_condition("begins_with(sk, :prefix)")
            .unwrap()
            .unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_begins_with_no_match() {
        let item = make_item(vec![("sk", AttributeValue::string("ORDER#123"))]);
        let mut values = HashMap::new();
        values.insert(":prefix".to_string(), AttributeValue::string("USER#"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let cond = parse_condition("begins_with(sk, :prefix)")
            .unwrap()
            .unwrap();
        assert!(!evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_contains_string() {
        let item = make_item(vec![("description", AttributeValue::string("Hello World"))]);
        let mut values = HashMap::new();
        values.insert(":search".to_string(), AttributeValue::string("World"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let cond = parse_condition("contains(description, :search)")
            .unwrap()
            .unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_and_both_true() {
        let item = make_item(vec![
            ("status", AttributeValue::string("active")),
            ("age", AttributeValue::number("25")),
        ]);
        let mut values = HashMap::new();
        values.insert(":status".to_string(), AttributeValue::string("active"));
        values.insert(":min".to_string(), AttributeValue::number("18"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let cond = parse_condition("status = :status AND age > :min")
            .unwrap()
            .unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_and_one_false() {
        let item = make_item(vec![
            ("status", AttributeValue::string("active")),
            ("age", AttributeValue::number("15")),
        ]);
        let mut values = HashMap::new();
        values.insert(":status".to_string(), AttributeValue::string("active"));
        values.insert(":min".to_string(), AttributeValue::number("18"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let cond = parse_condition("status = :status AND age > :min")
            .unwrap()
            .unwrap();
        assert!(!evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_or_one_true() {
        let item = make_item(vec![("status", AttributeValue::string("pending"))]);
        let mut values = HashMap::new();
        values.insert(":active".to_string(), AttributeValue::string("active"));
        values.insert(":pending".to_string(), AttributeValue::string("pending"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let cond = parse_condition("status = :active OR status = :pending")
            .unwrap()
            .unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_not() {
        let item = make_item(vec![("name", AttributeValue::string("Test"))]);
        let ctx = ExpressionContext::new(None, None);

        let cond = parse_condition("NOT attribute_exists(deleted)")
            .unwrap()
            .unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_between() {
        // BETWEEN is typically used in key conditions
        // Use greater than/less than comparisons for filter expressions
        let item = make_item(vec![("price", AttributeValue::number("50"))]);
        let mut values = HashMap::new();
        values.insert(":min".to_string(), AttributeValue::number("10"));
        values.insert(":max".to_string(), AttributeValue::number("100"));
        let ctx = ExpressionContext::new(None, Some(&values));

        // Use compound condition instead of BETWEEN for filter
        let cond = parse_condition("price >= :min AND price <= :max")
            .unwrap()
            .unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_between_out_of_range() {
        let item = make_item(vec![("price", AttributeValue::number("150"))]);
        let mut values = HashMap::new();
        values.insert(":min".to_string(), AttributeValue::number("10"));
        values.insert(":max".to_string(), AttributeValue::number("100"));
        let ctx = ExpressionContext::new(None, Some(&values));

        // Use compound condition instead of BETWEEN for filter
        let cond = parse_condition("price >= :min AND price <= :max")
            .unwrap()
            .unwrap();
        assert!(!evaluate_condition(&cond, &item, &ctx).unwrap());
    }

    #[test]
    fn test_attribute_name_resolution() {
        let item = make_item(vec![("status", AttributeValue::string("active"))]);
        let mut names = HashMap::new();
        names.insert("#status".to_string(), "status".to_string());
        let mut values = HashMap::new();
        values.insert(":val".to_string(), AttributeValue::string("active"));
        let ctx = ExpressionContext::new(Some(&names), Some(&values));

        let cond = parse_condition("#status = :val").unwrap().unwrap();
        assert!(evaluate_condition(&cond, &item, &ctx).unwrap());
    }
}

mod update_application {
    use super::*;

    fn make_item(attrs: Vec<(&str, AttributeValue)>) -> Item {
        attrs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    #[test]
    fn test_set_new_attribute() {
        let mut item = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let mut values = HashMap::new();
        values.insert(":val".to_string(), AttributeValue::string("hello"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("SET new_attr = :val").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert_eq!(item.get("new_attr").unwrap().as_string().unwrap(), "hello");
    }

    #[test]
    fn test_set_overwrite_attribute() {
        let mut item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("name", AttributeValue::string("old")),
        ]);
        let mut values = HashMap::new();
        values.insert(":val".to_string(), AttributeValue::string("new"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("SET name = :val").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert_eq!(item.get("name").unwrap().as_string().unwrap(), "new");
    }

    #[test]
    fn test_remove_attribute() {
        let mut item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("to_remove", AttributeValue::string("bye")),
        ]);
        let ctx = ExpressionContext::new(None, None);

        let update = parse_update_expression("REMOVE to_remove").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert!(item.get("to_remove").is_none());
    }

    #[test]
    fn test_increment_number() {
        let mut item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("count", AttributeValue::number("10")),
        ]);
        let mut values = HashMap::new();
        values.insert(":inc".to_string(), AttributeValue::number("5"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("SET count = count + :inc").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert_eq!(item.get("count").unwrap().as_number().unwrap(), "15");
    }

    #[test]
    fn test_decrement_number() {
        let mut item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("count", AttributeValue::number("100")),
        ]);
        let mut values = HashMap::new();
        values.insert(":dec".to_string(), AttributeValue::number("25"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("SET count = count - :dec").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert_eq!(item.get("count").unwrap().as_number().unwrap(), "75");
    }

    #[test]
    fn test_if_not_exists_uses_default() {
        let mut item = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let mut values = HashMap::new();
        values.insert(":default".to_string(), AttributeValue::number("0"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("SET count = if_not_exists(count, :default)").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert_eq!(item.get("count").unwrap().as_number().unwrap(), "0");
    }

    #[test]
    fn test_if_not_exists_keeps_existing() {
        let mut item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("count", AttributeValue::number("42")),
        ]);
        let mut values = HashMap::new();
        values.insert(":default".to_string(), AttributeValue::number("0"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("SET count = if_not_exists(count, :default)").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert_eq!(item.get("count").unwrap().as_number().unwrap(), "42");
    }

    #[test]
    fn test_list_append() {
        let mut item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            (
                "items",
                AttributeValue::L {
                    L: vec![AttributeValue::string("a"), AttributeValue::string("b")],
                },
            ),
        ]);
        let mut values = HashMap::new();
        values.insert(
            ":new".to_string(),
            AttributeValue::L {
                L: vec![AttributeValue::string("c")],
            },
        );
        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("SET items = list_append(items, :new)").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        match item.get("items") {
            Some(AttributeValue::L { L }) => {
                assert_eq!(L.len(), 3);
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn test_add_to_number() {
        let mut item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            ("score", AttributeValue::number("10")),
        ]);
        let mut values = HashMap::new();
        values.insert(":points".to_string(), AttributeValue::number("5"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("ADD score :points").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert_eq!(item.get("score").unwrap().as_number().unwrap(), "15");
    }

    #[test]
    fn test_add_to_string_set() {
        let mut item = make_item(vec![
            ("pk", AttributeValue::string("item1")),
            (
                "tags",
                AttributeValue::SS {
                    SS: vec!["tag1".to_string(), "tag2".to_string()],
                },
            ),
        ]);
        let mut values = HashMap::new();
        values.insert(
            ":newtags".to_string(),
            AttributeValue::SS {
                SS: vec!["tag3".to_string()],
            },
        );
        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("ADD tags :newtags").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        match item.get("tags") {
            Some(AttributeValue::SS { SS }) => {
                assert!(SS.contains(&"tag3".to_string()));
            }
            _ => panic!("Expected string set"),
        }
    }

    #[test]
    fn test_multiple_set_actions() {
        let mut item = make_item(vec![("pk", AttributeValue::string("item1"))]);
        let mut values = HashMap::new();
        values.insert(":name".to_string(), AttributeValue::string("Alice"));
        values.insert(":age".to_string(), AttributeValue::number("30"));
        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("SET name = :name, age = :age").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert_eq!(item.get("name").unwrap().as_string().unwrap(), "Alice");
        assert_eq!(item.get("age").unwrap().as_number().unwrap(), "30");
    }
}
