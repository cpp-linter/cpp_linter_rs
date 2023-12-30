//! This crate is the home of functionality that uses the REST API of various git-based
//! servers.
//!
//! Currently, only Github is supported.

use std::path::PathBuf;

// non-std crates
use reqwest::header::{HeaderMap, HeaderValue};

// project specific modules/crates
pub mod github_api;
use crate::clang_tools::{clang_format::FormatAdvice, clang_tidy::TidyNotification};
use crate::common_fs::FileObj;

/// A custom trait that templates necessary functionality with a Git server's REST API.
pub trait RestApiClient {
    /// A way to set output variables specific to cpp_linter executions in CI.
    fn set_exit_code(
        &self,
        checks_failed: i32,
        format_checks_failed: Option<i32>,
        tidy_checks_failed: Option<i32>,
    ) -> i32;

    /// A convenience method to create the headers attached to all REST API calls.
    ///
    /// If an authentication token is provided, this method shall include the relative
    /// information in the returned [HeaderMap].
    fn make_headers(&self, use_diff: Option<bool>) -> HeaderMap<HeaderValue>;

    /// A way to get the list of changed files using REST API calls. It is this method's
    /// job to parse diff blobs and return a list of changed files.
    ///
    /// The context of the file changes are subject to the type of event in which
    /// cpp_linter package is used.
    fn get_list_of_changed_files(
        &self,
        extensions: &[&str],
        ignored: &[String],
        not_ignored: &[String],
    ) -> Vec<FileObj>;

    /// Makes a comment in MarkDown syntax based on the concerns in `format_advice` and
    /// `tidy_advice` about the given set of `files`.
    ///
    /// This method has a default definition and should not need to be redefined by
    /// implementors.
    ///
    /// Returns the markdown comment as a string as well as the total count of
    /// `format_checks_failed` and `tidy_checks_failed` (in respective order).
    fn make_comment(
        &self,
        files: &[FileObj],
        format_advice: &[FormatAdvice],
        tidy_advice: &[Vec<TidyNotification>],
    ) -> (String, i32, i32) {
        let mut comment = String::from("<!-- cpp linter action -->\n# Cpp-Linter Report ");
        let mut format_checks_failed = 0;
        let mut tidy_checks_failed = 0;
        let mut format_comment = String::new();
        for (index, fmt_advice) in format_advice.iter().enumerate() {
            if !fmt_advice.replacements.is_empty() {
                format_comment.push_str(
                    &format!(
                        "- {}\n",
                        files[index].name.to_string_lossy().replace('\\', "/")
                    )
                    .to_string(),
                );
                format_checks_failed += 1;
            }
        }

        let mut tidy_comment = String::new();
        for (index, tidy_notes) in tidy_advice.iter().enumerate() {
            for tidy_note in tidy_notes {
                let file_path = PathBuf::from(&tidy_note.filename);
                if file_path == files[index].name {
                    tidy_comment.push_str(&format!("- {}\n\n", tidy_note.filename));
                    tidy_comment.push_str(&format!(
                        "   <strong>{filename}:{line}:{cols}:</strong> {severity}: [{diagnostic}]\n   > {rationale}\n{concerned_code}",
                        filename = tidy_note.filename,
                        line = tidy_note.line,
                        cols = tidy_note.cols,
                        severity = tidy_note.severity,
                        diagnostic = tidy_note.diagnostic,
                        rationale = tidy_note.rationale,
                        concerned_code = if tidy_note.suggestion.is_empty() {String::from("")} else {
                            format!("\n   ```{ext}\n   {suggestion}\n   ```\n",
                                ext = file_path.extension().expect("file extension was not determined").to_string_lossy(),
                                suggestion = tidy_note.suggestion.join("\n    "),
                            ).to_string()
                        },
                    ).to_string());
                    tidy_checks_failed += 1;
                }
            }
        }
        if format_checks_failed > 0 || tidy_checks_failed > 0 {
            comment.push_str(":warning:\nSome files did not pass the configured checks!\n");
            if format_checks_failed > 0 {
                comment.push_str(&format!("\n<details><summary>clang-format reports: <strong>{} file(s) not formatted</strong></summary>\n\n{}\n</details>", format_checks_failed, &format_comment));
            }
            if tidy_checks_failed > 0 {
                comment.push_str(&format!("\n<details><summary>clang-tidy reports: <strong>{} concern(s)</strong></summary>\n\n{}\n</details>", tidy_checks_failed, tidy_comment));
            }
        } else {
            comment.push_str(":heavy_check_mark:\nNo problems need attention.");
        }
        comment.push_str("\n\nHave any feedback or feature suggestions? [Share it here.](https://github.com/cpp-linter/cpp-linter-action/issues)");
        log::info!("{format_checks_failed} clang-format-checks-failed");
        log::info!("{tidy_checks_failed} clang-tidy-checks-failed");
        log::info!(
            "{} checks-failed",
            format_checks_failed + tidy_checks_failed
        );
        (comment, format_checks_failed, tidy_checks_failed)
    }

    /// A way to post feedback in the form of `thread_comments`, `file_annotations`, and
    /// `step_summary`.
    ///
    /// The given `files` should've been gathered from `get_list_of_changed_files()` or
    /// `list_source_files()`.
    ///
    /// The `format_advice` and `tidy_advice` should be a result of parsing output from
    /// clang-format and clang-tidy (see `capture_clang_tools_output()`).
    ///
    /// All other parameters correspond to CLI arguments.
    #[allow(clippy::too_many_arguments)]
    fn post_feedback(
        &self,
        files: &[FileObj],
        format_advice: &[FormatAdvice],
        tidy_advice: &[Vec<TidyNotification>],
        thread_comments: &str,
        no_lgtm: bool,
        step_summary: bool,
        file_annotations: bool,
        style: &str,
        lines_changed_only: u8,
    );
}
