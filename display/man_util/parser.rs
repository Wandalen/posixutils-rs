use std::io::{BufRead, BufReader, Cursor};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MdocParseError {
    #[error("Error reading line {line_number}: {error}")]
    ReadLine {
        line_number: usize,
        error: std::io::Error,
    },
}

pub struct MdocPage {}

pub struct MdocParser {}

impl MdocParser {
    pub fn parse(input: impl AsRef<[u8]>) -> (MdocPage, Vec<MdocParseError>) {
        let mut parse_errors = vec![];
        let buf = BufReader::new(Cursor::new(input.as_ref()));

        let mdoc_page = MdocPage {};

        for (line_number, line_res) in buf.lines().enumerate() {
            // Attempt to convert string
            let mut line = match line_res {
                Ok(line) => line,
                Err(error) => {
                    parse_errors.push(MdocParseError::ReadLine { line_number, error });
                    continue;
                }
            };

            // Handle comments
            if line.starts_with("\\\"") || line.starts_with(".\\\"") {
                continue;
            } else if let Some(comment_start) = line.find("\\\"") {
                line = line[..comment_start].to_string()
            }

            println!("{line_number:<2}: {line}")
        }

        (mdoc_page, parse_errors)
    }
}
