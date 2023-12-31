//! This module holds functionality specific to using Github's REST API.

use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::{Read, Write};

// non-std crates
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Method;
use serde::Deserialize;
use serde_json;

// project specific modules/crates
use crate::clang_tools::{clang_format::FormatAdvice, clang_tidy::TidyNotification};
use crate::common_fs::FileObj;
use crate::git::{get_diff, open_repo, parse_diff, parse_diff_from_buf};

use super::RestApiClient;

/// A structure to work with Github REST API.
pub struct GithubApiClient {
    /// The HTTP request client to be used for all REST API calls.
    client: Client,

    /// The CI run's event payload from the webhook that triggered the workflow.
    event_payload: Option<serde_json::Value>,

    /// The name of the event that was triggered when running cpp_linter.
    pub event_name: String,

    /// The value of the `GITHUB_API_URL` environment variable.
    api_url: String,

    /// The value of the `GITHUB_REPOSITORY` environment variable.
    repo: Option<String>,

    /// The value of the `GITHUB_SHA` environment variable.
    sha: Option<String>,

    /// The value of the `ACTIONS_STEP_DEBUG` environment variable.
    pub debug_enabled: bool,
}

impl Default for GithubApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GithubApiClient {
    pub fn new() -> Self {
        GithubApiClient {
            client: reqwest::blocking::Client::new(),
            event_payload: {
                let event_payload_path = env::var("GITHUB_EVENT_PATH");
                if event_payload_path.is_ok() {
                    let file_buf = &mut String::new();
                    OpenOptions::new()
                        .read(true)
                        .open(event_payload_path.unwrap())
                        .unwrap()
                        .read_to_string(file_buf)
                        .unwrap();
                    Some(serde_json::from_str(file_buf.as_str()).unwrap())
                } else {
                    None
                }
            },
            event_name: env::var("GITHUB_EVENT_NAME").unwrap_or(String::from("default")),
            api_url: env::var("GITHUB_API_URL").unwrap_or(String::from("https://api.github.com")),
            repo: if let Ok(val) = env::var("GITHUB_REPOSITORY") {
                Some(val)
            } else {
                None
            },
            sha: if let Ok(val) = env::var("GITHUB_SHA") {
                Some(val)
            } else {
                None
            },
            debug_enabled: match env::var("ACTIONS_STEP_DEBUG") {
                Ok(val) => val == "true",
                Err(_) => false,
            },
        }
    }
}

// implement the RestApiClient trait for the GithubApiClient
impl RestApiClient for GithubApiClient {
    fn set_exit_code(
        &self,
        checks_failed: i32,
        format_checks_failed: Option<i32>,
        tidy_checks_failed: Option<i32>,
    ) -> i32 {
        if let Ok(gh_out) = env::var("GITHUB_OUTPUT") {
            let mut gh_out_file = OpenOptions::new()
                .append(true)
                .open(gh_out)
                .expect("GITHUB_OUTPUT file could not be opened");
            if let Err(e) = writeln!(
                gh_out_file,
                "checks-failed={}\nformat-checks-failed={}\ntidy-checks-failed={}",
                checks_failed,
                format_checks_failed.unwrap_or(0),
                tidy_checks_failed.unwrap_or(0),
            ) {
                panic!("Could not write to GITHUB_OUTPUT file: {}", e);
            }
        }
        log::info!(
            "{} clang-format-checks-failed",
            format_checks_failed.unwrap_or(0)
        );
        log::info!(
            "{} clang-tidy-checks-failed",
            tidy_checks_failed.unwrap_or(0)
        );
        log::info!("{checks_failed} checks-failed");
        checks_failed
    }

    fn make_headers(&self, use_diff: Option<bool>) -> HeaderMap<HeaderValue> {
        let gh_token = env::var("GITHUB_TOKEN");
        let mut headers = HeaderMap::new();
        let return_fmt = "application/vnd.github.".to_owned()
            + if use_diff.is_some_and(|val| val) {
                "diff"
            } else {
                "text+json"
            };
        headers.insert("Accept", return_fmt.parse().unwrap());
        let user_agent =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0";
        headers.insert("User-Agent", user_agent.parse().unwrap());
        if let Ok(token) = gh_token {
            headers.insert("Authorization", token.parse().unwrap());
        }
        headers
    }

    fn get_list_of_changed_files(
        &self,
        extensions: &[&str],
        ignored: &[String],
        not_ignored: &[String],
    ) -> Vec<FileObj> {
        if env::var("CI").is_ok_and(|val| val.as_str() == "true")
            && self.repo.is_some()
            && self.sha.is_some()
        {
            // get diff from Github REST API
            let url = format!(
                "{}/repos/{}/{}",
                self.api_url,
                self.repo.as_ref().unwrap(),
                if self.event_name == "pull_request" {
                    let pr_number = &self.event_payload.as_ref().unwrap()["number"];
                    format!("pulls/{}", &pr_number)
                } else {
                    format!("commits/{}", self.sha.as_ref().unwrap())
                }
            );
            let response = self
                .client
                .get(url)
                .headers(self.make_headers(Some(true)))
                .send()
                .unwrap()
                .bytes()
                .unwrap();

            parse_diff_from_buf(&response, extensions, ignored, not_ignored)
        } else {
            // get diff from libgit2 API
            let repo = open_repo(".")
                .expect("Please ensure the repository is checked out before running cpp-linter.");
            let list = parse_diff(&get_diff(&repo), extensions, ignored, not_ignored);
            list
        }
    }

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
    ) {
        let (comment, format_checks_failed, tidy_checks_failed) =
            self.make_comment(files, format_advice, tidy_advice);
        if thread_comments != "false" {
            // post thread comment for PR or push event
            if let Some(repo) = &self.repo {
                let is_pr = self.event_name == "pull_request";
                let base_url = format!("{}/repos/{}/", &self.api_url, &repo);
                let comments_url = if is_pr {
                    format!(
                        "{base_url}issues/{}",
                        &self.event_payload.as_ref().unwrap()["number"]
                    )
                } else {
                    format!("{base_url}/commits/{}", &self.sha.as_ref().unwrap())
                };

                // get count of comments
                let request = self
                    .client
                    .get(&comments_url)
                    .headers(self.make_headers(None))
                    .send();
                if let Ok(response) = request {
                    let json = response.json::<serde_json::Value>().unwrap();
                    let count = if is_pr {
                        json["comments"].as_u64().unwrap()
                    } else {
                        json["commit"]["comment_count"].as_u64().unwrap()
                    };
                    let user_id: u64 = 41898282;
                    self.update_comment(
                        &format!("{}/comments", &comments_url),
                        &comment,
                        count,
                        user_id,
                        no_lgtm,
                        format_checks_failed + tidy_checks_failed == 0,
                        thread_comments == "update",
                    );
                } else {
                    let error = request.unwrap_err();
                    if let Some(status) = error.status() {
                        log::error!(
                            "Could not get comment count. Got response {:?} from {comments_url}",
                            status
                        );
                    } else {
                        log::error!("attempt GET comment count failed");
                    }
                }
            }
        }
        if file_annotations {
            self.post_annotations(files, format_advice, tidy_advice, style, lines_changed_only);
        }
        if step_summary {
            self.post_step_summary(&comment);
        }
        self.set_exit_code(
            format_checks_failed + tidy_checks_failed,
            Some(format_checks_failed),
            Some(tidy_checks_failed),
        );
    }
}

impl GithubApiClient {
    fn post_step_summary(&self, comment: &String) {
        if let Ok(gh_out) = env::var("GITHUB_STEP_SUMMARY") {
            let mut gh_out_file = OpenOptions::new()
                .append(true)
                .open(gh_out)
                .expect("GITHUB_STEP_SUMMARY file could not be opened");
            if let Err(e) = writeln!(gh_out_file, "\n{}\n", comment,) {
                panic!("Could not write to GITHUB_STEP_SUMMARY file: {}", e);
            }
        }
    }

    fn post_annotations(
        &self,
        files: &[FileObj],
        format_advice: &[FormatAdvice],
        tidy_advice: &[Vec<TidyNotification>],
        style: &str,
        lines_changed_only: u8,
    ) {
        if !format_advice.is_empty() {
            // formalize the style guide name
            let style_guide =
                if ["google", "chromium", "microsoft", "mozilla", "webkit"].contains(&style) {
                    // capitalize the first letter
                    let mut char_iter = style.chars();
                    match char_iter.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + char_iter.as_str(),
                    }
                } else if style == "llvm" || style == "gnu" {
                    style.to_ascii_uppercase()
                } else {
                    String::from("Custom")
                };

            // iterate over clang-format and post applicable annotations (according to line filtering)
            for (index, advice) in format_advice.iter().enumerate() {
                // get the ranges of lines for the corresponding file
                let ranges = if lines_changed_only == 0 {
                    None
                } else if lines_changed_only == 1 {
                    Some(&files[index].added_ranges)
                } else {
                    Some(&files[index].diff_chunks)
                };

                // assemble a list of line numbers (as strings)
                let mut lines: Vec<usize> = Vec::new();
                for replacement in &advice.replacements {
                    if let Some(line_int) = replacement.line {
                        if !lines.contains(&line_int) {
                            if let Some(line_ranges) = ranges {
                                for line_range in line_ranges {
                                    if line_range.contains(&line_int.try_into().unwrap()) {
                                        lines.push(line_int);
                                        break;
                                    }
                                }
                            } else {
                                lines.push(line_int);
                            }
                        }
                    }
                }
                // post annotation if any applicable lines were format
                if !lines.is_empty() {
                    println!(
                        "::notice file={name},title=Run clang-format on {name}::File {name} does not conform to {style_guide} style guidelines. (lines {line_set})",
                        name = &files[index].name.to_string_lossy().replace('\\', "/"),
                        line_set = lines.iter().map(|val| val.to_string()).collect::<Vec<_>>().join(","),
                    );
                }
            }
        } // end format_advice iterations

        // iterate over clang-tidy advice and post annotations
        // The tidy_advice vector is parallel to the files vector; meaning it serves as a file filterer.
        // lines are already filter as specified to clang-tidy CLI.
        for (index, advice) in tidy_advice.iter().enumerate() {
            for note in advice {
                if note.filename == files[index].name.to_string_lossy().replace('\\', "/") {
                    println!(
                        "::{severity} file={file},line={line},title={file}:{line}:{cols} [{diag}]::{info}",
                        severity = if note.severity == *"note" { "notice".to_string() } else {note.severity.clone()},
                        file = note.filename,
                        line = note.line,
                        cols = note.cols,
                        diag = note.diagnostic,
                        info = note.rationale,
                    );
                }
            }
        }
    }

    /// update existing comment or remove old comment(s) and post a new comment
    #[allow(clippy::too_many_arguments)]
    fn update_comment(
        &self,
        url: &String,
        comment: &String,
        count: u64,
        user_id: u64,
        no_lgtm: bool,
        is_lgtm: bool,
        update_only: bool,
    ) {
        let comment_url =
            self.remove_bot_comments(url, user_id, count, !update_only || (is_lgtm && no_lgtm));
        #[allow(clippy::nonminimal_bool)] // an inaccurate assessment
        if (is_lgtm && !no_lgtm) || !is_lgtm {
            let payload = HashMap::from([("body", comment)]);
            log::debug!("payload body:\n{:?}", payload);
            let req_meth = if comment_url.is_some() {
                Method::PATCH
            } else {
                Method::POST
            };
            if let Ok(response) = self
                .client
                .request(
                    req_meth.clone(),
                    if let Some(_url) = comment_url {
                        _url
                    } else {
                        url.to_string()
                    },
                )
                .headers(self.make_headers(None))
                .json(&payload)
                .send()
            {
                log::info!(
                    "Got {} response from {:?}ing comment",
                    response.status(),
                    req_meth,
                );
            }
        }
    }

    fn remove_bot_comments(
        &self,
        url: &String,
        count: u64,
        user_id: u64,
        delete: bool,
    ) -> Option<String> {
        let mut page = 1;
        let mut comment_url = None;
        let mut total = count;
        while total > 0 {
            let request = self.client.get(format!("{url}/?page={page}")).send();
            if request.is_err() {
                log::error!("Failed to get list of existing comments");
                return None;
            } else if let Ok(response) = request {
                let payload: JsonCommentsPayload = response.json().unwrap();
                let mut comment_count = 0;
                for comment in payload.comments {
                    if comment.body.starts_with("<!-- cpp linter action -->")
                        && comment.user.id == user_id
                    {
                        log::debug!(
                            "comment id {} from user {} ({})",
                            comment.id,
                            comment.user.login,
                            comment.user.id,
                        );
                        #[allow(clippy::nonminimal_bool)] // an inaccurate assessment
                        if delete || (!delete && comment_url.is_none()) {
                            // if not updating: remove all outdated comments
                            // if updating: remove all outdated comments except the last one

                            // use last saved comment_url (if not None) or current comment url
                            let del_url = if let Some(last_url) = &comment_url {
                                last_url
                            } else {
                                &comment.url
                            };
                            if let Ok(response) = self
                                .client
                                .delete(del_url)
                                .headers(self.make_headers(None))
                                .send()
                            {
                                log::info!(
                                    "Got {} from DELETE {}",
                                    response.status(),
                                    del_url.strip_prefix(&self.api_url).unwrap(),
                                )
                            } else {
                                log::error!("Unable to remove old bot comment");
                                return None; // exit early as this is most likely due to rate limit.
                            }
                        }
                        if !delete {
                            comment_url = Some(comment.url)
                        }
                    }
                    comment_count += 1;
                }
                total -= comment_count;
                page += 1;
            }
        }
        comment_url
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct JsonCommentsPayload {
    comments: Vec<Comment>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
struct Comment {
    pub id: i64,
    pub url: String,
    pub body: String,
    pub user: User,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
struct User {
    pub login: String,
    pub id: u64,
}
