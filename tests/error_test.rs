//! Integration tests for SchemaError and SchemaErrors.

use postmortem::{JsonPath, SchemaError, SchemaErrors, ValidationResult};
use stillwater::prelude::*;
use stillwater::Validation;

#[test]
fn test_schema_error_full_context() {
    let error = SchemaError::new(JsonPath::root().push_field("email"), "invalid email format")
        .with_code("invalid_email")
        .with_got("not-an-email")
        .with_expected("valid email address");

    assert_eq!(error.path.to_string(), "email");
    assert_eq!(error.message, "invalid email format");
    assert_eq!(error.code, "invalid_email");
    assert_eq!(error.got, Some("not-an-email".to_string()));
    assert_eq!(error.expected, Some("valid email address".to_string()));
}

#[test]
fn test_schema_errors_never_empty() {
    let error = SchemaError::new(JsonPath::root(), "test error");
    let errors = SchemaErrors::single(error);

    // is_empty always returns false for SchemaErrors (guarantees at least one error)
    assert!(!errors.is_empty());
    assert_eq!(errors.len(), 1);
}

#[test]
fn test_errors_combine_via_semigroup() {
    let e1 = SchemaErrors::single(SchemaError::new(
        JsonPath::root().push_field("name"),
        "name is required",
    ));
    let e2 = SchemaErrors::single(SchemaError::new(
        JsonPath::root().push_field("email"),
        "email is invalid",
    ));
    let e3 = SchemaErrors::single(SchemaError::new(
        JsonPath::root().push_field("age"),
        "age must be positive",
    ));

    let combined = e1.combine(e2).combine(e3);

    assert_eq!(combined.len(), 3);

    let messages: Vec<&str> = combined.iter().map(|e| e.message.as_str()).collect();
    assert!(messages.contains(&"name is required"));
    assert!(messages.contains(&"email is invalid"));
    assert!(messages.contains(&"age must be positive"));
}

#[test]
fn test_validation_success() {
    let result: ValidationResult<i32> = Validation::Success(42);

    match result {
        Validation::Success(v) => assert_eq!(v, 42),
        Validation::Failure(_) => panic!("Expected success"),
    }
}

#[test]
fn test_validation_failure() {
    let errors = SchemaErrors::single(SchemaError::new(JsonPath::root(), "invalid"));
    let result: ValidationResult<i32> = Validation::Failure(errors);

    match result {
        Validation::Success(_) => panic!("Expected failure"),
        Validation::Failure(e) => assert_eq!(e.len(), 1),
    }
}

#[test]
fn test_validation_and_accumulates_errors() {
    // Two failing validations
    let v1: ValidationResult<i32> = Validation::Failure(SchemaErrors::single(SchemaError::new(
        JsonPath::root().push_field("a"),
        "error a",
    )));
    let v2: ValidationResult<i32> = Validation::Failure(SchemaErrors::single(SchemaError::new(
        JsonPath::root().push_field("b"),
        "error b",
    )));

    // Combine with .and() - should accumulate both errors
    let combined = v1.and(v2);

    match combined {
        Validation::Failure(errors) => {
            assert_eq!(errors.len(), 2);
            let paths: Vec<String> = errors.iter().map(|e| e.path.to_string()).collect();
            assert!(paths.contains(&"a".to_string()));
            assert!(paths.contains(&"b".to_string()));
        }
        Validation::Success(_) => panic!("Expected failure"),
    }
}

#[test]
fn test_validation_map() {
    let result: ValidationResult<i32> = Validation::Success(10);
    let mapped = result.map(|x| x * 2);

    match mapped {
        Validation::Success(v) => assert_eq!(v, 20),
        Validation::Failure(_) => panic!("Expected success"),
    }
}

#[test]
fn test_validation_map_on_failure() {
    let errors = SchemaErrors::single(SchemaError::new(JsonPath::root(), "error"));
    let result: ValidationResult<i32> = Validation::Failure(errors);
    let mapped = result.map(|x| x * 2);

    match mapped {
        Validation::Success(_) => panic!("Expected failure"),
        Validation::Failure(e) => assert_eq!(e.len(), 1),
    }
}

#[test]
fn test_validation_and_then_short_circuits() {
    // and_then is fail-fast, not applicative
    let v1: ValidationResult<i32> = Validation::Failure(SchemaErrors::single(SchemaError::new(
        JsonPath::root().push_field("first"),
        "first error",
    )));

    // This closure should never be called because v1 is already a failure
    let result = v1.and_then(|_| -> ValidationResult<i32> {
        Validation::Failure(SchemaErrors::single(SchemaError::new(
            JsonPath::root().push_field("second"),
            "second error",
        )))
    });

    match result {
        Validation::Failure(errors) => {
            // Only the first error, not both
            assert_eq!(errors.len(), 1);
            assert_eq!(errors.first().path.to_string(), "first");
        }
        Validation::Success(_) => panic!("Expected failure"),
    }
}

#[test]
fn test_query_errors_by_path() {
    let path_email = JsonPath::root().push_field("email");
    let path_name = JsonPath::root().push_field("name");

    let errors = SchemaErrors::single(
        SchemaError::new(path_email.clone(), "invalid format").with_code("format"),
    )
    .combine(SchemaErrors::single(
        SchemaError::new(path_email.clone(), "domain blocked").with_code("blocked"),
    ))
    .combine(SchemaErrors::single(
        SchemaError::new(path_name.clone(), "required").with_code("required"),
    ));

    let email_errors = errors.at_path(&path_email);
    assert_eq!(email_errors.len(), 2);

    let name_errors = errors.at_path(&path_name);
    assert_eq!(name_errors.len(), 1);
}

#[test]
fn test_query_errors_by_code() {
    let errors = SchemaErrors::single(
        SchemaError::new(JsonPath::root().push_field("a"), "error").with_code("required"),
    )
    .combine(SchemaErrors::single(
        SchemaError::new(JsonPath::root().push_field("b"), "error").with_code("format"),
    ))
    .combine(SchemaErrors::single(
        SchemaError::new(JsonPath::root().push_field("c"), "error").with_code("required"),
    ));

    let required = errors.with_code("required");
    assert_eq!(required.len(), 2);

    let format = errors.with_code("format");
    assert_eq!(format.len(), 1);

    let nonexistent = errors.with_code("nonexistent");
    assert_eq!(nonexistent.len(), 0);
}

#[test]
fn test_errors_into_vec() {
    let e1 = SchemaError::new(JsonPath::root().push_field("a"), "error a");
    let e2 = SchemaError::new(JsonPath::root().push_field("b"), "error b");

    let errors = SchemaErrors::single(e1).combine(SchemaErrors::single(e2));
    let vec = errors.into_vec();

    assert_eq!(vec.len(), 2);
}

#[test]
fn test_schema_error_display() {
    let error = SchemaError::new(
        JsonPath::root()
            .push_field("users")
            .push_index(0)
            .push_field("age"),
        "must be positive",
    )
    .with_expected("positive integer")
    .with_got("-5");

    let display = error.to_string();
    assert!(display.contains("users[0].age"));
    assert!(display.contains("must be positive"));
    assert!(display.contains("expected: positive integer"));
    assert!(display.contains("got: -5"));
}

#[test]
fn test_schema_errors_display() {
    let errors = SchemaErrors::single(SchemaError::new(
        JsonPath::root().push_field("name"),
        "required",
    ))
    .combine(SchemaErrors::single(SchemaError::new(
        JsonPath::root().push_field("email"),
        "invalid",
    )));

    let display = errors.to_string();
    assert!(display.contains("2 error(s)"));
    assert!(display.contains("1. name: required"));
    assert!(display.contains("2. email: invalid"));
}

#[test]
fn test_complex_validation_scenario() {
    // Simulating validation of a user registration form
    fn validate_name(name: &str) -> ValidationResult<String> {
        if name.is_empty() {
            Validation::Failure(SchemaErrors::single(
                SchemaError::new(JsonPath::root().push_field("name"), "name is required")
                    .with_code("required"),
            ))
        } else {
            Validation::Success(name.to_string())
        }
    }

    fn validate_email(email: &str) -> ValidationResult<String> {
        if !email.contains('@') {
            Validation::Failure(SchemaErrors::single(
                SchemaError::new(JsonPath::root().push_field("email"), "invalid email format")
                    .with_code("invalid_email")
                    .with_got(email)
                    .with_expected("valid email address"),
            ))
        } else {
            Validation::Success(email.to_string())
        }
    }

    fn validate_age(age: i32) -> ValidationResult<i32> {
        if age < 0 {
            Validation::Failure(SchemaErrors::single(
                SchemaError::new(
                    JsonPath::root().push_field("age"),
                    "age must be non-negative",
                )
                .with_code("min_value")
                .with_got(age.to_string())
                .with_expected("value >= 0"),
            ))
        } else if age > 150 {
            Validation::Failure(SchemaErrors::single(
                SchemaError::new(JsonPath::root().push_field("age"), "age must be realistic")
                    .with_code("max_value")
                    .with_got(age.to_string())
                    .with_expected("value <= 150"),
            ))
        } else {
            Validation::Success(age)
        }
    }

    // All invalid inputs
    let name_result = validate_name("");
    let email_result = validate_email("not-an-email");
    let age_result = validate_age(-5);

    // Combine all validations - should accumulate all errors
    let combined = name_result
        .and(email_result)
        .and(age_result)
        .map(|_| "valid user");

    match combined {
        Validation::Failure(errors) => {
            assert_eq!(errors.len(), 3);

            // Check we can find errors by code
            assert_eq!(errors.with_code("required").len(), 1);
            assert_eq!(errors.with_code("invalid_email").len(), 1);
            assert_eq!(errors.with_code("min_value").len(), 1);
        }
        Validation::Success(_) => panic!("Expected validation to fail"),
    }
}
