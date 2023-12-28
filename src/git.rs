//! This module is primarily used to parse diff blobs.
//!
//! It can also be used (locally) to get a list of files changes from either the last
//! commit or the next commit's staging area.
//!
//! This also includes a private module that is used as a fallback (brute force)
//! mechanism when parsing diffs fail using libgit2. NOTE: parsing a diff from a buffer
//! (str or bytes) only happens in CI or when libgit2 cannot be used to initialize a
//! repository.

use std::{ops::RangeInclusive, path::PathBuf};

// non-std crates
use git2::{Diff, Error, Patch, Repository};

// project specific modules/crates
use crate::common_fs::{is_source_or_ignored, FileObj};

/// This (re-)initializes the repository located in the specified `path`.
///
/// This is actually not used in CI for file permissions and ownership reasons.
/// Rather this is only (supposed to be) used when executed on a local developer
/// machine.
pub fn open_repo(path: &str) -> Result<Repository, Error> {
    Repository::open(PathBuf::from(path).as_path())
}

/// Fetches the SHA1 of the commit for the specified [`git2::Repository`].
///
/// The optionally specified `depth` can be used to traverse the tree a number of times
/// since the current `"HEAD"`.
fn get_sha(repo: &Repository, depth: Option<u32>) -> Result<git2::Object<'_>, Error> {
    match depth {
        Some(int) => repo.revparse_single(format!("HEAD~{}", int).as_str()),
        None => repo.revparse_single("HEAD"),
    }
}

/// Fetch the [`git2::Diff`] about a given [`git2::Repository`].
///
/// This is actually not used in CI for file permissions and ownership reasons.
/// Rather this is only (supposed to be) used when executed on a local developer
/// machine.
///
/// If there are files staged for a commit, then the resulting [`Diff`] will describe
/// the staged changes. However, if there are no staged changes, then the last commit's
/// [`Diff`] is returned.
pub fn get_diff(repo: &Repository) -> git2::Diff {
    let head = get_sha(repo, None).unwrap().peel_to_tree().unwrap();
    let mut has_staged_files = false;
    for entry in repo.statuses(None).unwrap().iter() {
        if entry.status().bits()
            & (git2::Status::INDEX_NEW.bits()
                | git2::Status::INDEX_MODIFIED.bits()
                | git2::Status::INDEX_RENAMED.bits())
            > 0
        {
            has_staged_files = true;
            break;
        }
    }

    if has_staged_files {
        // get diff for staged files only
        repo.diff_tree_to_index(Some(&head), None, None)
            .expect("Could not get diff for current changes in local repo index")
    } else {
        // get diff for last commit only
        let base = get_sha(repo, Some(1)).unwrap().peel_to_tree().unwrap();
        repo.diff_tree_to_tree(Some(&base), Some(&head), None)
            .expect("could not get diff for last commit")
    }
}

/// Parses a patch for a single file in a diff.
///
/// Returns the list of line numbers that have additions and the ranges spanning each
/// chunk present in the `patch`.
fn parse_patch(patch: &Patch) -> (Vec<u32>, Vec<RangeInclusive<u32>>) {
    let mut additions = Vec::new();
    let mut diff_hunks = Vec::new();
    for hunk_idx in 0..patch.num_hunks() {
        let (hunk, line_count) = patch.hunk(hunk_idx).unwrap();
        diff_hunks.push(RangeInclusive::new(
            hunk.new_start(),
            hunk.new_start() + hunk.new_lines(),
        ));
        for line in 0..line_count {
            let diff_line = patch.line_in_hunk(hunk_idx, line).unwrap();
            if diff_line.origin_value() == git2::DiffLineType::Addition {
                additions.push(diff_line.new_lineno().unwrap());
            }
        }
    }
    (additions, diff_hunks)
}

/// Parses a given [`git2::Diff`] and returns a list of [`FileObj`]s.
///
/// The specified list of `extensions`, `ignored` and `not_ignored` files are used as
/// filters to expedite the process and only focus on the data cpp_linter can use.
pub fn parse_diff(
    diff: &git2::Diff,
    extensions: &[&str],
    ignored: &[String],
    not_ignored: &[String],
) -> Vec<FileObj> {
    let mut files: Vec<FileObj> = Vec::new();
    for file_idx in 0..diff.deltas().count() {
        let diff_delta = diff.get_delta(file_idx).unwrap();
        let file_path = diff_delta.new_file().path().unwrap().to_path_buf();
        if [
            git2::Delta::Added,
            git2::Delta::Modified,
            git2::Delta::Renamed,
        ]
        .contains(&diff_delta.status())
            && is_source_or_ignored(&file_path, extensions, ignored, not_ignored)
        {
            let (added_lines, diff_chunks) =
                parse_patch(&Patch::from_diff(diff, file_idx).unwrap().unwrap());
            files.push(FileObj::from(file_path, added_lines, diff_chunks));
        }
    }
    files
}

/// Same as [`parse_diff`] but takes a buffer of bytes instead of a [`git2::Diff`].
///
/// In the case that libgit2 fails to parse the buffer of bytes, a private algorithm is
/// used. In such a case, brute force parsing the diff as a string can be costly. So, a
/// log warning and error are output when this occurs. Please report this instance for
/// troubleshooting/diagnosis as this likely means the diff is malformed or there is a
/// bug in libgit2 source.
pub fn parse_diff_from_buf(
    buff: &[u8],
    extensions: &[&str],
    ignored: &[String],
    not_ignored: &[String],
) -> Vec<FileObj> {
    if let Ok(diff_obj) = &Diff::from_buffer(buff) {
        parse_diff(diff_obj, extensions, ignored, not_ignored)
    } else {
        log::warn!("libgit2 failed to parse the diff");
        brute_force_parse_diff::parse_diff(
            &String::from_utf8_lossy(buff),
            extensions,
            ignored,
            not_ignored,
        )
    }
}

mod brute_force_parse_diff {
    //! A private module to house the brute force algorithms of parsing a diff as a string.
    //! This module is only intended as a fall back mechanism when [super::parse_diff_from_buf]
    //! fails to use libgit2 C bindings.
    //!
    //! Since this is a fail safe, there are log messages that indicate when it is used.
    //! Any instance where this mechanism is used should be reported as it is likely a bug
    //! in libgit2 source.

    use regex::Regex;
    use std::{ops::RangeInclusive, path::PathBuf};

    use crate::common_fs::{is_source_or_ignored, FileObj};

    fn get_filename_from_front_matter(front_matter: &str) -> Option<&str> {
        let diff_file_name = Regex::new(r"(?m)^\+\+\+\sb?/(.*)$").unwrap();
        let diff_renamed_file = Regex::new(r"(?m)^rename to (.*)$").unwrap();
        let diff_binary_file = Regex::new(r"(?m)^Binary\sfiles\s").unwrap();
        if let Some(captures) = diff_file_name.captures(front_matter) {
            return Some(captures.get(1).unwrap().as_str());
        }
        if front_matter.trim_start().starts_with("similarity") {
            if let Some(captures) = diff_renamed_file.captures(front_matter) {
                return Some(captures.get(1).unwrap().as_str());
            }
        }
        if diff_binary_file.is_match(front_matter) {
            log::warn!("Unrecognized diff starting with:\n{}", front_matter);
        }
        None
    }

    /// A regex pattern used in multiple functions
    static HUNK_INFO_PATTERN: &str = r"(?m)@@\s\-\d+,\d+\s\+(\d+,\d+)\s@@";

    /// Parses a single file's patch containing one or more hunks
    /// Returns a 3-item tuple:
    /// - the line numbers that contain additions
    /// - the ranges of lines that span each hunk
    fn parse_patch(patch: &str) -> (Vec<u32>, Vec<RangeInclusive<u32>>) {
        let mut diff_chunks = Vec::new();
        let mut additions = Vec::new();

        let hunk_info = Regex::new(HUNK_INFO_PATTERN).unwrap();
        if let Some(hunk_headers) = hunk_info.captures(patch) {
            for (index, (hunk, header)) in
                hunk_info.split(patch).zip(hunk_headers.iter()).enumerate()
            {
                if index == 0 {
                    continue; // we don't need the whole match, just the capture groups
                }
                let new_range: Vec<u32> = header
                    .unwrap()
                    .as_str()
                    .split(',')
                    .take(2)
                    .map(|val| val.parse::<u32>().unwrap())
                    .collect();
                let start_line = new_range[0];
                let end_range = new_range[1];
                let mut line_numb_in_diff = start_line;
                diff_chunks.push(RangeInclusive::new(start_line, start_line + end_range));
                for (line_index, line) in hunk.split('\n').enumerate() {
                    if line.starts_with('+') {
                        additions.push(line_numb_in_diff);
                    }
                    if line_index > 0 && !line.starts_with('-') {
                        line_numb_in_diff += 1;
                    }
                }
            }
        }
        (additions, diff_chunks)
    }

    pub fn parse_diff(
        diff: &str,
        extensions: &[&str],
        ignored: &[String],
        not_ignored: &[String],
    ) -> Vec<FileObj> {
        log::error!("Using brute force diff parsing!");
        let mut results = Vec::new();
        let diff_file_delimiter = Regex::new(r"(?m)^diff --git a/.*$").unwrap();
        let hunk_info = Regex::new(HUNK_INFO_PATTERN).unwrap();

        let file_diffs = diff_file_delimiter.split(diff);
        for file_diff in file_diffs {
            if file_diff.is_empty() || file_diff.starts_with("deleted file") {
                continue;
            }
            if let Some(first_hunk) = hunk_info.find(file_diff) {
                let front_matter = &file_diff[..first_hunk.start()];
                if let Some(file_name) = get_filename_from_front_matter(front_matter) {
                    let file_path = PathBuf::from(file_name);
                    if is_source_or_ignored(&file_path, extensions, ignored, not_ignored) {
                        let (added_lines, diff_chunks) =
                            parse_patch(&file_diff[first_hunk.start()..]);
                        results.push(FileObj::from(file_path, added_lines, diff_chunks));
                    }
                }
            } else {
                // file has no changed content. moving on
                continue;
            }
        }
        results
    }

    // ******************* UNIT TESTS ***********************
    #[cfg(test)]
    mod test {

        use super::parse_diff;
        use crate::{common_fs::FileObj, git::parse_diff_from_buf, logger};

        static RENAMED_DIFF: &str = r"diff --git a/tests/demo/some source.cpp b/tests/demo/some source.cpp
similarity index 100%
rename from /tests/demo/some source.cpp
rename to /tests/demo/some source.cpp\n";

        static RENAMED_DIFF_WITH_CHANGES: &str = r#"diff --git a/tests/demo/some source.cpp b/tests/demo/some source.cpp
similarity index 99%
rename from /tests/demo/some source.cpp
rename to /tests/demo/some source.cpp
@@ -3,7 +3,7 @@
\n \n \n-#include "iomanip"
+#include <iomanip>\n \n \n \n"#;

        #[test]
        fn parse_renamed_diff() {
            let diff_buf = RENAMED_DIFF.as_bytes();
            let files = parse_diff_from_buf(diff_buf, &[&String::from("cpp")], &[], &[]);
            assert!(files.is_empty());
        }

        #[test]
        fn parse_renamed_diff_with_patch() {
            let diff_buf = RENAMED_DIFF_WITH_CHANGES.as_bytes();
            let files = parse_diff_from_buf(diff_buf, &[&String::from("cpp")], &[], &[]);
            assert!(!files.is_empty());
        }

        /// Used to parse the same string buffer using both libgit2 and brute force regex.
        /// Returns 2 vectors of [FileObj] that should be equivalent.
        fn setup_parsed(buf: &str, extensions: &[&str]) -> (Vec<FileObj>, Vec<FileObj>) {
            logger::init().unwrap_or_default();
            (
                parse_diff_from_buf(buf.as_bytes(), extensions, &[], &[]),
                parse_diff(buf, extensions, &[], &[]),
            )
        }

        fn assert_files_eq(files_from_a: &Vec<FileObj>, files_from_b: &Vec<FileObj>) {
            assert_eq!(files_from_a.len(), files_from_b.len());
            for (a, b) in files_from_a.iter().zip(files_from_b) {
                assert_eq!(a.name, b.name);
                assert_eq!(a.added_lines, b.added_lines);
                assert_eq!(a.added_ranges, b.added_ranges);
                assert_eq!(a.diff_chunks, b.diff_chunks);
            }
        }

        #[test]
        fn parse_typical_diff() {
            let diff_buf = "diff --git a/path/for/Some file.cpp b/path/to/Some file.cpp\n\
                            --- a/path/for/Some file.cpp\n\
                            +++ b/path/to/Some file.cpp\n\
                            @@ -3,7 +3,7 @@\n \n \n \n\
                            -#include <some_lib/render/animation.hpp>\n\
                            +#include <some_lib/render/animations.hpp>\n \n \n \n";

            let (files_from_buf, files_from_str) = setup_parsed(diff_buf, &[&String::from("cpp")]);
            assert!(!files_from_buf.is_empty());
            assert_files_eq(&files_from_buf, &files_from_str);
        }

        #[test]
        fn parse_binary_diff() {
            let diff_buf = "diff --git a/some picture.png b/some picture.png\n\
                new file mode 100644\n\
                Binary files /dev/null and b/some picture.png differ\n";

            let (files_from_buf, files_from_str) = setup_parsed(diff_buf, &[&String::from("png")]);
            assert!(files_from_buf.is_empty());
            assert_files_eq(&files_from_buf, &files_from_str);
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        env::{current_dir, set_current_dir},
        fs::read,
    };

    use git2::build::CheckoutBuilder;
    use git2::{ApplyLocation, Diff, IndexAddOption, Repository};

    // used to setup a testing stage
    fn clone_repo(url: &str, sha: &str, path: &str, patch_path: Option<&str>) {
        let repo = Repository::clone(url, path).unwrap();
        let commit = repo.revparse_single(sha).unwrap();
        repo.checkout_tree(
            &commit,
            Some(CheckoutBuilder::new().force().recreate_missing(true)),
        )
        .unwrap();
        repo.set_head_detached(commit.id()).unwrap();
        if let Some(patch) = patch_path {
            let diff = Diff::from_buffer(&read(patch).unwrap()).unwrap();
            repo.apply(&diff, ApplyLocation::Both, None).unwrap();
            let mut index = repo.index().unwrap();
            index
                .add_all(["tests/demo/demo.*"], IndexAddOption::DEFAULT, None)
                .unwrap();
            index.write().unwrap();
        }
    }

    use tempfile::{tempdir, TempDir};

    use crate::{cli::parse_ignore, github_api::GithubApiClient, rest_api::RestApiClient};

    fn get_temp_dir() -> TempDir {
        let tmp = tempdir().unwrap();
        println!("Using temp folder at {:?}", tmp.path());
        tmp
    }

    fn checkout_cpp_linter_py_repo(
        sha: &str,
        extensions: &[&str],
        tmp: &TempDir,
        patch_path: Option<&str>,
    ) -> Vec<crate::common_fs::FileObj> {
        let url = "https://github.com/cpp-linter/cpp-linter";
        clone_repo(
            url,
            sha,
            tmp.path().as_os_str().to_str().unwrap(),
            patch_path,
        );
        let rest_api_client = GithubApiClient::new();
        let (ignored, not_ignored) = parse_ignore(&["target"]);
        set_current_dir(tmp).unwrap();
        rest_api_client.get_list_of_changed_files(extensions, &ignored, &not_ignored)
    }

    #[test]
    fn with_no_changed_sources() {
        // commit with no modified C/C++ sources
        let sha = "0c236809891000b16952576dc34de082d7a40bf3";
        let cur_dir = current_dir().unwrap();
        let tmp = get_temp_dir();
        let extensions = vec!["cpp", "hpp"];
        let files = checkout_cpp_linter_py_repo(sha, &extensions, &tmp, None);
        assert!(files.is_empty());
        set_current_dir(cur_dir).unwrap(); // prep to delete temp_folder
        drop(tmp); // delete temp_folder
    }

    #[test]
    fn with_changed_sources() {
        // commit with modified C/C++ sources
        let sha = "950ff0b690e1903797c303c5fc8d9f3b52f1d3c5";
        let cur_dir = current_dir().unwrap();
        let tmp = get_temp_dir();
        let extensions = vec!["cpp", "hpp"];
        let files = checkout_cpp_linter_py_repo(sha, &extensions, &tmp, None);
        assert_eq!(files.len(), 2);
        for file in files {
            assert!(extensions.contains(
                &file
                    .name
                    .extension()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
                    .as_str()
            ));
        }
        set_current_dir(cur_dir).unwrap(); // prep to delete temp_folder
        drop(tmp); // delete temp_folder
    }

    #[test]
    fn with_staged_changed_sources() {
        // commit with no modified C/C++ sources
        let sha = "0c236809891000b16952576dc34de082d7a40bf3";
        let cur_dir = current_dir().unwrap();
        let tmp = get_temp_dir();
        let extensions = vec!["cpp", "hpp"];
        let files = checkout_cpp_linter_py_repo(
            sha,
            &extensions,
            &tmp,
            Some("tests/capture_tools_output/cpp-linter/cpp-linter/test_git_lib.patch"),
        );
        assert!(!files.is_empty());
        for file in files {
            assert!(extensions.contains(
                &file
                    .name
                    .extension()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
                    .as_str()
            ));
        }
        set_current_dir(cur_dir).unwrap(); // prep to delete temp_folder
        drop(tmp); // delete temp_folder
    }
}
