---
number: 6
title: String Formats and Custom Validators
category: foundation
priority: high
status: draft
dependencies: [1, 2]
created: 2025-11-26
---

# Specification 006: String Formats and Custom Validators

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Specs 001, 002 (Core Types, String Schema)

## Context

Real-world APIs frequently validate common string formats like emails, URLs, UUIDs, and dates. Rather than requiring users to write custom regex patterns for these common cases, postmortem should provide built-in format validators with clear, actionable error messages.

Additionally, users need the ability to define custom validators for domain-specific validation requirements that aren't covered by built-ins.

## Objective

Extend string schema with:
1. Built-in format validators (email, URL, UUID, datetime, IP)
2. Enumeration constraints (one_of)
3. String transformations (trim, lowercase)
4. Custom validator support for arbitrary validation logic

## Requirements

### Functional Requirements

1. **Built-in Format Validators**
   - `.email()` - RFC 5322 email validation
   - `.url()` - URL validation (http/https)
   - `.uuid()` - UUID (any version) validation
   - `.date()` - ISO 8601 date (YYYY-MM-DD)
   - `.datetime()` - ISO 8601 datetime
   - `.ip()` - IPv4 or IPv6 address
   - `.ipv4()` - IPv4 only
   - `.ipv6()` - IPv6 only

2. **String Enumeration**
   - `.one_of(values)` - value must be one of the provided strings
   - Clear error message showing valid options

3. **String Prefix/Suffix/Contains**
   - `.starts_with(prefix)` - value must start with prefix
   - `.ends_with(suffix)` - value must end with suffix
   - `.contains(substring)` - value must contain substring

4. **String Transformations**
   - `.trim()` - trim whitespace before validation
   - `.lowercase()` - convert to lowercase before validation
   - Transformations apply before other constraints

5. **Custom Validators**
   - `.custom(fn)` - arbitrary validation function
   - Custom function returns `Validation<(), SchemaErrors>`
   - Multiple custom validators can be chained

6. **Integer Enumeration (bonus)**
   - `.one_of(values)` on integer schema
   - `.multiple_of(n)` - divisibility constraint

### Non-Functional Requirements

- Format validators should use well-tested libraries where appropriate
- Feature-gated optional dependencies for specialized formats
- Clear, format-specific error messages
- Efficient validation (compiled regex cached)

## Acceptance Criteria

- [ ] `.email()` validates RFC 5322 format
- [ ] `.email()` rejects invalid emails with clear error
- [ ] `.url()` validates URLs with http/https schemes
- [ ] `.uuid()` validates any UUID version
- [ ] `.date()` validates YYYY-MM-DD format
- [ ] `.datetime()` validates ISO 8601 datetime
- [ ] `.ip()` validates IPv4 and IPv6
- [ ] `.ipv4()` validates IPv4 only
- [ ] `.ipv6()` validates IPv6 only
- [ ] `.one_of(["a", "b"])` rejects values not in list
- [ ] `.starts_with("http")` validates prefix
- [ ] `.ends_with(".json")` validates suffix
- [ ] `.contains("@")` validates substring
- [ ] `.trim()` removes whitespace before validation
- [ ] `.lowercase()` lowercases before validation
- [ ] `.custom(fn)` allows arbitrary validation
- [ ] Multiple custom validators accumulate errors
- [ ] Format errors use descriptive codes (e.g., `invalid_email`)

## Technical Details

### Implementation Approach

```rust
// Extend StringSchema with format methods
impl StringSchema {
    pub fn email(mut self) -> Self {
        self.constraints.push(StringConstraint::Format {
            format: Format::Email,
            message: None,
        });
        self
    }

    pub fn url(mut self) -> Self {
        self.constraints.push(StringConstraint::Format {
            format: Format::Url,
            message: None,
        });
        self
    }

    pub fn one_of<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let values: Vec<String> = values.into_iter().map(Into::into).collect();
        self.constraints.push(StringConstraint::OneOf {
            values,
            message: None,
        });
        self
    }

    pub fn starts_with(mut self, prefix: impl Into<String>) -> Self {
        self.constraints.push(StringConstraint::StartsWith {
            prefix: prefix.into(),
            message: None,
        });
        self
    }

    pub fn trim(mut self) -> Self {
        self.transforms.push(Transform::Trim);
        self
    }

    pub fn lowercase(mut self) -> Self {
        self.transforms.push(Transform::Lowercase);
        self
    }

    pub fn custom<F>(mut self, validator: F) -> Self
    where
        F: Fn(&str, &JsonPath) -> Validation<(), SchemaErrors> + 'static,
    {
        self.custom_validators.push(Box::new(validator));
        self
    }
}

#[derive(Clone)]
enum Format {
    Email,
    Url,
    Uuid,
    Date,
    DateTime,
    Ip,
    Ipv4,
    Ipv6,
}

enum StringConstraint {
    // Existing...
    Format { format: Format, message: Option<String> },
    OneOf { values: Vec<String>, message: Option<String> },
    StartsWith { prefix: String, message: Option<String> },
    EndsWith { suffix: String, message: Option<String> },
    Contains { substring: String, message: Option<String> },
}

enum Transform {
    Trim,
    Lowercase,
}

// Format validation implementations
fn validate_email(s: &str) -> bool {
    #[cfg(feature = "email")]
    {
        email_address::EmailAddress::parse(s, None).is_ok()
    }
    #[cfg(not(feature = "email"))]
    {
        // Fallback: basic regex check
        static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap());
        RE.is_match(s)
    }
}

fn validate_url(s: &str) -> bool {
    #[cfg(feature = "url")]
    {
        match url::Url::parse(s) {
            Ok(url) => url.scheme() == "http" || url.scheme() == "https",
            Err(_) => false,
        }
    }
    #[cfg(not(feature = "url"))]
    {
        s.starts_with("http://") || s.starts_with("https://")
    }
}

fn validate_uuid(s: &str) -> bool {
    #[cfg(feature = "uuid")]
    {
        uuid::Uuid::parse_str(s).is_ok()
    }
    #[cfg(not(feature = "uuid"))]
    {
        static RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$").unwrap()
        });
        RE.is_match(s)
    }
}

fn validate_date(s: &str) -> bool {
    #[cfg(feature = "datetime")]
    {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
    }
    #[cfg(not(feature = "datetime"))]
    {
        static RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap()
        });
        RE.is_match(s)
    }
}
```

### Architecture Changes

- Create `src/validation/formats.rs` for format validators
- Add transforms to StringSchema
- Add custom validator support to schema

### Data Structures

- `Format`: Enum of built-in format types
- `Transform`: Enum of string transformations
- Custom validator: `Box<dyn Fn(&str, &JsonPath) -> Validation<(), SchemaErrors>>`

### APIs and Interfaces

```rust
// Format validators
StringSchema::email(self) -> Self
StringSchema::url(self) -> Self
StringSchema::uuid(self) -> Self
StringSchema::date(self) -> Self
StringSchema::datetime(self) -> Self
StringSchema::ip(self) -> Self
StringSchema::ipv4(self) -> Self
StringSchema::ipv6(self) -> Self

// Enumeration
StringSchema::one_of<I, S>(self, values: I) -> Self

// String matching
StringSchema::starts_with(self, prefix: impl Into<String>) -> Self
StringSchema::ends_with(self, suffix: impl Into<String>) -> Self
StringSchema::contains(self, substring: impl Into<String>) -> Self

// Transforms
StringSchema::trim(self) -> Self
StringSchema::lowercase(self) -> Self

// Custom validators
StringSchema::custom<F>(self, validator: F) -> Self
```

## Dependencies

- **Prerequisites**: Specs 001, 002
- **Affected Components**: StringSchema
- **External Dependencies** (optional, feature-gated):
  - `email_address` for email validation
  - `url` for URL validation
  - `uuid` for UUID validation
  - `chrono` for date/datetime validation

## Testing Strategy

- **Unit Tests**:
  - Each format with valid/invalid examples
  - One-of with various values
  - Prefix/suffix/contains matching
  - Trim transformation
  - Lowercase transformation
  - Custom validator execution
  - Multiple custom validators

- **Edge Cases**:
  - Empty strings
  - Unicode strings
  - Very long strings
  - Edge cases for each format
  - Transform + constraint ordering

## Documentation Requirements

- **Code Documentation**: Examples for each format
- **User Documentation**: Format validation guide
- **Architecture Updates**: Feature flag documentation

## Implementation Notes

- Transforms apply in order they're added
- Transforms run before constraints
- Custom validators receive the (possibly transformed) string
- Feature flags allow minimal dependencies
- Fallback implementations for when features disabled

## Migration and Compatibility

No migration needed - extends existing string schema.

## Files to Create/Modify

```
src/validation/formats.rs
src/schema/string.rs (extend)
tests/formats_test.rs
tests/custom_validator_test.rs
```

## Example Usage

```rust
use postmortem::Schema;

// Email validation
let email = Schema::string().email();

// URL with custom error
let website = Schema::string()
    .url()
    .error("must be a valid HTTP(S) URL");

// Enum values
let status = Schema::string()
    .one_of(["pending", "active", "completed"]);

// With transforms
let username = Schema::string()
    .trim()
    .lowercase()
    .min_len(3)
    .max_len(20)
    .pattern(r"^[a-z0-9_]+$").unwrap();

// Custom validator
let password = Schema::string()
    .min_len(8)
    .custom(|s, path| {
        let has_uppercase = s.chars().any(|c| c.is_uppercase());
        let has_digit = s.chars().any(|c| c.is_numeric());

        let mut errors = vec![];
        if !has_uppercase {
            errors.push(SchemaError::new(path.clone(), "must contain uppercase")
                .with_code("password_uppercase"));
        }
        if !has_digit {
            errors.push(SchemaError::new(path.clone(), "must contain digit")
                .with_code("password_digit"));
        }

        if errors.is_empty() {
            Validation::valid(())
        } else {
            Validation::invalid(SchemaErrors::from_vec(errors).unwrap())
        }
    });
```
