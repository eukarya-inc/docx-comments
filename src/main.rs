use std::path::PathBuf;

use clap::{ArgAction, Parser};
use docx_comment_json::{ExtractOptions, extract_output_document, serialize_output_json};

#[derive(Debug, Parser)]
#[command(name = "docx-comment-json")]
struct Cli {
    input: PathBuf,
    #[arg(long, action = ArgAction::SetTrue)]
    pretty: bool,
    #[arg(long, default_value_t = false, action = ArgAction::Set)]
    fail_on_missing_extended: bool,
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    include_raw: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), docx_comment_json::error::AppError> {
    let cli = Cli::parse();
    let output = extract_output_document(
        &cli.input,
        &ExtractOptions {
            pretty: cli.pretty,
            include_raw: cli.include_raw,
            fail_on_missing_extended: cli.fail_on_missing_extended,
        },
    )?;
    let json = serialize_output_json(&output, cli.pretty)?;
    println!("{json}");
    Ok(())
}
