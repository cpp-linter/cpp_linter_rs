//! This module holds functionality specific to running clang-format and parsing it's
//! output.

use std::process::Command;

// non-std crates
use serde::Deserialize;
use serde_xml_rs::de::Deserializer;

// project-specific crates/modules
use crate::common_fs::{get_line_cols_from_offset, FileObj};

/// A Structure used to deserialize clang-format's XML output.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "replacements")]
pub struct FormatAdvice {
    /// A list of [`Replacement`]s that clang-tidy wants to make.
    #[serde(rename = "$value")]
    pub replacements: Vec<Replacement>,
}

/// A single replacement that clang-format wants to make.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Replacement {
    /// The byte offset where the replacement will start.
    pub offset: usize,

    /// The amount of bytes that will be removed.
    pub length: usize,

    /// The bytes (UTF-8 encoded) that will be added at the [`Replacement::offset`] position.
    #[serde(rename = "$value")]
    pub value: Option<String>,

    /// The line number described by the [`Replacement::offset`].
    ///
    /// This value is not provided by the XML output, but we calculate it after
    /// deserialization.
    pub line: Option<usize>,

    /// The column number on the line described by the [`Replacement::offset`].
    ///
    /// This value is not provided by the XML output, but we calculate it after
    /// deserialization.
    pub cols: Option<usize>,
}

/// Run clang-tidy for a specific `file`, then parse and return it's XML output.
pub fn run_clang_format(
    cmd: &mut Command,
    file: &FileObj,
    style: &str,
    lines_changed_only: u8,
) -> FormatAdvice {
    cmd.args(["--style", style, "--output-replacements-xml"]);
    if lines_changed_only > 0 {
        let ranges = if lines_changed_only == 2 {
            &file.diff_chunks
        } else {
            &file.added_ranges
        };
        for range in ranges {
            cmd.arg(format!("--lines={}:{}", range.start(), range.end()));
        }
    }
    cmd.arg(file.name.to_string_lossy().as_ref());
    log::info!(
        "Running \"{} {}\"",
        cmd.get_program().to_string_lossy(),
        cmd.get_args()
            .map(|x| x.to_str().unwrap())
            .collect::<Vec<&str>>()
            .join(" ")
    );
    let output = cmd.output().unwrap();
    if !output.stderr.is_empty() || !output.status.success() {
        log::debug!(
            "clang-format raised the follow errors:\n{}",
            String::from_utf8(output.stderr).unwrap()
        );
    }
    // log::debug!(
    //     "clang-format XML output:\n{}",
    //     String::from_utf8(output.stdout.clone()).unwrap()
    // );
    if output.stdout.is_empty() {
        return FormatAdvice {
            replacements: vec![],
        };
    }
    let xml = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .collect::<Vec<&str>>()
        .join("");
    let config = serde_xml_rs::ParserConfig::new()
        .trim_whitespace(false)
        .whitespace_to_characters(true)
        .ignore_root_level_whitespace(true);
    let event_reader = serde_xml_rs::EventReader::new_with_config(xml.as_bytes(), config);
    let mut format_advice: FormatAdvice =
        FormatAdvice::deserialize(&mut Deserializer::new(event_reader)).unwrap_or(FormatAdvice {
            replacements: vec![],
        });
    if !format_advice.replacements.is_empty() {
        for replacement in &mut format_advice.replacements {
            let (line_number, columns) = get_line_cols_from_offset(&file.name, replacement.offset);
            replacement.line = Some(line_number);
            replacement.cols = Some(columns);
        }
    }
    format_advice
}

#[cfg(test)]
mod tests {
    use super::{FormatAdvice, Replacement};
    use serde::Deserialize;

    #[test]
    fn parse_xml() {
        let xml_raw = r#"<?xml version='1.0'?>
<replacements xml:space='preserve' incomplete_format='false'>
<replacement offset='113' length='5'>&#10;      </replacement>
<replacement offset='147' length='0'> </replacement>
<replacement offset='161' length='0'></replacement>
<replacement offset='165' length='19'>&#10;&#10;</replacement>
</replacements>"#;
        //since whitespace is part of the elements' body, we need to remove the LFs first
        let xml = xml_raw.lines().collect::<Vec<&str>>().join("");

        let expected = FormatAdvice {
            replacements: vec![
                Replacement {
                    offset: 113,
                    length: 5,
                    value: Some(String::from("\n      ")),
                    line: None,
                    cols: None,
                },
                Replacement {
                    offset: 147,
                    length: 0,
                    value: Some(String::from(" ")),
                    line: None,
                    cols: None,
                },
                Replacement {
                    offset: 161,
                    length: 0,
                    value: None,
                    line: None,
                    cols: None,
                },
                Replacement {
                    offset: 165,
                    length: 19,
                    value: Some(String::from("\n\n")),
                    line: None,
                    cols: None,
                },
            ],
        };
        let config = serde_xml_rs::ParserConfig::new()
            .trim_whitespace(false)
            .whitespace_to_characters(true)
            .ignore_root_level_whitespace(true);
        let event_reader = serde_xml_rs::EventReader::new_with_config(xml.as_bytes(), config);
        let document =
            FormatAdvice::deserialize(&mut serde_xml_rs::de::Deserializer::new(event_reader))
                .unwrap();
        assert_eq!(expected, document);
    }
}
