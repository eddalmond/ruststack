//! CloudFormation Template Parser
//!
//! Provides parsing and resource resolution for AWS CloudFormation templates.
//! Supports YAML and JSON formats, and resolves resource dependencies.

pub mod handlers;
pub mod storage;

pub use handlers::handle_request;
pub use storage::{CloudFormationError, CloudFormationState};

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A CloudFormation template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    #[serde(default)]
    pub awstemplate_format_version: Option<String>,
    #[serde(default, rename = "Description")]
    pub description: Option<String>,
    #[serde(default, rename = "Parameters")]
    pub parameters: HashMap<String, Parameter>,
    #[serde(default, rename = "Mappings")]
    pub mappings: HashMap<String, serde_json::Value>,
    #[serde(default, rename = "Resources")]
    pub resources: HashMap<String, Resource>,
    #[serde(default, rename = "Outputs")]
    pub outputs: HashMap<String, Output>,
}

/// A CloudFormation parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub allowed_values: Option<Vec<String>>,
}

/// A CloudFormation resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    #[serde(rename = "Type")]
    pub resource_type: String,
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
    #[serde(default, rename = "DependsOn")]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// A CloudFormation output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub value: serde_json::Value,
    #[serde(default)]
    pub export: Option<Export>,
    #[serde(default)]
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    pub name: serde_json::Value,
}

/// CloudFormation parser errors
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Failed to parse YAML: {0}")]
    YamlError(#[from] serde_yaml::Error),
    #[error("Failed to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Template is missing required section: {0}")]
    MissingSection(String),
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),
}

/// Parse a CloudFormation template from YAML
pub fn parse_yaml(yaml: &str) -> Result<Template, ParseError> {
    let template: Template = serde_yaml::from_str(yaml)?;
    Ok(template)
}

/// Parse a CloudFormation template from JSON
pub fn parse_json(json: &str) -> Result<Template, ParseError> {
    let template: Template = serde_json::from_str(json)?;
    Ok(template)
}

/// Extract resource dependencies from a template
pub fn get_dependencies(template: &Template) -> HashMap<String, Vec<String>> {
    let mut deps = HashMap::new();

    for (name, resource) in &template.resources {
        let mut resource_deps = Vec::new();

        // Explicit depends_on
        resource_deps.extend(resource.depends_on.clone());

        // Implicit dependencies from Ref and Fn::GetAtt
        for prop_value in resource.properties.values() {
            find_ref_dependencies(prop_value, &mut resource_deps);
        }

        deps.insert(name.clone(), resource_deps);
    }

    deps
}

/// Find Ref and Fn::GetAtt dependencies in a property value
fn find_ref_dependencies(value: &serde_json::Value, deps: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            // Check for { "Ref": "ResourceName" } format
            if let Ok(obj) = serde_json::from_str::<HashMap<String, String>>(s) {
                for (key, val) in obj {
                    if key == "Ref" || key == "Fn::GetAtt" {
                        deps.push(val.clone());
                    }
                }
            }
        }
        serde_json::Value::Object(obj) => {
            for (key, val) in obj {
                if key == "Ref" || key == "Fn::GetAtt" {
                    if let Some(name) = val.as_str() {
                        deps.push(name.to_string());
                    } else if let Some(arr) = val.as_array() {
                        // Fn::GetAtt can be ["ResourceName", "Attribute"]
                        if !arr.is_empty() {
                            if let Some(name) = arr[0].as_str() {
                                deps.push(name.to_string());
                            }
                        }
                    }
                } else {
                    find_ref_dependencies(val, deps);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for val in arr {
                find_ref_dependencies(val, deps);
            }
        }
        _ => {}
    }
}

/// Resolve resources in dependency order using topological sort
pub fn resolve_order(template: &Template) -> Result<Vec<String>, ParseError> {
    let deps = get_dependencies(template);
    let mut resolved = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();

    fn visit(
        name: &str,
        template: &Template,
        deps: &HashMap<String, Vec<String>>,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
        resolved: &mut Vec<String>,
    ) -> Result<(), ParseError> {
        if visited.contains(name) {
            return Ok(());
        }

        if visiting.contains(name) {
            return Err(ParseError::CircularDependency(name.to_string()));
        }

        visiting.insert(name.to_string());

        if let Some(resource_deps) = deps.get(name) {
            for dep in resource_deps {
                if template.resources.contains_key(dep) {
                    visit(dep, template, deps, visiting, visited, resolved)?;
                }
            }
        }

        visiting.remove(name);
        visited.insert(name.to_string());
        resolved.push(name.to_string());

        Ok(())
    }

    for name in template.resources.keys() {
        visit(
            name,
            template,
            &deps,
            &mut visiting,
            &mut visited,
            &mut resolved,
        )?;
    }

    Ok(resolved)
}

/// Extract Fn::GetAtt references from a property
pub fn getatt_references(value: &serde_json::Value) -> Vec<(String, String)> {
    let mut refs = Vec::new();
    extract_getatt(value, &mut refs);
    refs
}

fn extract_getatt(value: &serde_json::Value, refs: &mut Vec<(String, String)>) {
    match value {
        serde_json::Value::Object(obj) => {
            if let Some(arr) = obj.get("Fn::GetAtt") {
                if let Some(arr) = arr.as_array() {
                    if arr.len() >= 2 {
                        if let (Some(logical_id), Some(attr)) = (arr[0].as_str(), arr[1].as_str()) {
                            refs.push((logical_id.to_string(), attr.to_string()));
                        }
                    }
                }
            }
            for val in obj.values() {
                extract_getatt(val, refs);
            }
        }
        serde_json::Value::Array(arr) => {
            for val in arr {
                extract_getatt(val, refs);
            }
        }
        _ => {}
    }
}

/// Extract Ref references from a property
pub fn ref_references(value: &serde_json::Value) -> Vec<String> {
    let mut refs = Vec::new();
    extract_ref(value, &mut refs);
    refs
}

fn extract_ref(value: &serde_json::Value, refs: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(obj) => {
            if let Some(name) = obj.get("Ref") {
                if let Some(s) = name.as_str() {
                    refs.push(s.to_string());
                }
            }
            for val in obj.values() {
                extract_ref(val, refs);
            }
        }
        serde_json::Value::Array(arr) => {
            for val in arr {
                extract_ref(val, refs);
            }
        }
        _ => {}
    }
}

/// Get a logical resource ID from a Ref or Fn::GetAtt reference
pub fn resolve_reference(template: &Template, ref_value: &serde_json::Value) -> Option<String> {
    // Handle { "Ref": "LogicalId" }
    if let Some(obj) = ref_value.as_object() {
        if let Some(name) = obj.get("Ref").or(obj.get("Fn::GetAtt")) {
            if let Some(logical_id) = name.as_str() {
                // Return the logical ID - actual value resolution depends on resource type
                return Some(logical_id.to_string());
            }
        }
    }

    // Handle direct string for Ref (legacy format)
    if let Some(s) = ref_value.as_str() {
        if template.parameters.contains_key(s) {
            return Some(format!("parameter:{}", s));
        }
        if template.resources.contains_key(s) {
            return Some(format!("resource:{}", s));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
AWSTemplateFormatVersion: '2010-09-09'
Description: Test template
Resources:
  MyBucket:
    Type: AWS::S3::Bucket
    Properties:
      BucketName: test-bucket
Outputs:
  BucketArn:
    Value:
      Fn::GetAtt: [MyBucket, Arn]
"#;
        let template = parse_yaml(yaml).unwrap();
        assert!(template.description.is_some());
        assert!(template.resources.contains_key("MyBucket"));
    }

    #[test]
    fn test_parse_json() {
        let json = r#"{
  "AWSTemplateFormatVersion": "2010-09-09",
  "Resources": {
    "MyBucket": {
      "Type": "AWS::S3::Bucket"
    }
  }
}"#;
        let template = parse_json(json).unwrap();
        assert!(template.resources.contains_key("MyBucket"));
    }

    #[test]
    fn test_resource_parsing() {
        let json = r#"{
  "Resources": {
    "Bucket": {
      "Type": "AWS::S3::Bucket"
    },
    "Policy": {
      "Type": "AWS::S3::BucketPolicy"
    }
  }
}"#;
        let template = parse_json(json).unwrap();

        assert!(template.resources.contains_key("Bucket"));
        assert!(template.resources.contains_key("Policy"));
    }

    #[test]
    fn test_getatt_references() {
        let json = serde_json::json!({
            "Value": {
                "Fn::GetAtt": ["MyBucket", "Arn"]
            }
        });
        let refs = getatt_references(&json);
        assert_eq!(refs, vec![("MyBucket".to_string(), "Arn".to_string())]);
    }

    #[test]
    fn test_ref_references() {
        let json = serde_json::json!({
            "Bucket": {"Ref": "MyBucket"}
        });
        let refs = ref_references(&json);
        assert_eq!(refs, vec!["MyBucket".to_string()]);
    }

    #[test]
    fn test_template_with_mappings() {
        let json = r#"{
  "AWSTemplateFormatVersion": "2010-09-09",
  "Mappings": {
    "RegionMap": {
      "us-east-1": {"AMI": "ami-12345678"},
      "us-west-2": {"AMI": "ami-87654321"}
    }
  },
  "Resources": {
    "MyInstance": {
      "Type": "AWS::EC2::Instance"
    }
  }
}"#;
        let template = parse_json(json).unwrap();
        assert!(template.mappings.contains_key("RegionMap"));
        assert!(template.resources.contains_key("MyInstance"));
    }

    #[test]
    fn test_template_with_parameters() {
        let json = r#"{
  "AWSTemplateFormatVersion": "2010-09-09",
  "Parameters": {
    "InstanceType": {
      "Type": "String",
      "Default": "t2.micro",
      "Description": "EC2 instance type"
    }
  },
  "Resources": {}
}"#;
        let template = parse_json(json).unwrap();
        assert!(template.parameters.contains_key("InstanceType"));
    }

    #[test]
    fn test_template_with_outputs() {
        let json = r#"{
  "AWSTemplateFormatVersion": "2010-09-09",
  "Outputs": {
    "BucketName": {
      "Description": "S3 Bucket Name",
      "Value": "my-bucket"
    }
  },
  "Resources": {}
}"#;
        let template = parse_json(json).unwrap();
        assert!(template.outputs.contains_key("BucketName"));
    }

    #[test]
    fn test_circular_dependency_detection() {
        let json = r#"{
  "Resources": {
    "ResourceA": {
      "Type": "AWS::S3::Bucket",
      "DependsOn": ["ResourceB"]
    },
    "ResourceB": {
      "Type": "AWS::S3::Bucket",
      "DependsOn": ["ResourceA"]
    }
  }
}"#;
        let template = parse_json(json).unwrap();
        let deps = get_dependencies(&template);

        let a_deps = deps.get("ResourceA").unwrap();
        let b_deps = deps.get("ResourceB").unwrap();

        assert!(a_deps.contains(&"ResourceB".to_string()));
        assert!(b_deps.contains(&"ResourceA".to_string()));
    }

    #[test]
    fn test_dependencies_extraction() {
        let json = r#"{
  "Resources": {
    "ResourceA": {
      "Type": "AWS::S3::Bucket"
    },
    "ResourceB": {
      "Type": "AWS::S3::Bucket",
      "DependsOn": ["ResourceA"]
    },
    "ResourceC": {
      "Type": "AWS::S3::Bucket",
      "DependsOn": ["ResourceA", "ResourceB"]
    }
  }
}"#;
        let template = parse_json(json).unwrap();
        let deps = get_dependencies(&template);

        assert!(deps.get("ResourceA").unwrap().is_empty());
        assert!(deps
            .get("ResourceB")
            .unwrap()
            .contains(&"ResourceA".to_string()));

        let c_deps = deps.get("ResourceC").unwrap();
        assert!(c_deps.contains(&"ResourceA".to_string()));
        assert!(c_deps.contains(&"ResourceB".to_string()));
    }

    #[test]
    fn test_resolve_reference() {
        let json = r#"{
  "Parameters": {
    "BucketName": {
      "Type": "String"
    }
  },
  "Resources": {
    "Bucket": {
      "Type": "AWS::S3::Bucket"
    }
  }
}"#;
        let template = parse_json(json).unwrap();

        let ref_json = serde_json::json!({"Ref": "BucketName"});
        let result = resolve_reference(&template, &ref_json);
        assert!(result.is_some());

        let ref_bucket = serde_json::json!({"Ref": "Bucket"});
        let result2 = resolve_reference(&template, &ref_bucket);
        assert!(result2.is_some());
    }
}
