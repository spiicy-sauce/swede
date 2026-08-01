//! Coded diagnostics (SPEC §4).

use serde::Serialize;
use swede_syntax::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn error(code: &str, message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            code: code.to_string(),
            severity: Severity::Error,
            message: message.into(),
            span,
        }
    }
    pub fn warning(code: &str, message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            code: code.to_string(),
            severity: Severity::Warning,
            message: message.into(),
            span,
        }
    }
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// One-line human summary, e.g. `E001 [3:12] unknown reference 'foo'`.
pub fn format_line(d: &Diagnostic) -> String {
    let sev = match d.severity {
        Severity::Error => "error",
        Severity::Warning => "warn ",
    };
    format!(
        "{} {} [{}:{}] {}",
        sev,
        d.code,
        d.span.start_row + 1,
        d.span.start_col + 1,
        d.message
    )
}
