use std::env;
use std::error::Error;

use cpp_linter::cli::parse_ignore;
use cpp_linter::github_api::GithubApiClient;

// needed to use trait implementations (ie `get_list_of_changed_files()`)
use cpp_linter::rest_api::RestApiClient;

pub fn main() -> Result<(), Box<dyn Error>> {
    env::set_var("GITHUB_SHA", "950ff0b690e1903797c303c5fc8d9f3b52f1d3c5");
    env::set_var("GITHUB_REPOSITORY", "cpp-linter/cpp-linter");
    let client_controller = GithubApiClient::new();

    let extensions = vec!["cpp", "hpp"];
    let (ignored, not_ignored) = parse_ignore(&Vec::from_iter(["target", ".github"]));

    env::set_var("CI", "true"); // needed for get_list_of_changed_files() to use REST API
    let files = client_controller.get_list_of_changed_files(&extensions, &ignored, &not_ignored);

    for file in &files {
        println!("{}", file.name.to_string_lossy());
        println!("lines with additions: {:?}", file.added_lines);
        println!("ranges of added lines: {:?}", file.added_ranges);
        println!("ranges of diff hunks: {:?}", file.diff_chunks);
    }
    println!("found {} files in diff", files.len());
    Ok(())
}
