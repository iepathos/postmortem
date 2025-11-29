//! Async validation support via Effect.
//!
//! This module provides async validation capabilities for scenarios that
//! require I/O operations, such as database lookups or API calls.
//!
//! # API Design Note
//!
//! This implementation uses a simplified API compatible with stillwater 0.12.
//! Rather than wrapping validators in Effect<E, Er, R> types, we use direct
//! trait methods that accept environment parameters and return Validation
//! results.
//!
//! This approach provides the same dependency injection benefits as a full
//! Effect system while maintaining API simplicity:
//! - Environment dependencies passed explicitly via type parameters
//! - Validation results use stillwater's Validation type for error accumulation
//! - Testability through trait-based environment abstraction
//!
//! The `validate_with_env` extension method on schema types provides ergonomic
//! integration with custom validation logic that needs access to environment
//! dependencies like databases or external APIs.

use rayon::prelude::*;
use serde_json::Value;
use stillwater::Validation;

use crate::error::SchemaErrors;
use crate::path::JsonPath;
use crate::schema::StringSchema;

/// Trait for async validators that use Effect for dependency injection.
///
/// Async validators can perform I/O operations during validation,
/// such as checking uniqueness constraints against a database.
///
/// # Example
///
/// ```rust,ignore
/// use postmortem::effect::AsyncValidator;
/// use stillwater::Validation;
///
/// struct UniqueEmailValidator {
///     db: DatabaseConnection,
/// }
///
/// impl<E> AsyncValidator<E> for UniqueEmailValidator {
///     fn validate_async(
///         &self,
///         value: &Value,
///         path: &JsonPath,
///         env: &E,
///     ) -> Validation<(), SchemaErrors> {
///         let email = value.as_str().unwrap_or("");
///         let exists = self.db.email_exists(email);
///
///         if exists {
///             Validation::Failure(SchemaErrors::single(
///                 SchemaError::new(path.clone(), "email already exists")
///             ))
///         } else {
///             Validation::Success(())
///         }
///     }
/// }
/// ```
pub trait AsyncValidator<E>: Send + Sync {
    /// Validates a value synchronously using environment dependencies.
    ///
    /// Returns a Validation result directly. This is called "async" because
    /// it takes an environment parameter that could provide async resources,
    /// but the validation itself is synchronous for simplicity.
    fn validate_async(
        &self,
        value: &Value,
        path: &JsonPath,
        env: &E,
    ) -> Validation<(), SchemaErrors>;
}

/// An async string schema that combines sync and async validators.
///
/// This wrapper runs synchronous validators first, then async validators
/// only if sync validation passes. Errors from both types accumulate.
pub struct AsyncStringSchema<E> {
    sync_schema: StringSchema,
    async_validators: Vec<Box<dyn AsyncValidator<E>>>,
}

impl<E> AsyncStringSchema<E> {
    /// Creates a new async string schema from a sync schema.
    pub fn new(sync_schema: StringSchema) -> Self {
        Self {
            sync_schema,
            async_validators: Vec::new(),
        }
    }

    /// Adds an async custom validator.
    pub fn async_custom<V>(mut self, validator: V) -> Self
    where
        V: AsyncValidator<E> + 'static,
    {
        self.async_validators.push(Box::new(validator));
        self
    }

    /// Validates a value with both sync and async validators.
    ///
    /// The validation process:
    /// 1. Runs sync validators first
    /// 2. If sync fails, returns those errors immediately
    /// 3. If sync passes, runs all async validators
    /// 4. Accumulates errors from all async validators
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use postmortem::Schema;
    /// use postmortem::effect::AsyncStringSchema;
    ///
    /// let schema = AsyncStringSchema::new(Schema::string().min_len(3))
    ///     .async_custom(UniqueEmailValidator::new());
    ///
    /// let result = schema.validate_with_env(&json!("test@example.com"), &JsonPath::root(), &env);
    /// ```
    pub fn validate_with_env(
        &self,
        value: &Value,
        path: &JsonPath,
        env: &E,
    ) -> Validation<String, SchemaErrors> {
        // Run sync validation first
        let sync_result = self.sync_schema.validate(value, path);

        match sync_result {
            Validation::Failure(errors) => {
                // If sync fails, return those errors
                Validation::Failure(errors)
            }
            Validation::Success(validated) => {
                // Run async validators
                let mut all_errors = Vec::new();

                for validator in &self.async_validators {
                    let result = validator.validate_async(value, path, env);
                    if let Validation::Failure(errors) = result {
                        all_errors.extend(errors.into_iter());
                    }
                }

                if all_errors.is_empty() {
                    Validation::Success(validated)
                } else {
                    Validation::Failure(SchemaErrors::from_vec(all_errors))
                }
            }
        }
    }

    /// Validates a value with both sync and async validators, running async validators in parallel.
    ///
    /// The validation process:
    /// 1. Runs sync validators first
    /// 2. If sync fails, returns those errors immediately
    /// 3. If sync passes, runs all async validators in parallel using rayon
    /// 4. Accumulates errors from all async validators
    ///
    /// This method is useful when you have multiple independent async validators
    /// that can be executed concurrently for better performance.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use postmortem::Schema;
    /// use postmortem::effect::AsyncStringSchema;
    ///
    /// let schema = AsyncStringSchema::new(Schema::string().min_len(3))
    ///     .async_custom(UniqueEmailValidator::new())
    ///     .async_custom(EmailDomainValidator::new());
    ///
    /// let result = schema.validate_with_env_parallel(&json!("test@example.com"), &JsonPath::root(), &env);
    /// ```
    pub fn validate_with_env_parallel(
        &self,
        value: &Value,
        path: &JsonPath,
        env: &E,
    ) -> Validation<String, SchemaErrors>
    where
        E: Sync,
    {
        // Run sync validation first
        let sync_result = self.sync_schema.validate(value, path);

        match sync_result {
            Validation::Failure(errors) => {
                // If sync fails, return those errors
                Validation::Failure(errors)
            }
            Validation::Success(validated) => {
                // Run async validators in parallel
                let all_errors: Vec<_> = self
                    .async_validators
                    .par_iter()
                    .flat_map(|validator| {
                        let result = validator.validate_async(value, path, env);
                        match result {
                            Validation::Failure(errors) => errors.into_iter().collect::<Vec<_>>(),
                            Validation::Success(_) => Vec::new(),
                        }
                    })
                    .collect();

                if all_errors.is_empty() {
                    Validation::Success(validated)
                } else {
                    Validation::Failure(SchemaErrors::from_vec(all_errors))
                }
            }
        }
    }
}

impl StringSchema {
    /// Converts this string schema into an async schema.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use postmortem::Schema;
    ///
    /// let schema = Schema::string()
    ///     .min_len(3)
    ///     .to_async::<AppEnv>()
    ///     .async_custom(UniqueEmailValidator::new());
    /// ```
    pub fn to_async<E>(self) -> AsyncStringSchema<E> {
        AsyncStringSchema::new(self)
    }

    /// Convenience method to create an async schema with a validator.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use postmortem::Schema;
    ///
    /// let schema = Schema::string()
    ///     .min_len(3)
    ///     .async_custom(UniqueEmailValidator::new());
    /// ```
    pub fn async_custom<E, V>(self, validator: V) -> AsyncStringSchema<E>
    where
        V: AsyncValidator<E> + 'static,
    {
        AsyncStringSchema::new(self).async_custom(validator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SchemaError;
    use crate::Schema;
    use serde_json::json;

    struct TestEnv;

    struct AlwaysFailValidator {
        message: String,
    }

    impl AsyncValidator<TestEnv> for AlwaysFailValidator {
        fn validate_async(
            &self,
            _value: &Value,
            path: &JsonPath,
            _env: &TestEnv,
        ) -> Validation<(), SchemaErrors> {
            Validation::Failure(SchemaErrors::single(SchemaError::new(
                path.clone(),
                self.message.clone(),
            )))
        }
    }

    struct AlwaysPassValidator;

    impl AsyncValidator<TestEnv> for AlwaysPassValidator {
        fn validate_async(
            &self,
            _value: &Value,
            _path: &JsonPath,
            _env: &TestEnv,
        ) -> Validation<(), SchemaErrors> {
            Validation::Success(())
        }
    }

    #[test]
    fn test_async_validator_pass() {
        let schema = Schema::string()
            .min_len(3)
            .async_custom(AlwaysPassValidator);

        let env = TestEnv;
        let result = schema.validate_with_env(&json!("hello"), &JsonPath::root(), &env);

        assert!(result.is_success());
    }

    #[test]
    fn test_async_validator_fail() {
        let schema = Schema::string()
            .min_len(3)
            .async_custom(AlwaysFailValidator {
                message: "async validation failed".to_string(),
            });

        let env = TestEnv;
        let result = schema.validate_with_env(&json!("hello"), &JsonPath::root(), &env);

        assert!(result.is_failure());
    }

    #[test]
    fn test_sync_fail_skips_async() {
        let schema = Schema::string()
            .min_len(10)
            .async_custom(AlwaysPassValidator);

        let env = TestEnv;
        let result = schema.validate_with_env(&json!("hi"), &JsonPath::root(), &env);

        // Should fail on sync validation, not reach async
        assert!(result.is_failure());
    }

    #[test]
    fn test_multiple_async_validators() {
        let schema = Schema::string()
            .min_len(3)
            .async_custom(AlwaysFailValidator {
                message: "first error".to_string(),
            })
            .async_custom(AlwaysFailValidator {
                message: "second error".to_string(),
            });

        let env = TestEnv;
        let result = schema.validate_with_env(&json!("hello"), &JsonPath::root(), &env);

        assert!(result.is_failure());
        if let Validation::Failure(errors) = result {
            assert_eq!(errors.len(), 2);
        }
    }

    #[test]
    fn test_parallel_async_validator_pass() {
        let schema = Schema::string()
            .min_len(3)
            .async_custom(AlwaysPassValidator)
            .async_custom(AlwaysPassValidator);

        let env = TestEnv;
        let result = schema.validate_with_env_parallel(&json!("hello"), &JsonPath::root(), &env);

        assert!(result.is_success());
    }

    #[test]
    fn test_parallel_async_validator_fail() {
        let schema = Schema::string()
            .min_len(3)
            .async_custom(AlwaysFailValidator {
                message: "async validation failed".to_string(),
            });

        let env = TestEnv;
        let result = schema.validate_with_env_parallel(&json!("hello"), &JsonPath::root(), &env);

        assert!(result.is_failure());
    }

    #[test]
    fn test_parallel_sync_fail_skips_async() {
        let schema = Schema::string()
            .min_len(10)
            .async_custom(AlwaysPassValidator);

        let env = TestEnv;
        let result = schema.validate_with_env_parallel(&json!("hi"), &JsonPath::root(), &env);

        // Should fail on sync validation, not reach async
        assert!(result.is_failure());
    }

    #[test]
    fn test_parallel_multiple_async_validators() {
        let schema = Schema::string()
            .min_len(3)
            .async_custom(AlwaysFailValidator {
                message: "first error".to_string(),
            })
            .async_custom(AlwaysFailValidator {
                message: "second error".to_string(),
            })
            .async_custom(AlwaysPassValidator);

        let env = TestEnv;
        let result = schema.validate_with_env_parallel(&json!("hello"), &JsonPath::root(), &env);

        assert!(result.is_failure());
        if let Validation::Failure(errors) = result {
            assert_eq!(errors.len(), 2);
        }
    }
}
