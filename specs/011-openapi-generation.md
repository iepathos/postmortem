---
number: 11
title: OpenAPI Generation
category: compatibility
priority: medium
status: draft
dependencies: [9, 10]
created: 2025-11-26
---

# Specification 011: OpenAPI Generation

**Category**: compatibility
**Priority**: medium
**Status**: draft
**Dependencies**: Specs 009, 010 (Registry, JSON Schema)

## Context

OpenAPI (formerly Swagger) is the standard for API documentation. APIs typically define request/response schemas in the `components/schemas` section. This specification enables postmortem schemas to be exported as OpenAPI 3.0/3.1 schema components, enabling integration with API documentation tools like Swagger UI, Redoc, and code generators.

## Objective

Implement OpenAPI schema generation:
1. Export schemas as OpenAPI 3.0/3.1 schema objects
2. Generate complete components/schemas section from registry
3. Support OpenAPI-specific annotations (descriptions, examples)
4. Enable integration with OpenAPI document builders

## Requirements

### Functional Requirements

1. **OpenAPI Schema Generation**
   - `schema.to_openapi()` generates OpenAPI schema object
   - Compatible with both OpenAPI 3.0 and 3.1
   - Support nullable annotation for optional types
   - Support description and example annotations

2. **Schema Annotations**
   - `.description(text)` - schema description
   - `.example(value)` - example value
   - `.deprecated()` - mark as deprecated
   - Annotations preserved in OpenAPI output

3. **Components Generation**
   - Registry exports as `components/schemas`
   - References use `#/components/schemas/{name}` format
   - Support for schema grouping/tagging

4. **OpenAPI Document Building**
   - Helper to create complete OpenAPI document structure
   - Support for paths, security, servers (basic)
   - Focus on schema components (main value)

### Non-Functional Requirements

- Generated schemas valid against OpenAPI spec
- Support both 3.0.x and 3.1.x formats
- Clear mapping of postmortem types to OpenAPI types
- Preserve type information accurately

## Acceptance Criteria

- [ ] `schema.to_openapi()` generates valid OpenAPI schema
- [ ] OpenAPI 3.0 uses nullable: true for optional
- [ ] OpenAPI 3.1 uses type: ["string", "null"] syntax
- [ ] `.description("text")` appears in schema
- [ ] `.example(value)` appears in schema
- [ ] Registry exports as components/schemas
- [ ] References use #/components/schemas/{name}
- [ ] Generated schemas validate against OpenAPI spec
- [ ] Can build minimal complete OpenAPI document

## Technical Details

### Implementation Approach

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpenApiVersion {
    V3_0,
    V3_1,
}

pub struct OpenApiOptions {
    pub version: OpenApiVersion,
}

impl Default for OpenApiOptions {
    fn default() -> Self {
        Self { version: OpenApiVersion::V3_1 }
    }
}

pub trait ToOpenApi {
    fn to_openapi(&self, options: &OpenApiOptions) -> serde_json::Value;
}

// Schema annotations
pub struct SchemaMetadata {
    pub description: Option<String>,
    pub example: Option<Value>,
    pub deprecated: bool,
}

impl StringSchema {
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.metadata.description = Some(text.into());
        self
    }

    pub fn example(mut self, value: impl Into<Value>) -> Self {
        self.metadata.example = Some(value.into());
        self
    }

    pub fn deprecated(mut self) -> Self {
        self.metadata.deprecated = true;
        self
    }
}

// Similar for other schema types...

impl ToOpenApi for StringSchema {
    fn to_openapi(&self, options: &OpenApiOptions) -> Value {
        let mut schema = self.to_json_schema(); // Reuse JSON Schema generation

        // Add OpenAPI-specific fields
        if let Some(desc) = &self.metadata.description {
            schema["description"] = json!(desc);
        }
        if let Some(ex) = &self.metadata.example {
            schema["example"] = ex.clone();
        }
        if self.metadata.deprecated {
            schema["deprecated"] = json!(true);
        }

        schema
    }
}

impl ToOpenApi for CombinatorSchema {
    fn to_openapi(&self, options: &OpenApiOptions) -> Value {
        match self {
            CombinatorSchema::Optional { inner } => {
                let inner_schema = inner.to_openapi(options);

                match options.version {
                    OpenApiVersion::V3_0 => {
                        // OpenAPI 3.0: use nullable: true
                        let mut schema = inner_schema;
                        schema["nullable"] = json!(true);
                        schema
                    }
                    OpenApiVersion::V3_1 => {
                        // OpenAPI 3.1: use type array with null
                        // Similar to JSON Schema 2020-12
                        json!({
                            "oneOf": [
                                { "type": "null" },
                                inner_schema
                            ]
                        })
                    }
                }
            }
            _ => self.to_json_schema(), // Reuse JSON Schema for other combinators
        }
    }
}

impl ToOpenApi for RefSchema {
    fn to_openapi(&self, _options: &OpenApiOptions) -> Value {
        json!({
            "$ref": format!("#/components/schemas/{}", self.name)
        })
    }
}

// Registry OpenAPI export
impl SchemaRegistry {
    pub fn to_openapi_components(&self, options: &OpenApiOptions) -> Value {
        let schemas = self.schemas.read();
        let mut components = serde_json::Map::new();

        for (name, schema) in schemas.iter() {
            components.insert(name.clone(), schema.to_openapi(options));
        }

        json!({
            "schemas": components
        })
    }

    pub fn to_openapi_document(
        &self,
        info: OpenApiInfo,
        options: &OpenApiOptions,
    ) -> Value {
        let version = match options.version {
            OpenApiVersion::V3_0 => "3.0.3",
            OpenApiVersion::V3_1 => "3.1.0",
        };

        json!({
            "openapi": version,
            "info": {
                "title": info.title,
                "version": info.version,
                "description": info.description,
            },
            "paths": {},
            "components": self.to_openapi_components(options),
        })
    }
}

pub struct OpenApiInfo {
    pub title: String,
    pub version: String,
    pub description: Option<String>,
}

// OpenAPI document builder for more complex documents
pub struct OpenApiDocumentBuilder {
    info: OpenApiInfo,
    options: OpenApiOptions,
    paths: serde_json::Map<String, Value>,
    security_schemes: serde_json::Map<String, Value>,
    servers: Vec<Value>,
}

impl OpenApiDocumentBuilder {
    pub fn new(info: OpenApiInfo) -> Self {
        Self {
            info,
            options: OpenApiOptions::default(),
            paths: serde_json::Map::new(),
            security_schemes: serde_json::Map::new(),
            servers: Vec::new(),
        }
    }

    pub fn version(mut self, version: OpenApiVersion) -> Self {
        self.options.version = version;
        self
    }

    pub fn server(mut self, url: impl Into<String>, description: Option<String>) -> Self {
        let mut server = json!({ "url": url.into() });
        if let Some(desc) = description {
            server["description"] = json!(desc);
        }
        self.servers.push(server);
        self
    }

    pub fn path(
        mut self,
        path: impl Into<String>,
        method: &str,
        operation: OpenApiOperation,
    ) -> Self {
        let path_str = path.into();
        let path_item = self.paths.entry(path_str)
            .or_insert(json!({}));

        path_item[method] = operation.to_value();
        self
    }

    pub fn build(self, registry: &SchemaRegistry) -> Value {
        let version = match self.options.version {
            OpenApiVersion::V3_0 => "3.0.3",
            OpenApiVersion::V3_1 => "3.1.0",
        };

        let mut doc = json!({
            "openapi": version,
            "info": {
                "title": self.info.title,
                "version": self.info.version,
            },
            "paths": self.paths,
            "components": registry.to_openapi_components(&self.options),
        });

        if let Some(desc) = self.info.description {
            doc["info"]["description"] = json!(desc);
        }

        if !self.servers.is_empty() {
            doc["servers"] = json!(self.servers);
        }

        if !self.security_schemes.is_empty() {
            doc["components"]["securitySchemes"] = json!(self.security_schemes);
        }

        doc
    }
}

pub struct OpenApiOperation {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub request_body: Option<String>, // Schema name
    pub responses: Vec<(String, String)>, // (status, schema name)
}

impl OpenApiOperation {
    fn to_value(&self) -> Value {
        let mut op = json!({});

        if let Some(summary) = &self.summary {
            op["summary"] = json!(summary);
        }
        if let Some(desc) = &self.description {
            op["description"] = json!(desc);
        }

        if let Some(body_schema) = &self.request_body {
            op["requestBody"] = json!({
                "required": true,
                "content": {
                    "application/json": {
                        "schema": { "$ref": format!("#/components/schemas/{}", body_schema) }
                    }
                }
            });
        }

        let mut responses = serde_json::Map::new();
        for (status, schema) in &self.responses {
            responses.insert(status.clone(), json!({
                "description": format!("{} response", status),
                "content": {
                    "application/json": {
                        "schema": { "$ref": format!("#/components/schemas/{}", schema) }
                    }
                }
            }));
        }
        op["responses"] = json!(responses);

        op
    }
}
```

### Architecture Changes

- Create `src/interop/openapi.rs` for OpenAPI support
- Add metadata fields to all schema types
- Add `ToOpenApi` trait

### Data Structures

- `OpenApiVersion`: Enum for 3.0 vs 3.1
- `OpenApiOptions`: Generation options
- `SchemaMetadata`: Description, example, deprecated
- `OpenApiInfo`: Document info fields
- `OpenApiDocumentBuilder`: Builder for complete documents

### APIs and Interfaces

```rust
// Trait for OpenAPI generation
trait ToOpenApi {
    fn to_openapi(&self, options: &OpenApiOptions) -> Value;
}

// Schema annotations (on all schema types)
.description(text: &str) -> Self
.example(value: Value) -> Self
.deprecated() -> Self

// Registry methods
SchemaRegistry::to_openapi_components(&self, options: &OpenApiOptions) -> Value
SchemaRegistry::to_openapi_document(&self, info: OpenApiInfo, options: &OpenApiOptions) -> Value

// Document builder
OpenApiDocumentBuilder::new(info: OpenApiInfo) -> Self
OpenApiDocumentBuilder::version(self, version: OpenApiVersion) -> Self
OpenApiDocumentBuilder::server(self, url: &str, description: Option<&str>) -> Self
OpenApiDocumentBuilder::path(self, path: &str, method: &str, operation: OpenApiOperation) -> Self
OpenApiDocumentBuilder::build(self, registry: &SchemaRegistry) -> Value
```

## Dependencies

- **Prerequisites**: Specs 009, 010
- **Affected Components**: All schema types (metadata)
- **External Dependencies**: None beyond serde_json

## Testing Strategy

- **Unit Tests**:
  - OpenAPI generation for each schema type
  - Nullable handling for 3.0 vs 3.1
  - Annotation preservation
  - Reference format

- **Integration Tests**:
  - Complete document generation
  - Registry export
  - Document builder

- **Validation Tests**:
  - Generated schemas validate against OpenAPI spec
  - Compatible with Swagger UI / Redoc

## Documentation Requirements

- **Code Documentation**: Examples of OpenAPI generation
- **User Documentation**: Guide to API documentation
- **Architecture Updates**: Document OpenAPI layer

## Implementation Notes

- Reuse JSON Schema generation where possible
- Handle nullable differently for 3.0 vs 3.1
- References use #/components/schemas/ prefix
- Focus on schema generation (main value proposition)
- Path generation is secondary/helper functionality

## Migration and Compatibility

No migration needed. Adds metadata fields to schemas but they're optional.

## Files to Create/Modify

```
src/interop/openapi.rs
tests/openapi_test.rs
```

## Example Usage

```rust
use postmortem::{Schema, SchemaRegistry, OpenApiVersion, OpenApiOptions};

// Create schemas with metadata
let user = Schema::object()
    .description("A user in the system")
    .field("id", Schema::integer().positive().description("Unique identifier"))
    .field("email", Schema::string().email().description("User email address"))
    .field("name", Schema::string()
        .max_len(100)
        .description("Display name")
        .example(json!("John Doe")))
    .example(json!({
        "id": 123,
        "email": "john@example.com",
        "name": "John Doe"
    }));

let error = Schema::object()
    .description("Error response")
    .field("code", Schema::string())
    .field("message", Schema::string());

// Register schemas
let registry = SchemaRegistry::new();
registry.register("User", user).unwrap();
registry.register("Error", error).unwrap();

// Generate OpenAPI document
let options = OpenApiOptions { version: OpenApiVersion::V3_1 };
let doc = registry.to_openapi_document(
    OpenApiInfo {
        title: "User API".to_string(),
        version: "1.0.0".to_string(),
        description: Some("API for managing users".to_string()),
    },
    &options,
);

println!("{}", serde_json::to_string_pretty(&doc)?);
// {
//   "openapi": "3.1.0",
//   "info": { "title": "User API", "version": "1.0.0", ... },
//   "paths": {},
//   "components": {
//     "schemas": {
//       "User": {
//         "type": "object",
//         "description": "A user in the system",
//         "properties": { ... },
//         "example": { ... }
//       },
//       "Error": { ... }
//     }
//   }
// }

// Or use the builder for more control
let doc = OpenApiDocumentBuilder::new(OpenApiInfo {
    title: "User API".to_string(),
    version: "1.0.0".to_string(),
    description: None,
})
.version(OpenApiVersion::V3_0)
.server("https://api.example.com", Some("Production"))
.path("/users", "post", OpenApiOperation {
    summary: Some("Create a user".to_string()),
    description: Some("Creates a new user in the system".to_string()),
    request_body: Some("User".to_string()),
    responses: vec![
        ("201".to_string(), "User".to_string()),
        ("400".to_string(), "Error".to_string()),
    ],
})
.build(&registry);
```
