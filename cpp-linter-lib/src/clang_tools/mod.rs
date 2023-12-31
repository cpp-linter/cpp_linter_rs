//! This crate holds the functionality related to running clang-format and/or
//! clang-tidy.

use std::{env::current_dir, fs, path::PathBuf, process::Command};

// non-std crates
use lenient_semver;
use semver::Version;
use which::{which, which_in};

// project-specific modules/crates
use super::common_fs::FileObj;
use crate::logger::{end_log_group, start_log_group};
pub mod clang_format;
use clang_format::{run_clang_format, FormatAdvice};
pub mod clang_tidy;
use clang_tidy::{run_clang_tidy, CompilationDatabase, TidyNotification};

/// Fetch the path to a clang tool by `name` (ie `"clang-tidy"` or `"clang-format"`) and
/// `version`.
///
/// The specified `version` can be either
///
/// - a full or partial semantic version specification
/// - a path to a directory containing the executable binary `name`d
///
/// If the executable is not found using the specified `version`, then the tool is
/// sought only by it's `name`.
///
/// The only reason this function would return an error is if the specified tool is not
/// installed or present on the system (nor in the `$PATH` environment variable).
pub fn get_clang_tool_exe(name: &str, version: &str) -> Result<PathBuf, &'static str> {
    if version.is_empty() {
        // The default CLI value is an empty string.
        // Thus, we should use whatever is installed and added to $PATH.
        if let Ok(cmd) = which(name) {
            return Ok(cmd);
        } else {
            return Err("Could not find clang tool by name");
        }
    }
    if let Ok(semver) = lenient_semver::parse_into::<Version>(version) {
        // `version` specified has at least a major version number
        if let Ok(cmd) = which(format!("{}-{}", name, semver.major)) {
            Ok(cmd)
        } else if let Ok(cmd) = which(name) {
            // USERS SHOULD MAKE SURE THE PROPER VERSION IS INSTALLED BEFORE USING CPP-LINTER!!!
            // This block essentially ignores the version specified as a fail-safe.
            //
            // On Windows, the version's major number is typically not appended to the name of
            // the executable (or symlink for executable), so this is useful in that scenario.
            // On Unix systems, this block is not likely reached. Typically, installing clang
            // will produce a symlink to the executable with the major version appended to the
            // name.
            return Ok(cmd);
        } else {
            return Err("Could not find clang tool by name and version");
        }
    } else {
        // `version` specified is not a semantic version; treat as path/to/bin
        if let Ok(exe_path) = which_in(name, Some(version), current_dir().unwrap()) {
            Ok(exe_path)
        } else {
            Err("Could not find clang tool by path")
        }
    }
}

/// Runs clang-tidy and/or clang-format and returns the parsed output from each.
///
/// The returned list of [`FormatAdvice`] is parallel to the `files` list passed in
/// here. The returned 2D list of [`TidyNotification`] is also parallel on the first
/// dimension. The second dimension is a list of notes specific to a translation unit
/// (each element of `files`).
///
/// If `tidy_checks` is `"-*"` then clang-tidy is not executed.
/// If `style` is a blank string (`""`), then clang-format is not executed.
pub fn capture_clang_tools_output(
    files: &Vec<FileObj>,
    version: &str,
    tidy_checks: &str,
    style: &str,
    lines_changed_only: u8,
    database: Option<PathBuf>,
    extra_args: Option<Vec<&str>>,
) -> (Vec<FormatAdvice>, Vec<Vec<TidyNotification>>) {
    // find the executable paths for clang-tidy and/or clang-format and show version
    // info as debugging output.
    let clang_tidy_command = if tidy_checks != "-*" {
        let cmd = get_clang_tool_exe("clang-tidy", version).unwrap();
        log::debug!(
            "{} --version\n{}",
            &cmd.to_string_lossy(),
            String::from_utf8_lossy(&Command::new(&cmd).arg("--version").output().unwrap().stdout)
        );
        Some(cmd)
    } else {
        None
    };
    let clang_format_command = if !style.is_empty() {
        let cmd = get_clang_tool_exe("clang-format", version).unwrap();
        log::debug!(
            "{} --version\n{}",
            &cmd.to_string_lossy(),
            String::from_utf8_lossy(&Command::new(&cmd).arg("--version").output().unwrap().stdout)
        );
        Some(cmd)
    } else {
        None
    };

    // parse database (if provided) to match filenames when parsing clang-tidy's stdout
    let database_json: Option<CompilationDatabase> = if let Some(db_path) = &database {
        if let Ok(db_str) = fs::read(db_path) {
            Some(
                serde_json::from_str::<CompilationDatabase>(
                    String::from_utf8(db_str).unwrap().as_str(),
                )
                .unwrap(),
            )
        } else {
            None
        }
    } else {
        None
    };

    // iterate over the discovered files and run the clang tools
    let mut all_format_advice: Vec<clang_format::FormatAdvice> = Vec::with_capacity(files.len());
    let mut all_tidy_advice: Vec<Vec<clang_tidy::TidyNotification>> =
        Vec::with_capacity(files.len());
    for file in files {
        start_log_group(format!("Analyzing {}", file.name.to_string_lossy()));
        if let Some(tidy_cmd) = &clang_tidy_command {
            all_tidy_advice.push(run_clang_tidy(
                &mut Command::new(tidy_cmd),
                file,
                tidy_checks,
                lines_changed_only,
                &database,
                &extra_args,
                &database_json,
            ));
        }
        if let Some(format_cmd) = &clang_format_command {
            all_format_advice.push(run_clang_format(
                &mut Command::new(format_cmd),
                file,
                style,
                lines_changed_only,
            ));
        }
        end_log_group();
    }
    (all_format_advice, all_tidy_advice)
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::get_clang_tool_exe;

    const TOOL_NAME: &str = "clang-format";

    #[test]
    fn get_exe_by_version() {
        let clang_version = env::var("CLANG_VERSION").unwrap_or("16".to_string());
        let tool_exe = get_clang_tool_exe(TOOL_NAME, clang_version.as_str());
        println!("tool_exe: {:?}", tool_exe);
        assert!(tool_exe.is_ok_and(|val| val
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string()
            .contains(TOOL_NAME)));
    }

    #[test]
    fn get_exe_by_default() {
        let tool_exe = get_clang_tool_exe(TOOL_NAME, "");
        println!("tool_exe: {:?}", tool_exe);
        assert!(tool_exe.is_ok_and(|val| val
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string()
            .contains(TOOL_NAME)));
    }

    use which::which;

    #[test]
    fn get_exe_by_path() {
        let clang_version = which(TOOL_NAME).unwrap();
        let bin_path = clang_version.parent().unwrap().to_str().unwrap();
        println!("binary exe path: {bin_path}");
        let tool_exe = get_clang_tool_exe(TOOL_NAME, bin_path);
        println!("tool_exe: {:?}", tool_exe);
        assert!(tool_exe.is_ok_and(|val| val
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string()
            .contains(TOOL_NAME)));
    }
}
