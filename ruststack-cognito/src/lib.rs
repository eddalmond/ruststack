//! AWS Cognito emulation for RustStack
//!
//! Provides local Cognito User Pools with JWT token generation:
//! - CreateUserPool, ListUserPools
//! - CreateUser, AdminGetUser, AdminDeleteUser
//! - InitiateAuth, AdminCreateUser
//! - JWT token generation (ID token, Access token)

pub mod handlers;
mod jwt;
mod storage;

pub use handlers::handle_request;
pub use storage::CognitoState;
