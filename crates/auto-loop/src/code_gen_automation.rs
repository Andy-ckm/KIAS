//! # Code Generation & Testing Automation
//!
//! Automatically generates Rust code from templates, produces unit test skeletons,
//! and runs quality audits on generated code.
//!
//! ## Core components
//!
//! - [`CodeGenerator`] — generates Rust code from parameterized templates
//! - [`TestGenerator`] — creates unit test skeletons from functions/types
//! - [`AuditChecker`] — runs clippy/fmt/rustc checks on generated code
//! - [`GenerationReport`] — summary of generation + audit results

use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, error, warn};

/// Represents a Rust source file to be generated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFile {
    /// Module path, e.g. "src/handlers/agent_handler.rs"
    pub path: String,
    /// Full source code content.
    pub content: String,
}

/// Result of a single code generation step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationStep {
    pub template_name: String,
    pub params: HashMap<String, String>,
    pub output_path: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Summary report for a full code generation run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerationReport {
    pub total_steps: usize,
    pub successful_steps: usize,
    pub failed_steps: usize,
    pub generated_files: Vec<String>,
    pub audit_results: Vec<AuditResult>,
}

/// Result of an audit check on a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResult {
    pub file_path: String,
    pub clippy_ok: bool,
    pub fmt_ok: bool,
    pub rustc_ok: bool,
    pub clippy_warnings: Vec<String>,
    pub fmt_violations: Vec<String>,
    pub rustc_errors: Vec<String>,
}

impl AuditResult {
    pub fn all_passed(&self) -> bool {
        self.clippy_ok && self.fmt_ok && self.rustc_ok
    }
}

/// Code template with placeholders for parameterization.
#[derive(Debug, Clone)]
pub struct CodeTemplate {
    pub name: String,
    /// Full template text with `{{param_name}}` placeholders.
    pub template: String,
}

impl CodeTemplate {
    /// Render the template by substituting all `{{key}}` placeholders.
    pub fn render(&self, params: &HashMap<String, String>) -> String {
        let mut result = self.template.clone();
        for (key, value) in params {
            let placeholder = format!("{{{{{key}}}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }
}

/// Generates Rust source code from parameterized templates.
pub struct CodeGenerator {
    templates: HashMap<String, CodeTemplate>,
    output_dir: PathBuf,
}

impl CodeGenerator {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            templates: HashMap::new(),
            output_dir: output_dir.into(),
        }
    }

    /// Register a named template.
    pub fn register_template(&mut self, name: &str, template: CodeTemplate) {
        self.templates.insert(name.to_string(), template);
    }

    /// Register a template from its string content.
    pub fn add_template(&mut self, name: &str, template_text: &str) {
        self.templates.insert(
            name.to_string(),
            CodeTemplate {
                name: name.to_string(),
                template: template_text.to_string(),
            },
        );
    }

    /// Generate code from a named template with the given parameters.
    pub fn generate(
        &self,
        template_name: &str,
        params: &HashMap<String, String>,
        output_path: &str,
    ) -> KiasResult<SourceFile> {
        let template = self
            .templates
            .get(template_name)
            .ok_or_else(|| KiasError::Config(format!("Template '{template_name}' not found")))?;

        let content = template.render(params);
        Ok(SourceFile {
            path: output_path.to_string(),
            content,
        })
    }

    /// Generate and write files to disk.
    pub fn generate_and_write(
        &self,
        template_name: &str,
        params: &HashMap<String, String>,
        output_path: &str,
    ) -> KiasResult<GenerationStep> {
        match self.generate(template_name, params, output_path) {
            Ok(file) => {
                let full_path = self.output_dir.join(&file.path);
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| KiasError::Io(std::io::Error::other(e.to_string())))?;
                }
                std::fs::write(&full_path, &file.content)
                    .map_err(|e| KiasError::Io(std::io::Error::other(e.to_string())))?;
                debug!("Generated: {}", full_path.display());
                Ok(GenerationStep {
                    template_name: template_name.to_string(),
                    params: params.clone(),
                    output_path: output_path.to_string(),
                    success: true,
                    error: None,
                })
            }
            Err(e) => Ok(GenerationStep {
                template_name: template_name.to_string(),
                params: params.clone(),
                output_path: output_path.to_string(),
                success: false,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Build a standard set of templates for a new Rust module.
    pub fn add_standard_module_templates(&mut self) {
        self.add_template(
            "module_rs",
            r#"//! {{module_description}}
//!
//! {{auto_generated}}

use kias_common::{{KiasError, KiasResult}};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

/// Configuration for {{module_name}}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {{StructName}}Config {
    pub name: String,
    pub enabled: bool,
}

impl Default for {{StructName}}Config {
    fn default() -> Self {
        Self { name: "{{default_name}}".to_string(), enabled: true }
    }
}

/// {{struct_name}} handles {{purpose}}.
#[derive(Debug, Clone)]
pub struct {{StructName}} {
    config: {{StructName}}Config,
}

impl {{StructName}} {
    /// Create a new {{struct_name}} with the given configuration.
    pub fn new(config: {{StructName}}Config) -> Self {
        info!("Initializing {{struct_name}}: {{name}}", name = config.name);
        Self { config }
    }

    /// Returns the name of this {{struct_name}}.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Returns whether this {{struct_name}} is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}
"#,
        );

        self.add_template(
            "lib_rs",
            r#"//! {{crate_description}}
//!
//! {{auto_generated}}

pub mod {{module_name}};
pub use {{module_name}}::{{StructName}};
"#,
        );
    }
}

/// Creates unit test skeletons from Rust functions and types.
pub struct TestGenerator {
    templates: HashMap<String, String>,
}

impl TestGenerator {
    pub fn new() -> Self {
        let mut templates = HashMap::new();
        templates.insert(
            "fn_test".to_string(),
            r#"    #[test]
    fn test_{{test_name}}() {
        // Given: {{given}}
        // When: {{when}}
        // Then: {{then}}
        todo!("implement test")
    }"#
            .to_string(),
        );
        templates.insert(
            "async_fn_test".to_string(),
            r#"    #[tokio::test]
    async fn test_{{test_name}}() {
        // Given: {{given}}
        // When: {{when}}
        // Then: {{then}}
        todo!("implement test")
    }"#
            .to_string(),
        );
        templates.insert(
            "struct_test".to_string(),
            r#"    #[test]
    fn test_{{struct_name}}_new() {
        // Given: {{given}}
        // When: constructing a new {{struct_name}}
        // Then: it should be properly initialized
        let result = {{StructName}}::new({{StructName}}Config::default());
        assert!(result.is_enabled());
    }

    #[test]
    fn test_{{struct_name}}_clone() {
        let original = {{StructName}}::new({{StructName}}Config::default());
        let cloned = original.clone();
        assert_eq!(cloned.name(), original.name());
    }

    #[test]
    fn test_{{struct_name}}_debug() {
        let instance = {{StructName}}::new({{StructName}}Config::default());
        let debug_str = format!("{:?}", instance);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_{{struct_name}}_enabled_flag() {
        let disabled = {{StructName}}::new({{StructName}}Config { name: "test".into(), enabled: false });
        assert!(!disabled.is_enabled());
    }
}"#
            .to_string(),
        );
        Self { templates }
    }

    /// Generate a test for a synchronous function.
    pub fn generate_fn_test(&self, fn_name: &str, given: &str, when: &str, then: &str) -> String {
        let mut params = HashMap::new();
        params.insert("test_name".to_string(), fn_name.to_string());
        params.insert("given".to_string(), given.to_string());
        params.insert("when".to_string(), when.to_string());
        params.insert("then".to_string(), then.to_string());
        self.render_template("fn_test", &params)
    }

    /// Generate a test for an async function.
    pub fn generate_async_fn_test(
        &self,
        fn_name: &str,
        given: &str,
        when: &str,
        then: &str,
    ) -> String {
        let mut params = HashMap::new();
        params.insert("test_name".to_string(), fn_name.to_string());
        params.insert("given".to_string(), given.to_string());
        params.insert("when".to_string(), when.to_string());
        params.insert("then".to_string(), then.to_string());
        self.render_template("async_fn_test", &params)
    }

    /// Generate a struct test suite.
    pub fn generate_struct_tests(&self, struct_name: &str, given: &str) -> String {
        let mut params = HashMap::new();
        params.insert("struct_name".to_string(), struct_name.to_string());
        params.insert("StructName".to_string(), Self::to_pascal_case(struct_name));
        params.insert("given".to_string(), given.to_string());
        self.render_template("struct_test", &params)
    }

    /// Generate a full test module for a struct.
    pub fn generate_struct_test_module(&self, struct_name: &str, given: &str) -> String {
        let tests = self.generate_struct_tests(struct_name, given);
        format!(
            r#"#[cfg(test)]
mod {struct_name}_tests {{
use super::*;

{tests}
}}"#
        )
    }

    fn render_template(&self, name: &str, params: &HashMap<String, String>) -> String {
        let template = self.templates.get(name).cloned().unwrap_or_else(|| {
            warn!("Template '{name}' not found, using empty string");
            String::new()
        });
        let mut result = template;
        for (key, value) in params {
            let placeholder = format!("{{{{{key}}}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }

    fn to_pascal_case(s: &str) -> String {
        let mut result = String::new();
        for part in s.split('_') {
            for (i, c) in part.chars().enumerate() {
                result.push(if i == 0 { c.to_ascii_uppercase() } else { c });
            }
        }
        result
    }
}

impl Default for TestGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs clippy/fmt/rustc quality checks on generated Rust code.
pub struct AuditChecker {
    workspace_root: PathBuf,
}

impl AuditChecker {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// Audit a single generated file.
    pub fn audit_file(&self, file_path: &str) -> KiasResult<AuditResult> {
        let full_path = self.workspace_root.join(file_path);
        let result = self.run_checks(&full_path)?;
        Ok(result)
    }

    /// Audit multiple generated files.
    pub fn audit_files(&self, file_paths: &[String]) -> Vec<AuditResult> {
        file_paths
            .iter()
            .filter_map(|f| self.audit_file(f).ok())
            .collect()
    }

    fn run_checks(&self, file_path: &Path) -> KiasResult<AuditResult> {
        let file_str = file_path.to_string_lossy().to_string();

        // Run rustc --emit=metadata for type checking
        let rustc_errors = self.run_rustc(file_path);

        // Run clippy
        let (clippy_ok, clippy_warnings) = self.run_clippy(file_path);

        // Run fmt check
        let (fmt_ok, fmt_violations) = self.run_fmt_check(file_path);

        Ok(AuditResult {
            file_path: file_str,
            clippy_ok,
            fmt_ok,
            rustc_ok: rustc_errors.is_empty(),
            clippy_warnings,
            fmt_violations,
            rustc_errors,
        })
    }

    fn run_rustc(&self, file_path: &Path) -> Vec<String> {
        let output = Command::new("rustc")
            .args(["--edition", "2021", "--emit=metadata", "-o", "/dev/null"])
            .arg(file_path)
            .output();

        match output {
            Ok(out) if out.status.success() => Vec::new(),
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                stderr.lines().map(|s| s.to_string()).collect()
            }
            Err(e) => vec![format!("rustc failed to run: {e}")],
        }
    }

    fn run_clippy(&self, file_path: &Path) -> (bool, Vec<String>) {
        let parent = file_path.parent().unwrap_or(Path::new("."));
        let file_name = file_path.file_name().unwrap_or_default();
        let output = Command::new("cargo")
            .args([
                "clippy",
                "--",
                "-W",
                "clippy::all",
                "-A",
                "clippy::derivable_impls",
            ])
            .current_dir(parent)
            .output();

        match output {
            Ok(out) if out.status.success() => (true, Vec::new()),
            Ok(out) => {
                let combined = String::from_utf8_lossy(&out.stderr);
                let warnings: Vec<String> = combined
                    .lines()
                    .filter(|l| l.contains(file_name.to_str().unwrap_or("")))
                    .map(|s| s.to_string())
                    .collect();
                (false, warnings)
            }
            Err(e) => (false, vec![format!("clippy failed to run: {e}")]),
        }
    }

    fn run_fmt_check(&self, file_path: &Path) -> (bool, Vec<String>) {
        let output = Command::new("rustfmt")
            .args(["--check", "--edition", "2021"])
            .arg(file_path)
            .output();

        match output {
            Ok(out) if out.status.success() => (true, Vec::new()),
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let violations: Vec<String> = stderr
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                (false, violations)
            }
            Err(e) => (false, vec![format!("rustfmt failed to run: {e}")]),
        }
    }
}

/// Full pipeline: generate code, write files, run audits.
pub struct CodeGenPipeline {
    generator: CodeGenerator,
    audit_checker: AuditChecker,
}

impl CodeGenPipeline {
    pub fn new(workspace_root: impl Into<PathBuf>, output_dir: impl Into<PathBuf>) -> Self {
        Self {
            generator: CodeGenerator::new(output_dir),
            audit_checker: AuditChecker::new(workspace_root),
        }
    }

    /// Run the full pipeline: generate + audit for a list of generation steps.
    pub fn run(&self, steps: Vec<(String, HashMap<String, String>, String)>) -> GenerationReport {
        let mut report = GenerationReport::default();

        for (template_name, params, output_path) in steps {
            let step = self
                .generator
                .generate_and_write(&template_name, &params, &output_path);
            match step {
                Ok(s) if s.success => {
                    report.successful_steps += 1;
                    report.generated_files.push(output_path.clone());
                    // Audit the generated file
                    if let Ok(audit_result) = self.audit_checker.audit_file(&output_path) {
                        report.audit_results.push(audit_result);
                    }
                }
                Ok(s) => {
                    report.failed_steps += 1;
                    error!(
                        "Generation step '{}' failed: {:?}",
                        s.template_name, s.error
                    );
                }
                Err(e) => {
                    report.failed_steps += 1;
                    error!("Generation step '{template_name}' error: {e}");
                }
            }
            report.total_steps += 1;
        }

        report
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_template_render() {
        let tmpl = CodeTemplate {
            name: "test".to_string(),
            template: "Hello, {{name}}! You have {{count}} messages.".to_string(),
        };
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Alice".to_string());
        params.insert("count".to_string(), "5".to_string());
        let rendered = tmpl.render(&params);
        assert_eq!(rendered, "Hello, Alice! You have 5 messages.");
    }

    #[test]
    fn test_code_template_missing_param() {
        let tmpl = CodeTemplate {
            name: "test".to_string(),
            template: "Hello, {{name}}! Age: {{age}}".to_string(),
        };
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Bob".to_string());
        let rendered = tmpl.render(&params);
        assert_eq!(rendered, "Hello, Bob! Age: {{age}}");
    }

    #[test]
    fn test_test_generator_fn_test() {
        let gen = TestGenerator::new();
        let test =
            gen.generate_fn_test("add_numbers", "two numbers", "adding them", "sum returned");
        assert!(test.contains("fn test_add_numbers()"));
        assert!(test.contains("Given: two numbers"));
    }

    #[test]
    fn test_test_generator_async_fn_test() {
        let gen = TestGenerator::new();
        let test =
            gen.generate_async_fn_test("fetch_data", "API endpoint", "calling it", "data returned");
        assert!(test.contains("async fn test_fetch_data()"));
        assert!(test.contains("#[tokio::test]"));
    }

    #[test]
    fn test_test_generator_struct_tests() {
        let gen = TestGenerator::new();
        let tests = gen.generate_struct_tests("my_handler", "a valid config");
        assert!(tests.contains("fn test_my_handler_new()"));
        assert!(tests.contains("fn test_my_handler_clone()"));
    }

    #[test]
    fn test_test_generator_to_pascal_case() {
        let gen = TestGenerator::new();
        assert_eq!(
            gen.generate_struct_tests("foo_bar", ""),
            gen.generate_struct_tests("foo_bar", "")
        );
        // Test the helper via struct tests
        let tests = gen.generate_struct_tests("agent_controller", "config");
        assert!(tests.contains("AgentController"));
    }

    #[test]
    fn test_code_generator_registers_and_generates() {
        let mut gen = CodeGenerator::new("/tmp");
        gen.add_template("hello", "pub fn {{fn_name}}() {{}}");
        let mut params = HashMap::new();
        params.insert("fn_name".to_string(), "greet".to_string());
        let file = gen.generate("hello", &params, "greet.rs").unwrap();
        assert!(file.content.contains("pub fn greet()"));
    }

    #[test]
    fn test_code_generator_unknown_template() {
        let gen = CodeGenerator::new("/tmp");
        let result = gen.generate("nonexistent", &HashMap::new(), "out.rs");
        assert!(result.is_err());
    }

    #[test]
    fn test_audit_result_all_passed() {
        let r = AuditResult {
            file_path: "test.rs".to_string(),
            clippy_ok: true,
            fmt_ok: true,
            rustc_ok: true,
            clippy_warnings: Vec::new(),
            fmt_violations: Vec::new(),
            rustc_errors: Vec::new(),
        };
        assert!(r.all_passed());
    }

    #[test]
    fn test_audit_result_failure() {
        let r = AuditResult {
            file_path: "test.rs".to_string(),
            clippy_ok: false,
            fmt_ok: true,
            rustc_ok: true,
            clippy_warnings: vec!["warning: useless conversion".to_string()],
            fmt_violations: Vec::new(),
            rustc_errors: Vec::new(),
        };
        assert!(!r.all_passed());
    }

    #[test]
    fn test_generation_report_default() {
        let report = GenerationReport::default();
        assert_eq!(report.total_steps, 0);
        assert_eq!(report.successful_steps, 0);
        assert!(report.generated_files.is_empty());
    }

    #[test]
    fn test_code_gen_pipeline_creation() {
        let pipeline = CodeGenPipeline::new("/workspace", "/tmp/output");
        let report = pipeline.run(Vec::new());
        assert_eq!(report.total_steps, 0);
    }

    #[test]
    fn test_source_file_content() {
        let file = SourceFile {
            path: "src/foo.rs".to_string(),
            content: "pub fn foo() {}".to_string(),
        };
        assert_eq!(file.path, "src/foo.rs");
        assert!(file.content.contains("pub fn foo()"));
    }
}
