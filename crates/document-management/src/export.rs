//! Document import/export — PDF, Word, Markdown.
//!
//! Supports reading and writing documents in multiple formats.

use serde::{Deserialize, Serialize};

/// Supported export formats.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ExportFormat {
    Markdown,
    Pdf,
    Html,
    Json,
    Csv,
}

/// Supported import formats.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ImportFormat {
    Markdown,
    PlainText,
    Json,
    Csv,
    Html,
}

/// Metadata for exported documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMetadata {
    pub title: String,
    pub author: String,
    pub created_at: String,
    pub version: String,
    pub classification: Option<String>,
    pub tags: Vec<String>,
}

/// Result of an import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub content: String,
    pub title: Option<String>,
    pub detected_format: ImportFormat,
    pub metadata: std::collections::HashMap<String, String>,
    pub warnings: Vec<String>,
}

/// Document import/export handler.
pub struct DocumentExporter;

impl DocumentExporter {
    /// Export a document to Markdown format.
    pub fn to_markdown(content: &str, meta: &ExportMetadata) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {}\n\n", meta.title));
        md.push_str(&format!("**Author:** {}  \n", meta.author));
        md.push_str(&format!("**Version:** {}  \n", meta.version));
        md.push_str(&format!("**Created:** {}  \n", meta.created_at));
        if let Some(ref cls) = meta.classification {
            md.push_str(&format!("**Classification:** {}  \n", cls));
        }
        if !meta.tags.is_empty() {
            md.push_str(&format!("**Tags:** {}  \n", meta.tags.join(", ")));
        }
        md.push_str("\n---\n\n");
        md.push_str(content);
        md
    }

    /// Export a document to HTML format.
    pub fn to_html(content: &str, meta: &ExportMetadata) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{title}</title>
<style>
body {{ font-family: sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }}
.meta {{ color: #666; margin-bottom: 20px; }}
.classification {{ color: red; font-weight: bold; }}
</style>
</head>
<body>
<h1>{title}</h1>
<div class="meta">
<p>Author: {author} | Version: {version} | Created: {created}</p>
{classification}{tags}
</div>
<hr>
{content}
</body>
</html>"#,
            title = html_escape(&meta.title),
            author = html_escape(&meta.author),
            version = meta.version,
            created = meta.created_at,
            classification = meta
                .classification
                .as_ref()
                .map(|c| format!(r#"<p class="classification">Classification: {}</p>"#, c))
                .unwrap_or_default(),
            tags = if meta.tags.is_empty() {
                String::new()
            } else {
                format!("<p>Tags: {}</p>", html_escape(&meta.tags.join(", ")))
            },
            content = markdown_to_html(content),
        )
    }

    /// Export a document to JSON format.
    pub fn to_json(content: &str, meta: &ExportMetadata) -> String {
        let export = serde_json::json!({
            "title": meta.title,
            "author": meta.author,
            "version": meta.version,
            "created_at": meta.created_at,
            "classification": meta.classification,
            "tags": meta.tags,
            "content": content,
        });
        serde_json::to_string_pretty(&export).unwrap_or_else(|_| "{}".to_string())
    }

    /// Export content to CSV (for tabular data).
    pub fn to_csv(headers: &[&str], rows: &[Vec<&str>]) -> String {
        let mut csv = String::new();
        csv.push_str(&headers.join(","));
        csv.push('\n');
        for row in rows {
            let escaped: Vec<String> = row
                .iter()
                .map(|cell| {
                    if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                        format!("\"{}\"", cell.replace('"', "\"\""))
                    } else {
                        cell.to_string()
                    }
                })
                .collect();
            csv.push_str(&escaped.join(","));
            csv.push('\n');
        }
        csv
    }

    /// Import content from plain text / markdown.
    pub fn from_text(content: &str) -> ImportResult {
        let lines: Vec<&str> = content.lines().collect();
        let title = lines
            .first()
            .filter(|l| l.starts_with('#'))
            .map(|l| l.trim_start_matches('#').trim().to_string());

        ImportResult {
            content: content.to_string(),
            title,
            detected_format: ImportFormat::Markdown,
            metadata: std::collections::HashMap::new(),
            warnings: Vec::new(),
        }
    }

    /// Import from JSON.
    pub fn from_json(json_str: &str) -> Result<ImportResult, String> {
        let value: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| format!("Invalid JSON: {}", e))?;

        let content = value
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let title = value
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut metadata = std::collections::HashMap::new();
        if let Some(author) = value.get("author").and_then(|v| v.as_str()) {
            metadata.insert("author".to_string(), author.to_string());
        }
        if let Some(version) = value.get("version").and_then(|v| v.as_str()) {
            metadata.insert("version".to_string(), version.to_string());
        }

        Ok(ImportResult {
            content,
            title,
            detected_format: ImportFormat::Json,
            metadata,
            warnings: Vec::new(),
        })
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn markdown_to_html(md: &str) -> String {
    let mut html = String::new();
    let mut in_code_block = false;

    for line in md.lines() {
        if line.starts_with("```") {
            if in_code_block {
                html.push_str("</code></pre>\n");
                in_code_block = false;
            } else {
                html.push_str("<pre><code>");
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            html.push_str(&html_escape(line));
            html.push('\n');
            continue;
        }

        if line.starts_with("### ") {
            html.push_str(&format!("<h3>{}</h3>\n", html_escape(&line[4..])));
        } else if line.starts_with("## ") {
            html.push_str(&format!("<h2>{}</h3>\n", html_escape(&line[3..])));
        } else if line.starts_with("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", html_escape(&line[2..])));
        } else if line.starts_with("- ") {
            html.push_str(&format!("<li>{}</li>\n", html_escape(&line[2..])));
        } else if line.trim().is_empty() {
            html.push('\n');
        } else {
            html.push_str(&format!("<p>{}</p>\n", html_escape(line)));
        }
    }

    html
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> ExportMetadata {
        ExportMetadata {
            title: "Test Doc".to_string(),
            author: "Agent".to_string(),
            created_at: "2026-01-01".to_string(),
            version: "1.0".to_string(),
            classification: Some("Internal".to_string()),
            tags: vec!["test".to_string(), "demo".to_string()],
        }
    }

    #[test]
    fn test_export_markdown() {
        let meta = sample_meta();
        let md = DocumentExporter::to_markdown("Hello world", &meta);
        assert!(md.contains("# Test Doc"));
        assert!(md.contains("Hello world"));
        assert!(md.contains("Agent"));
    }

    #[test]
    fn test_export_html() {
        let meta = sample_meta();
        let html = DocumentExporter::to_html("Content here", &meta);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Test Doc"));
    }

    #[test]
    fn test_export_json() {
        let meta = sample_meta();
        let json = DocumentExporter::to_json("content", &meta);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["title"], "Test Doc");
    }

    #[test]
    fn test_export_csv() {
        let csv =
            DocumentExporter::to_csv(&["name", "age"], &[vec!["Alice", "30"], vec!["Bob", "25"]]);
        assert!(csv.contains("name,age"));
        assert!(csv.contains("Alice,30"));
    }

    #[test]
    fn test_csv_escape() {
        let csv = DocumentExporter::to_csv(&["name", "desc"], &[vec!["Test", "has, comma"]]);
        assert!(csv.contains("\"has, comma\""));
    }

    #[test]
    fn test_import_text() {
        let result = DocumentExporter::from_text("# Title\n\nBody text");
        assert_eq!(result.title, Some("Title".to_string()));
        assert_eq!(result.detected_format, ImportFormat::Markdown);
    }

    #[test]
    fn test_import_json() {
        let json = r#"{"title":"My Doc","content":"Hello","author":"Me"}"#;
        let result = DocumentExporter::from_json(json).unwrap();
        assert_eq!(result.title, Some("My Doc".to_string()));
        assert_eq!(result.content, "Hello");
        assert_eq!(result.metadata.get("author").unwrap(), "Me");
    }

    #[test]
    fn test_import_json_invalid() {
        let result = DocumentExporter::from_json("not json");
        assert!(result.is_err());
    }
}
