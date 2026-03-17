use crate::parse::DiagnosticWarning;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputDocument {
    pub schema_version: String,
    pub source: OutputSource,
    pub comments: Vec<OutputComment>,
    pub diagnostics: OutputDiagnostics,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputSource {
    pub file: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputComment {
    pub comment_id: String,
    pub author: Option<String>,
    pub created_at: Option<String>,
    pub done: Option<bool>,
    pub anchor: OutputAnchor,
    pub comment: OutputText,
    pub replies: Vec<OutputReply>,
    pub metadata: OutputMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputReply {
    pub comment_id: String,
    pub author: Option<String>,
    pub created_at: Option<String>,
    pub done: Option<bool>,
    pub comment: OutputText,
    pub metadata: OutputMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputAnchor {
    pub target_type: String,
    pub raw_text: Option<String>,
    pub locator: OutputAnchorLocator,
    pub context: OutputAnchorContext,
    pub image: Option<OutputImageAnchor>,
    pub table: Option<OutputTableAnchor>,
    pub start_hint: Option<String>,
    pub end_hint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputAnchorLocator {
    pub heading_path: Vec<String>,
    pub before_context: Option<String>,
    pub after_context: Option<String>,
    pub table_cell: Option<OutputTableCellLocator>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputAnchorContext {
    pub caption: Option<OutputAnchorCaption>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputAnchorCaption {
    pub text: Option<String>,
    pub source: String,
    pub confidence: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputImageAnchor {
    pub name: Option<String>,
    pub alt_text: Option<String>,
    pub relationship_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputTableAnchor {
    pub index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputTableCellLocator {
    pub row_index: usize,
    pub column_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputText {
    pub raw_text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputMetadata {
    pub parent_comment_id: Option<String>,
    pub comment_para_id: Option<String>,
    pub has_extended_properties: bool,
    pub source_parts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputDiagnostics {
    pub warnings: Vec<DiagnosticWarning>,
    pub missing_parts: Vec<String>,
}
