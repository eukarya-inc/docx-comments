use std::path::Path;

use crate::docx::{COMMENTS_EXTENDED_XML_PATH, read_docx_parts};
use crate::error::AppError;
use std::collections::HashMap;

use crate::output::{
    OutputAnchor, OutputAnchorCaption, OutputAnchorContext, OutputAnchorLocator, OutputComment,
    OutputDiagnostics, OutputDocument, OutputImageAnchor, OutputMetadata, OutputReply,
    OutputSource, OutputTableAnchor, OutputTableCellLocator, OutputText,
};
use crate::parse::{
    DiagnosticWarning, ParseDiagnostics, ParsedAnchor, ParsedAnchorTargetType, ParsedComment,
    ParsedCommentExtended, parse_comments_extended, parse_comments_xml,
    parse_document_comment_ranges,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractOptions {
    pub pretty: bool,
    pub include_raw: bool,
    pub fail_on_missing_extended: bool,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            pretty: false,
            include_raw: true,
            fail_on_missing_extended: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildOptions {
    pub include_raw: bool,
    pub source_file: String,
    pub document_part_present: bool,
    pub document_anchor_data_available: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            include_raw: true,
            source_file: String::new(),
            document_part_present: false,
            document_anchor_data_available: false,
        }
    }
}

pub fn extract_output_document(
    path: &Path,
    options: &ExtractOptions,
) -> Result<OutputDocument, AppError> {
    let docx = read_docx_parts(path)?;
    let comments_xml = docx
        .parts
        .comments_xml
        .as_deref()
        .ok_or(AppError::MissingRequiredPart("word/comments.xml"))?;
    let comments = parse_comments_xml(comments_xml)?;

    let mut diagnostics = ParseDiagnostics {
        warnings: Vec::new(),
        missing_parts: docx.missing_parts,
    };

    let (anchors, document_anchor_data_available) =
        parse_document_part(docx.parts.document_xml.as_deref(), &mut diagnostics)?;
    let extended = parse_extended_part(
        docx.parts.comments_extended_xml.as_deref(),
        options.fail_on_missing_extended,
        &mut diagnostics,
    )?;

    Ok(build_output_document(
        &comments,
        &anchors,
        &extended,
        diagnostics,
        &BuildOptions {
            include_raw: options.include_raw,
            source_file: path.display().to_string(),
            document_part_present: docx.parts.document_xml.is_some(),
            document_anchor_data_available,
        },
    ))
}

pub fn serialize_output_json(output: &OutputDocument, pretty: bool) -> Result<String, AppError> {
    if pretty {
        serde_json::to_string_pretty(output).map_err(AppError::from)
    } else {
        serde_json::to_string(output).map_err(AppError::from)
    }
}

pub fn build_output_document(
    comments: &[ParsedComment],
    anchors: &[ParsedAnchor],
    extended: &[ParsedCommentExtended],
    diagnostics: ParseDiagnostics,
    options: &BuildOptions,
) -> OutputDocument {
    let anchor_map = build_anchor_map(anchors);
    let extended_map = build_extended_map(extended);
    let para_to_comment = build_para_to_comment_map(comments);
    let mut warnings = diagnostics.warnings;
    let reply_parent_map =
        build_reply_parent_map(comments, &extended_map, &para_to_comment, &mut warnings);
    if options.document_anchor_data_available {
        warnings.extend(missing_anchor_warnings(
            comments,
            &anchor_map,
            &reply_parent_map,
        ));
    }

    let comments = comments
        .iter()
        .filter(|comment| !reply_parent_map.contains_key(&comment.comment_id))
        .map(|comment| {
            build_output_comment(
                comment,
                comments,
                &anchor_map,
                &extended_map,
                &reply_parent_map,
                options,
            )
        })
        .collect();

    OutputDocument {
        schema_version: "1.0".to_string(),
        source: OutputSource {
            file: options.source_file.clone(),
        },
        comments,
        diagnostics: OutputDiagnostics {
            warnings,
            missing_parts: diagnostics.missing_parts,
        },
    }
}

fn build_output_comment(
    comment: &ParsedComment,
    comments: &[ParsedComment],
    anchor_map: &HashMap<String, ParsedAnchor>,
    extended_map: &HashMap<String, ParsedCommentExtended>,
    reply_parent_map: &HashMap<String, String>,
    options: &BuildOptions,
) -> OutputComment {
    let replies = comments
        .iter()
        .filter(|candidate| candidate.comment_id != comment.comment_id)
        .filter(|candidate| {
            thread_root_comment_id(&candidate.comment_id, reply_parent_map).as_deref()
                == Some(comment.comment_id.as_str())
        })
        .map(|reply| build_output_reply(reply, extended_map, reply_parent_map, options))
        .collect();

    OutputComment {
        comment_id: comment.comment_id.clone(),
        author: comment.author.clone(),
        created_at: comment.created_at.clone(),
        done: comment_done(comment, extended_map),
        anchor: build_anchor(comment, anchor_map, options),
        comment: build_text(comment.raw_text.clone(), options.include_raw),
        replies,
        metadata: build_metadata(comment, None, extended_map, options.document_part_present),
    }
}

fn build_output_reply(
    comment: &ParsedComment,
    extended_map: &HashMap<String, ParsedCommentExtended>,
    reply_parent_map: &HashMap<String, String>,
    options: &BuildOptions,
) -> OutputReply {
    OutputReply {
        comment_id: comment.comment_id.clone(),
        author: comment.author.clone(),
        created_at: comment.created_at.clone(),
        done: comment_done(comment, extended_map),
        comment: build_text(comment.raw_text.clone(), options.include_raw),
        metadata: build_metadata(
            comment,
            reply_parent_map.get(&comment.comment_id).cloned(),
            extended_map,
            options.document_part_present,
        ),
    }
}

fn build_anchor(
    comment: &ParsedComment,
    anchor_map: &HashMap<String, ParsedAnchor>,
    options: &BuildOptions,
) -> OutputAnchor {
    let anchor = anchor_map.get(&comment.comment_id);
    let raw_text = anchor.and_then(|anchor| anchor.raw_text.clone());

    OutputAnchor {
        target_type: anchor
            .map(anchor_target_type)
            .unwrap_or_else(|| "text".to_string()),
        raw_text: include_raw(raw_text, options.include_raw),
        locator: OutputAnchorLocator {
            heading_path: anchor
                .map(|anchor| anchor.locator.heading_path.clone())
                .unwrap_or_default(),
            before_context: anchor.and_then(|anchor| {
                include_raw(anchor.locator.before_context.clone(), options.include_raw)
            }),
            after_context: anchor.and_then(|anchor| {
                include_raw(anchor.locator.after_context.clone(), options.include_raw)
            }),
            table_cell: anchor.and_then(|anchor| {
                anchor
                    .locator
                    .table_cell
                    .as_ref()
                    .map(|cell| OutputTableCellLocator {
                        row_index: cell.row_index,
                        column_index: cell.column_index,
                    })
            }),
        },
        context: OutputAnchorContext {
            caption: anchor
                .and_then(|anchor| anchor.inferred_caption.as_ref())
                .map(|caption| OutputAnchorCaption {
                    text: include_raw(Some(caption.clone()), options.include_raw),
                    source: "inferred_from_nearby_paragraph".to_string(),
                    confidence: "medium".to_string(),
                }),
        },
        image: anchor.and_then(|anchor| {
            anchor.image.as_ref().map(|image| OutputImageAnchor {
                name: image.name.clone(),
                alt_text: image.alt_text.clone(),
                relationship_id: image.relationship_id.clone(),
            })
        }),
        table: anchor.and_then(|anchor| {
            anchor
                .table
                .as_ref()
                .map(|table| OutputTableAnchor { index: table.index })
        }),
        start_hint: None,
        end_hint: None,
    }
}

fn anchor_target_type(anchor: &ParsedAnchor) -> String {
    match anchor.target_type {
        ParsedAnchorTargetType::Text => "text",
        ParsedAnchorTargetType::Image => "image",
        ParsedAnchorTargetType::Table => "table",
    }
    .to_string()
}

fn build_text(raw_text: Option<String>, include_raw_text: bool) -> OutputText {
    OutputText {
        raw_text: include_raw(raw_text, include_raw_text),
    }
}

fn include_raw(raw_text: Option<String>, include_raw_text: bool) -> Option<String> {
    if include_raw_text { raw_text } else { None }
}

fn build_anchor_map(anchors: &[ParsedAnchor]) -> HashMap<String, ParsedAnchor> {
    anchors
        .iter()
        .cloned()
        .map(|anchor| (anchor.comment_id.clone(), anchor))
        .collect()
}

fn build_extended_map(
    extended: &[ParsedCommentExtended],
) -> HashMap<String, ParsedCommentExtended> {
    extended
        .iter()
        .cloned()
        .map(|entry| (entry.para_id.clone(), entry))
        .collect()
}

fn build_para_to_comment_map(comments: &[ParsedComment]) -> HashMap<String, String> {
    comments
        .iter()
        .filter_map(|comment| {
            comment
                .para_id
                .clone()
                .map(|para_id| (para_id, comment.comment_id.clone()))
        })
        .collect()
}

fn build_reply_parent_map(
    comments: &[ParsedComment],
    extended_map: &HashMap<String, ParsedCommentExtended>,
    para_to_comment: &HashMap<String, String>,
    warnings: &mut Vec<DiagnosticWarning>,
) -> HashMap<String, String> {
    comments
        .iter()
        .filter_map(|comment| {
            let para_id = comment.para_id.as_ref()?;
            let extended = extended_map.get(para_id)?;
            let parent_para_id = extended.parent_para_id.as_ref()?;
            match para_to_comment.get(parent_para_id) {
                Some(parent_comment_id) => {
                    Some((comment.comment_id.clone(), parent_comment_id.clone()))
                }
                None => {
                    warnings.push(DiagnosticWarning {
                        code: "orphan_reply".to_string(),
                        message: format!(
                            "Reply comment {} points to missing parent paraId {}",
                            comment.comment_id, parent_para_id
                        ),
                        comment_id: Some(comment.comment_id.clone()),
                        source_part: Some("word/commentsExtended.xml".to_string()),
                    });
                    None
                }
            }
        })
        .collect()
}

fn comment_done(
    comment: &ParsedComment,
    extended_map: &HashMap<String, ParsedCommentExtended>,
) -> Option<bool> {
    comment
        .para_id
        .as_ref()
        .and_then(|para_id| extended_map.get(para_id))
        .and_then(|entry| entry.done)
}

fn build_metadata(
    comment: &ParsedComment,
    parent_comment_id: Option<String>,
    extended_map: &HashMap<String, ParsedCommentExtended>,
    document_part_present: bool,
) -> OutputMetadata {
    let has_extended_properties = comment
        .para_id
        .as_ref()
        .map(|para_id| extended_map.contains_key(para_id))
        .unwrap_or(false);

    let mut source_parts = vec!["word/comments.xml".to_string()];
    if has_extended_properties {
        source_parts.push("word/commentsExtended.xml".to_string());
    }
    if parent_comment_id.is_none() && document_part_present {
        source_parts.push("word/document.xml".to_string());
    }

    OutputMetadata {
        parent_comment_id,
        comment_para_id: comment.para_id.clone(),
        has_extended_properties,
        source_parts,
    }
}

fn missing_anchor_warnings(
    comments: &[ParsedComment],
    anchor_map: &HashMap<String, ParsedAnchor>,
    reply_parent_map: &HashMap<String, String>,
) -> Vec<DiagnosticWarning> {
    comments
        .iter()
        .filter(|comment| !reply_parent_map.contains_key(&comment.comment_id))
        .filter(|comment| !anchor_map.contains_key(&comment.comment_id))
        .map(|comment| DiagnosticWarning {
            code: "broken_anchor".to_string(),
            message: format!("No anchor text found for comment {}", comment.comment_id),
            comment_id: Some(comment.comment_id.clone()),
            source_part: Some("word/document.xml".to_string()),
        })
        .collect()
}

fn thread_root_comment_id(
    comment_id: &str,
    reply_parent_map: &HashMap<String, String>,
) -> Option<String> {
    let mut current = comment_id;
    let mut visited = std::collections::HashSet::new();

    while let Some(parent_id) = reply_parent_map.get(current) {
        if !visited.insert(current.to_string()) {
            return None;
        }
        current = parent_id;
    }

    Some(current.to_string())
}

fn parse_document_part(
    xml: Option<&str>,
    diagnostics: &mut ParseDiagnostics,
) -> Result<(Vec<ParsedAnchor>, bool), AppError> {
    match xml {
        Some(xml) => match parse_document_comment_ranges(xml) {
            Ok((anchors, warnings)) => {
                diagnostics.warnings.extend(warnings);
                Ok((anchors, true))
            }
            Err(AppError::MalformedXml { part, message }) => {
                diagnostics.warnings.push(DiagnosticWarning {
                    code: "malformed_xml".to_string(),
                    message,
                    comment_id: None,
                    source_part: Some(part.to_string()),
                });
                Ok((Vec::new(), false))
            }
            Err(error) => Err(error),
        },
        None => Ok((Vec::new(), false)),
    }
}

fn parse_extended_part(
    xml: Option<&str>,
    fail_on_missing_extended: bool,
    diagnostics: &mut ParseDiagnostics,
) -> Result<Vec<ParsedCommentExtended>, AppError> {
    match xml {
        Some(xml) => match parse_comments_extended(xml) {
            Ok(extended) => Ok(extended),
            Err(AppError::MalformedXml { part, message }) => {
                diagnostics.warnings.push(DiagnosticWarning {
                    code: "malformed_xml".to_string(),
                    message,
                    comment_id: None,
                    source_part: Some(part.to_string()),
                });
                Ok(Vec::new())
            }
            Err(error) => Err(error),
        },
        None if fail_on_missing_extended => {
            Err(AppError::MissingRequiredPart(COMMENTS_EXTENDED_XML_PATH))
        }
        None => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{
        ParsedAnchor, ParsedAnchorTargetType, ParsedComment, ParsedCommentExtended,
    };

    #[test]
    fn builds_ai_friendly_output_with_reply_and_resolution_state() {
        let comments = vec![
            ParsedComment {
                comment_id: "5".to_string(),
                author: Some("Alice".to_string()),
                initials: Some("AL".to_string()),
                created_at: Some("2024-01-01T10:00:00Z".to_string()),
                para_id: Some("p1".to_string()),
                raw_text: Some("根拠を\n追加してください".to_string()),
            },
            ParsedComment {
                comment_id: "6".to_string(),
                author: Some("Bob".to_string()),
                initials: Some("BO".to_string()),
                created_at: Some("2024-01-01T11:00:00Z".to_string()),
                para_id: Some("p2".to_string()),
                raw_text: Some("明日対応します".to_string()),
            },
        ];
        let anchors = vec![ParsedAnchor {
            comment_id: "5".to_string(),
            raw_text: Some("導入の説明".to_string()),
            target_type: ParsedAnchorTargetType::Text,
            locator: Default::default(),
            inferred_caption: None,
            image: None,
            table: None,
        }];
        let extended = vec![
            ParsedCommentExtended {
                para_id: "p1".to_string(),
                parent_para_id: None,
                done: Some(false),
            },
            ParsedCommentExtended {
                para_id: "p2".to_string(),
                parent_para_id: Some("p1".to_string()),
                done: None,
            },
        ];

        let output = build_output_document(
            &comments,
            &anchors,
            &extended,
            ParseDiagnostics::default(),
            &BuildOptions {
                include_raw: true,
                source_file: "input.docx".to_string(),
                document_part_present: true,
                document_anchor_data_available: true,
            },
        );

        assert_eq!(output.schema_version, "1.0");
        assert_eq!(output.comments.len(), 1);
        assert_eq!(output.comments[0].comment_id, "5");
        assert_eq!(output.comments[0].done, Some(false));
        assert_eq!(output.comments[0].anchor.target_type, "text");
        assert_eq!(
            output.comments[0].anchor.raw_text.as_deref(),
            Some("導入の説明")
        );
        assert_eq!(
            output.comments[0].comment.raw_text.as_deref(),
            Some("根拠を\n追加してください")
        );
        assert_eq!(output.comments[0].replies.len(), 1);
        assert_eq!(output.comments[0].replies[0].comment_id, "6");
        assert_eq!(
            output.comments[0].replies[0]
                .metadata
                .parent_comment_id
                .as_deref(),
            Some("5")
        );
        assert!(output.diagnostics.warnings.is_empty());
    }

    #[test]
    fn records_orphan_reply_without_turning_it_into_a_nested_reply() {
        let comments = vec![ParsedComment {
            comment_id: "9".to_string(),
            author: Some("Bob".to_string()),
            initials: None,
            created_at: None,
            para_id: Some("missing-child".to_string()),
            raw_text: Some("親が見つからない返信".to_string()),
        }];
        let extended = vec![ParsedCommentExtended {
            para_id: "missing-child".to_string(),
            parent_para_id: Some("unknown-parent".to_string()),
            done: None,
        }];

        let output = build_output_document(
            &comments,
            &[],
            &extended,
            ParseDiagnostics::default(),
            &BuildOptions {
                include_raw: true,
                source_file: "input.docx".to_string(),
                document_part_present: true,
                document_anchor_data_available: false,
            },
        );

        assert_eq!(output.comments.len(), 1);
        assert!(output.comments[0].replies.is_empty());
        assert_eq!(output.diagnostics.warnings.len(), 1);
        assert_eq!(output.diagnostics.warnings[0].code, "orphan_reply");
    }

    #[test]
    fn keeps_reply_descendants_under_the_root_thread_while_preserving_direct_parent_ids() {
        let comments = vec![
            ParsedComment {
                comment_id: "5".to_string(),
                author: Some("Alice".to_string()),
                initials: None,
                created_at: None,
                para_id: Some("p1".to_string()),
                raw_text: Some("親コメント".to_string()),
            },
            ParsedComment {
                comment_id: "6".to_string(),
                author: Some("Bob".to_string()),
                initials: None,
                created_at: None,
                para_id: Some("p2".to_string()),
                raw_text: Some("返信".to_string()),
            },
            ParsedComment {
                comment_id: "7".to_string(),
                author: Some("Carol".to_string()),
                initials: None,
                created_at: None,
                para_id: Some("p3".to_string()),
                raw_text: Some("返信への返信".to_string()),
            },
        ];
        let extended = vec![
            ParsedCommentExtended {
                para_id: "p1".to_string(),
                parent_para_id: None,
                done: Some(false),
            },
            ParsedCommentExtended {
                para_id: "p2".to_string(),
                parent_para_id: Some("p1".to_string()),
                done: None,
            },
            ParsedCommentExtended {
                para_id: "p3".to_string(),
                parent_para_id: Some("p2".to_string()),
                done: None,
            },
        ];

        let output = build_output_document(
            &comments,
            &[],
            &extended,
            ParseDiagnostics::default(),
            &BuildOptions {
                include_raw: true,
                source_file: "input.docx".to_string(),
                document_part_present: false,
                document_anchor_data_available: false,
            },
        );

        assert_eq!(output.comments.len(), 1);
        assert_eq!(output.comments[0].comment_id, "5");
        assert_eq!(output.comments[0].replies.len(), 2);
        assert_eq!(output.comments[0].replies[0].comment_id, "6");
        assert_eq!(output.comments[0].replies[1].comment_id, "7");
        assert_eq!(
            output.comments[0].replies[1]
                .metadata
                .parent_comment_id
                .as_deref(),
            Some("6")
        );
    }
}
