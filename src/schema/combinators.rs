//! Schema combinators for composing validation logic.
//!
//! This module provides combinators that allow schemas to be composed in different ways:
//! - `one_of`: Exactly one schema must match (discriminated unions)
//! - `any_of`: At least one schema must match (flexible unions)
//! - `all_of`: All schemas must match (intersection/merging)
//! - `optional`: Value can be null
//!
//! # Example
//!
//! ```rust
//! use postmortem::{Schema, ValueValidator, JsonPath};
//! use serde_json::json;
//!
//! // Discriminated union - either a circle or rectangle
//! let shape = Schema::one_of(vec![
//!     Box::new(Schema::object()
//!         .field("type", Schema::string())
//!         .field("radius", Schema::integer().positive())) as Box<dyn ValueValidator>,
//!     Box::new(Schema::object()
//!         .field("type", Schema::string())
//!         .field("width", Schema::integer().positive())
//!         .field("height", Schema::integer().positive())) as Box<dyn ValueValidator>,
//! ]);
//!
//! // Flexible type - string or integer ID
//! let id = Schema::any_of(vec![
//!     Box::new(Schema::string().min_len(1)) as Box<dyn ValueValidator>,
//!     Box::new(Schema::integer().positive()) as Box<dyn ValueValidator>,
//! ]);
//! ```

use serde_json::{json, Value};
use std::sync::Arc;
use stillwater::Validation;

use crate::error::{SchemaError, SchemaErrors};
use crate::interop::ToJsonSchema;
use crate::path::JsonPath;
use crate::schema::traits::{SchemaLike, ValueValidator};
use crate::validation::ValidationContext;

/// Type alias for validation function stored in combinators.
pub(crate) type ValidatorFn =
    Arc<dyn Fn(&Value, &JsonPath) -> Validation<Value, SchemaErrors> + Send + Sync>;

/// Schema combinators for composing validation logic.
///
/// `CombinatorSchema` provides four composition patterns:
/// - `OneOf`: Exactly one schema must match (discriminated unions)
/// - `AnyOf`: At least one schema must match (flexible unions)
/// - `AllOf`: All schemas must match (intersection)
/// - `Optional`: Value can be null
///
/// Each combinator implements `SchemaLike` and can be used anywhere a schema is expected.
#[derive(Clone)]
pub enum CombinatorSchema {
    /// Exactly one schema must match.
    ///
    /// Validates the value against all schemas. Succeeds if exactly one matches,
    /// fails if none or multiple match. Ideal for discriminated unions where
    /// a value must be one of several distinct types.
    OneOf {
        schemas: Vec<ValidatorFn>,
        validators: Vec<Arc<dyn ValueValidator>>,
    },

    /// At least one schema must match.
    ///
    /// Validates the value against schemas in order, short-circuiting on the
    /// first match. Fails only if none match. More permissive than `OneOf`.
    AnyOf {
        schemas: Vec<ValidatorFn>,
        validators: Vec<Arc<dyn ValueValidator>>,
    },

    /// All schemas must match.
    ///
    /// Validates the value against all schemas. Succeeds only if all pass,
    /// accumulating errors from any that fail. Useful for schema composition
    /// and intersection.
    AllOf {
        schemas: Vec<ValidatorFn>,
        validators: Vec<Arc<dyn ValueValidator>>,
    },

    /// Value can be null.
    ///
    /// Null values pass validation. Non-null values are validated against
    /// the inner schema.
    Optional {
        inner: ValidatorFn,
        validator: Arc<dyn ValueValidator>,
    },
}

impl CombinatorSchema {
    /// Validates a value against exactly one of the provided schemas.
    ///
    /// Returns success if exactly one schema matches, failure if none or multiple match.
    fn validate_one_of(
        schemas: &[ValidatorFn],
        value: &Value,
        path: &JsonPath,
    ) -> Validation<Value, SchemaErrors> {
        let results: Vec<_> = schemas
            .iter()
            .enumerate()
            .map(|(i, validator)| (i, validator(value, path)))
            .collect();

        let valid: Vec<_> = results.iter().filter(|(_, r)| r.is_success()).collect();

        match valid.len() {
            0 => {
                // None matched - report with count
                let error = SchemaError::new(
                    path.clone(),
                    format!("value did not match any of {} schemas", schemas.len()),
                )
                .with_code("one_of_none_matched");

                Validation::Failure(SchemaErrors::single(error))
            }
            1 => {
                // Exactly one matched - success
                let (_, result) = valid.into_iter().next().unwrap();
                // Extract the value from the successful result
                match result {
                    Validation::Success(v) => Validation::Success(v.clone()),
                    _ => unreachable!(),
                }
            }
            n => {
                // Multiple matched - ambiguous
                let indices: Vec<_> = valid.iter().map(|(i, _)| i).collect();
                let error = SchemaError::new(
                    path.clone(),
                    format!(
                        "value matched {} schemas (indices {:?}), expected exactly one",
                        n, indices
                    ),
                )
                .with_code("one_of_multiple_matched");

                Validation::Failure(SchemaErrors::single(error))
            }
        }
    }

    /// Validates a value against at least one of the provided schemas.
    ///
    /// Short-circuits on the first match. Returns failure only if none match.
    fn validate_any_of(
        schemas: &[ValidatorFn],
        value: &Value,
        path: &JsonPath,
    ) -> Validation<Value, SchemaErrors> {
        for validator in schemas {
            match validator(value, path) {
                Validation::Success(v) => return Validation::Success(v),
                Validation::Failure(_) => continue,
            }
        }

        // None matched
        let error = SchemaError::new(
            path.clone(),
            format!("value did not match any of {} schemas", schemas.len()),
        )
        .with_code("any_of_none_matched");

        Validation::Failure(SchemaErrors::single(error))
    }

    /// Validates a value against all of the provided schemas.
    ///
    /// Returns success only if all schemas pass, accumulating errors from failures.
    fn validate_all_of(
        schemas: &[ValidatorFn],
        value: &Value,
        path: &JsonPath,
    ) -> Validation<Value, SchemaErrors> {
        let mut all_errors = Vec::new();
        let mut last_valid = None;

        for validator in schemas {
            match validator(value, path) {
                Validation::Success(v) => last_valid = Some(v),
                Validation::Failure(e) => all_errors.extend(e.into_iter()),
            }
        }

        if all_errors.is_empty() {
            Validation::Success(last_valid.unwrap_or_else(|| value.clone()))
        } else {
            Validation::Failure(SchemaErrors::from_vec(all_errors))
        }
    }

    /// Validates a value as optional (can be null).
    ///
    /// Null values pass. Non-null values are validated against the inner schema.
    fn validate_optional(
        inner: &ValidatorFn,
        value: &Value,
        path: &JsonPath,
    ) -> Validation<Value, SchemaErrors> {
        if value.is_null() {
            Validation::Success(Value::Null)
        } else {
            inner(value, path)
        }
    }

    /// Validates a value against exactly one of the provided schemas with context.
    fn validate_one_of_with_context(
        validators: &[Arc<dyn ValueValidator>],
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        let results: Vec<_> = validators
            .iter()
            .enumerate()
            .map(|(i, validator)| {
                (
                    i,
                    validator.validate_value_with_context(value, path, context),
                )
            })
            .collect();

        let valid: Vec<_> = results.iter().filter(|(_, r)| r.is_success()).collect();

        match valid.len() {
            0 => {
                let error = SchemaError::new(
                    path.clone(),
                    format!("value did not match any of {} schemas", validators.len()),
                )
                .with_code("one_of_none_matched");

                Validation::Failure(SchemaErrors::single(error))
            }
            1 => {
                let (_, result) = valid.into_iter().next().unwrap();
                match result {
                    Validation::Success(v) => Validation::Success(v.clone()),
                    _ => unreachable!(),
                }
            }
            n => {
                let indices: Vec<_> = valid.iter().map(|(i, _)| i).collect();
                let error = SchemaError::new(
                    path.clone(),
                    format!(
                        "value matched {} schemas (indices {:?}), expected exactly one",
                        n, indices
                    ),
                )
                .with_code("one_of_multiple_matched");

                Validation::Failure(SchemaErrors::single(error))
            }
        }
    }

    /// Validates a value against at least one of the provided schemas with context.
    fn validate_any_of_with_context(
        validators: &[Arc<dyn ValueValidator>],
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        for validator in validators {
            match validator.validate_value_with_context(value, path, context) {
                Validation::Success(v) => return Validation::Success(v),
                Validation::Failure(_) => continue,
            }
        }

        let error = SchemaError::new(
            path.clone(),
            format!("value did not match any of {} schemas", validators.len()),
        )
        .with_code("any_of_none_matched");

        Validation::Failure(SchemaErrors::single(error))
    }

    /// Validates a value against all of the provided schemas with context.
    fn validate_all_of_with_context(
        validators: &[Arc<dyn ValueValidator>],
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        let mut all_errors = Vec::new();
        let mut last_valid = None;

        for validator in validators {
            match validator.validate_value_with_context(value, path, context) {
                Validation::Success(v) => last_valid = Some(v),
                Validation::Failure(e) => all_errors.extend(e.into_iter()),
            }
        }

        if all_errors.is_empty() {
            Validation::Success(last_valid.unwrap_or_else(|| value.clone()))
        } else {
            Validation::Failure(SchemaErrors::from_vec(all_errors))
        }
    }

    /// Validates a value as optional with context.
    fn validate_optional_with_context(
        validator: &Arc<dyn ValueValidator>,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        if value.is_null() {
            Validation::Success(Value::Null)
        } else {
            validator.validate_value_with_context(value, path, context)
        }
    }
}

impl SchemaLike for CombinatorSchema {
    type Output = Value;

    fn validate(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        match self {
            CombinatorSchema::OneOf { schemas, .. } => Self::validate_one_of(schemas, value, path),
            CombinatorSchema::AnyOf { schemas, .. } => Self::validate_any_of(schemas, value, path),
            CombinatorSchema::AllOf { schemas, .. } => Self::validate_all_of(schemas, value, path),
            CombinatorSchema::Optional { inner, .. } => Self::validate_optional(inner, value, path),
        }
    }

    fn validate_to_value(&self, value: &Value, path: &JsonPath) -> Validation<Value, SchemaErrors> {
        self.validate(value, path)
    }

    fn validate_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        match self {
            CombinatorSchema::OneOf { validators, .. } => {
                Self::validate_one_of_with_context(validators, value, path, context)
            }
            CombinatorSchema::AnyOf { validators, .. } => {
                Self::validate_any_of_with_context(validators, value, path, context)
            }
            CombinatorSchema::AllOf { validators, .. } => {
                Self::validate_all_of_with_context(validators, value, path, context)
            }
            CombinatorSchema::Optional { validator, .. } => {
                Self::validate_optional_with_context(validator, value, path, context)
            }
        }
    }

    fn validate_to_value_with_context(
        &self,
        value: &Value,
        path: &JsonPath,
        context: &ValidationContext,
    ) -> Validation<Value, SchemaErrors> {
        self.validate_with_context(value, path, context)
    }

    fn collect_refs(&self, refs: &mut Vec<String>) {
        match self {
            CombinatorSchema::OneOf { validators, .. } => {
                for validator in validators {
                    validator.collect_refs(refs);
                }
            }
            CombinatorSchema::AnyOf { validators, .. } => {
                for validator in validators {
                    validator.collect_refs(refs);
                }
            }
            CombinatorSchema::AllOf { validators, .. } => {
                for validator in validators {
                    validator.collect_refs(refs);
                }
            }
            CombinatorSchema::Optional { validator, .. } => {
                validator.collect_refs(refs);
            }
        }
    }
}

impl ToJsonSchema for CombinatorSchema {
    fn to_json_schema(&self) -> Value {
        match self {
            CombinatorSchema::OneOf { validators, .. } => {
                json!({
                    "oneOf": validators.iter().map(|v| v.to_json_schema()).collect::<Vec<_>>()
                })
            }
            CombinatorSchema::AnyOf { validators, .. } => {
                json!({
                    "anyOf": validators.iter().map(|v| v.to_json_schema()).collect::<Vec<_>>()
                })
            }
            CombinatorSchema::AllOf { validators, .. } => {
                json!({
                    "allOf": validators.iter().map(|v| v.to_json_schema()).collect::<Vec<_>>()
                })
            }
            CombinatorSchema::Optional { validator, .. } => {
                json!({
                    "oneOf": [
                        json!({ "type": "null" }),
                        validator.to_json_schema()
                    ]
                })
            }
        }
    }
}
