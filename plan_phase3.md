# Phase 3: Advanced Orchestration & Shift-Left Security

**Objective:** Implement CloudFormation parsing, Step Functions, and deterministic IAM evaluation.

**Timeline:** Months 7-12

---

## Task 3.1: CloudFormation Parsing (CDK Support) ✅ COMPLETED

### Overview
Implement CloudFormation template parsing to enable `cdklocal` and `cloudformation` CLI compatibility.

### Completed Steps

1. ✅ Created new CloudFormation crate: `ruststack-cloudformation/`
2. ✅ Added to workspace in `Cargo.toml`
3. ✅ Created `ruststack-cloudformation/Cargo.toml` with serde_yaml, serde_json, thiserror, tracing, anyhow, regex
4. ✅ Added serde_yaml to workspace dependencies
5. ✅ Implemented CloudFormation handlers with:
   - Template struct for YAML/JSON parsing
   - Resource struct with Type, Properties, DependsOn
   - Parameter and Output structs
   - parse_yaml() and parse_json() functions
6. ✅ Implemented dependency resolution with topological sort in resolve_order()
7. ✅ Added utility functions:
   - get_dependencies() - Extract resource dependencies
   - find_ref_dependencies() - Find Ref and Fn::GetAtt references
   - getatt_references() - Extract Fn::GetAtt references
   - ref_references() - Extract Ref references
   - resolve_reference() - Resolve references to logical IDs
8. ✅ Added comprehensive tests

### Next Steps for Full Integration
- Implement stack operations (CreateStack, DescribeStacks, DeleteStack)
- Implement resource instantiation (deploy resources to S3, DynamoDB, etc.)
- Add CloudFormation endpoints to router

1. **Create new CloudFormation crate:**
   ```bash
   mkdir -p ruststack-cloudformation/src
   ```

2. **Add to workspace in `Cargo.toml`:**
   ```toml
   "ruststack-cloudformation",
   ```

3. **Create `ruststack-cloudformation/Cargo.toml`:**
   ```toml
   [package]
   name = "ruststack-cloudformation"
   version.workspace = true
   edition.workspace = true
   
   [dependencies]
   axum.workspace = true
   tokio.workspace = true
   serde.workspace = true
   serde_json.workspace = true
   thiserror.workspace = true
   parking_lot.workspace = true
   serde_yaml.workspace = true
   
   [lib]
   ```

4. **Add YAML support to workspace:**
   ```toml
   serde_yaml = "0.9"
   ```

5. **Implement CloudFormation handlers:**

   Create `ruststack-cloudformation/src/handlers.rs`:
   ```rust
   use serde::{Deserialize, Serialize};
   use std::collections::HashMap;
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct CloudFormationState {
       pub stacks: parking_lot::RwLock<HashMap<String, Stack>>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Stack {
       pub name: String,
       pub template: Template,
       pub resources: Vec<StackResource>,
       pub outputs: HashMap<String, String>,
       pub creation_time: chrono::DateTime<chrono::Utc>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Template {
       #[serde(flatten)]
       pub resources: HashMap<String, Resource>,
       #[serde(default)]
       pub outputs: HashMap<String, Output>,
       #[serde(default)]
       pub parameters: HashMap<String, Parameter>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Resource {
       #[serde(rename = "Type")]
       pub resource_type: String,
       #[serde(rename = "Properties")]
       pub properties: serde_json::Value,
       #[serde(default)]
       pub depends_on: Vec<String>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Output {
       #[serde(rename = "Description")]
       pub description: Option<String>,
       #[serde(rename = "Value")]
       pub value: serde_json::Value,
       #[serde(default, rename = "Export")]
       pub export: Option<Export>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Export {
       #[serde(rename = "Name")]
       pub name: serde_json::Value,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Parameter {
       #[serde(rename = "Type")]
       pub param_type: String,
       #[serde(default)]
       pub default: Option<String>,
   }
   ```

6. **Implement dependency resolution:**
   ```rust
   impl Template {
       /// Resolve resource creation order based on DependsOn
       pub fn resolve_creation_order(&self) -> Vec<String> {
           let mut resolved = Vec::new();
           let mut pending: HashSet<String> = self.resources.keys().cloned().collect();
           let mut resolved_set: HashSet<String> = HashSet::new();
           
           while !pending.is_empty() {
               for resource_name in pending.iter().cloned().collect::<Vec<_>>() {
                   let resource = &self.resources[&resource_name];
                   let dependencies_met = resource.depends_on.iter()
                       .all(|dep| resolved_set.contains(dep));
                   
                   if dependencies_met {
                       resolved.push(resource_name.clone());
                       resolved_set.insert(resource_name.clone());
                       pending.remove(&resource_name);
                   }
               }
           }
           
           resolved
       }
   }
   ```

7. **Implement stack operations:**
   ```rust
   pub async fn create_stack(
       State(state): State<Arc<CloudFormationState>>,
       Json(req): Json<CreateStackInput>,
   ) -> Response;
   
   pub async fn describe_stacks(
       State(state): State<Arc<CloudFormationState>>,
   ) -> Response;
   
   pub async fn delete_stack(
       State(state): State<Arc<CloudFormationState>>,
       Json(req): Json<DeleteStackInput>,
   ) -> Response;
   
   pub async fn validate_template(
       State(state): State<Arc<CloudFormationState>>,
       Json(req): Json<ValidateTemplateInput>,
   ) -> Response;
   ```

8. **Implement resource instantiation:**
   ```rust
   impl Stack {
       pub async fn deploy(&self) -> Result<HashMap<String, String>, CloudFormationError> {
           let creation_order = self.template.resolve_creation_order();
           let mut outputs = HashMap::new();
           
           for resource_name in creation_order {
               let resource = &self.template.resources[&resource_name];
               
               match resource.resource_type.as_str() {
                   "AWS::S3::Bucket" => {
                       // Create S3 bucket
                       let bucket_name = extract_bucket_name(&resource.properties);
                       create_s3_bucket(&bucket_name).await?;
                       outputs.insert(resource_name, bucket_name);
                   }
                   "AWS::DynamoDB::Table" => {
                       // Create DynamoDB table
                       let table_name = extract_table_name(&resource.properties);
                       create_dynamodb_table(&table_name, &resource.properties).await?;
                       outputs.insert(resource_name, table_name);
                   }
                   // ... handle other resource types
                   _ => {
                       tracing::warn!("Unsupported resource type: {}", resource.resource_type);
                   }
               }
           }
           
           Ok(outputs)
       }
   }
   ```

9. **Add CloudFormation endpoints:**
   - `POST /` - CreateStack
   - `GET /` - ListStacks  
   - `GET /{stackName}` - DescribeStacks
   - `DELETE /{stackName}` - DeleteStack
   - `POST /validateTemplate` - ValidateTemplate

10. **Test with CDK:**
    ```bash
    # Create CDK app that creates S3 bucket
    cdklocal synth
    cdklocal deploy
    
    # Or use cloudformation CLI directly
    aws cloudformation create-stack \
      --stack-name test-stack \
      --template-body file://template.yaml \
      --endpoint-url http://localhost:4566
    ```

---

## Task 3.2: AWS Step Functions (Offline ASL) ✅ COMPLETED

### Overview
Implement Amazon States Language (ASL) parser and state machine execution engine.

### Completed Steps

1. ✅ Created new StepFunctions crate: `ruststack-stepfunctions/`
2. ✅ Added to workspace in `Cargo.toml`
3. ✅ Implemented ASL parser with:
   - StateMachine struct
   - State enum (Pass, Task, Choice, Wait, Succeed, Fail, Parallel, Map)
   - ChoiceRule struct with all comparison operators
   - Retry and Catcher structs
   - parse_state_machine() function
4. ✅ Implemented utility functions:
   - get_next_state() - Get next state name
   - evaluate_choice() - Evaluate choice rules
   - apply_result_path() - Apply ResultPath
   - extract_path() - Extract values using JSON paths
5. ✅ Added storage layer (StepFunctionsState)
6. ✅ Implemented handlers:
   - CreateStateMachine
   - DescribeStateMachine
   - DeleteStateMachine
   - ListStateMachines
   - StartExecution
   - DescribeExecution
   - ListExecutions
   - StopExecution
7. ✅ Integrated into main router
8. ✅ Added comprehensive tests

1. **Create new StepFunctions crate:**
   ```bash
   mkdir -p ruststack-stepfunctions/src
   ```

2. **Add to workspace:**
   ```toml
   "ruststack-stepfunctions",
   ```

3. **Implement ASL parser:**

   Create `ruststack-stepfunctions/src/asl.rs`:
   ```rust
   use serde::{Deserialize, Serialize};
   use std::collections::HashMap;
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct StateMachine {
       pub start_at: String,
       pub states: HashMap<String, State>,
       #[serde(default)]
       pub comment: Option<String>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(tag = "Type")]
   pub enum State {
       #[serde(rename = "Pass")]
       Pass {
           #[serde(default)]
           result: Option<serde_json::Value>,
           #[serde(default)]
           result_path: Option<String>,
           #[serde(default)]
           output_path: Option<String>,
           #[serde(default)]
           end: bool,
       },
       
       #[serde(rename = "Task")]
       Task {
           resource: String,
           #[serde(default)]
           result_path: Option<String>,
           #[serde(default)]
           output_path: Option<String>,
           #[serde(default)]
           retry: Vec<Retry>,
           #[serde(default)]
           catch: Vec<Catcher>,
           #[serde(default)]
           end: bool,
       },
       
       #[serde(rename = "Choice")]
       Choice {
           choices: Vec<ChoiceRule>,
           #[serde(default)]
           default: Option<String>,
           #[serde(default)]
           end: bool,
       },
       
       #[serde(rename = "Wait")]
       Wait {
           #[serde(default)]
           seconds: Option<u64>,
           #[serde(default)]
           seconds_path: Option<String>,
           #[serde(default)]
           end: bool,
       },
       
       #[serde(rename = "Succeed")]
       Succeed {
           #[serde(default)]
           output: Option<serde_json::Value>,
       },
       
       #[serde(rename = "Fail")]
       Fail {
           error: String,
           cause: String,
       },
       
       #[serde(rename = "Parallel")]
       Parallel {
           branches: Vec<StateMachine>,
           #[serde(default)]
           result_path: Option<String>,
           #[serde(default)]
           end: bool,
       },
       
       #[serde(rename = "Map")]
       Map {
           iterator: Box<StateMachine>,
           #[serde(default)]
           items_path: Option<String>,
           #[serde(default)]
           result_path: Option<String>,
           #[serde(default)]
           end: bool,
       },
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ChoiceRule {
       #[serde(rename = "Variable")]
       pub variable: String,
       #[serde(rename = "StringEquals")]
       pub string_equals: Option<String>,
       #[serde(rename = "NumericEquals")]
       pub numeric_equals: Option<f64>,
       #[serde(rename = "BooleanEquals")]
       pub boolean_equals: Option<bool>,
       #[serde(rename = "Next")]
       pub next: String,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Retry {
       #[serde(rename = "ErrorEquals")]
       pub error_equals: Vec<String>,
       #[serde(default)]
       pub interval_seconds: Option<u64>,
       #[serde(default)]
       pub max_attempts: Option<u64>,
       #[serde(default)]
       pub backoff_rate: Option<f64>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Catcher {
       #[serde(rename = "ErrorEquals")]
       pub error_equals: Vec<String>,
       #[serde(rename = "Next")]
       pub next: String,
       #[serde(default)]
       pub result_path: Option<String>,
   }
   ```

4. **Implement state machine executor:**

   Create `ruststack-stepfunctions/src/executor.rs`:
   ```rust
   pub struct ExecutionEngine {
       lambda_client: LambdaClient,
   }
   
   #[derive(Debug, Clone)]
   pub struct ExecutionContext {
       pub input: serde_json::Value,
       pub state_name: String,
       pub variables: HashMap<String, serde_json::Value>,
   }
   
   impl ExecutionEngine {
       pub async fn execute(
           &self,
           state_machine: &StateMachine,
           input: serde_json::Value,
       ) -> Result<serde_json::Value, ExecutionError> {
           let mut ctx = ExecutionContext {
               input,
               state_name: state_machine.start_at.clone(),
               variables: HashMap::new(),
           };
           
           loop {
               let state = state_machine.states.get(&ctx.state_name)
                   .ok_or(ExecutionError::InvalidState)?;
               
               match state {
                   State::Pass { result, result_path, output_path, end } => {
                       if let Some(r) = result {
                           apply_result_path(&mut ctx, r, result_path);
                       }
                       if let Some(output) = output_path {
                           ctx.input = extract_path(&ctx.input, output)?;
                       }
                       if *end {
                           return Ok(ctx.input);
                       }
                       ctx.state_name = get_next_state(state_machine, &ctx.state_name);
                   }
                   
                   State::Task { resource, result_path, end, .. } => {
                       let result = self.invoke_lambda(resource, &ctx.input).await?;
                       apply_result_path(&mut ctx, result, result_path);
                       if *end {
                           return Ok(ctx.input);
                       }
                       ctx.state_name = get_next_state(state_machine, &ctx.state_name);
                   }
                   
                   State::Choice { choices, default, .. } => {
                       ctx.state_name = evaluate_choice(&choices, &ctx.input)
                           .or(default.clone())
                           .ok_or(ExecutionError::NoMatchingChoice)?;
                   }
                   
                   // ... handle other states
                   
                   State::Succeed { output } => {
                       return Ok(output.clone().unwrap_or(ctx.input));
                   }
                   
                   State::Fail { error, cause } => {
                       return Err(ExecutionError::TaskFailed(error.clone(), cause.clone()));
                   }
                   
                   _ => unimplemented!(),
               }
           }
       }
   }
   ```

5. **Add Step Functions endpoints:**
   - `POST /` - CreateStateMachine
   - `GET /{stateMachineArn}` - DescribeStateMachine
   - `GET /{stateMachineArn}/executions` - ListExecutions
   - `POST /{stateMachineArn}/executions` - StartExecution
   - `GET /{executionArn}` - DescribeExecution
   - `GET /{executionArn}/history` - GetExecutionHistory

6. **Test Step Functions:**
   ```bash
   # Create state machine
   aws stepfunctions create-state-machine \
     --name "my-state-machine" \
     --definition file://statemachine.json \
     --role-arn "arn:aws:iam::123456789012:role/stepfunctions-role" \
     --endpoint-url http://localhost:4566
   
   # Start execution
   aws stepfunctions start-execution \
     --state-machine-arn "arn:aws:states:us-east-1:123456789012:stateMachine:my-state-machine" \
     --input '{"key":"value"}' \
     --endpoint-url http://localhost:4566
   ```

---

## Task 3.3: Shift-Left Security (Explainable IAM)

### Overview
Implement deterministic IAM policy evaluation for local access control.

### Steps for LLM Agent

1. **Review existing IAM implementation:**
   ```bash
   cat ruststack-iam/src/handlers.rs
   cat ruststack-iam/src/storage.rs
   ```

2. **Add IAM enforcement environment variable:**
   
   In `ruststack/src/main.rs`:
   ```rust
   #[derive(Parser, Debug)]
   struct Args {
       // ... existing args
       
       /// Enable IAM enforcement
       #[arg(long, default_value = "false", env = "RUSTSTACK_ENFORCE_IAM")]
       enforce_iam: bool,
   }
   ```

3. **Create policy evaluation engine:**
   
   Create `ruststack-iam/src/policy.rs`:
   ```rust
   use serde::{Deserialize, Serialize};
   use std::collections::HashMap;
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Policy {
       #[serde(rename = "Version")]
       pub version: Option<String>,
       #[serde(rename = "Statement")]
       pub statements: Vec<Statement>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Statement {
       #[serde(rename = "Sid")]
       pub sid: Option<String>,
       #[serde(rename = "Effect")]
       pub effect: Effect,
       #[serde(rename = "Principal")]
       pub principal: Option<Principal>,
       #[serde(rename = "Action")]
       pub action: Vec<String>,
       #[serde(rename = "Resource")]
       pub resource: Vec<String>,
       #[serde(rename = "Condition")]
       pub condition: Option<Condition>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
   pub enum Effect {
       #[serde(rename = "Allow")]
       Allow,
       #[serde(rename = "Deny")]
       Deny,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(untagged)]
   pub enum Principal {
       All,
       AWS(String),
       Service(String),
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Condition {
       #[serde(flatten)]
       pub conditions: HashMap<String, HashMap<String, Vec<String>>>,
   }
   
   #[derive(Debug, Clone, PartialEq)]
   pub enum Decision {
       Allow,
       Deny,
       ImplicitDeny,
   }
   
   pub fn evaluate_policy(
       policy: &Policy,
       action: &str,
       resource: &str,
       context: &HashMap<String, String>,
   ) -> Decision {
       let mut has_explicit_deny = false;
       let mut has_explicit_allow = false;
       
       for statement in &policy.statements {
           // Check if statement applies
           if !matches_action(&statement.action, action) {
               continue;
           }
           if !matches_resource(&statement.resource, resource) {
               continue;
           }
           if let Some(ref cond) = statement.condition {
               if !evaluate_condition(cond, context) {
                   continue;
               }
           }
           
           match statement.effect {
               Effect::Deny => has_explicit_deny = true,
               Effect::Allow => has_explicit_allow = true,
           }
       }
       
       if has_explicit_deny {
           Decision::Deny
       } else if has_explicit_allow {
           Decision::Allow
       } else {
           Decision::ImplicitDeny
       }
   }
   
   fn matches_action(actions: &[String], requested: &str) -> bool {
       for action in actions {
           if action == "*" {
               return true;
           }
           // Support IAM-style wildcards: "s3:Get*"
           if action.ends_with('*') {
               let prefix = &action[..action.len() - 1];
               if requested.starts_with(prefix) {
                   return true;
               }
           }
           if action == requested {
               return true;
           }
       }
       false
   }
   
   fn matches_resource(resources: &[String], requested: &str) -> bool {
       for resource in resources {
           if resource == "*" {
               return true;
           }
           // Support ARN patterns
           if resource.contains('*') || resource.contains('?') {
               if matches_glob(resource, requested) {
                   return true;
               }
           }
           if resource == requested {
               return true;
           }
       }
       false
   }
   ```

4. **Add IAM enforcement middleware:**

   Create `ruststack-iam/src/middleware.rs`:
   ```rust
   use axum::{
       body::Body,
       extract::Request,
       http::{header, StatusCode},
       middleware::Next,
       response::Response,
   };
   
   pub async fn enforce_iam(
       request: Request<Body>,
       next: Next,
   ) -> Response {
       // Only enforce if IAM enforcement is enabled
       if !is_iam_enforced() {
           return next.run(request).await;
       }
       
       // Extract credentials from request (SigV4 headers or query string)
       let credentials = extract_credentials(&request);
       
       // Get requested action and resource
       let (action, resource) = extract_action_and_resource(&request);
       
       // Evaluate policies
       let decision = evaluate_policies(&credentials, &action, &resource);
       
       match decision {
           Decision::Allow => next.run(request).await,
           Decision::Deny | Decision::ImplicitDeny => {
               Response::builder()
                   .status(StatusCode::FORBIDDEN)
                   .header(header::CONTENT_TYPE, "application/json")
                   .body(Body::from(
                       r#"{"__type":"AccessDeniedException","message":"Access denied"}"#
                   ))
                   .unwrap()
           }
       }
   }
   ```

5. **Apply IAM middleware to services:**

   In `ruststack/src/router.rs`:
   ```rust
   use ruststack_iam::middleware::enforce_iam;
   
   pub fn create_router(state: AppState) -> Router {
       // ... existing setup
       
       Router::new()
           .layer(middleware::from_fn(enforce_iam))
           // ... routes
   }
   ```

6. **Test IAM enforcement:**
   ```bash
   # Enable IAM enforcement
   RUSTSTACK_ENFORCE_IAM=true cargo run &
   
   # Try to access S3 without proper IAM
   aws s3 ls s3://test-bucket --endpoint-url http://localhost:4566
   # Should return: AccessDeniedException
   
   # Create role with S3 access
   aws iam create-role --role-name test-role \
     --assume-role-policy-document '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}' \
     --endpoint-url http://localhost:4566
   
   # Attach policy allowing S3 access
   aws iam put-role-policy --role-name test-role \
     --policy-name s3-access \
     --policy-document '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["s3:*"],"Resource":["*"]}]}' \
     --endpoint-url http://localhost:4566
   
   # Now S3 access should work (with assumed role credentials)
   ```

7. **Acceptance Test:**
   - `ENFORCE_IAM=true` blocks unauthorized access
   - Policies with Allow effect permit access
   - Policies with Deny effect block access
   - Explicit deny overrides allow
   - Wildcard actions work (`s3:Get*`)

---

## Phase 3 Acceptance Criteria Summary

| Criterion | Test | Expected Result |
|-----------|------|-----------------|
| CloudFormation deploy | `cfn create-stack` with S3 | S3 bucket created |
| CloudFormation CDK | `cdklocal deploy` | CDK resources created |
| Step Functions create | Create state machine | Machine created |
| Step Functions execute | Start execution | Execution runs |
| Step Functions choices | Choice state | Branch taken correctly |
| IAM enforcement enabled | Access without policy | AccessDenied |
| IAM allow | Policy with Allow | Access granted |
| IAM deny | Policy with Deny | Access denied |

---

## Notes for LLM Agent

- **New crates to create:**
  - `ruststack-cloudformation/`
  - `ruststack-stepfunctions/`

- **Key files:**
  - `ruststack-cloudformation/src/handlers.rs` - CloudFormation API
  - `ruststack-cloudformation/src/asl.rs` - Template parsing
  - `ruststack-stepfunctions/src/handlers.rs` - Step Functions API
  - `ruststack-stepfunctions/src/executor.rs` - State machine execution
  - `ruststack-iam/src/policy.rs` - Policy evaluation

- **Dependencies to add:**
  - `serde_yaml`

- **Testing:**
  - Use AWS CLI cloudformation commands
  - Use AWS CLI stepfunctions commands
  - Create test policies and verify enforcement

- **CDK support:**
  - Must handle common CDK resource types
  - Must support CloudFormation intrinsic functions (Ref, Fn::GetAtt, etc.)
