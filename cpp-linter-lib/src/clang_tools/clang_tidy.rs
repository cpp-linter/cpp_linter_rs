//! This module holds functionality specific to running clang-tidy and parsing it's
//! output.

use std::{
    env::{consts::OS, current_dir},
    path::PathBuf,
    process::Command,
};

// non-std crates
use regex::Regex;
use serde::Deserialize;

// project-specific modules/crates
use crate::common_fs::{normalize_path, FileObj};

/// Used to deserialize a JSON compilation database
#[derive(Deserialize, Debug)]
pub struct CompilationDatabase {
    /// A list of [`CompilationUnit`]
    units: Vec<CompilationUnit>,
}

/// Used to deserialize a json compilation database's translation unit.
///
/// The only purpose this serves is to normalize relative paths for build systems that
/// use/need relative paths (ie ninja).
#[derive(Deserialize, Debug)]
struct CompilationUnit {
    /// The directory of the build environment
    directory: String,

    /// The file path of the translation unit.
    ///
    /// Sometimes, this is relative to the build [`CompilationUnit::directory`].
    ///
    /// This is typically the path that clang-tidy uses in its stdout (for a dry run).
    /// So, having this information helps with matching clang-tidy's stdout with the
    /// repository files.
    file: String,
}

/// A structure that represents a single notification parsed from clang-tidy's stdout.
pub struct TidyNotification {
    /// The file's path and name (supposedly relative to the repository root folder).
    pub filename: String,

    /// The line number from which the notification originated.
    pub line: u32,

    /// The column offset on the line from which the notification originated.
    pub cols: u32,

    /// The severity (ie error/warning/note) of the [`TidyNotification::diagnostic`]
    /// that caused the notification.
    pub severity: String,

    /// A helpful message explaining why the notification exists.
    pub rationale: String,

    /// The diagnostic name as used when configuring clang-tidy.
    pub diagnostic: String,

    /// A code block that points directly to the origin of the notification.
    ///
    /// Sometimes, this code block doesn't exist. Sometimes, it contains suggested
    /// fixes/advice. This information is purely superfluous.
    pub suggestion: Vec<String>,
}

/// Parses clang-tidy stdout.
///
/// Here it helps to have the JSON database deserialized for normalizing paths present
/// in the notifications.
fn parse_tidy_output(
    tidy_stdout: &[u8],
    database_json: &Option<CompilationDatabase>,
) -> Vec<TidyNotification> {
    let note_header = Regex::new(r"^(.+):(\d+):(\d+):\s(\w+):(.*)\[([a-zA-Z\d\-\.]+)\]$").unwrap();
    let mut notification = None;
    let mut result = Vec::new();
    for line in String::from_utf8(tidy_stdout.to_vec()).unwrap().lines() {
        if let Some(captured) = note_header.captures(line) {
            if let Some(note) = notification {
                result.push(note);
            }

            // normalize the filename path and try to make it relative to the repo root
            let mut filename = PathBuf::from(&captured[1]);
            if filename.is_relative() {
                // if database was given try to use that first
                if let Some(db_json) = &database_json {
                    let mut found_unit = false;
                    for unit in &db_json.units {
                        if unit.file == captured[0] {
                            filename =
                                normalize_path(&PathBuf::from_iter([&unit.directory, &unit.file]));
                            found_unit = true;
                            break;
                        }
                    }
                    if !found_unit {
                        // file was not a named unit in the database;
                        // try to normalize path as if relative to working directory.
                        // NOTE: This shouldn't happen with a properly formed JSON database
                        filename = normalize_path(&PathBuf::from_iter([
                            &current_dir().unwrap(),
                            &filename,
                        ]));
                    }
                } else {
                    // still need to normalize the relative path despite missing database info.
                    // let's assume the file is relative to current working directory.
                    filename =
                        normalize_path(&PathBuf::from_iter([&current_dir().unwrap(), &filename]));
                }
            }
            assert!(filename.is_absolute());
            if filename.is_absolute() {
                filename = filename
                    .strip_prefix(current_dir().unwrap())
                    .expect("cannot determine filename by relative path.")
                    .to_path_buf();
            }

            notification = Some(TidyNotification {
                filename: filename.to_string_lossy().to_string().replace('\\', "/"),
                line: captured[2].parse::<u32>().unwrap(),
                cols: captured[3].parse::<u32>().unwrap(),
                severity: String::from(&captured[4]),
                rationale: String::from(&captured[5]),
                diagnostic: String::from(&captured[6]),
                suggestion: Vec::new(),
            });
        } else if let Some(note) = &mut notification {
            // append lines of code that are part of
            // the previous line's notification
            note.suggestion.push(line.to_string());
        }
    }
    if let Some(note) = notification {
        result.push(note);
    }
    result
}

/// Run clang-tidy, then parse and return it's output.
pub fn run_clang_tidy(
    cmd: &mut Command,
    file: &FileObj,
    checks: &str,
    lines_changed_only: u8,
    database: &Option<PathBuf>,
    extra_args: &Option<Vec<&str>>,
    database_json: &Option<CompilationDatabase>,
) -> Vec<TidyNotification> {
    if !checks.is_empty() {
        cmd.args(["-checks", checks]);
    }
    if let Some(db) = database {
        cmd.args(["-p", &db.to_string_lossy()]);
    }
    if let Some(extras) = extra_args {
        for arg in extras {
            cmd.args(["--extra-arg", format!("\"{}\"", arg).as_str()]);
        }
    }
    if lines_changed_only > 0 {
        let ranges = file.get_ranges(lines_changed_only);
        let filter = format!(
            "[{{\"name\":{:?},\"lines\":{:?}}}]",
            &file
                .name
                .to_string_lossy()
                .replace('/', if OS == "windows" { "\\" } else { "/" }),
            ranges
                .iter()
                .map(|r| [r.start(), r.end()])
                .collect::<Vec<_>>()
        );
        cmd.args(["--line-filter", filter.as_str()]);
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
    log::debug!(
        "Output from clang-tidy:\n{}",
        String::from_utf8(output.stdout.to_vec()).unwrap()
    );
    if !output.stderr.is_empty() {
        log::debug!(
            "clang-tidy made the following summary:\n{}",
            String::from_utf8(output.stderr).unwrap()
        );
    }
    parse_tidy_output(&output.stdout, database_json)
}

#[cfg(test)]
mod test {
    #[test]
    fn test_capture() {
        let src = "tests/demo/demo.hpp:11:11: warning: use a trailing return type for this function [modernize-use-trailing-return-type]";
        let pat =
            regex::Regex::new(r"^(.+):(\d+):(\d+):\s(\w+):(.*)\[([a-zA-Z\d\-\.]+)\]$").unwrap();
        let cap = pat.captures(src).unwrap();
        assert_eq!(
            cap.get(0).unwrap().as_str(),
            format!(
                "{}:{}:{}: {}:{}[{}]",
                cap.get(1).unwrap().as_str(),
                cap.get(2).unwrap().as_str(),
                cap.get(3).unwrap().as_str(),
                cap.get(4).unwrap().as_str(),
                cap.get(5).unwrap().as_str(),
                cap.get(6).unwrap().as_str()
            )
            .as_str()
        )
    }
}
