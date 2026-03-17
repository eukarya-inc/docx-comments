use std::fs::File;
use std::io::Write;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[test]
fn cli_outputs_structured_json_for_comments_replies_and_done_state() {
    let docx_path = write_docx_fixture(
        "threaded.docx",
        Some(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
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
                  </w:p>
                </w:body>
              </w:document>"#,
        ),
        r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                       xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
              <w:comment w:id="5" w:author="Alice" w:date="2024-01-01T10:00:00Z">
                <w:p w15:paraId="p1"><w:r><w:t>根拠を追加してください</w:t></w:r></w:p>
              </w:comment>
              <w:comment w:id="6" w:author="Bob">
                <w:p w15:paraId="p2"><w:r><w:t>明日対応します</w:t></w:r></w:p>
              </w:comment>
            </w:comments>"#,
        Some(
            r#"<w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
                <w15:commentEx w15:paraId="p1" w15:done="0" />
                <w15:commentEx w15:paraId="p2" w15:paraIdParent="p1" />
              </w15:commentsEx>"#,
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_docx-comment-json"))
        .arg(&docx_path)
        .arg("--pretty")
        .output()
        .expect("binary should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert_eq!(json["schema_version"], "1.0");
    assert_eq!(json["comments"][0]["comment_id"], "5");
    assert_eq!(json["comments"][0]["done"], false);
    assert_eq!(json["comments"][0]["anchor"]["raw_text"], "導入の説明");
    assert_eq!(
        json["comments"][0]["anchor"]["locator"]["heading_path"][0],
        "第1章"
    );
    assert_eq!(json["comments"][0]["replies"][0]["comment_id"], "6");
    assert_eq!(
        json["comments"][0]["replies"][0]["metadata"]["parent_comment_id"],
        "5"
    );
}

#[test]
fn cli_allows_missing_extended_part_by_default_and_reports_it() {
    let docx_path = write_docx_fixture(
        "missing-extended.docx",
        Some(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:body>
                  <w:p>
                    <w:commentRangeStart w:id="5" />
                    <w:r><w:t>対象箇所</w:t></w:r>
                    <w:commentRangeEnd w:id="5" />
                  </w:p>
                </w:body>
              </w:document>"#,
        ),
        r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:comment w:id="5" w:author="Alice">
                <w:p><w:r><w:t>本文</w:t></w:r></w:p>
              </w:comment>
            </w:comments>"#,
        None,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_docx-comment-json"))
        .arg(&docx_path)
        .output()
        .expect("binary should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert_eq!(json["comments"][0]["done"], Value::Null);
    assert_eq!(
        json["diagnostics"]["missing_parts"][0],
        "word/commentsExtended.xml"
    );
}

#[test]
fn cli_can_fail_on_missing_extended_part_when_requested() {
    let docx_path = write_docx_fixture(
        "strict-missing-extended.docx",
        Some(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:body>
                  <w:p>
                    <w:commentRangeStart w:id="5" />
                    <w:r><w:t>対象箇所</w:t></w:r>
                    <w:commentRangeEnd w:id="5" />
                  </w:p>
                </w:body>
              </w:document>"#,
        ),
        r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:comment w:id="5" w:author="Alice">
                <w:p><w:r><w:t>本文</w:t></w:r></w:p>
              </w:comment>
            </w:comments>"#,
        None,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_docx-comment-json"))
        .arg(&docx_path)
        .arg("--fail-on-missing-extended=true")
        .output()
        .expect("binary should execute");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("word/commentsExtended.xml"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_can_hide_raw_text_fields_when_include_raw_is_false() {
    let docx_path = write_docx_fixture(
        "no-raw.docx",
        Some(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:body>
                  <w:p>
                    <w:commentRangeStart w:id="5" />
                    <w:r><w:t>対象箇所</w:t></w:r>
                    <w:commentRangeEnd w:id="5" />
                  </w:p>
                </w:body>
              </w:document>"#,
        ),
        r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:comment w:id="5" w:author="Alice">
                <w:p><w:r><w:t>本文</w:t></w:r></w:p>
              </w:comment>
            </w:comments>"#,
        None,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_docx-comment-json"))
        .arg(&docx_path)
        .arg("--include-raw=false")
        .output()
        .expect("binary should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert_eq!(json["comments"][0]["comment"]["raw_text"], Value::Null);
    assert_eq!(json["comments"][0]["anchor"]["raw_text"], Value::Null);
}

#[test]
fn cli_reports_malformed_document_xml_as_diagnostic_and_continues() {
    let docx_path = write_docx_fixture(
        "malformed-document.docx",
        Some("<w:document"),
        r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:comment w:id="5" w:author="Alice">
                <w:p><w:r><w:t>本文</w:t></w:r></w:p>
              </w:comment>
            </w:comments>"#,
        None,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_docx-comment-json"))
        .arg(&docx_path)
        .output()
        .expect("binary should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert_eq!(json["comments"][0]["anchor"]["raw_text"], Value::Null);
    assert_eq!(json["diagnostics"]["warnings"][0]["code"], "malformed_xml");
    assert_eq!(
        json["diagnostics"]["warnings"][0]["source_part"],
        "word/document.xml"
    );
}

#[test]
fn cli_falls_back_to_paragraph_anchor_when_comment_range_is_missing() {
    let docx_path = write_docx_fixture(
        "reference-fallback.docx",
        Some(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:body>
                  <w:p>
                    <w:r><w:t>この段落全体を確認してください</w:t></w:r>
                    <w:r><w:commentReference w:id="5" /></w:r>
                  </w:p>
                </w:body>
              </w:document>"#,
        ),
        r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:comment w:id="5" w:author="Alice">
                <w:p><w:r><w:t>本文</w:t></w:r></w:p>
              </w:comment>
            </w:comments>"#,
        None,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_docx-comment-json"))
        .arg(&docx_path)
        .output()
        .expect("binary should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert_eq!(
        json["comments"][0]["anchor"]["raw_text"],
        "この段落全体を確認してください"
    );
    assert_eq!(
        json["diagnostics"]["warnings"][0]["code"],
        "anchor_fallback_reference"
    );
}

#[test]
fn cli_outputs_image_anchor_with_caption_context() {
    let docx_path = write_docx_fixture(
        "image-anchor.docx",
        Some(
            r#"<w:document
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
                    <w:r><w:commentReference w:id="5" /></w:r>
                  </w:p>
                  <w:p>
                    <w:r><w:t>図1 システム構成図</w:t></w:r>
                  </w:p>
                </w:body>
              </w:document>"#,
        ),
        r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:comment w:id="5" w:author="Alice">
                <w:p><w:r><w:t>画像コメント</w:t></w:r></w:p>
              </w:comment>
            </w:comments>"#,
        None,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_docx-comment-json"))
        .arg(&docx_path)
        .output()
        .expect("binary should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert_eq!(json["comments"][0]["anchor"]["target_type"], "image");
    assert_eq!(
        json["comments"][0]["anchor"]["context"]["caption"]["text"],
        "図1 システム構成図"
    );
    assert_eq!(json["comments"][0]["anchor"]["image"]["name"], "Picture 3");
}

#[test]
fn cli_outputs_table_anchor_type() {
    let docx_path = write_docx_fixture(
        "table-anchor.docx",
        Some(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
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
                          <w:r><w:commentReference w:id="5" /></w:r>
                        </w:p>
                      </w:tc>
                    </w:tr>
                  </w:tbl>
                  <w:p>
                    <w:r><w:t>表1 テストデータ</w:t></w:r>
                  </w:p>
                </w:body>
              </w:document>"#,
        ),
        r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:comment w:id="5" w:author="Alice">
                <w:p><w:r><w:t>表コメント</w:t></w:r></w:p>
              </w:comment>
            </w:comments>"#,
        None,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_docx-comment-json"))
        .arg(&docx_path)
        .output()
        .expect("binary should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert_eq!(json["comments"][0]["anchor"]["target_type"], "table");
    assert_eq!(
        json["comments"][0]["anchor"]["context"]["caption"]["text"],
        "表1 テストデータ"
    );
    assert_eq!(json["comments"][0]["anchor"]["table"]["index"], 1);
    assert_eq!(
        json["comments"][0]["anchor"]["locator"]["heading_path"][0],
        "表セクション"
    );
    assert_eq!(
        json["comments"][0]["anchor"]["locator"]["table_cell"]["row_index"],
        1
    );
    assert_eq!(
        json["comments"][0]["anchor"]["locator"]["table_cell"]["column_index"],
        1
    );
}

#[test]
fn cli_restores_replies_using_last_comment_paragraph_para_id() {
    let docx_path = write_docx_fixture(
        "reply-last-para-id.docx",
        Some(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:body>
                  <w:p>
                    <w:commentRangeStart w:id="5" />
                    <w:r><w:t>本文</w:t></w:r>
                    <w:commentRangeEnd w:id="5" />
                  </w:p>
                </w:body>
              </w:document>"#,
        ),
        r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                       xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
              <w:comment w:id="5" w:author="Alice">
                <w:p w15:paraId="p-root-1"><w:r><w:t>親コメント1段落目</w:t></w:r></w:p>
                <w:p w15:paraId="p-root-last"><w:r><w:t>親コメント2段落目</w:t></w:r></w:p>
              </w:comment>
              <w:comment w:id="6" w:author="Bob">
                <w:p w15:paraId="p-reply-last"><w:r><w:t>返信コメント</w:t></w:r></w:p>
              </w:comment>
            </w:comments>"#,
        Some(
            r#"<w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
                <w15:commentEx w15:paraId="p-root-last" />
                <w15:commentEx w15:paraId="p-reply-last" w15:paraIdParent="p-root-last" />
              </w15:commentsEx>"#,
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_docx-comment-json"))
        .arg(&docx_path)
        .output()
        .expect("binary should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert_eq!(json["comments"][0]["comment_id"], "5");
    assert_eq!(json["comments"][0]["replies"][0]["comment_id"], "6");
}

#[test]
fn cli_restores_replies_when_comments_extended_part_uses_alternate_name() {
    let temp_dir = tempdir().expect("temp dir should be created");
    let path = temp_dir.path().join("alt-extended.docx");
    let file = File::create(&path).expect("fixture file should be created");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    zip.start_file("word/document.xml", options)
        .expect("document.xml entry should be created");
    zip.write_all(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
             <w:body>
               <w:p>
                 <w:commentRangeStart w:id="5" />
                 <w:r><w:t>本文</w:t></w:r>
                 <w:commentRangeEnd w:id="5" />
               </w:p>
             </w:body>
           </w:document>"#
            .as_bytes(),
    )
    .expect("document.xml should be written");

    zip.start_file("word/comments.xml", options)
        .expect("comments.xml entry should be created");
    zip.write_all(
        r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                        xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
              <w:comment w:id="5" w:author="Alice">
                <w:p w15:paraId="p-root-last"><w:r><w:t>親コメント</w:t></w:r></w:p>
              </w:comment>
              <w:comment w:id="6" w:author="Bob">
                <w:p w15:paraId="p-reply-last"><w:r><w:t>返信コメント</w:t></w:r></w:p>
              </w:comment>
            </w:comments>"#
            .as_bytes(),
    )
    .expect("comments.xml should be written");

    zip.start_file("word/commentsExtended2.xml", options)
        .expect("alternate commentsExtended entry should be created");
    zip.write_all(
        r#"<w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
              <w15:commentEx w15:paraId="p-root-last" />
              <w15:commentEx w15:paraId="p-reply-last" w15:paraIdParent="p-root-last" />
            </w15:commentsEx>"#
            .as_bytes(),
    )
    .expect("alternate commentsExtended should be written");

    zip.finish().expect("zip should be finalized");
    let _ = temp_dir.keep();

    let output = Command::new(env!("CARGO_BIN_EXE_docx-comment-json"))
        .arg(&path)
        .output()
        .expect("binary should execute");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert_eq!(json["comments"][0]["replies"][0]["comment_id"], "6");
}

fn write_docx_fixture(
    filename: &str,
    document_xml: Option<&str>,
    comments_xml: &str,
    comments_extended_xml: Option<&str>,
) -> std::path::PathBuf {
    let temp_dir = tempdir().expect("temp dir should be created");
    let path = temp_dir.path().join(filename);
    let file = File::create(&path).expect("fixture file should be created");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    if let Some(document_xml) = document_xml {
        zip.start_file("word/document.xml", options)
            .expect("document.xml entry should be created");
        zip.write_all(document_xml.as_bytes())
            .expect("document.xml should be written");
    }

    zip.start_file("word/comments.xml", options)
        .expect("comments.xml entry should be created");
    zip.write_all(comments_xml.as_bytes())
        .expect("comments.xml should be written");

    if let Some(comments_extended_xml) = comments_extended_xml {
        zip.start_file("word/commentsExtended.xml", options)
            .expect("commentsExtended.xml entry should be created");
        zip.write_all(comments_extended_xml.as_bytes())
            .expect("commentsExtended.xml should be written");
    }

    zip.finish().expect("zip should be finalized");
    let _ = temp_dir.keep();
    path
}
