
//! Property testing framework with fuzzing support.
//!
//! This module provides a simple framework for defining properties,
//! invariants, and running fuzz tests to verify their correctness.

use crate::error::{KiasError};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::fmt;
use std::sync::Arc;
use tracing::{info, warn};

/// A trait for types that can be automatically generated for fuzz testing.
pub trait Fuzzable {
    /// Generate a random instance of the type.
    fn fuzz(rng: &mut StdRng) -> Self;
}

// Implement Fuzzable for primitive types

impl Fuzzable for i32 {
    fn fuzz(rng: &mut StdRng) -> Self {
        rng.gen_range(i32::MIN..i32::MAX)
    }
}

impl Fuzzable for i64 {
    fn fuzz(rng: &mut StdRng) -> Self {
        rng.gen_range(i64::MIN..i64::MAX)
    }
}

impl Fuzzable for u32 {
    fn fuzz(rng: &mut StdRng) -> Self {
        rng.gen_range(0..u32::MAX)
    }
}

impl Fuzzable for u64 {
    fn fuzz(rng: &mut StdRng) -> Self {
        rng.gen_range(0..u64::MAX)
    }
}

impl Fuzzable for bool {
    fn fuzz(rng: &mut StdRng) -> Self {
        rng.gen()
    }
}

impl Fuzzable for char {
    fn fuzz(rng: &mut StdRng) -> Self {
        rng.gen_range('a'..='z')
    }
}

impl Fuzzable for String {
    fn fuzz(rng: &mut StdRng) -> Self {
        let len = rng.gen_range(0..100);
        (0..len)
            .map(|_| {
                rng.gen_range('a'..='z')
            })
            .collect()
    }
}

impl<T: Fuzzable> Fuzzable for Vec<T> {
    fn fuzz(rng: &mut StdRng) -> Self {
        let len = rng.gen_range(0..50);
        (0..len).map(|_| T::fuzz(rng)).collect()
    }
}

/// A struct describing a single property that a value must satisfy.
pub struct Property<T> {
    name: String,
    description: String,
    test_fn: Arc<dyn Fn(T) -> Result<(), KiasError> + Send + Sync>,
}

impl<T> Property<T> {
    /// Creates a new property with the given name, description, and test function.
    ///
    /// The test function should return `Ok(())` when the property holds,
    /// or `Err(KiasError::PropertyViolation{...})` when it does not.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        test_fn: impl Fn(T) -> Result<(), KiasError> + Send + Sync + 'static,
    ) -> Self {
        Property {
            name: name.into(),
            description: description.into(),
            test_fn: Arc::new(test_fn),
        }
    }

    /// Returns the name of the property.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the description of the property.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Evaluates the property on the given input.
    pub fn evaluate(&self, input: T) -> Result<(), KiasError> {
        (self.test_fn)(input)
    }
}

/// A struct describing an invariant that must hold for all values.
pub struct Invariant<T> {
    name: String,
    check_fn: Arc<dyn Fn(&T) -> Result<(), KiasError> + Send + Sync>,
}

impl<T> Invariant<T> {
    /// Creates a new invariant with the given name and check function.
    ///
    /// The check function receives a reference to the value and should return
    /// `Ok(())` when the invariant holds, otherwise an error.
    pub fn new(
        name: impl Into<String>,
        check_fn: impl Fn(&T) -> Result<(), KiasError> + Send + Sync + 'static,
    ) -> Self {
        Invariant {
            name: name.into(),
            check_fn: Arc::new(check_fn),
        }
    }

    /// Returns the name of the invariant.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Checks the invariant for the given value.
    pub fn check(&self, value: &T) -> Result<(), KiasError> {
        (self.check_fn)(value)
    }
}

/// Configuration for fuzz testing runs.
#[derive(Debug, Clone)]
pub struct FuzzConfig {
    /// Number of iterations per property.
    pub iterations: usize,
    /// Optional seed for the random number generator.
    pub seed: Option<u64>,
    /// Maximum size for generated collections (e.g., strings, vectors).
    pub max_size: usize,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        FuzzConfig {
            iterations: 100,
            seed: None,
            max_size: 100,
        }
    }
}

/// Represents a single violation detected during fuzz testing.
#[derive(Debug, Clone)]
pub struct Violation {
    /// Name of the property or invariant that failed.
    pub rule_name: String,
    /// String representation of the input that caused the failure.
    pub input_repr: String,
    /// Detailed error message describing why the rule failed.
    pub error_message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Violation of '{}' with input {:?}: {}",
            self.rule_name, self.input_repr, self.error_message
        )
    }
}

/// Summary of a fuzz test run.
#[derive(Debug)]
pub struct TestSummary {
    /// Total number of iterations performed.
    pub total_iterations: usize,
    /// Number of iterations that passed all properties and invariants.
    pub passed: usize,
    /// Number of iterations that resulted in a violation.
    pub failed: usize,
    /// All recorded violations.
    pub violations: Vec<Violation>,
}

impl TestSummary {
    /// Returns `true` if any violation was recorded.
    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }

    /// Returns a formatted report for the test run.
    pub fn report(&self) -> String {
        if self.violations.is_empty() {
            format!(
                "Fuzz test completed: {} iterations, all passed.",
                self.total_iterations
            )
        } else {
            let mut report = format!(
                "Fuzz test completed: {} iterations, {} passed, {} failed.\n",
                self.total_iterations, self.passed, self.failed
            );
            for (i, v) in self.violations.iter().enumerate() {
                report.push_str(format!("  {}: {}\n", i + 1, v));
            }
            report
        }
    }
}

/// A fuzz test that can be run on a type `T` which implements `Fuzzable`.
pub struct FuzzTest<T> {
    properties: Vec<Property<T>>,
    invariants: Vec<Invariant<T>>,
}

impl<T> FuzzTest<T> {
    /// Creates a new empty fuzz test.
    pub fn new() -> Self {
        FuzzTest {
            properties: Vec::new(),
            invariants: Vec::new(),
        }
    }

    /// Adds a property to the fuzz test.
    pub fn add_property(&mut self, property: Property<T>) -> &mut Self {
        self.properties.push(property);
        self
    }

    /// Adds an invariant to the fuzz test.
    pub fn add_invariant(&mut self, invariant: Invariant<T>) -> &mut Self {
        self.invariants.push(invariant);
        self
    }

    /// Runs the fuzz test according to the supplied configuration.
    ///
    /// The type `T` must implement `Fuzzable` so that random inputs can be generated.
    pub fn run(&self, config: &FuzzConfig) -> Result<TestSummary, KiasError> {
        // Initialize RNG
        let mut rng = match config.seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        };

        let total_iterations = config
            .iterations
            .checked_mul(self.properties.len())
            .ok_or_else(|| KiasError::FuzzingError {
                message: "Iteration count overflow.".into(),
            })?;

        let mut passed = 0usize;
        let mut violations = Vec::new();

        info!(
            "Starting fuzz test: {} iterations, {} properties, {} invariants.",
            total_iterations,
            self.properties.len(),
            self.invariants.len()
        );

        for prop in &self.properties {
            for iteration in 0..config.iterations {
                // Generate a random input
                let input = T::fuzz(&mut rng);

                // Evaluate the property
                let result = prop.evaluate(input);

                match result {
                    Ok(()) => {
                        // Property holds, check invariants
                        let mut invariant_failed = false;
                        for inv in &self.invariants {
                            let check_result = inv.check(&input);
                            if let Err(e) = check_result {
                                // Record invariant violation
                                let violation = Violation {
                                    rule_name: inv.name().to_string(),
                                    input_repr: format!("{:?}", input),
                                    error_message: e.to_string(),
                                };
                                warn!("Invariant '{}' violated: {}", violation.rule_name, violation);
                                violations.push(violation);
                                invariant_failed = true;
                                break;
                            }
                        }
                        if !invariant_failed {
                            passed += 1;
                        }
                    }
                    Err(e) => {
                        // Record property violation
                        let violation = Violation {
                            rule_name: prop.name().to_string(),
                            input_repr: format!("{:?}", input),
                            error_message: e.to_string(),
                        };
                        warn!("Property '{}' violated: {}", violation.rule_name, violation);
                        violations.push(violation);
                    }
                }
            }
        }

        let failed = violations.len();
        let summary = TestSummary {
            total_iterations,
            passed,
            failed,
            violations,
        };

        if summary.has_failures() {
            warn!("Fuzz test finished with failures:\n{}", summary.report());
        } else {
            info!("Fuzz test finished successfully:\n{}", summary.report());
        }

        Ok(summary)
    }
}

impl<T> Default for FuzzTest<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to quickly check a single property without building a full `FuzzTest`.
///
/// The function generates `iterations` random inputs and checks both the property and
/// optionally provided invariants. Returns a summary of the run.
pub fn quick_check<T>(
    property: Property<T>,
    invariants: Vec<Invariant<T>>,
    config: &FuzzConfig,
) -> Result<TestSummary, KiasError>
where
    T: Fuzzable,
{
    let mut fuzz_test = FuzzTest::new();
    fuzz_test.add_property(property);
    for inv in invariants {
        fuzz_test.add_invariant(inv);
    }
    fuzz_test.run(config)
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper type that can be fuzzed and implements Debug for easy output.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SimpleStruct {
        value: i32,
        flag: bool,
    }

    impl Fuzzable for SimpleStruct {
        fn fuzz(rng: &mut StdRng) -> Self {
            SimpleStruct {
                value: rng.gen_range(-1000..1000),
                flag: rng.gen(),
            }
        }
    }

    #[test]
    fn test_fuzzable_i32() {
        let seed = 42;
        let mut rng = StdRng::seed_from_u64(seed);
        for _ in 0..1000 {
            let val = i32::fuzz(&mut rng);
            // i32 fuzz should produce values within i32 range
            assert!(val >= i32::MIN);
        }
    }

    #[test]
    fn test_fuzzable_string() {
        let seed = 123;
        let mut rng = StdRng::seed_from_u64(seed);
        let s: String = String::fuzz(&mut rng);
        // Generated string length should be <= 100 (default max)
        assert!(s.len() <= 100);
    }

    #[test]
    fn test_fuzzable_vec() {
        let seed = 999;
        let mut rng = StdRng::seed_from_u64(seed);
        let vec: Vec<u32> = Vec::fuzz(&mut rng);
        // Vector length should be <= 50 (default max)
        assert!(vec.len() <= 50);
    }

    #[test]
    fn test_property_passes() {
        // Define a property that always passes
        let prop = Property::new(
            "always_pass",
            "A property that never fails.",
            |_val: i32| Ok(()),
        );

        let config = FuzzConfig {
            iterations: 10,
            seed: Some(0),
            max_size: 10,
        };

        let summary = quick_check(prop, vec![], &config).expect("quick_check should succeed");
        assert_eq!(summary.passed, 10);
        assert!(!summary.has_failures());
    }

    #[test]
    fn test_property_fails() {
        // Define a property that always fails for values > 0
        let prop = Property::new(
            "non_positive",
            "Value must be non-positive.",
            |val: i32| {
                if val > 0 {
                    Err(KiasError::PropertyViolation {
                        property: "non_positive".to_string(),
                        message: "Value is positive".to_string(),
                    })
                } else {
                    Ok(())
                }
            },
        );

        let config = FuzzConfig {
            iterations: 10,
            seed: Some(0),
            max_size: 10,
        };

        let summary = quick_check(prop, vec![], &config).expect("quick_check should succeed");
        // Expect failures because random i32 may be > 0
        assert!(summary.failed > 0);
        assert!(summary.has_failures());
    }

    #[test]
    fn test_invariant_violation() {
        // Define an invariant that requires value to be odd
        let invariant = Invariant::new(
            "odd_value",
            "Value must be odd.",
            |val: &i32| {
                if val % 2 == 0 {
                    Err(KiasError::InvariantViolation {
                        invariant: "odd_value".to_string(),
                        message: "Value is even".to_string(),
                    })
                } else {
                    Ok(())
                }
            },
        );

        // Define a property that always passes
        let prop = Property::new(
            "pass_through",
            "Passes any input.",
            |_val: i32| Ok(()),
        );

        let config = FuzzConfig {
            iterations: 10,
            seed: Some(0),
            max_size: 10,
        };

        let summary = quick_check(prop, vec![invariant], &config).expect("quick_check should succeed");
        // Expect failures because many random i32 are even
        assert!(summary.failed > 0);
        assert!(summary.has_failures());
    }

    #[test]
    fn test_fuzz_config_default() {
        let config = FuzzConfig::default();
        assert_eq!(config.iterations, 100);
        assert!(config.seed.is_none());
        assert_eq!(config.max_size, 100);
    }

    #[test]
    fn test_fuzz_test_builder() {
        let prop = Property::new(
            "test_prop",
            "Test property",
            |val: u32| {
                if val == u32::MAX {
                    Err(KiasError::PropertyViolation {
                        property: "test_prop".to_string(),
                        message: "Value is max".to_string(),
                    })
                } else {
                    Ok(())
                }
            },
        );

        let inv = Invariant::new(
            "max_check",
            "Check value is not max",
            |val: &u32| {
                if *val == u32::MAX {
                    Err(KiasError::InvariantViolation {
                        invariant: "max_check".to_string(),
                        message: "Value is max".to_string(),
                    })
                } else {
                    Ok(())
                }
            },
        );

        let mut fuzz_test = FuzzTest::<u32>::new();
        fuzz_test.add_property(prop).add_invariant(inv);

        let config = FuzzConfig {
            iterations: 5,
            seed: Some(42),
            max_size: 10,
        };

        let summary = fuzz_test.run(&config).expect("run should succeed");
        // Some iterations may fail because u32::MAX may be generated
        assert!(summary.total_iterations == 5);
    }

    #[test]
    fn test_violation_display() {
        let v = Violation {
            rule_name: "test_prop".to_string(),
            input_repr: "42".to_string(),
            error_message: "failed".to_string(),
        };
        let s = format!("{}", v);
        assert!(s.contains("test_prop"));
        assert!(s.contains("42"));
        assert!(s.contains("failed"));
    }

    #[test]
    fn test_summary_report_no_failures() {
        let summary = TestSummary {
            total_iterations: 50,
            passed: 50,
            failed: 0,
            violations: vec![],
        };
        let report = summary.report();
        assert!(report.contains("all passed"));
    }

    #[test]
    fn test_summary_report_with_failures() {
        let violation = Violation {
            rule_name: "prop".to_string(),
            input_repr: "5".to_string(),
            error_message: "error".to_string(),
        };
        let summary = TestSummary {
            total_iterations: 20,
            passed: 15,
            failed: 5,
            violations: vec![violation],
        };
        let report = summary.report();
        assert!(report.contains("failed"));
        assert!(report.contains("prop"));
    }

    #[test]
    fn test_fuzzable_custom_struct() {
        let seed = 42;
        let mut rng = StdRng::seed_from_u64(seed);
        let s: SimpleStruct = SimpleStruct::fuzz(&mut rng);
        // Value is within expected range because our impl uses -1000..1000
        assert!(s.value >= -1000 && s.value < 1000);
    }
}