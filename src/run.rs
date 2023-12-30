//! This module is the native backend of the cpp-linter package written in Rust.
//!
//! In python, this module is exposed as `cpp_linter.run` that has 1 function exposed:
//! [`main()`].

use std::env;
use std::path::{Path, PathBuf};

// non-std crates
use log::{set_max_level, LevelFilter};
#[cfg(features = "openssl-vendored")]
use openssl_probe;
use pyo3::prelude::*;

// project specific modules/crates
use crate::clang_tools::capture_clang_tools_output;
use crate::cli::{get_arg_parser, parse_ignore};
use crate::common_fs::{list_source_files, FileObj};
use crate::github_api::GithubApiClient;
use crate::logger::{self, end_log_group, start_log_group};
use crate::rest_api::RestApiClient;

#[cfg(features = "openssl-vendored")]
fn probe_ssl_certs() {
    openssl_probe::init_ssl_cert_env_vars();
}

#[cfg(not(openssl_probe))]
fn probe_ssl_certs() {}

/// This is the backend entry point for console applications.
///
/// The idea here is that all functionality is implemented in Rust. However, passing
/// command line arguments is done differently in Python or Rust.
///
/// - In python, the `sys.argv` list is passed from the `cpp_linter.entry_point` script
///   to `run.main()`.
/// - In rust, the [`std::env::args`] is passed to `run::main()` in the binary driver
///   source `bin.rs`.
///
/// This is done because of the way the python entry point is invoked. If [`std::env::args`]
/// is used instead of python's `sys.argv`, then the list of strings includes the entry point
/// alias ("path/to/cpp-linter.exe"). Thus, the parser in [`crate::cli`] will halt on an error
/// because it is not configured to handle positional arguments.
#[pyfunction]
pub fn main(args: Vec<String>) -> i32 {
    probe_ssl_certs();

    let arg_parser = get_arg_parser();
    let args = arg_parser.get_matches_from(args);

    logger::init().unwrap();

    let root_path = args.get_one::<String>("repo-root").unwrap();
    if root_path != &String::from(".") {
        env::set_current_dir(Path::new(root_path)).unwrap();
    }

    let database_path = if let Some(database) = args.get_one::<String>("database") {
        if !database.is_empty() {
            Some(PathBuf::from(database).canonicalize().unwrap())
        } else {
            None
        }
    } else {
        None
    };

    let rest_api_client = GithubApiClient::new();
    let verbosity = args.get_one::<String>("verbosity").unwrap().as_str() == "debug";
    set_max_level(if verbosity || rest_api_client.debug_enabled {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
    log::info!("Processing event {}", rest_api_client.event_name);

    let extensions = args
        .get_many::<String>("extensions")
        .unwrap()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();
    let ignore = args
        .get_many::<String>("ignore")
        .unwrap()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();
    let (ignored, not_ignored) = parse_ignore(&ignore);

    let lines_changed_only = match args
        .get_one::<String>("lines-changed-only")
        .unwrap()
        .as_str()
    {
        "false" => 0,
        "true" => 1,
        "diff" => 2,
        _ => unreachable!(),
    };
    let files_changed_only = args.get_flag("files-changed-only");

    start_log_group(String::from("Get list of specified source files"));
    let files: Vec<FileObj> = if lines_changed_only != 0 || files_changed_only {
        // parse_diff(github_rest_api_payload)
        rest_api_client.get_list_of_changed_files(&extensions, &ignored, &not_ignored)
    } else {
        // walk the folder and look for files with specified extensions according to ignore values.
        list_source_files(&extensions, &ignored, &not_ignored, ".")
    };
    log::info!("Giving attention to the following files:");
    for file in &files {
        log::info!("  ./{}", file.name.to_string_lossy().replace('\\', "/"));
    }
    end_log_group();

    let style = args.get_one::<String>("style").unwrap();
    let (format_advice, tidy_advice) = capture_clang_tools_output(
        &files,
        args.get_one::<String>("version").unwrap(),
        args.get_one::<String>("tidy-checks").unwrap(),
        style,
        lines_changed_only,
        database_path,
        if let Ok(extra_args) = args.try_get_many::<String>("extra-arg") {
            extra_args.map(|extras| extras.map(|val| val.as_str()).collect())
        } else {
            None
        },
    );
    start_log_group(String::from("Posting feedback"));
    let no_lgtm = args.get_flag("no-lgtm");
    let step_summary = args.get_flag("step-summary");
    let thread_comments = args.get_one::<String>("thread-comments").unwrap();
    let file_annotations = args.get_flag("file-annotations");
    rest_api_client.post_feedback(
        &files,
        &format_advice,
        &tidy_advice,
        thread_comments,
        no_lgtm,
        step_summary,
        file_annotations,
        style,
        lines_changed_only,
    );
    end_log_group();
    0
}
