pub mod app;
pub mod docx;
pub mod error;
pub mod output;
pub mod parse;

pub use app::{
    BuildOptions, ExtractOptions, build_output_document, extract_output_document,
    serialize_output_json,
};
