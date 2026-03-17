use std::fs::File;
use std::io::Read;
use std::path::Path;

use zip::ZipArchive;

use crate::error::AppError;

pub const DOCUMENT_XML_PATH: &str = "word/document.xml";
pub const COMMENTS_XML_PATH: &str = "word/comments.xml";
pub const COMMENTS_EXTENDED_XML_PATH: &str = "word/commentsExtended.xml";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocxParts {
    pub document_xml: Option<String>,
    pub comments_xml: Option<String>,
    pub comments_extended_xml: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadDocxParts {
    pub parts: DocxParts,
    pub missing_parts: Vec<String>,
}

pub fn read_docx_parts(path: &Path) -> Result<ReadDocxParts, AppError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let document_xml = read_optional_part(&mut archive, DOCUMENT_XML_PATH)?;
    let comments_xml = read_part_with_fallback(&mut archive, COMMENTS_XML_PATH, |name| {
        is_word_xml_part(name) && file_stem_matches(name, "comments")
    })?;
    let comments_extended_xml =
        read_part_with_fallback(&mut archive, COMMENTS_EXTENDED_XML_PATH, |name| {
            is_word_xml_part(name)
                && (name.to_ascii_lowercase().contains("commentsextended")
                    || name.to_ascii_lowercase().contains("commentsex"))
        })?;

    let comments_xml = comments_xml.ok_or(AppError::MissingRequiredPart(COMMENTS_XML_PATH))?;

    let missing_parts = [DOCUMENT_XML_PATH, COMMENTS_EXTENDED_XML_PATH]
        .into_iter()
        .filter(|part| match *part {
            DOCUMENT_XML_PATH => document_xml.is_none(),
            COMMENTS_EXTENDED_XML_PATH => comments_extended_xml.is_none(),
            _ => false,
        })
        .map(str::to_string)
        .collect();

    Ok(ReadDocxParts {
        parts: DocxParts {
            document_xml,
            comments_xml: Some(comments_xml),
            comments_extended_xml,
        },
        missing_parts,
    })
}

fn read_optional_part(
    archive: &mut ZipArchive<File>,
    path: &'static str,
) -> Result<Option<String>, AppError> {
    match archive.by_name(path) {
        Ok(mut file) => {
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            Ok(Some(contents))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(error) => Err(AppError::Zip(error)),
    }
}

fn read_part_with_fallback<F>(
    archive: &mut ZipArchive<File>,
    preferred_path: &'static str,
    predicate: F,
) -> Result<Option<String>, AppError>
where
    F: Fn(&str) -> bool,
{
    if let Some(contents) = read_optional_part(archive, preferred_path)? {
        return Ok(Some(contents));
    }

    let fallback_path = archive
        .file_names()
        .find(|name| predicate(name))
        .map(str::to_string);

    match fallback_path {
        Some(path) => read_dynamic_part(archive, &path),
        None => Ok(None),
    }
}

fn read_dynamic_part(
    archive: &mut ZipArchive<File>,
    path: &str,
) -> Result<Option<String>, AppError> {
    match archive.by_name(path) {
        Ok(mut file) => {
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            Ok(Some(contents))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(error) => Err(AppError::Zip(error)),
    }
}

fn is_word_xml_part(name: &str) -> bool {
    name.starts_with("word/") && name.ends_with(".xml")
}

fn file_stem_matches(name: &str, stem: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == format!("word/{stem}.xml") || lower.starts_with(&format!("word/{stem}"))
}
