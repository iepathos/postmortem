---
number: 12
title: API Integration
category: compatibility
priority: high
status: draft
dependencies: [1, 4]
created: 2025-11-26
---

# Specification 012: API Integration

**Category**: compatibility
**Priority**: high
**Status**: draft
**Dependencies**: Specs 001, 004 (Core Types, Object Schema)

## Context

Validation errors need to be returned to API consumers in a structured, actionable format. Different frameworks have different conventions for error responses. This specification defines standard error response types and framework-specific integrations for axum and actix-web.

The goal is to make it trivial to use postmortem validation in web APIs with minimal boilerplate and consistent error formats.

## Objective

Implement API-friendly error handling:
1. Standard validation error response format
2. Conversion from SchemaErrors to API responses
3. Framework integrations (axum, actix-web)
4. Customizable response formatting

## Requirements

### Functional Requirements

1. **API Error Response Types**
   - `ApiValidationError` - top-level error response
   - `ApiFieldError` - per-field error details
   - Serializable to JSON
   - Consistent structure across all errors

2. **Error Conversion**
   - `SchemaErrors::to_api_response()` - convert to API format
   - Group errors by field path
   - Include error codes for programmatic handling
   - Configurable response structure

3. **axum Integration** (feature: `axum`)
   - `impl IntoResponse for ApiValidationError`
   - Returns 422 Unprocessable Entity
   - Proper Content-Type header
   - Easy to use in handlers

4. **actix-web Integration** (feature: `actix-web`)
   - `impl ResponseError for ApiValidationError`
   - Returns 422 Unprocessable Entity
   - Proper error body formatting
   - Easy to use in handlers

5. **Response Customization**
   - Configurable HTTP status code
   - Configurable error structure
   - Support for wrapping in envelope

### Non-Functional Requirements

- Zero-cost when framework features not enabled
- Consistent error format across frameworks
- Clear, actionable error messages
- Support for i18n (error codes for translation)

## Acceptance Criteria

- [ ] `ApiValidationError` serializes to consistent JSON
- [ ] `ApiFieldError` contains path, message, code fields
- [ ] `SchemaErrors::to_api_response()` groups by path
- [ ] axum `IntoResponse` returns 422 status
- [ ] actix-web `ResponseError` returns 422 status
- [ ] Response structure matches API conventions
- [ ] Multiple errors for same field are grouped
- [ ] Error codes are machine-readable

## Technical Details

### Implementation Approach

```rust
use serde::{Deserialize, Serialize};

/// API validation error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiValidationError {
    /// HTTP status code (usually 422)
    #[serde(skip)]
    pub status: u16,

    /// Error type identifier
    pub error: String,

    /// Human-readable error message
    pub message: String,

    /// Detailed field-level errors
    pub details: Vec<ApiFieldError>,
}

/// Per-field validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiFieldError {
    /// JSON path to the field (e.g., "user.email" or "items[0].name")
    pub field: String,

    /// Human-readable error message
    pub message: String,

    /// Machine-readable error code for i18n/programmatic handling
    pub code: String,

    /// What value was received (optional, for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received: Option<serde_json::Value>,

    /// What was expected (optional, for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
}

impl ApiValidationError {
    /// Create a new validation error with default settings
    pub fn new(message: impl Into<String>, details: Vec<ApiFieldError>) -> Self {
        Self {
            status: 422,
            error: "validation_error".to_string(),
            message: message.into(),
            details,
        }
    }

    /// Set custom HTTP status code
    pub fn with_status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    /// Set custom error type identifier
    pub fn with_error_type(mut self, error: impl Into<String>) -> Self {
        self.error = error.into();
        self
    }
}

impl SchemaErrors {
    /// Convert to API-friendly response format
    pub fn to_api_response(&self) -> ApiValidationError {
        let details: Vec<ApiFieldError> = self
            .iter()
            .map(|e| ApiFieldError {
                field: e.path.to_string(),
                message: e.message.clone(),
                code: e.code.clone(),
                received: e.got.as_ref().map(|s| json!(s)),
                expected: e.expected.clone(),
            })
            .collect();

        ApiValidationError::new(
            format!("Validation failed with {} error(s)", details.len()),
            details,
        )
    }

    /// Convert to API response, grouping errors by field
    pub fn to_api_response_grouped(&self) -> ApiValidationError {
        use std::collections::BTreeMap;

        let mut by_field: BTreeMap<String, Vec<ApiFieldError>> = BTreeMap::new();

        for error in self.iter() {
            let field = error.path.to_string();
            by_field.entry(field.clone()).or_default().push(ApiFieldError {
                field,
                message: error.message.clone(),
                code: error.code.clone(),
                received: error.got.as_ref().map(|s| json!(s)),
                expected: error.expected.clone(),
            });
        }

        let details: Vec<ApiFieldError> = by_field
            .into_values()
            .flatten()
            .collect();

        ApiValidationError::new(
            format!("Validation failed with {} error(s)", details.len()),
            details,
        )
    }
}

// axum integration (feature-gated)
#[cfg(feature = "axum")]
mod axum_integration {
    use super::*;
    use axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
        Json,
    };

    impl IntoResponse for ApiValidationError {
        fn into_response(self) -> Response {
            let status = StatusCode::from_u16(self.status)
                .unwrap_or(StatusCode::UNPROCESSABLE_ENTITY);

            (status, Json(self)).into_response()
        }
    }

    // Helper trait for validation in handlers
    pub trait ValidateRequest {
        fn validate<S: SchemaLike>(
            self,
            schema: &S,
        ) -> Result<ValidatedValue, ApiValidationError>;
    }

    impl ValidateRequest for serde_json::Value {
        fn validate<S: SchemaLike>(
            self,
            schema: &S,
        ) -> Result<ValidatedValue, ApiValidationError> {
            match schema.validate(&self, &JsonPath::root()) {
                Validation::Valid(v) => Ok(v),
                Validation::Invalid(errors) => Err(errors.to_api_response()),
            }
        }
    }
}

// actix-web integration (feature-gated)
#[cfg(feature = "actix-web")]
mod actix_integration {
    use super::*;
    use actix_web::{HttpResponse, ResponseError, http::StatusCode};

    impl std::fmt::Display for ApiValidationError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl ResponseError for ApiValidationError {
        fn status_code(&self) -> StatusCode {
            StatusCode::from_u16(self.status)
                .unwrap_or(StatusCode::UNPROCESSABLE_ENTITY)
        }

        fn error_response(&self) -> HttpResponse {
            HttpResponse::build(self.status_code())
                .json(self)
        }
    }
}

// Response envelope wrapper (optional)
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiValidationError>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(error: ApiValidationError) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}
```

### Standard Response Format

```json
{
  "error": "validation_error",
  "message": "Validation failed with 3 error(s)",
  "details": [
    {
      "field": "email",
      "message": "must be a valid email address",
      "code": "invalid_email"
    },
    {
      "field": "age",
      "message": "must be at least 18",
      "code": "min_value",
      "received": "16",
      "expected": "minimum 18"
    },
    {
      "field": "items[0].name",
      "message": "required field 'name' is missing",
      "code": "required"
    }
  ]
}
```

### Architecture Changes

- Create `src/error/api.rs` for API types
- Create `src/integrations/` for framework integrations
- Add feature flags for framework support

### Data Structures

- `ApiValidationError`: Top-level error response
- `ApiFieldError`: Individual field error
- `ApiResponse<T>`: Optional envelope wrapper

### APIs and Interfaces

```rust
// Error types
ApiValidationError::new(message: &str, details: Vec<ApiFieldError>) -> Self
ApiValidationError::with_status(self, status: u16) -> Self
ApiValidationError::with_error_type(self, error: &str) -> Self

// Conversion
SchemaErrors::to_api_response(&self) -> ApiValidationError
SchemaErrors::to_api_response_grouped(&self) -> ApiValidationError

// Framework traits (feature-gated)
impl IntoResponse for ApiValidationError  // axum
impl ResponseError for ApiValidationError  // actix-web

// Response envelope
ApiResponse::success(data: T) -> ApiResponse<T>
ApiResponse::error(error: ApiValidationError) -> ApiResponse<()>
```

## Dependencies

- **Prerequisites**: Specs 001, 004
- **Affected Components**: Error types
- **External Dependencies** (feature-gated):
  - `axum` for axum integration
  - `actix-web` for actix-web integration

## Testing Strategy

- **Unit Tests**:
  - ApiValidationError serialization
  - ApiFieldError serialization
  - SchemaErrors conversion
  - Grouped vs flat conversion

- **Integration Tests**:
  - axum handler test
  - actix-web handler test
  - Full request/response cycle

- **Compatibility Tests**:
  - JSON output matches expected format
  - Status codes are correct

## Documentation Requirements

- **Code Documentation**: Examples for each framework
- **User Documentation**: API integration guide
- **Architecture Updates**: Document integration layer

## Implementation Notes

- Use `skip_serializing_if` for optional fields
- Default status is 422 (Unprocessable Entity)
- Error codes should be snake_case
- Field paths use dot notation with bracket indices
- Consider adding `timestamp` field optionally

## Migration and Compatibility

No migration needed - new optional features.

## Files to Create/Modify

```
src/error/api.rs
src/integrations/mod.rs
src/integrations/axum.rs
src/integrations/actix.rs
tests/api_response_test.rs
```

## Feature Flags

```toml
[features]
axum = ["dep:axum"]
actix-web = ["dep:actix-web"]
```

## Example Usage

### axum

```rust
use axum::{Json, routing::post, Router};
use postmortem::{Schema, ValidateRequest};
use serde_json::Value;

async fn create_user(Json(body): Json<Value>) -> Result<Json<Value>, ApiValidationError> {
    let schema = Schema::object()
        .field("email", Schema::string().email())
        .field("age", Schema::integer().min(18));

    let validated = body.validate(&schema)?;

    Ok(Json(json!({ "id": 1, "status": "created" })))
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/users", post(create_user));
    // ...
}
```

### actix-web

```rust
use actix_web::{web, App, HttpResponse, HttpServer};
use postmortem::Schema;
use serde_json::Value;

async fn create_user(body: web::Json<Value>) -> Result<HttpResponse, ApiValidationError> {
    let schema = Schema::object()
        .field("email", Schema::string().email())
        .field("age", Schema::integer().min(18));

    match schema.validate(&body, &JsonPath::root()) {
        Validation::Valid(user) => Ok(HttpResponse::Created().json(user)),
        Validation::Invalid(errors) => Err(errors.to_api_response()),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new().route("/users", web::post().to(create_user))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```

### Response Example

Request:
```json
{
  "email": "invalid-email",
  "age": 16
}
```

Response (422):
```json
{
  "error": "validation_error",
  "message": "Validation failed with 2 error(s)",
  "details": [
    {
      "field": "email",
      "message": "must be a valid email address",
      "code": "invalid_email",
      "received": "invalid-email"
    },
    {
      "field": "age",
      "message": "must be at least 18",
      "code": "min_value",
      "received": "16",
      "expected": "minimum 18"
    }
  ]
}
```
