# Tech Debt

## Overview
Issues identified during development that were temporarily suppressed rather than properly addressed.

## Completed Items

### ruststack-cognito

| Item | Status |
|------|--------|
| ListUserPools endpoint with pagination | ✅ Implemented |
| AdminEnableUser handler | ✅ Implemented |
| AdminDisableUser handler | ✅ Implemented |
| InitiateAuth flow validation | ✅ Implemented |
| AdminCreateUser fields (except message_action) | ✅ Used |

## Remaining Issues

### ruststack-cognito/src/handlers.rs

| Line | Issue | Status |
|------|-------|--------|
| ~111 | `AdminCreateUserRequest.message_action` unused | Pending - could implement RESEND/SUPPRESS behavior |
| ~197 | `InitiateAuthRequest.client_id` unused | Pending - could validate against pool's client ID |
| ~431 | `admin_enable_user` unused `state` parameter | Prefixed with `_` (acceptable) |
| ~432 | `admin_enable_user` unused `req` variable | Prefixed with `_` (acceptable) |
| ~451 | `admin_disable_user` unused `state` parameter | Prefixed with `_` (acceptable) |
| ~452 | `admin_disable_user` unused `req` variable | Prefixed with `_` (acceptable) |

### ruststack-cognito/src/jwt.rs

| Line | Issue | Status |
|------|-------|--------|
| 118 | `verify_token` function never called | Pending - integrate with auth flow |

### ruststack-cognito/src/storage.rs

| Line | Issue | Status |
|------|-------|--------|
| 80 | `RESET_REQUIRED` variant naming | ✅ Fixed with serde rename |

## Action Items

1. **Implement message_action handling** - Handle RESEND (resend verification) and SUPPRESS (skip sending) in AdminCreateUser
2. **Validate client_id** - Verify client_id matches the user pool's client ID in InitiateAuth
3. **Integrate verify_token** - Wire up JWT verification into the authentication flow
4. **Remove duplicate structs** - Clean up any duplicate struct definitions

## Notes

- Variables prefixed with `_` are acceptable for async handler signatures that require certain parameters but don't use them
- `message_action` is the only remaining dead code field in AdminCreateUserRequest
