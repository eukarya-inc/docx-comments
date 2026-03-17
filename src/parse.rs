use roxmltree::{Document, Node};

use crate::error::AppError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedComment {
    pub comment_id: String,
    pub author: Option<String>,
    pub initials: Option<String>,
    pub created_at: Option<String>,
    pub para_id: Option<String>,
    pub raw_text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedAnchor {
    pub comment_id: String,
    pub raw_text: Option<String>,
    pub target_type: ParsedAnchorTargetType,
    pub locator: ParsedAnchorLocator,
    pub inferred_caption: Option<String>,
    pub image: Option<ParsedImageAnchor>,
    pub table: Option<ParsedTableAnchor>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedAnchorTargetType {
    Text,
    Image,
    Table,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedImageAnchor {
    pub name: Option<String>,
    pub alt_text: Option<String>,
    pub relationship_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedAnchorLocator {
    pub heading_path: Vec<String>,
    pub before_context: Option<String>,
    pub after_context: Option<String>,
    pub table_cell: Option<ParsedTableCellLocator>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedTableAnchor {
    pub index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedTableCellLocator {
    pub row_index: usize,
    pub column_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCommentExtended {
    pub para_id: String,
    pub parent_para_id: Option<String>,
    pub done: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParseDiagnostics {
    pub warnings: Vec<DiagnosticWarning>,
    pub missing_parts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DiagnosticWarning {
    pub code: String,
    pub message: String,
    pub comment_id: Option<String>,
    pub source_part: Option<String>,
}

pub fn parse_comments_xml(xml: &str) -> Result<Vec<ParsedComment>, AppError> {
    let document = parse_xml(xml, "word/comments.xml")?;

    document
        .descendants()
        .filter(|node| is_element(node, Some(NS_W), "comment"))
        .map(parse_comment_node)
        .collect()
}

pub fn parse_document_comment_ranges(
    xml: &str,
) -> Result<(Vec<ParsedAnchor>, Vec<DiagnosticWarning>), AppError> {
    let document = parse_xml(xml, "word/document.xml")?;
    let mut state = AnchorState::default();

    walk_document(document.root_element(), &mut state);
    collect_reference_fallback_anchors(&document, &mut state);
    apply_paragraph_locators(&document, &mut state);

    let mut warnings = state
        .active_comment_ids
        .into_iter()
        .map(|comment_id| DiagnosticWarning {
            code: "broken_anchor".to_string(),
            message: format!("Comment range for {comment_id} was not closed"),
            comment_id: Some(comment_id),
            source_part: Some("word/document.xml".to_string()),
        })
        .collect::<Vec<_>>();
    warnings.extend(state.reference_fallback_warnings);

    let mut anchor_meta = state.anchors;
    let mut anchors = state
        .buffers
        .into_iter()
        .map(
            |(comment_id, raw_text)| match anchor_meta.remove(&comment_id) {
                Some(mut anchor) => {
                    anchor.comment_id = comment_id;
                    anchor.raw_text = empty_to_none(raw_text);
                    anchor
                }
                None => ParsedAnchor {
                    comment_id,
                    raw_text: empty_to_none(raw_text),
                    target_type: ParsedAnchorTargetType::Text,
                    locator: ParsedAnchorLocator::default(),
                    inferred_caption: None,
                    image: None,
                    table: None,
                },
            },
        )
        .collect::<Vec<_>>();
    anchors.extend(anchor_meta.into_values());

    Ok((anchors, warnings))
}

pub fn parse_comments_extended(xml: &str) -> Result<Vec<ParsedCommentExtended>, AppError> {
    let document = parse_xml(xml, "word/commentsExtended.xml")?;

    document
        .descendants()
        .filter(|node| is_element(node, Some(NS_W15), "commentEx"))
        .map(parse_comment_extended_node)
        .collect()
}

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const NS_W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
const NS_WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn parse_xml<'a>(xml: &'a str, part: &'static str) -> Result<Document<'a>, AppError> {
    Document::parse(xml).map_err(|error| AppError::MalformedXml {
        part,
        message: error.to_string(),
    })
}

fn parse_comment_node(node: Node<'_, '_>) -> Result<ParsedComment, AppError> {
    let comment_id = required_attr(node, &[NS_W], "id", "word/comments.xml")?;
    let author = optional_attr(node, &[NS_W], "author");
    let initials = optional_attr(node, &[NS_W], "initials");
    let created_at = optional_attr(node, &[NS_W], "date");
    let para_id = node
        .descendants()
        .filter(|child| is_element(child, Some(NS_W), "p"))
        .filter_map(paragraph_para_id)
        .next_back();
    let paragraphs = node
        .descendants()
        .filter(|child| is_element(child, Some(NS_W), "p"))
        .map(collect_text_from_node)
        .collect::<Vec<_>>();

    Ok(ParsedComment {
        comment_id,
        author,
        initials,
        created_at,
        para_id,
        raw_text: empty_to_none(paragraphs.join("\n")),
    })
}

fn parse_comment_extended_node(node: Node<'_, '_>) -> Result<ParsedCommentExtended, AppError> {
    let para_id = required_attr(node, &[NS_W15], "paraId", "word/commentsExtended.xml")?;
    let parent_para_id = optional_attr(node, &[NS_W15], "paraIdParent");
    let done =
        optional_attr(node, &[NS_W15], "done").map(|value| matches!(value.as_str(), "1" | "true"));

    Ok(ParsedCommentExtended {
        para_id,
        parent_para_id,
        done,
    })
}

#[derive(Default)]
struct AnchorState {
    active_comment_ids: Vec<String>,
    buffers: std::collections::BTreeMap<String, String>,
    anchors: std::collections::BTreeMap<String, ParsedAnchor>,
    reference_fallback_warnings: Vec<DiagnosticWarning>,
    table_stack: Vec<usize>,
    next_table_index: usize,
}

fn walk_document(node: Node<'_, '_>, state: &mut AnchorState) {
    if is_element(&node, Some(NS_W), "tbl") {
        state.next_table_index += 1;
        let current_index = state.next_table_index;
        state.table_stack.push(current_index);
        for child in node.children() {
            walk_document(child, state);
        }
        state.table_stack.pop();
        return;
    }

    if is_element(&node, Some(NS_W), "commentRangeStart") {
        if let Some(comment_id) = optional_attr(node, &[NS_W], "id") {
            state.active_comment_ids.push(comment_id.clone());
            if let Some(table_index) = state.table_stack.last().copied() {
                let inferred_caption = node
                    .ancestors()
                    .find(|ancestor| is_element(ancestor, Some(NS_W), "tbl"))
                    .and_then(infer_caption_from_container);
                state
                    .anchors
                    .entry(comment_id.clone())
                    .or_insert_with(|| ParsedAnchor {
                        comment_id: comment_id.clone(),
                        raw_text: None,
                        target_type: ParsedAnchorTargetType::Table,
                        locator: ParsedAnchorLocator::default(),
                        inferred_caption,
                        image: None,
                        table: Some(ParsedTableAnchor { index: table_index }),
                    });
            }
        }
        return;
    }

    if is_element(&node, Some(NS_W), "commentRangeEnd") {
        if let Some(comment_id) = optional_attr(node, &[NS_W], "id") {
            state
                .active_comment_ids
                .retain(|active_id| active_id != &comment_id);
        }
        return;
    }

    if is_element(&node, Some(NS_W), "t") {
        append_to_active_comments(state, node.text().unwrap_or_default());
    }

    if is_element(&node, Some(NS_W), "tab") {
        append_to_active_comments(state, "\t");
    }

    if is_element(&node, Some(NS_W), "br") || is_element(&node, Some(NS_W), "cr") {
        append_to_active_comments(state, "\n");
    }

    for child in node.children() {
        walk_document(child, state);
    }

    if is_element(&node, Some(NS_W), "p") {
        append_to_active_comments(state, "\n");
    }
}

fn append_to_active_comments(state: &mut AnchorState, text: &str) {
    if state.active_comment_ids.is_empty() || text.is_empty() {
        return;
    }

    state.active_comment_ids.iter().for_each(|comment_id| {
        state
            .buffers
            .entry(comment_id.clone())
            .and_modify(|buffer| buffer.push_str(text))
            .or_insert_with(|| text.to_string());
        if let Some(table_index) = state.table_stack.last().copied() {
            state
                .anchors
                .entry(comment_id.clone())
                .and_modify(|anchor| {
                    anchor.target_type = ParsedAnchorTargetType::Table;
                    anchor.table = Some(ParsedTableAnchor { index: table_index });
                })
                .or_insert_with(|| ParsedAnchor {
                    comment_id: comment_id.clone(),
                    raw_text: None,
                    target_type: ParsedAnchorTargetType::Table,
                    locator: ParsedAnchorLocator::default(),
                    inferred_caption: None,
                    image: None,
                    table: Some(ParsedTableAnchor { index: table_index }),
                });
        }
    });
}

fn collect_reference_fallback_anchors(document: &Document<'_>, state: &mut AnchorState) {
    document
        .descendants()
        .filter(|node| is_element(node, Some(NS_W), "p"))
        .for_each(|paragraph| {
            let paragraph_text = empty_to_none(collect_text_from_node(paragraph));
            let comment_ids = paragraph_comment_ids(paragraph);
            if comment_ids.is_empty() {
                return;
            }

            let inferred_anchor = if let Some(image) = paragraph_image_anchor(paragraph) {
                Some(ParsedAnchor {
                    comment_id: String::new(),
                    raw_text: None,
                    target_type: ParsedAnchorTargetType::Image,
                    locator: ParsedAnchorLocator::default(),
                    inferred_caption: infer_nearby_caption(paragraph),
                    image: Some(image),
                    table: None,
                })
            } else if paragraph_inside_table(paragraph) {
                Some(ParsedAnchor {
                    comment_id: String::new(),
                    raw_text: paragraph_text.or_else(|| infer_table_preview(paragraph)),
                    target_type: ParsedAnchorTargetType::Table,
                    locator: ParsedAnchorLocator::default(),
                    inferred_caption: infer_table_caption(paragraph),
                    image: None,
                    table: table_index_for_paragraph(paragraph)
                        .map(|index| ParsedTableAnchor { index }),
                })
            } else {
                paragraph_text.map(|text| ParsedAnchor {
                    comment_id: String::new(),
                    raw_text: Some(text),
                    target_type: ParsedAnchorTargetType::Text,
                    locator: ParsedAnchorLocator::default(),
                    inferred_caption: None,
                    image: None,
                    table: None,
                })
            };

            let Some(inferred_anchor) = inferred_anchor else {
                return;
            };

            comment_ids.into_iter().for_each(|comment_id| {
                if state.buffers.contains_key(&comment_id) || state.anchors.contains_key(&comment_id)
                {
                    return;
                }

                let warning_code = match inferred_anchor.target_type {
                    ParsedAnchorTargetType::Text => "anchor_fallback_reference",
                    ParsedAnchorTargetType::Image => "anchor_fallback_image_context",
                    ParsedAnchorTargetType::Table => "anchor_fallback_table_context",
                };
                let warning_message = match inferred_anchor.target_type {
                    ParsedAnchorTargetType::Text => format!(
                        "Anchor for comment {comment_id} was inferred from the containing paragraph because no comment range was found"
                    ),
                    ParsedAnchorTargetType::Image => format!(
                        "Anchor for comment {comment_id} was inferred from an image paragraph because no text range was found"
                    ),
                    ParsedAnchorTargetType::Table => format!(
                        "Anchor for comment {comment_id} was inferred from a table paragraph because no text range was found"
                    ),
                };

                let mut anchor = inferred_anchor.clone();
                anchor.comment_id = comment_id.clone();
                state.anchors.insert(comment_id.clone(), anchor);
                state.reference_fallback_warnings.push(DiagnosticWarning {
                    code: warning_code.to_string(),
                    message: warning_message,
                    comment_id: Some(comment_id),
                    source_part: Some("word/document.xml".to_string()),
                });
            });
        });
}

fn apply_paragraph_locators(document: &Document<'_>, state: &mut AnchorState) {
    let mut heading_stack: Vec<(usize, String)> = Vec::new();

    document
        .descendants()
        .filter(|node| is_element(node, Some(NS_W), "p"))
        .for_each(|paragraph| {
            update_heading_stack(paragraph, &mut heading_stack);
            let heading_path = heading_stack
                .iter()
                .map(|(_, text)| text.clone())
                .collect::<Vec<_>>();
            let paragraph_text = empty_to_none(collect_text_from_node(paragraph));
            let direct_anchor_texts = paragraph_direct_anchor_texts(paragraph);
            let table_cell = table_cell_locator(paragraph);

            paragraph_comment_ids(paragraph)
                .into_iter()
                .for_each(|comment_id| {
                    let snippet = direct_anchor_texts
                        .get(&comment_id)
                        .cloned()
                        .or_else(|| paragraph_text.clone());
                    let (before_context, after_context) = snippet
                        .as_deref()
                        .and_then(|snippet| {
                            paragraph_text
                                .as_deref()
                                .and_then(|full| split_context_window(full, snippet))
                        })
                        .unwrap_or((None, None));

                    state
                        .anchors
                        .entry(comment_id.clone())
                        .and_modify(|anchor| {
                            if anchor.locator.heading_path.is_empty() {
                                anchor.locator.heading_path = heading_path.clone();
                            }
                            if anchor.locator.before_context.is_none() {
                                anchor.locator.before_context = before_context.clone();
                            }
                            if anchor.locator.after_context.is_none() {
                                anchor.locator.after_context = after_context.clone();
                            }
                            if anchor.locator.table_cell.is_none() {
                                anchor.locator.table_cell = table_cell.clone();
                            }
                        })
                        .or_insert_with(|| ParsedAnchor {
                            comment_id,
                            raw_text: None,
                            target_type: ParsedAnchorTargetType::Text,
                            locator: ParsedAnchorLocator {
                                heading_path: heading_path.clone(),
                                before_context,
                                after_context,
                                table_cell: table_cell.clone(),
                            },
                            inferred_caption: None,
                            image: None,
                            table: None,
                        });
                });
        });
}

fn update_heading_stack(paragraph: Node<'_, '_>, heading_stack: &mut Vec<(usize, String)>) {
    if let Some((level, text)) = heading_info(paragraph) {
        while heading_stack
            .last()
            .is_some_and(|(current_level, _)| *current_level >= level)
        {
            heading_stack.pop();
        }
        heading_stack.push((level, text));
    }
}

fn heading_info(paragraph: Node<'_, '_>) -> Option<(usize, String)> {
    let style = paragraph
        .descendants()
        .find(|node| is_element(node, Some(NS_W), "pStyle"))
        .and_then(|node| optional_attr(node, &[NS_W], "val"))?;
    let level = heading_level_from_style(&style)?;
    let text = empty_to_none(collect_text_from_node(paragraph))?;
    Some((level, text))
}

fn heading_level_from_style(style: &str) -> Option<usize> {
    if style == "Title" {
        return Some(0);
    }

    style
        .strip_prefix("Heading")
        .and_then(|suffix| suffix.parse::<usize>().ok())
}

fn paragraph_direct_anchor_texts(
    paragraph: Node<'_, '_>,
) -> std::collections::BTreeMap<String, String> {
    let mut active_comment_ids = Vec::new();
    let mut buffers = std::collections::BTreeMap::new();
    walk_paragraph_for_anchor_text(paragraph, &mut active_comment_ids, &mut buffers);
    buffers
        .into_iter()
        .map(|(comment_id, text)| (comment_id, trim_trailing_newlines(text)))
        .collect()
}

fn walk_paragraph_for_anchor_text(
    node: Node<'_, '_>,
    active_comment_ids: &mut Vec<String>,
    buffers: &mut std::collections::BTreeMap<String, String>,
) {
    if is_element(&node, Some(NS_W), "commentRangeStart") {
        if let Some(comment_id) = optional_attr(node, &[NS_W], "id") {
            active_comment_ids.push(comment_id);
        }
        return;
    }

    if is_element(&node, Some(NS_W), "commentRangeEnd") {
        if let Some(comment_id) = optional_attr(node, &[NS_W], "id") {
            active_comment_ids.retain(|active_id| active_id != &comment_id);
        }
        return;
    }

    if is_element(&node, Some(NS_W), "t") {
        append_to_comment_buffers(active_comment_ids, buffers, node.text().unwrap_or_default());
    }

    if is_element(&node, Some(NS_W), "tab") {
        append_to_comment_buffers(active_comment_ids, buffers, "\t");
    }

    if is_element(&node, Some(NS_W), "br") || is_element(&node, Some(NS_W), "cr") {
        append_to_comment_buffers(active_comment_ids, buffers, "\n");
    }

    for child in node.children() {
        walk_paragraph_for_anchor_text(child, active_comment_ids, buffers);
    }
}

fn append_to_comment_buffers(
    active_comment_ids: &[String],
    buffers: &mut std::collections::BTreeMap<String, String>,
    text: &str,
) {
    if active_comment_ids.is_empty() || text.is_empty() {
        return;
    }

    active_comment_ids.iter().for_each(|comment_id| {
        buffers
            .entry(comment_id.clone())
            .and_modify(|buffer| buffer.push_str(text))
            .or_insert_with(|| text.to_string());
    });
}

fn split_context_window(
    full_text: &str,
    snippet: &str,
) -> Option<(Option<String>, Option<String>)> {
    if snippet.is_empty() || full_text == snippet {
        return Some((None, None));
    }

    let start = full_text.find(snippet)?;
    let end = start + snippet.len();
    let before = truncate_left(&full_text[..start], 80);
    let after = truncate_right(&full_text[end..], 80);
    Some((empty_to_none(before), empty_to_none(after)))
}

fn truncate_left(text: &str, max_chars: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(max_chars);
    chars[start..].iter().collect()
}

fn truncate_right(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn paragraph_comment_ids(paragraph: Node<'_, '_>) -> Vec<String> {
    let mut ids = std::collections::BTreeSet::new();
    paragraph
        .descendants()
        .filter(|node| {
            is_element(node, Some(NS_W), "commentReference")
                || is_element(node, Some(NS_W), "commentRangeStart")
        })
        .filter_map(|node| optional_attr(node, &[NS_W], "id"))
        .for_each(|comment_id| {
            ids.insert(comment_id);
        });
    ids.into_iter().collect()
}

fn paragraph_image_anchor(paragraph: Node<'_, '_>) -> Option<ParsedImageAnchor> {
    let drawing = paragraph
        .descendants()
        .find(|node| is_element(node, Some(NS_W), "drawing"))?;
    let doc_pr = drawing
        .descendants()
        .find(|node| is_element(node, Some(NS_WP), "docPr"));

    Some(ParsedImageAnchor {
        name: doc_pr.and_then(|node| optional_attr_no_namespace(node, "name")),
        alt_text: doc_pr.and_then(|node| optional_attr_no_namespace(node, "descr")),
        relationship_id: drawing
            .descendants()
            .find(|node| is_element(node, Some(NS_A), "blip"))
            .and_then(|node| optional_attr(node, &[NS_R], "embed")),
    })
}

fn paragraph_inside_table(paragraph: Node<'_, '_>) -> bool {
    table_index_for_paragraph(paragraph).is_some()
}

fn table_index_for_paragraph(paragraph: Node<'_, '_>) -> Option<usize> {
    let table = paragraph
        .ancestors()
        .find(|node| is_element(node, Some(NS_W), "tbl"))?;
    Some(
        table
            .document()
            .descendants()
            .filter(|node| is_element(node, Some(NS_W), "tbl"))
            .take_while(|node| *node != table)
            .count()
            + 1,
    )
}

fn table_cell_locator(paragraph: Node<'_, '_>) -> Option<ParsedTableCellLocator> {
    let cell = paragraph
        .ancestors()
        .find(|node| is_element(node, Some(NS_W), "tc"))?;
    let row = cell
        .ancestors()
        .find(|node| is_element(node, Some(NS_W), "tr"))?;
    let row_parent = row.parent()?;
    let cell_parent = cell.parent()?;

    Some(ParsedTableCellLocator {
        row_index: row_parent
            .children()
            .filter(|node| is_element(node, Some(NS_W), "tr"))
            .position(|node| node == row)
            .map(|index| index + 1)?,
        column_index: cell_parent
            .children()
            .filter(|node| is_element(node, Some(NS_W), "tc"))
            .position(|node| node == cell)
            .map(|index| index + 1)?,
    })
}

fn infer_table_preview(paragraph: Node<'_, '_>) -> Option<String> {
    paragraph
        .ancestors()
        .find(|node| is_element(node, Some(NS_W), "tbl"))
        .map(collect_text_from_node)
        .and_then(empty_to_none)
}

fn infer_table_caption(paragraph: Node<'_, '_>) -> Option<String> {
    paragraph
        .ancestors()
        .find(|node| is_element(node, Some(NS_W), "tbl"))
        .and_then(infer_caption_from_container)
}

fn infer_nearby_caption(paragraph: Node<'_, '_>) -> Option<String> {
    infer_caption_from_container(paragraph)
}

fn infer_caption_from_container(node: Node<'_, '_>) -> Option<String> {
    node.next_sibling_element()
        .filter(|sibling| is_element(sibling, Some(NS_W), "p"))
        .and_then(caption_from_paragraph)
        .or_else(|| {
            node.prev_sibling_element()
                .filter(|sibling| is_element(sibling, Some(NS_W), "p"))
                .and_then(caption_from_paragraph)
        })
}

fn caption_from_paragraph(paragraph: Node<'_, '_>) -> Option<String> {
    let text = empty_to_none(collect_text_from_node(paragraph))?;
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let looks_like_caption = normalized.starts_with("Figure")
        || normalized.starts_with("Fig.")
        || normalized.starts_with("図")
        || normalized.starts_with("Table")
        || normalized.starts_with("表")
        || has_paragraph_style(paragraph, "Caption");

    looks_like_caption.then_some(text)
}

fn has_paragraph_style(paragraph: Node<'_, '_>, style_id: &str) -> bool {
    paragraph
        .descendants()
        .find(|node| is_element(node, Some(NS_W), "pStyle"))
        .and_then(|node| optional_attr(node, &[NS_W], "val"))
        .as_deref()
        == Some(style_id)
}

fn collect_text_from_node(node: Node<'_, '_>) -> String {
    let mut buffer = String::new();
    walk_text(node, &mut buffer);
    trim_trailing_newlines(buffer)
}

fn walk_text(node: Node<'_, '_>, buffer: &mut String) {
    if is_element(&node, Some(NS_W), "t") {
        buffer.push_str(node.text().unwrap_or_default());
    }

    if is_element(&node, Some(NS_W), "tab") {
        buffer.push('\t');
    }

    if is_element(&node, Some(NS_W), "br") || is_element(&node, Some(NS_W), "cr") {
        buffer.push('\n');
    }

    for child in node.children() {
        walk_text(child, buffer);
    }
}

fn trim_trailing_newlines(mut text: String) -> String {
    while text.ends_with('\n') {
        text.pop();
    }
    text
}

fn paragraph_para_id(node: Node<'_, '_>) -> Option<String> {
    optional_attr(node, &[NS_W15, NS_W14], "paraId")
}

fn required_attr(
    node: Node<'_, '_>,
    namespaces: &[&str],
    local_name: &str,
    part: &'static str,
) -> Result<String, AppError> {
    optional_attr(node, namespaces, local_name).ok_or_else(|| AppError::MalformedXml {
        part,
        message: format!("missing required attribute {local_name}"),
    })
}

fn optional_attr(node: Node<'_, '_>, namespaces: &[&str], local_name: &str) -> Option<String> {
    node.attributes().find_map(|attr| {
        namespaces
            .iter()
            .any(|namespace| attr.namespace() == Some(*namespace) && attr.name() == local_name)
            .then(|| attr.value().to_string())
    })
}

fn optional_attr_no_namespace(node: Node<'_, '_>, local_name: &str) -> Option<String> {
    node.attributes()
        .find(|attr| attr.namespace().is_none() && attr.name() == local_name)
        .map(|attr| attr.value().to_string())
}

fn is_element(node: &Node<'_, '_>, namespace: Option<&str>, local_name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == local_name
        && node.tag_name().namespace() == namespace
}

fn empty_to_none(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedAnchorTargetType, parse_comments_extended, parse_comments_xml,
        parse_document_comment_ranges,
    };

    #[test]
    fn parses_comments_xml_with_metadata_and_multiline_text() {
        let xml = r#"
            <w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                        xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
              <w:comment w:id="5" w:author="Alice" w:initials="AL" w:date="2024-01-01T10:00:00Z">
                <w:p w15:paraId="p1">
                  <w:r><w:t>根拠を</w:t></w:r>
                </w:p>
                <w:p>
                  <w:r><w:t>追加してください</w:t></w:r>
                </w:p>
              </w:comment>
            </w:comments>
        "#;

        let comments = parse_comments_xml(xml).expect("comments.xml should parse");

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].comment_id, "5");
        assert_eq!(comments[0].author.as_deref(), Some("Alice"));
        assert_eq!(comments[0].para_id.as_deref(), Some("p1"));
        assert_eq!(
            comments[0].raw_text.as_deref(),
            Some("根拠を\n追加してください")
        );
    }

    #[test]
    fn parses_comments_xml_text_nested_inside_tables() {
        let xml = r#"
            <w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                        xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
              <w:comment w:id="8" w:author="Alice">
                <w:tbl>
                  <w:tr>
                    <w:tc>
                      <w:p w15:paraId="p8">
                        <w:r><w:t>表の中のコメント</w:t></w:r>
                      </w:p>
                    </w:tc>
                  </w:tr>
                </w:tbl>
              </w:comment>
            </w:comments>
        "#;

        let comments = parse_comments_xml(xml).expect("comments.xml should parse");

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].para_id.as_deref(), Some("p8"));
        assert_eq!(comments[0].raw_text.as_deref(), Some("表の中のコメント"));
    }

    #[test]
    fn uses_last_paragraph_para_id_for_thread_matching() {
        let xml = r#"
            <w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                        xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
              <w:comment w:id="8" w:author="Alice">
                <w:p w15:paraId="p-first">
                  <w:r><w:t>1段落目</w:t></w:r>
                </w:p>
                <w:p w15:paraId="p-last">
                  <w:r><w:t>2段落目</w:t></w:r>
                </w:p>
              </w:comment>
            </w:comments>
        "#;

        let comments = parse_comments_xml(xml).expect("comments.xml should parse");

        assert_eq!(comments[0].para_id.as_deref(), Some("p-last"));
    }

    #[test]
    fn parses_document_comment_ranges_across_runs() {
        let xml = r#"
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:body>
                <w:p>
                  <w:pPr><w:pStyle w:val="Heading1" /></w:pPr>
                  <w:r><w:t>第1章</w:t></w:r>
                </w:p>
                <w:p>
                  <w:r><w:t>前</w:t></w:r>
                  <w:commentRangeStart w:id="5" />
                  <w:r><w:t>導入</w:t></w:r>
                  <w:r><w:t>の説明</w:t></w:r>
                  <w:commentRangeEnd w:id="5" />
                  <w:r><w:t>後</w:t></w:r>
                </w:p>
              </w:body>
            </w:document>
        "#;

        let (anchors, warnings) =
            parse_document_comment_ranges(xml).expect("document.xml should parse");

        assert_eq!(warnings.len(), 0);
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].comment_id, "5");
        assert_eq!(anchors[0].raw_text.as_deref(), Some("導入の説明"));
        assert!(matches!(
            anchors[0].target_type,
            ParsedAnchorTargetType::Text
        ));
        assert_eq!(anchors[0].locator.heading_path, vec!["第1章".to_string()]);
        assert_eq!(anchors[0].locator.before_context.as_deref(), Some("前"));
        assert_eq!(anchors[0].locator.after_context.as_deref(), Some("後"));
    }

    #[test]
    fn falls_back_to_paragraph_text_when_only_comment_reference_exists() {
        let xml = r#"
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:body>
                <w:p>
                  <w:pPr><w:pStyle w:val="Heading1" /></w:pPr>
                  <w:r><w:t>概要</w:t></w:r>
                </w:p>
                <w:p>
                  <w:r><w:t>レビュー対象の段落です</w:t></w:r>
                  <w:r><w:commentReference w:id="5" /></w:r>
                </w:p>
              </w:body>
            </w:document>
        "#;

        let (anchors, warnings) =
            parse_document_comment_ranges(xml).expect("document.xml should parse");

        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].comment_id, "5");
        assert_eq!(
            anchors[0].raw_text.as_deref(),
            Some("レビュー対象の段落です")
        );
        assert_eq!(anchors[0].locator.heading_path, vec!["概要".to_string()]);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "anchor_fallback_reference");
    }

    #[test]
    fn infers_image_anchor_and_caption_from_nearby_paragraph() {
        let xml = r#"
            <w:document
              xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
              xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
              xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
              xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
              <w:body>
                <w:p>
                  <w:r>
                    <w:drawing>
                      <wp:inline>
                        <wp:docPr id="1" name="Picture 3" descr="システム構成図" />
                        <a:graphic>
                          <a:graphicData>
                            <a:pic>
                              <a:blipFill>
                                <a:blip r:embed="rId12" />
                              </a:blipFill>
                            </a:pic>
                          </a:graphicData>
                        </a:graphic>
                      </wp:inline>
                    </w:drawing>
                  </w:r>
                  <w:r><w:commentReference w:id="7" /></w:r>
                </w:p>
                <w:p>
                  <w:r><w:t>図1 システム構成図</w:t></w:r>
                </w:p>
              </w:body>
            </w:document>
        "#;

        let (anchors, warnings) =
            parse_document_comment_ranges(xml).expect("document.xml should parse");

        assert_eq!(anchors.len(), 1);
        assert!(matches!(
            anchors[0].target_type,
            ParsedAnchorTargetType::Image
        ));
        assert_eq!(anchors[0].raw_text, None);
        assert_eq!(
            anchors[0].inferred_caption.as_deref(),
            Some("図1 システム構成図")
        );
        assert_eq!(
            anchors[0]
                .image
                .as_ref()
                .and_then(|image| image.name.as_deref()),
            Some("Picture 3")
        );
        assert_eq!(warnings[0].code, "anchor_fallback_image_context");
    }

    #[test]
    fn infers_table_anchor_and_caption_from_nearby_paragraph() {
        let xml = r#"
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:body>
                <w:p>
                  <w:pPr><w:pStyle w:val="Heading2" /></w:pPr>
                  <w:r><w:t>表セクション</w:t></w:r>
                </w:p>
                <w:tbl>
                  <w:tr>
                    <w:tc>
                      <w:p>
                        <w:r><w:t>セル1</w:t></w:r>
                        <w:r><w:commentReference w:id="9" /></w:r>
                      </w:p>
                    </w:tc>
                  </w:tr>
                </w:tbl>
                <w:p>
                  <w:r><w:t>表1 テストデータ</w:t></w:r>
                </w:p>
              </w:body>
            </w:document>
        "#;

        let (anchors, warnings) =
            parse_document_comment_ranges(xml).expect("document.xml should parse");

        assert_eq!(anchors.len(), 1);
        assert!(matches!(
            anchors[0].target_type,
            ParsedAnchorTargetType::Table
        ));
        assert_eq!(
            anchors[0].locator.heading_path,
            vec!["表セクション".to_string()]
        );
        assert_eq!(anchors[0].raw_text.as_deref(), Some("セル1"));
        assert_eq!(anchors[0].table.as_ref().map(|table| table.index), Some(1));
        assert_eq!(
            anchors[0]
                .locator
                .table_cell
                .as_ref()
                .map(|cell| (cell.row_index, cell.column_index)),
            Some((1, 1))
        );
        assert_eq!(
            anchors[0].inferred_caption.as_deref(),
            Some("表1 テストデータ")
        );
        assert_eq!(warnings[0].code, "anchor_fallback_table_context");
    }

    #[test]
    fn direct_comment_range_inside_table_is_marked_as_table() {
        let xml = r#"
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:body>
                <w:p>
                  <w:pPr><w:pStyle w:val="Heading2" /></w:pPr>
                  <w:r><w:t>表セクション</w:t></w:r>
                </w:p>
                <w:tbl>
                  <w:tr>
                    <w:tc>
                      <w:p>
                        <w:commentRangeStart w:id="5" />
                        <w:r><w:t>セル1</w:t></w:r>
                        <w:commentRangeEnd w:id="5" />
                      </w:p>
                    </w:tc>
                  </w:tr>
                </w:tbl>
                <w:p>
                  <w:r><w:t>表1 テストデータ</w:t></w:r>
                </w:p>
              </w:body>
            </w:document>
        "#;

        let (anchors, warnings) =
            parse_document_comment_ranges(xml).expect("document.xml should parse");

        assert!(warnings.is_empty());
        assert_eq!(anchors[0].raw_text.as_deref(), Some("セル1"));
        assert!(matches!(
            anchors[0].target_type,
            ParsedAnchorTargetType::Table
        ));
        assert_eq!(
            anchors[0].locator.heading_path,
            vec!["表セクション".to_string()]
        );
        assert_eq!(anchors[0].table.as_ref().map(|table| table.index), Some(1));
        assert_eq!(
            anchors[0]
                .locator
                .table_cell
                .as_ref()
                .map(|cell| (cell.row_index, cell.column_index)),
            Some((1, 1))
        );
        assert_eq!(
            anchors[0].inferred_caption.as_deref(),
            Some("表1 テストデータ")
        );
    }

    #[test]
    fn parses_comments_extended_with_reply_relationship_and_done_state() {
        let xml = r#"
            <w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
              <w15:commentEx w15:paraId="p1" w15:done="0" />
              <w15:commentEx w15:paraId="p2" w15:paraIdParent="p1" />
            </w15:commentsEx>
        "#;

        let extended = parse_comments_extended(xml).expect("commentsExtended.xml should parse");

        assert_eq!(extended.len(), 2);
        assert_eq!(extended[0].para_id, "p1");
        assert_eq!(extended[0].done, Some(false));
        assert_eq!(extended[1].parent_para_id.as_deref(), Some("p1"));
    }
}
