# Phase 2: Compute Emulation & Paywall Breakers

**Objective:** Implement serverless compute, Cognito (user pools), and enhance Lambda execution.

**Timeline:** Months 4-6

---

## Task 2.1: The Paywall Breaker - AWS Secrets Manager ✅ COMPLETED

### Overview
Secrets Manager is already implemented. This task focuses on adding persistence and enhancing integration with other services.

### Current State
- ✓ CreateSecret, GetSecretValue, PutSecretValue, DeleteSecret
- ✓ DescribeSecret, ListSecrets
- ✓ SQLite persistence implemented
- ✗ Rotation not implemented

### Steps for LLM Agent

1. **Review existing implementation:**
   ```bash
   cat ruststack-secretsmanager/src/handlers.rs
   cat ruststack-secretsmanager/src/storage.rs
   ```

2. **Add SQLite persistence to Secrets Manager:**
   
   In `ruststack-secretsmanager/src/storage.rs`:
   ```rust
   use rusqlite::{Connection, params};
   
   pub struct PersistentStorage {
       conn: Connection,
   }
   
   impl PersistentStorage {
       pub fn new(data_dir: &Path) -> anyhow::Result<Self> {
           std::fs::create_dir_all(data_dir)?;
           let conn = Connection::open(data_dir / "secretsmanager.db")?;
           
           conn.execute(
               "CREATE TABLE IF NOT EXISTS secrets (
                   name TEXT PRIMARY KEY,
                   arn TEXT NOT NULL,
                   description TEXT,
                   kms_key_id TEXT,
                   current_version_id TEXT,
                   created_date TEXT NOT NULL,
                   last_changed_date TEXT NOT NULL,
                   deleted_date TEXT
               )",
               [],
           )?;
           
           conn.execute(
               "CREATE TABLE IF NOT EXISTS secret_versions (
                   secret_name TEXT NOT NULL,
                   version_id TEXT NOT NULL,
                   secret_string TEXT,
                   secret_binary TEXT,
                   version_stages TEXT NOT NULL,
                   created_date TEXT NOT NULL,
                   PRIMARY KEY (secret_name, version_id)
               )",
               [],
           )?;
           
           Ok(Self { conn })
       }
   }
   ```

3. **Test persistence:**
   ```bash
   # Create a secret
   aws secretsmanager create-secret \
     --name "my-secret" \
     --secret-string '{"password":"test123"}' \
     --endpoint-url http://localhost:4566
   
   # Restart server
   pkill ruststack && cargo run &
   
   # Verify secret still exists
   aws secretsmanager get-secret-value \
     --secret-id "my-secret" \
     --endpoint-url http://localhost:4566
   ```

4. **Acceptance Test:**
   - Secrets persist across restarts
   - Version history works correctly
   - Tags are stored and retrieved properly

---

## Task 2.2: The Paywall Breaker - Amazon Cognito ✅ COMPLETED

### Overview
Cognito is fully implemented. This is a key "paywall breaker" feature that allows local JWT generation without a paid AWS account.

### Steps for LLM Agent

1. **Create new Cognito crate:**
   ```bash
   mkdir -p ruststack-cognito/src
   ```

2. **Add to workspace in `Cargo.toml`:**
   ```toml
   "ruststack-cognito",
   ```

3. **Create `ruststack-cognito/Cargo.toml`:**
   ```toml
   [package]
   name = "ruststack-cognito"
   version.workspace = true
   edition.workspace = true
   
   [dependencies]
   axum.workspace = true
   tokio.workspace = true
   serde.workspace = true
   serde_json.workspace = true
   uuid.workspace = true
   chrono.workspace = true
   base64.workspace = true
   jsonwebtoken.workspace = true
   thiserror.workspace = true
   parking_lot.workspace = true
   
   [lib]
   ```

4. **Add JWT support to workspace:**
   
   In `Cargo.toml`:
   ```toml
   jsonwebtoken = "9.2"
   ```

5. **Implement User Pools:**

   Create `ruststack-cognito/src/handlers.rs`:
   ```rust
   use serde::{Deserialize, Serialize};
   use jsonwebtoken::{encode, decode, Header, Validation};
   use std::sync::Arc;
   
   pub struct CognitoState {
       pub user_pools: parking_lot::RwLock<HashMap<String, UserPool>>,
   }
   
   #[derive(Clone)]
   pub struct UserPool {
       pub id: String,
       pub name: String,
       pub users: HashMap<String, User>,
       pub client_id: String,
       pub client_secret: Option<String>,
       pub secret_key: String,  // For signing JWTs
   }
   
   #[derive(Clone)]
   pub struct User {
       pub username: String,
       pub password: String,
       pub email: String,
       pub enabled: bool,
       pub attributes: HashMap<String, String>,
   }
   ```

6. **Implement JWT generation:**
   ```rust
   pub fn generate_id_token(
       user_pool: &UserPool,
       username: &str,
       user: &User,
   ) -> String {
       let mut claims = serde_json::json!({
           "sub": user.username,
           "cognito:username": user.username,
           "email": user.email,
           "email_verified": true,
           "iss": format!("https://cognito-idp.us-east-1.amazonaws.com/{}", user_pool.id),
           "cognito:user_status": "CONFIRMED",
       });
       
       // Add custom attributes
       for (key, value) in &user.attributes {
           claims[key] = serde_json::json!(value);
       }
       
       encode(
           &Header::new(jsonwebtoken::Algorithm::HS256),
           &claims,
           &user_pool.secret_key.as_ref().unwrap().as_bytes(),
       ).unwrap()
   }
   
   pub fn generate_access_token(
       user_pool: &UserPool,
       username: &str,
   ) -> String {
       let claims = serde_json::json!({
           "sub": username,
           "iss": format!("https://cognito-idp.us-east-1.amazonaws.com/{}", user_pool.id),
           "client_id": user_pool.client_id,
           "event_id": uuid::Uuid::new_v4(),
           "token_use": "access",
           "scope": "aws.cognito.signin.user.admin",
       });
       
       encode(
           &Header::new(jsonwebtoken::Algorithm::HS256),
           &claims,
           &user_pool.secret_key.as_ref().unwrap().as_bytes(),
       ).unwrap()
   }
   ```

7. **Implement Cognito endpoints:**

   | Endpoint | Description |
   |----------|-------------|
   | `/` | List user pools |
   | `POST /` | Create user pool |
   | `POST /{poolId}/users` | AdminCreateUser |
   | `POST /{poolId}/tokens` | InitiateAuth (get tokens) |
   | `POST /{poolId}/logout` | Logout |
   | `GET /{poolId}/user/{username}` | AdminGetUser |
   | `POST /{poolId}/user/{username}/enable` | AdminEnableUser |
   | `POST /{poolId}/user/{username}/disable` | AdminDisableUser |
   | `POST /{poolId}/user/{username}/delete` | AdminDeleteUser |

8. **Test Cognito:**
   ```bash
   # Create user pool (via direct API call)
   curl -X POST http://localhost:4566/ \
     -H "Content-Type: application/json" \
     -d '{"PoolName":"test-pool"}'
   
   # Create user
   curl -X POST http://localhost:4566/{poolId}/users \
     -H "Content-Type: application/json" \
     -d '{"Username":"testuser","Password":"Test123!","UserAttributes":[{"Name":"email","Value":"test@example.com"}]}'
   
   # Get tokens
   curl -X POST http://localhost:4566/{poolId}/tokens \
     -H "Content-Type: application/json" \
     -d '{"Username":"testuser","Password":"Test123!"}'
   ```

9. **Acceptance Test:**
   - User pools can be created
   - Users can be created and enabled
   - JWT tokens are valid and can be verified
   - Applications can authenticate against local Cognito

---

## Task 2.3: AWS Lambda & API Gateway ✅ COMPLETED

### Overview
Lambda is already partially implemented. This task focuses on enhancing Lambda execution and API Gateway integration.

### Current State
- Lambda function CRUD ✓
- Subprocess executor ✓
- Docker executor ✓
- ✓ API Gateway integration completed

### Steps for LLM Agent

1. **Review current Lambda implementation:**
   ```bash
   cat ruststack-lambda/src/handlers.rs
   cat ruststack-lambda/src/service.rs
   cat ruststack-apigateway/src/handlers.rs
   ```

2. **Enhance API Gateway → Lambda integration:**

   In `ruststack-apigateway/src/handlers.rs`:
   ```rust
   pub async fn invoke_with_integration(
       api_id: &str,
       route_key: &str,
       body: Bytes,
   ) -> Response {
       // Get API configuration
       let api = get_api(api_id)?;
       
       // Find matching route
       let route = find_route(&api, route_key)?;
       
       // Get integration
       let integration = get_integration(&api, &route.integration_id)?;
       
       // If Lambda integration, invoke Lambda
       if integration.integration_type == "aws_proxy" || integration.integration_type == "lambda" {
           return invoke_lambda(
               integration.uri,  // arn:aws:apigateway:region:lambda:path/...
               body,
           ).await;
       }
       
       // Handle other integration types
   }
   ```

3. **Add Lambda URL support:**
   ```rust
   // Create /{function-name}/urls endpoint
   pub async fn create_function_url(
       State(state): State<Arc<LambdaState>>,
       Path(function_name): Path<String>,
       Json(req): Json<CreateFunctionUrlConfigRequest>,
   ) -> Response;
   
   pub async fn get_function_url_config(...) -> Response;
   
   pub async fn delete_function_url_config(...) -> Response;
   ```

4. **Add Lambda aliases and versions:**
   ```rust
   pub async fn publish_version(...) -> Response;
   pub async fn create_alias(...) -> Response;
   pub async fn get_alias(...) -> Response;
   pub async fn update_alias(...) -> Response;
   ```

5. **Add Lambda layer support:**
   ```rust
   pub async fn list_layers(...) -> Response;
   pub async fn get_layer_version(...) -> Response;
   ```

6. **Enhance Docker executor:**

   In `ruststack-lambda/src/docker.rs`:
   ```rust
   pub async fn execute_docker(
       &self,
       image: &str,
       handler: &str,
       env: &HashMap<String, String>,
       input: Bytes,
   ) -> Result<Bytes, LambdaError> {
       // Build container config with layers
       // Mount /opt with layer contents
       // Set handler and runtime in environment
       // Execute and return output
   }
   ```

7. **Test with SAM Local / CDK:**
   ```bash
   # Create a simple Lambda function
   aws lambda create-function \
     --function-name my-function \
     --runtime python3.9 \
     --handler index.handler \
     --zip-file fileb://function.zip \
     --role arn:aws:iam::123456789012:role/lambda-role \
     --endpoint-url http://localhost:4566
   
   # Invoke directly
   aws lambda invoke \
     --function-name my-function \
     --payload '{"key":"value"}' \
     --endpoint-url http://localhost:4566 \
     output.json
   ```

8. **Acceptance Test:**
   - Lambda functions can be created and invoked
   - API Gateway routes trigger Lambda
   - Lambda URLs work
   - Docker execution works for container images

---

## Phase 2 Acceptance Criteria Summary

| Criterion | Test | Expected Result |
|-----------|------|-----------------|
| Secrets Manager persistence | Create secret, restart, get secret | Secret persists |
| Cognito user pools | Create pool, create user | Pool and user created |
| Cognito JWT tokens | Authenticate, get token | Valid JWT returned |
| JWT verification | Verify JWT locally | Token verified |
| Lambda API Gateway | Create API with Lambda integration | Route triggers Lambda |
| Lambda URL | Create function URL | URL accessible |

---

## Notes for LLM Agent

- **Key files to create/modify:**
  - New crate: `ruststack-cognito/Cargo.toml`
  - New crate: `ruststack-cognito/src/lib.rs`
  - New crate: `ruststack-cognito/src/handlers.rs`
  - Existing: `ruststack-lambda/src/handlers.rs`
  - Existing: `ruststack-apigateway/src/handlers.rs`

- **Dependencies:**
  - jsonwebtoken (already added to workspace)
  - All existing AWS crates

- **Testing:**
  - Use `aws cognito-idp` CLI where possible
  - Use JWT decoder to verify token claims
  - Test with real application code

- **Cognito compatibility:**
  - Tokens should be compatible with standard JWT verification
  - User pool ID format: `us-east-1_XXXXX`
  - Client ID auto-generated
