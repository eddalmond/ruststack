//! DynamoDB expression parser and evaluator
//!
//! Supports:
//! - KeyConditionExpression (for Query)
//! - FilterExpression (for Query/Scan)
//! - ConditionExpression (for conditional writes)
//! - UpdateExpression (for UpdateItem)
//! - ProjectionExpression (for attribute selection)

use crate::storage::{AttributeValue, Item};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExpressionError {
    #[error("Invalid expression: {0}")]
    Invalid(String),

    #[error("Missing expression attribute name: {0}")]
    MissingAttributeName(String),

    #[error("Missing expression attribute value: {0}")]
    MissingAttributeValue(String),

    #[error("Type mismatch in expression")]
    TypeMismatch,

    #[error("Invalid operator: {0}")]
    InvalidOperator(String),
}

/// Comparison operators
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    Eq,      // =
    Ne,      // <>
    Lt,      // <
    Le,      // <=
    Gt,      // >
    Ge,      // >=
    Between, // BETWEEN
    In,      // IN
}

/// Condition operators
#[derive(Debug, Clone)]
pub enum ConditionOp {
    Comparison {
        path: String,
        op: ComparisonOp,
        values: Vec<String>, // Expression attribute value placeholders
    },
    AttributeExists(String),
    AttributeNotExists(String),
    AttributeType(String, String), // path, type
    BeginsWith(String, String),    // path, value placeholder
    Contains(String, String),      // path, value placeholder
    Size(String),                  // path - returns size for comparison
    And(Box<ConditionOp>, Box<ConditionOp>),
    Or(Box<ConditionOp>, Box<ConditionOp>),
    Not(Box<ConditionOp>),
}

/// Update action types
#[derive(Debug, Clone)]
pub enum UpdateAction {
    Set { path: String, value: UpdateValue },
    Remove { path: String },
    Add { path: String, value: String },    // value placeholder
    Delete { path: String, value: String }, // value placeholder (for sets)
}

/// Update value (can be a reference to expression value or computed)
#[derive(Debug, Clone)]
pub enum UpdateValue {
    Value(String),                         // :value placeholder
    Path(String),                          // attribute path
    IfNotExists(String, Box<UpdateValue>), // path, default
    ListAppend(Box<UpdateValue>, Box<UpdateValue>),
    Plus(Box<UpdateValue>, Box<UpdateValue>),
    Minus(Box<UpdateValue>, Box<UpdateValue>),
}

/// Parsed update expression
#[derive(Debug, Clone, Default)]
pub struct ParsedUpdateExpression {
    pub set_actions: Vec<UpdateAction>,
    pub remove_actions: Vec<UpdateAction>,
    pub add_actions: Vec<UpdateAction>,
    pub delete_actions: Vec<UpdateAction>,
}

/// Expression context for evaluation
pub struct ExpressionContext<'a> {
    pub attribute_names: Option<&'a HashMap<String, String>>,
    pub attribute_values: Option<&'a HashMap<String, AttributeValue>>,
}

impl<'a> ExpressionContext<'a> {
    pub fn new(
        names: Option<&'a HashMap<String, String>>,
        values: Option<&'a HashMap<String, AttributeValue>>,
    ) -> Self {
        Self {
            attribute_names: names,
            attribute_values: values,
        }
    }

    /// Resolve an attribute name (handles #name placeholders)
    pub fn resolve_name(&self, name: &str) -> Result<String, ExpressionError> {
        if name.starts_with('#') {
            self.attribute_names
                .and_then(|m| m.get(name))
                .cloned()
                .ok_or_else(|| ExpressionError::MissingAttributeName(name.to_string()))
        } else {
            Ok(name.to_string())
        }
    }

    /// Resolve an attribute value (handles :value placeholders)
    pub fn resolve_value(&self, placeholder: &str) -> Result<&AttributeValue, ExpressionError> {
        self.attribute_values
            .and_then(|m| m.get(placeholder))
            .ok_or_else(|| ExpressionError::MissingAttributeValue(placeholder.to_string()))
    }
}

/// Parse a key condition expression
///
/// Key conditions are restricted to:
/// - partition_key = :value
/// - partition_key = :value AND sort_key <op> :value
/// - partition_key = :value AND sort_key BETWEEN :v1 AND :v2
/// - partition_key = :value AND begins_with(sort_key, :v)
pub fn parse_key_condition(expression: &str) -> Result<Vec<KeyCondition>, ExpressionError> {
    let expression = expression.trim();
    if expression.is_empty() {
        return Ok(vec![]);
    }

    let mut conditions = Vec::new();

    // Split by AND (case insensitive)
    let parts: Vec<&str> = split_by_and(expression);

    for part in parts {
        let part = part.trim();
        conditions.push(parse_single_key_condition(part)?);
    }

    Ok(conditions)
}

/// A single key condition
#[derive(Debug, Clone)]
pub struct KeyCondition {
    pub attribute: String,
    pub op: ComparisonOp,
    pub values: Vec<String>,
}

fn parse_single_key_condition(expr: &str) -> Result<KeyCondition, ExpressionError> {
    let expr = expr.trim();

    // Check for begins_with function
    if let Some(inner) = extract_function_args(expr, "begins_with") {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(ExpressionError::Invalid(
                "begins_with requires 2 arguments".to_string(),
            ));
        }
        return Ok(KeyCondition {
            attribute: parts[0].to_string(),
            op: ComparisonOp::Eq, // Special handling needed
            values: vec![format!("BEGINS_WITH:{}", parts[1])],
        });
    }

    // Check for BETWEEN
    let between_re =
        regex_lite::Regex::new(r"(?i)^([#\w]+)\s+BETWEEN\s+([:\w]+)\s+AND\s+([:\w]+)$").unwrap();
    if let Some(caps) = between_re.captures(expr) {
        return Ok(KeyCondition {
            attribute: caps[1].to_string(),
            op: ComparisonOp::Between,
            values: vec![caps[2].to_string(), caps[3].to_string()],
        });
    }

    // Standard comparison: attr op value
    let comp_re = regex_lite::Regex::new(r"^([#\w]+)\s*(=|<>|<=|>=|<|>)\s*([:\w]+)$").unwrap();
    if let Some(caps) = comp_re.captures(expr) {
        let op = match &caps[2] {
            "=" => ComparisonOp::Eq,
            "<>" => ComparisonOp::Ne,
            "<" => ComparisonOp::Lt,
            "<=" => ComparisonOp::Le,
            ">" => ComparisonOp::Gt,
            ">=" => ComparisonOp::Ge,
            _ => return Err(ExpressionError::InvalidOperator(caps[2].to_string())),
        };
        return Ok(KeyCondition {
            attribute: caps[1].to_string(),
            op,
            values: vec![caps[3].to_string()],
        });
    }

    Err(ExpressionError::Invalid(format!(
        "Could not parse key condition: {}",
        expr
    )))
}

/// Parse a filter/condition expression
pub fn parse_condition(expression: &str) -> Result<Option<ConditionOp>, ExpressionError> {
    let expression = expression.trim();
    if expression.is_empty() {
        return Ok(None);
    }

    parse_condition_expr(expression).map(Some)
}

fn parse_condition_expr(expr: &str) -> Result<ConditionOp, ExpressionError> {
    let expr = expr.trim();

    // Handle parentheses
    if expr.starts_with('(') && expr.ends_with(')') {
        // Check if these parens wrap the whole expression
        if find_matching_paren(expr, 0) == Some(expr.len() - 1) {
            return parse_condition_expr(&expr[1..expr.len() - 1]);
        }
    }

    // Try to split by OR first (lowest precedence)
    if let Some((left, right)) = split_by_operator(expr, "OR") {
        let left_cond = parse_condition_expr(left)?;
        let right_cond = parse_condition_expr(right)?;
        return Ok(ConditionOp::Or(Box::new(left_cond), Box::new(right_cond)));
    }

    // Then AND
    if let Some((left, right)) = split_by_operator(expr, "AND") {
        let left_cond = parse_condition_expr(left)?;
        let right_cond = parse_condition_expr(right)?;
        return Ok(ConditionOp::And(Box::new(left_cond), Box::new(right_cond)));
    }

    // NOT
    if expr.to_uppercase().starts_with("NOT ") {
        let inner = &expr[4..];
        let inner_cond = parse_condition_expr(inner)?;
        return Ok(ConditionOp::Not(Box::new(inner_cond)));
    }

    // Functions
    if let Some(inner) = extract_function_args(expr, "attribute_exists") {
        return Ok(ConditionOp::AttributeExists(inner.trim().to_string()));
    }
    if let Some(inner) = extract_function_args(expr, "attribute_not_exists") {
        return Ok(ConditionOp::AttributeNotExists(inner.trim().to_string()));
    }
    if let Some(inner) = extract_function_args(expr, "attribute_type") {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(ExpressionError::Invalid(
                "attribute_type requires 2 arguments".to_string(),
            ));
        }
        return Ok(ConditionOp::AttributeType(
            parts[0].to_string(),
            parts[1].to_string(),
        ));
    }
    if let Some(inner) = extract_function_args(expr, "begins_with") {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(ExpressionError::Invalid(
                "begins_with requires 2 arguments".to_string(),
            ));
        }
        return Ok(ConditionOp::BeginsWith(
            parts[0].to_string(),
            parts[1].to_string(),
        ));
    }
    if let Some(inner) = extract_function_args(expr, "contains") {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(ExpressionError::Invalid(
                "contains requires 2 arguments".to_string(),
            ));
        }
        return Ok(ConditionOp::Contains(
            parts[0].to_string(),
            parts[1].to_string(),
        ));
    }

    // BETWEEN
    let between_re =
        regex_lite::Regex::new(r"(?i)^([#\w.]+)\s+BETWEEN\s+([:\w]+)\s+AND\s+([:\w]+)$").unwrap();
    if let Some(caps) = between_re.captures(expr) {
        return Ok(ConditionOp::Comparison {
            path: caps[1].to_string(),
            op: ComparisonOp::Between,
            values: vec![caps[2].to_string(), caps[3].to_string()],
        });
    }

    // IN operator
    let in_re = regex_lite::Regex::new(r"(?i)^([#\w.]+)\s+IN\s*\(([^)]+)\)$").unwrap();
    if let Some(caps) = in_re.captures(expr) {
        let values: Vec<String> = caps[2].split(',').map(|s| s.trim().to_string()).collect();
        return Ok(ConditionOp::Comparison {
            path: caps[1].to_string(),
            op: ComparisonOp::In,
            values,
        });
    }

    // Standard comparison
    let comp_re = regex_lite::Regex::new(r"^([#\w.]+)\s*(=|<>|<=|>=|<|>)\s*([:\w]+)$").unwrap();
    if let Some(caps) = comp_re.captures(expr) {
        let op = match &caps[2] {
            "=" => ComparisonOp::Eq,
            "<>" => ComparisonOp::Ne,
            "<" => ComparisonOp::Lt,
            "<=" => ComparisonOp::Le,
            ">" => ComparisonOp::Gt,
            ">=" => ComparisonOp::Ge,
            _ => return Err(ExpressionError::InvalidOperator(caps[2].to_string())),
        };
        return Ok(ConditionOp::Comparison {
            path: caps[1].to_string(),
            op,
            values: vec![caps[3].to_string()],
        });
    }

    Err(ExpressionError::Invalid(format!(
        "Could not parse condition: {}",
        expr
    )))
}

/// Parse an update expression
pub fn parse_update_expression(
    expression: &str,
) -> Result<ParsedUpdateExpression, ExpressionError> {
    let mut result = ParsedUpdateExpression::default();
    let expression = expression.trim();

    if expression.is_empty() {
        return Ok(result);
    }

    // Split into clauses (SET, REMOVE, ADD, DELETE)
    let clauses = split_update_clauses(expression);

    for (clause_type, clause_body) in clauses {
        match clause_type.to_uppercase().as_str() {
            "SET" => {
                for action in parse_set_clause(&clause_body)? {
                    result.set_actions.push(action);
                }
            }
            "REMOVE" => {
                for action in parse_remove_clause(&clause_body)? {
                    result.remove_actions.push(action);
                }
            }
            "ADD" => {
                for action in parse_add_clause(&clause_body)? {
                    result.add_actions.push(action);
                }
            }
            "DELETE" => {
                for action in parse_delete_clause(&clause_body)? {
                    result.delete_actions.push(action);
                }
            }
            _ => {
                return Err(ExpressionError::Invalid(format!(
                    "Unknown update clause: {}",
                    clause_type
                )))
            }
        }
    }

    Ok(result)
}

fn split_update_clauses(expr: &str) -> Vec<(String, String)> {
    let mut clauses = Vec::new();
    let keywords = ["SET", "REMOVE", "ADD", "DELETE"];

    let _upper = expr.to_uppercase();
    let mut positions: Vec<(usize, &str)> = keywords
        .iter()
        .filter_map(|&kw| {
            // Find keyword at word boundary
            let re = regex_lite::Regex::new(&format!(r"(?i)\b{}\s", kw)).unwrap();
            re.find(expr).map(|m| (m.start(), kw))
        })
        .collect();

    positions.sort_by_key(|(pos, _)| *pos);

    for i in 0..positions.len() {
        let (start, keyword) = positions[i];
        let clause_start = start + keyword.len();
        let clause_end = if i + 1 < positions.len() {
            positions[i + 1].0
        } else {
            expr.len()
        };

        let body = expr[clause_start..clause_end].trim().to_string();
        clauses.push((keyword.to_string(), body));
    }

    clauses
}

fn parse_set_clause(clause: &str) -> Result<Vec<UpdateAction>, ExpressionError> {
    let mut actions = Vec::new();

    // Split by comma (but not inside functions)
    let parts = split_by_comma(clause);

    for part in parts {
        let part = part.trim();
        // Format: path = value_expression
        if let Some(eq_pos) = part.find('=') {
            let path = part[..eq_pos].trim().to_string();
            let value_expr = part[eq_pos + 1..].trim();
            let value = parse_update_value(value_expr)?;
            actions.push(UpdateAction::Set { path, value });
        } else {
            return Err(ExpressionError::Invalid(format!(
                "Invalid SET action: {}",
                part
            )));
        }
    }

    Ok(actions)
}

fn parse_remove_clause(clause: &str) -> Result<Vec<UpdateAction>, ExpressionError> {
    let mut actions = Vec::new();

    let parts = split_by_comma(clause);

    for part in parts {
        let path = part.trim().to_string();
        if !path.is_empty() {
            actions.push(UpdateAction::Remove { path });
        }
    }

    Ok(actions)
}

fn parse_add_clause(clause: &str) -> Result<Vec<UpdateAction>, ExpressionError> {
    let mut actions = Vec::new();

    let parts = split_by_comma(clause);

    for part in parts {
        let part = part.trim();
        // Format: path value
        let space_pos = part
            .find(|c: char| c.is_whitespace())
            .ok_or_else(|| ExpressionError::Invalid(format!("Invalid ADD action: {}", part)))?;
        let path = part[..space_pos].trim().to_string();
        let value = part[space_pos..].trim().to_string();
        actions.push(UpdateAction::Add { path, value });
    }

    Ok(actions)
}

fn parse_delete_clause(clause: &str) -> Result<Vec<UpdateAction>, ExpressionError> {
    let mut actions = Vec::new();

    let parts = split_by_comma(clause);

    for part in parts {
        let part = part.trim();
        // Format: path value
        let space_pos = part
            .find(|c: char| c.is_whitespace())
            .ok_or_else(|| ExpressionError::Invalid(format!("Invalid DELETE action: {}", part)))?;
        let path = part[..space_pos].trim().to_string();
        let value = part[space_pos..].trim().to_string();
        actions.push(UpdateAction::Delete { path, value });
    }

    Ok(actions)
}

fn parse_update_value(expr: &str) -> Result<UpdateValue, ExpressionError> {
    let expr = expr.trim();

    // if_not_exists(path, value)
    if let Some(inner) = extract_function_args(expr, "if_not_exists") {
        let parts: Vec<&str> = split_function_args(inner);
        if parts.len() != 2 {
            return Err(ExpressionError::Invalid(
                "if_not_exists requires 2 arguments".to_string(),
            ));
        }
        return Ok(UpdateValue::IfNotExists(
            parts[0].trim().to_string(),
            Box::new(parse_update_value(parts[1])?),
        ));
    }

    // list_append(list1, list2)
    if let Some(inner) = extract_function_args(expr, "list_append") {
        let parts: Vec<&str> = split_function_args(inner);
        if parts.len() != 2 {
            return Err(ExpressionError::Invalid(
                "list_append requires 2 arguments".to_string(),
            ));
        }
        return Ok(UpdateValue::ListAppend(
            Box::new(parse_update_value(parts[0])?),
            Box::new(parse_update_value(parts[1])?),
        ));
    }

    // Arithmetic: path + value or path - value
    if let Some(plus_pos) = find_operator(expr, '+') {
        let left = &expr[..plus_pos];
        let right = &expr[plus_pos + 1..];
        return Ok(UpdateValue::Plus(
            Box::new(parse_update_value(left)?),
            Box::new(parse_update_value(right)?),
        ));
    }

    if let Some(minus_pos) = find_operator(expr, '-') {
        let left = &expr[..minus_pos];
        let right = &expr[minus_pos + 1..];
        return Ok(UpdateValue::Minus(
            Box::new(parse_update_value(left)?),
            Box::new(parse_update_value(right)?),
        ));
    }

    // Value placeholder (:value)
    if expr.starts_with(':') {
        return Ok(UpdateValue::Value(expr.to_string()));
    }

    // Attribute path
    Ok(UpdateValue::Path(expr.to_string()))
}

// === Evaluation Functions ===

/// Evaluate a condition against an item
pub fn evaluate_condition(
    condition: &ConditionOp,
    item: &Item,
    ctx: &ExpressionContext,
) -> Result<bool, ExpressionError> {
    match condition {
        ConditionOp::Comparison { path, op, values } => {
            let resolved_path = ctx.resolve_name(path)?;
            let item_value = get_nested_attribute(item, &resolved_path);

            match op {
                ComparisonOp::Eq => {
                    let expected = ctx.resolve_value(&values[0])?;
                    Ok(item_value
                        .map(|v| compare_attribute_values(v, expected) == std::cmp::Ordering::Equal)
                        .unwrap_or(false))
                }
                ComparisonOp::Ne => {
                    let expected = ctx.resolve_value(&values[0])?;
                    Ok(item_value
                        .map(|v| compare_attribute_values(v, expected) != std::cmp::Ordering::Equal)
                        .unwrap_or(true))
                }
                ComparisonOp::Lt => {
                    let expected = ctx.resolve_value(&values[0])?;
                    Ok(item_value
                        .map(|v| compare_attribute_values(v, expected) == std::cmp::Ordering::Less)
                        .unwrap_or(false))
                }
                ComparisonOp::Le => {
                    let expected = ctx.resolve_value(&values[0])?;
                    Ok(item_value
                        .map(|v| {
                            let cmp = compare_attribute_values(v, expected);
                            cmp == std::cmp::Ordering::Less || cmp == std::cmp::Ordering::Equal
                        })
                        .unwrap_or(false))
                }
                ComparisonOp::Gt => {
                    let expected = ctx.resolve_value(&values[0])?;
                    Ok(item_value
                        .map(|v| {
                            compare_attribute_values(v, expected) == std::cmp::Ordering::Greater
                        })
                        .unwrap_or(false))
                }
                ComparisonOp::Ge => {
                    let expected = ctx.resolve_value(&values[0])?;
                    Ok(item_value
                        .map(|v| {
                            let cmp = compare_attribute_values(v, expected);
                            cmp == std::cmp::Ordering::Greater || cmp == std::cmp::Ordering::Equal
                        })
                        .unwrap_or(false))
                }
                ComparisonOp::Between => {
                    let low = ctx.resolve_value(&values[0])?;
                    let high = ctx.resolve_value(&values[1])?;
                    Ok(item_value
                        .map(|v| {
                            let cmp_low = compare_attribute_values(v, low);
                            let cmp_high = compare_attribute_values(v, high);
                            (cmp_low == std::cmp::Ordering::Greater
                                || cmp_low == std::cmp::Ordering::Equal)
                                && (cmp_high == std::cmp::Ordering::Less
                                    || cmp_high == std::cmp::Ordering::Equal)
                        })
                        .unwrap_or(false))
                }
                ComparisonOp::In => {
                    if let Some(val) = item_value {
                        for v in values {
                            let expected = ctx.resolve_value(v)?;
                            if compare_attribute_values(val, expected) == std::cmp::Ordering::Equal
                            {
                                return Ok(true);
                            }
                        }
                    }
                    Ok(false)
                }
            }
        }
        ConditionOp::AttributeExists(path) => {
            let resolved_path = ctx.resolve_name(path)?;
            Ok(get_nested_attribute(item, &resolved_path).is_some())
        }
        ConditionOp::AttributeNotExists(path) => {
            let resolved_path = ctx.resolve_name(path)?;
            Ok(get_nested_attribute(item, &resolved_path).is_none())
        }
        ConditionOp::AttributeType(path, type_val) => {
            let resolved_path = ctx.resolve_name(path)?;
            let expected_type = ctx.resolve_value(type_val)?;
            if let Some(val) = get_nested_attribute(item, &resolved_path) {
                let type_str = match val {
                    AttributeValue::S { .. } => "S",
                    AttributeValue::N { .. } => "N",
                    AttributeValue::B { .. } => "B",
                    AttributeValue::BOOL { .. } => "BOOL",
                    AttributeValue::NULL { .. } => "NULL",
                    AttributeValue::L { .. } => "L",
                    AttributeValue::M { .. } => "M",
                    AttributeValue::SS { .. } => "SS",
                    AttributeValue::NS { .. } => "NS",
                    AttributeValue::BS { .. } => "BS",
                };
                if let AttributeValue::S { S: expected } = expected_type {
                    return Ok(type_str == expected);
                }
            }
            Ok(false)
        }
        ConditionOp::BeginsWith(path, value) => {
            let resolved_path = ctx.resolve_name(path)?;
            let prefix = ctx.resolve_value(value)?;
            if let Some(AttributeValue::S { S: item_str }) =
                get_nested_attribute(item, &resolved_path)
            {
                if let AttributeValue::S { S: prefix_str } = prefix {
                    return Ok(item_str.starts_with(prefix_str));
                }
            }
            Ok(false)
        }
        ConditionOp::Contains(path, value) => {
            let resolved_path = ctx.resolve_name(path)?;
            let search = ctx.resolve_value(value)?;
            if let Some(item_val) = get_nested_attribute(item, &resolved_path) {
                match (item_val, search) {
                    (AttributeValue::S { S: haystack }, AttributeValue::S { S: needle }) => {
                        return Ok(haystack.contains(needle));
                    }
                    (AttributeValue::L { L: list }, search_val) => {
                        for elem in list {
                            if compare_attribute_values(elem, search_val)
                                == std::cmp::Ordering::Equal
                            {
                                return Ok(true);
                            }
                        }
                    }
                    (AttributeValue::SS { SS: set }, AttributeValue::S { S: needle }) => {
                        return Ok(set.contains(needle));
                    }
                    (AttributeValue::NS { NS: set }, AttributeValue::N { N: needle }) => {
                        return Ok(set.contains(needle));
                    }
                    _ => {}
                }
            }
            Ok(false)
        }
        ConditionOp::Size(_path) => {
            // Size returns a number for comparison - this needs special handling
            // in the comparison that wraps it. For now, we can't evaluate standalone.
            Err(ExpressionError::Invalid(
                "size() must be used in a comparison".to_string(),
            ))
        }
        ConditionOp::And(left, right) => {
            Ok(evaluate_condition(left, item, ctx)? && evaluate_condition(right, item, ctx)?)
        }
        ConditionOp::Or(left, right) => {
            Ok(evaluate_condition(left, item, ctx)? || evaluate_condition(right, item, ctx)?)
        }
        ConditionOp::Not(inner) => Ok(!evaluate_condition(inner, item, ctx)?),
    }
}

/// Evaluate key conditions against an item
pub fn evaluate_key_conditions(
    conditions: &[KeyCondition],
    item: &Item,
    ctx: &ExpressionContext,
) -> Result<bool, ExpressionError> {
    for cond in conditions {
        let resolved_attr = ctx.resolve_name(&cond.attribute)?;
        let item_value = item.get(&resolved_attr);

        let matches = match &cond.op {
            ComparisonOp::Eq => {
                // Check for begins_with special case
                if cond.values.len() == 1 && cond.values[0].starts_with("BEGINS_WITH:") {
                    let prefix_placeholder = &cond.values[0]["BEGINS_WITH:".len()..];
                    let prefix = ctx.resolve_value(prefix_placeholder)?;
                    if let (
                        Some(AttributeValue::S { S: item_str }),
                        AttributeValue::S { S: prefix_str },
                    ) = (item_value, prefix)
                    {
                        item_str.starts_with(prefix_str)
                    } else {
                        false
                    }
                } else {
                    let expected = ctx.resolve_value(&cond.values[0])?;
                    item_value
                        .map(|v| compare_attribute_values(v, expected) == std::cmp::Ordering::Equal)
                        .unwrap_or(false)
                }
            }
            ComparisonOp::Lt => {
                let expected = ctx.resolve_value(&cond.values[0])?;
                item_value
                    .map(|v| compare_attribute_values(v, expected) == std::cmp::Ordering::Less)
                    .unwrap_or(false)
            }
            ComparisonOp::Le => {
                let expected = ctx.resolve_value(&cond.values[0])?;
                item_value
                    .map(|v| {
                        let cmp = compare_attribute_values(v, expected);
                        cmp == std::cmp::Ordering::Less || cmp == std::cmp::Ordering::Equal
                    })
                    .unwrap_or(false)
            }
            ComparisonOp::Gt => {
                let expected = ctx.resolve_value(&cond.values[0])?;
                item_value
                    .map(|v| compare_attribute_values(v, expected) == std::cmp::Ordering::Greater)
                    .unwrap_or(false)
            }
            ComparisonOp::Ge => {
                let expected = ctx.resolve_value(&cond.values[0])?;
                item_value
                    .map(|v| {
                        let cmp = compare_attribute_values(v, expected);
                        cmp == std::cmp::Ordering::Greater || cmp == std::cmp::Ordering::Equal
                    })
                    .unwrap_or(false)
            }
            ComparisonOp::Between => {
                let low = ctx.resolve_value(&cond.values[0])?;
                let high = ctx.resolve_value(&cond.values[1])?;
                item_value
                    .map(|v| {
                        let cmp_low = compare_attribute_values(v, low);
                        let cmp_high = compare_attribute_values(v, high);
                        (cmp_low == std::cmp::Ordering::Greater
                            || cmp_low == std::cmp::Ordering::Equal)
                            && (cmp_high == std::cmp::Ordering::Less
                                || cmp_high == std::cmp::Ordering::Equal)
                    })
                    .unwrap_or(false)
            }
            _ => false,
        };

        if !matches {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Apply update expression to an item
pub fn apply_update(
    item: &mut Item,
    update: &ParsedUpdateExpression,
    ctx: &ExpressionContext,
) -> Result<(), ExpressionError> {
    // Apply SET actions
    for action in &update.set_actions {
        if let UpdateAction::Set { path, value } = action {
            let resolved_path = ctx.resolve_name(path)?;
            let new_value = evaluate_update_value(value, item, ctx)?;
            set_nested_attribute(item, &resolved_path, new_value);
        }
    }

    // Apply REMOVE actions
    for action in &update.remove_actions {
        if let UpdateAction::Remove { path } = action {
            let resolved_path = ctx.resolve_name(path)?;
            remove_nested_attribute(item, &resolved_path);
        }
    }

    // Apply ADD actions (for numbers and sets)
    for action in &update.add_actions {
        if let UpdateAction::Add { path, value } = action {
            let resolved_path = ctx.resolve_name(path)?;
            let add_value = ctx.resolve_value(value)?;

            match (item.get(&resolved_path), add_value) {
                // Add to number
                (Some(AttributeValue::N { N: current }), AttributeValue::N { N: add }) => {
                    let current_num: f64 = current.parse().unwrap_or(0.0);
                    let add_num: f64 = add.parse().unwrap_or(0.0);
                    item.insert(
                        resolved_path,
                        AttributeValue::N {
                            N: (current_num + add_num).to_string(),
                        },
                    );
                }
                // Create number if doesn't exist
                (None, AttributeValue::N { N: add }) => {
                    item.insert(resolved_path, AttributeValue::N { N: add.clone() });
                }
                // Add to string set
                (Some(AttributeValue::SS { SS: current }), AttributeValue::SS { SS: add }) => {
                    let mut new_set = current.clone();
                    for s in add {
                        if !new_set.contains(s) {
                            new_set.push(s.clone());
                        }
                    }
                    item.insert(resolved_path, AttributeValue::SS { SS: new_set });
                }
                // Create string set if doesn't exist
                (None, AttributeValue::SS { SS: add }) => {
                    item.insert(resolved_path, AttributeValue::SS { SS: add.clone() });
                }
                // Add to number set
                (Some(AttributeValue::NS { NS: current }), AttributeValue::NS { NS: add }) => {
                    let mut new_set = current.clone();
                    for n in add {
                        if !new_set.contains(n) {
                            new_set.push(n.clone());
                        }
                    }
                    item.insert(resolved_path, AttributeValue::NS { NS: new_set });
                }
                // Create number set if doesn't exist
                (None, AttributeValue::NS { NS: add }) => {
                    item.insert(resolved_path, AttributeValue::NS { NS: add.clone() });
                }
                _ => return Err(ExpressionError::TypeMismatch),
            }
        }
    }

    // Apply DELETE actions (for sets)
    for action in &update.delete_actions {
        if let UpdateAction::Delete { path, value } = action {
            let resolved_path = ctx.resolve_name(path)?;
            let delete_value = ctx.resolve_value(value)?;

            match (item.get_mut(&resolved_path), delete_value) {
                (Some(AttributeValue::SS { SS: current }), AttributeValue::SS { SS: delete }) => {
                    current.retain(|s| !delete.contains(s));
                }
                (Some(AttributeValue::NS { NS: current }), AttributeValue::NS { NS: delete }) => {
                    current.retain(|n| !delete.contains(n));
                }
                _ => {} // No-op if types don't match
            }
        }
    }

    Ok(())
}

fn evaluate_update_value(
    value: &UpdateValue,
    item: &Item,
    ctx: &ExpressionContext,
) -> Result<AttributeValue, ExpressionError> {
    match value {
        UpdateValue::Value(placeholder) => Ok(ctx.resolve_value(placeholder)?.clone()),
        UpdateValue::Path(path) => {
            let resolved_path = ctx.resolve_name(path)?;
            get_nested_attribute(item, &resolved_path)
                .cloned()
                .ok_or(ExpressionError::MissingAttributeValue(resolved_path))
        }
        UpdateValue::IfNotExists(path, default) => {
            let resolved_path = ctx.resolve_name(path)?;
            if let Some(existing) = get_nested_attribute(item, &resolved_path) {
                Ok(existing.clone())
            } else {
                evaluate_update_value(default, item, ctx)
            }
        }
        UpdateValue::ListAppend(list1, list2) => {
            let val1 = evaluate_update_value(list1, item, ctx)?;
            let val2 = evaluate_update_value(list2, item, ctx)?;

            match (val1, val2) {
                (AttributeValue::L { L: mut l1 }, AttributeValue::L { L: l2 }) => {
                    l1.extend(l2);
                    Ok(AttributeValue::L { L: l1 })
                }
                _ => Err(ExpressionError::TypeMismatch),
            }
        }
        UpdateValue::Plus(left, right) => {
            let val1 = evaluate_update_value(left, item, ctx)?;
            let val2 = evaluate_update_value(right, item, ctx)?;

            match (val1, val2) {
                (AttributeValue::N { N: n1 }, AttributeValue::N { N: n2 }) => {
                    let num1: f64 = n1.parse().unwrap_or(0.0);
                    let num2: f64 = n2.parse().unwrap_or(0.0);
                    Ok(AttributeValue::N {
                        N: (num1 + num2).to_string(),
                    })
                }
                _ => Err(ExpressionError::TypeMismatch),
            }
        }
        UpdateValue::Minus(left, right) => {
            let val1 = evaluate_update_value(left, item, ctx)?;
            let val2 = evaluate_update_value(right, item, ctx)?;

            match (val1, val2) {
                (AttributeValue::N { N: n1 }, AttributeValue::N { N: n2 }) => {
                    let num1: f64 = n1.parse().unwrap_or(0.0);
                    let num2: f64 = n2.parse().unwrap_or(0.0);
                    Ok(AttributeValue::N {
                        N: (num1 - num2).to_string(),
                    })
                }
                _ => Err(ExpressionError::TypeMismatch),
            }
        }
    }
}

// === Helper Functions ===

fn split_by_and(expr: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut last_pos = 0;
    let upper = expr.to_uppercase();
    let bytes = expr.as_bytes();

    // Track if we're inside a BETWEEN clause
    let mut in_between = false;

    let mut i = 0;
    while i < expr.len() {
        if bytes[i] == b'(' {
            depth += 1;
        } else if bytes[i] == b')' {
            depth -= 1;
        } else if depth == 0 {
            // Check for BETWEEN keyword
            if i + 7 <= upper.len()
                && &upper[i..i + 7] == "BETWEEN"
                && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
            {
                in_between = true;
            }

            // Check for AND keyword
            if i + 3 <= expr.len()
                && &upper[i..i + 3] == "AND"
                && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
                && (i + 3 >= expr.len() || !bytes[i + 3].is_ascii_alphanumeric())
            {
                if in_between {
                    // This AND is part of BETWEEN...AND, not a condition separator
                    in_between = false;
                } else {
                    // This is a condition separator
                    parts.push(expr[last_pos..i].trim());
                    last_pos = i + 3;
                    i += 3;
                    continue;
                }
            }
        }
        i += 1;
    }

    parts.push(expr[last_pos..].trim());
    parts.into_iter().filter(|s| !s.is_empty()).collect()
}

fn split_by_operator<'a>(expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let upper = expr.to_uppercase();
    let _op_upper = format!(" {} ", op.to_uppercase());
    let bytes = expr.as_bytes();
    let mut depth = 0;

    let mut i = 0;
    while i < expr.len() {
        if bytes[i] == b'(' {
            depth += 1;
        } else if bytes[i] == b')' {
            depth -= 1;
        } else if depth == 0 && i + op.len() + 2 <= upper.len() {
            let _check = format!(" {} ", &upper[i..i + op.len()]);
            if i > 0
                && bytes[i - 1] == b' '
                && upper[i..i + op.len()] == op.to_uppercase()
                && i + op.len() < expr.len()
                && bytes[i + op.len()] == b' '
            {
                return Some((&expr[..i - 1], &expr[i + op.len() + 1..]));
            }
        }
        i += 1;
    }

    None
}

fn find_matching_paren(expr: &str, start: usize) -> Option<usize> {
    let bytes = expr.as_bytes();
    if bytes.get(start) != Some(&b'(') {
        return None;
    }

    let mut depth = 0;
    for (i, &b) in bytes[start..].iter().enumerate() {
        if b == b'(' {
            depth += 1;
        } else if b == b')' {
            depth -= 1;
            if depth == 0 {
                return Some(start + i);
            }
        }
    }
    None
}

fn extract_function_args<'a>(expr: &'a str, func_name: &str) -> Option<&'a str> {
    let lower = expr.to_lowercase();
    let func_lower = func_name.to_lowercase();

    if let Some(start) = lower.find(&func_lower) {
        let after_name = start + func_name.len();
        let trimmed = expr[after_name..].trim_start();
        if trimmed.starts_with('(') {
            let paren_start = after_name + (expr.len() - after_name - trimmed.len());
            if let Some(end) = find_matching_paren(expr, paren_start) {
                return Some(&expr[paren_start + 1..end]);
            }
        }
    }
    None
}

fn split_function_args(args: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut last_pos = 0;
    let bytes = args.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'(' {
            depth += 1;
        } else if b == b')' {
            depth -= 1;
        } else if b == b',' && depth == 0 {
            parts.push(&args[last_pos..i]);
            last_pos = i + 1;
        }
    }

    parts.push(&args[last_pos..]);
    parts
}

fn split_by_comma(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut last_pos = 0;
    let bytes = s.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'(' {
            depth += 1;
        } else if b == b')' {
            depth -= 1;
        } else if b == b',' && depth == 0 {
            parts.push(&s[last_pos..i]);
            last_pos = i + 1;
        }
    }

    parts.push(&s[last_pos..]);
    parts
}

fn find_operator(expr: &str, op: char) -> Option<usize> {
    let bytes = expr.as_bytes();
    let mut depth = 0;

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'(' {
            depth += 1;
        } else if b == b')' {
            depth -= 1;
        } else if depth == 0 && b == op as u8 {
            return Some(i);
        }
    }
    None
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
        (AttributeValue::BOOL { BOOL: b1 }, AttributeValue::BOOL { BOOL: b2 }) => b1.cmp(b2),
        _ => std::cmp::Ordering::Equal,
    }
}

/// Get a nested attribute from an item using dot notation
fn get_nested_attribute<'a>(item: &'a Item, path: &str) -> Option<&'a AttributeValue> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current: Option<&AttributeValue> = item.get(parts[0]);

    for part in &parts[1..] {
        match current {
            Some(AttributeValue::M { M: map }) => {
                current = map.get(*part);
            }
            _ => return None,
        }
    }

    current
}

/// Set a nested attribute in an item
fn set_nested_attribute(item: &mut Item, path: &str, value: AttributeValue) {
    let parts: Vec<&str> = path.split('.').collect();

    if parts.len() == 1 {
        item.insert(path.to_string(), value);
        return;
    }

    // Handle nested paths
    let first = parts[0];
    if !item.contains_key(first) {
        item.insert(first.to_string(), AttributeValue::M { M: HashMap::new() });
    }

    let mut current = item.get_mut(first);
    for part in parts[1..parts.len() - 1].iter() {
        match current {
            Some(AttributeValue::M { M: map }) => {
                if !map.contains_key(*part) {
                    map.insert(part.to_string(), AttributeValue::M { M: HashMap::new() });
                }
                current = map.get_mut(*part);
            }
            _ => return,
        }
    }

    if let Some(AttributeValue::M { M: map }) = current {
        map.insert(parts.last().unwrap().to_string(), value);
    }
}

/// Remove a nested attribute from an item
fn remove_nested_attribute(item: &mut Item, path: &str) {
    let parts: Vec<&str> = path.split('.').collect();

    if parts.len() == 1 {
        item.remove(path);
        return;
    }

    // Navigate to parent and remove
    let mut current = item.get_mut(parts[0]);
    for part in &parts[1..parts.len() - 1] {
        match current {
            Some(AttributeValue::M { M: map }) => {
                current = map.get_mut(*part);
            }
            _ => return,
        }
    }

    if let Some(AttributeValue::M { M: map }) = current {
        map.remove(*parts.last().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_key_condition() {
        let conditions = parse_key_condition("pk = :pk").unwrap();
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].attribute, "pk");
        assert_eq!(conditions[0].op, ComparisonOp::Eq);
    }

    #[test]
    fn test_parse_compound_key_condition() {
        let conditions = parse_key_condition("pk = :pk AND sk > :sk").unwrap();
        assert_eq!(conditions.len(), 2);
        assert_eq!(conditions[0].attribute, "pk");
        assert_eq!(conditions[1].attribute, "sk");
        assert_eq!(conditions[1].op, ComparisonOp::Gt);
    }

    #[test]
    fn test_parse_between_condition() {
        let conditions = parse_key_condition("pk = :pk AND sk BETWEEN :start AND :end").unwrap();
        assert_eq!(conditions.len(), 2);
        assert_eq!(conditions[1].op, ComparisonOp::Between);
        assert_eq!(conditions[1].values.len(), 2);
    }

    #[test]
    fn test_parse_filter_condition() {
        let condition = parse_condition("status = :status AND age > :age").unwrap();
        assert!(condition.is_some());
    }

    #[test]
    fn test_parse_attribute_exists() {
        let condition = parse_condition("attribute_exists(email)").unwrap();
        assert!(matches!(condition, Some(ConditionOp::AttributeExists(_))));
    }

    #[test]
    fn test_parse_update_expression() {
        let update =
            parse_update_expression("SET #name = :name, age = :age REMOVE old_field").unwrap();
        assert_eq!(update.set_actions.len(), 2);
        assert_eq!(update.remove_actions.len(), 1);
    }

    #[test]
    fn test_parse_update_with_if_not_exists() {
        let update =
            parse_update_expression("SET counter = if_not_exists(counter, :zero) + :inc").unwrap();
        assert_eq!(update.set_actions.len(), 1);
    }

    #[test]
    fn test_evaluate_simple_condition() {
        let mut item = Item::new();
        item.insert("name".to_string(), AttributeValue::string("Alice"));
        item.insert("age".to_string(), AttributeValue::number("30"));

        let mut values = HashMap::new();
        values.insert(":name".to_string(), AttributeValue::string("Alice"));

        let ctx = ExpressionContext::new(None, Some(&values));

        let condition = parse_condition("name = :name").unwrap().unwrap();
        assert!(evaluate_condition(&condition, &item, &ctx).unwrap());
    }

    #[test]
    fn test_apply_set_update() {
        let mut item = Item::new();
        item.insert("pk".to_string(), AttributeValue::string("key1"));
        item.insert("name".to_string(), AttributeValue::string("Alice"));

        let mut values = HashMap::new();
        values.insert(":newname".to_string(), AttributeValue::string("Bob"));

        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("SET name = :newname").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert_eq!(item.get("name").unwrap().as_string().unwrap(), "Bob");
    }

    #[test]
    fn test_apply_remove_update() {
        let mut item = Item::new();
        item.insert("pk".to_string(), AttributeValue::string("key1"));
        item.insert("name".to_string(), AttributeValue::string("Alice"));
        item.insert("temp".to_string(), AttributeValue::string("to_remove"));

        let ctx = ExpressionContext::new(None, None);

        let update = parse_update_expression("REMOVE temp").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert!(item.get("temp").is_none());
    }

    #[test]
    fn test_numeric_increment() {
        let mut item = Item::new();
        item.insert("pk".to_string(), AttributeValue::string("key1"));
        item.insert("count".to_string(), AttributeValue::number("5"));

        let mut values = HashMap::new();
        values.insert(":inc".to_string(), AttributeValue::number("1"));

        let ctx = ExpressionContext::new(None, Some(&values));

        let update = parse_update_expression("SET count = count + :inc").unwrap();
        apply_update(&mut item, &update, &ctx).unwrap();

        assert_eq!(item.get("count").unwrap().as_number().unwrap(), "6");
    }
}
