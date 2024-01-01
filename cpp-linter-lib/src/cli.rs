//! This module holds the Command Line Interface design.

use std::fs;

// non-std crates
use clap::builder::FalseyValueParser;
use clap::{Arg, ArgAction, ArgMatches, Command};

/// Builds and returns the Command Line Interface's argument parsing object.
pub fn get_arg_parser() -> Command {
    Command::new("cpp-linter")
        .arg(
            Arg::new("verbosity")
                .long("verbosity")
                .short('v')
                .default_value("info")
                .value_parser(["debug", "info"])
                .long_help(
                    "This controls the action's verbosity in the workflow's logs.
Supported options are defined by the `logging-level <logging-levels>`_.
This option does not affect the verbosity of resulting
thread comments or file annotations.
",
                ),
        )
        .arg(
            Arg::new("database")
                .long("database")
                .short('p')
                .long_help(
            "The path that is used to read a compile command database.
For example, it can be a CMake build directory in which a file named
compile_commands.json exists (set ``CMAKE_EXPORT_COMPILE_COMMANDS`` to ``ON``).
When no build path is specified, a search for compile_commands.json will be
attempted through all parent paths of the first input file. See
https://clang.llvm.org/docs/HowToSetupToolingForLLVM.html for an
example of setting up Clang Tooling on a source tree.",
        ))
        .arg(
            Arg::new("style")
                .short('s')
                .long("style")
                .default_value("llvm")
                .long_help(
                    "The style rules to use.

- Set this to ``file`` to have clang-format use the closest relative
  .clang-format file.
- Set this to a blank string (``''``) to disable using clang-format
  entirely.
",
                ),
        )
        .arg(
            Arg::new("tidy-checks")
                .short('c')
                .long("tidy-checks")
                .default_value(
                    "boost-*,bugprone-*,performance-*,readability-*,portability-*,modernize-*,clang-analyzer-*,cppcoreguidelines-*",
                )
                .long_help(
                    "A comma-separated list of globs with optional ``-`` prefix.
Globs are processed in order of appearance in the list.
Globs without ``-`` prefix add checks with matching names to the set,
globs with the ``-`` prefix remove checks with matching names from the set of
enabled checks. This option's value is appended to the value of the 'Checks'
option in a .clang-tidy file (if any).

- It is possible to disable clang-tidy entirely by setting this option to
  ``'-*'``.
- It is also possible to rely solely on a .clang-tidy config file by
  specifying this option as a blank string (``''``).

See also clang-tidy docs for more info.
",
                ),
        )
        .arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .default_value("")
                .long_help(
                    "The desired version of the clang tools to use. Accepted options are
strings which can be 8, 9, 10, 11, 12, 13, 14, 15, 16, 17.

- Set this option to a blank string (``''``) to use the
  platform's default installed version.
- This value can also be a path to where the clang tools are
  installed (if using a custom install location). All paths specified
  here are converted to absolute.
",
                ),
        )
        .arg(
            Arg::new("extensions")
                .short('e')
                .long("extensions")
                .value_delimiter(',')
                .default_value("c,h,C,H,cpp,hpp,cc,hh,c++,h++,cxx,hxx")
                .long_help("A comma-separated list of file extensions to analyze.
"),
        )
        .arg(
            Arg::new("repo-root")
                .short('r')
                .long("repo-root")
                .default_value(".")
                .long_help(
                    "The relative path to the repository root directory. This path is
relative to the runner's ``GITHUB_WORKSPACE`` environment variable (or
the current working directory if not using a CI runner).
",
                ),
        )
        .arg(
            Arg::new("ignore")
                .short('i')
                .long("ignore")
                .value_delimiter('|')
                .default_value(".github|target")
                .long_help(
                    "Set this option with path(s) to ignore (or not ignore).

- In the case of multiple paths, you can use `|` to separate each path.
- There is no need to use ``./`` for each entry; a blank string (``''``)
  represents the repo-root path.
- This can also have files, but the file's path (relative to
  the :std:option:`--repo-root`) has to be specified with the filename.
- Submodules are automatically ignored. Hidden directories (beginning
  with a ``.``) are also ignored automatically.
- Prefix a path with ``!`` to explicitly not ignore it. This can be
  applied to a submodule's path (if desired) but not hidden directories.
- Glob patterns are not supported here. All asterisk characters (``*``)
  are literal.
",
                ),
        )
        .arg(
            Arg::new("lines-changed-only")
                .short('l')
                .long("lines-changed-only")
                .value_parser(["true", "false", "diff"])
                .default_value("true")
                .long_help(
                    "This controls what part of the files are analyzed.
The following values are accepted:

- ``false``: All lines in a file are analyzed.
- ``true``: Only lines in the diff that contain additions are analyzed.
- ``diff``: All lines in the diff are analyzed (including unchanged
  lines but not subtractions).
",
                ),
        )
        .arg(
            Arg::new("files-changed-only")
                .short('f')
                .long("files-changed-only")
                .default_value("false")
                .value_parser(FalseyValueParser::new())
                .long_help(
                    "Set this option to false to analyze any source files in the repo.
This is automatically enabled if
:std:option:`--lines-changed-only` is enabled.

.. note::
    The ``GITHUB_TOKEN`` should be supplied when running on a
    private repository with this option enabled, otherwise the runner
    does not not have the privilege to list the changed files for an event.

    See `Authenticating with the GITHUB_TOKEN
    <https://docs.github.com/en/actions/reference/authentication-in-a-workflow>`_
",
                ),
        )
        .arg(
            Arg::new("extra-arg")
                .long("extra-arg")
                .short('x')
                .action(ArgAction::Append)
                .long_help(
                    "A string of extra arguments passed to clang-tidy for use as
compiler arguments. This can be specified more than once for each
additional argument. Recommend using quotes around the value and
avoid using spaces between name and value (use ``=`` instead):

.. code-block:: shell

    cpp-linter --extra-arg=\"-std=c++17\" --extra-arg=\"-Wall\"",
                ),
        )
        .arg(
            Arg::new("thread-comments")
                .long("thread-comments")
                .short('g')
                .value_parser(["true", "false", "updated"])
                .default_value("false")
                .long_help(
                    "Set this option to true to enable the use of thread comments as feedback.
Set this to ``update`` to update an existing comment if one exists;
the value 'true' will always delete an old comment and post a new one if necessary.

.. note::
    To use thread comments, the ``GITHUB_TOKEN`` (provided by
    Github to each repository) must be declared as an environment
    variable.

    See `Authenticating with the GITHUB_TOKEN
    <https://docs.github.com/en/actions/reference/authentication-in-a-workflow>`_

.. hint::
    If run on a private repository, then this feature is
    disabled because the GitHub REST API behaves
    differently for thread comments on a private repository.
",
                ),
        )
        .arg(
            Arg::new("no-lgtm")
                .long("no-lgtm")
                .short('t')
                .value_parser(FalseyValueParser::new())
                .default_value("true")
                .long_help(
                    "Set this option to true or false to enable or disable the use of a
thread comment that basically says 'Looks Good To Me' (when all checks pass).

.. seealso::
    The :std:option:`--thread-comments` option also notes further implications.
",
                ),
        )
        .arg(
            Arg::new("step-summary")
                .long("step-summary")
                .short('w')
                .value_parser(FalseyValueParser::new())
                .default_value("false")
                .long_help(
                    "Set this option to true or false to enable or disable the use of
a workflow step summary when the run has concluded.
",
                ),
        )
        .arg(
            Arg::new("file-annotations")
                .long("file-annotations")
                .short('a')
                .value_parser(FalseyValueParser::new())
                .default_value("true")
                .long_help(
                    "Set this option to false to disable the use of
file annotations as feedback.
",
                ),
        )
}

/// This will parse the list of paths specified from the CLI using the `--ignore`
/// argument.
///
/// It returns 2 lists (in order):
///
/// - `ignored` paths
/// - `not_ignored` paths
///
/// This function will also read a .gitmodules file located in the working directory.
/// The named submodules' paths will be automatically added to the ignored list,
/// unless the submodule's path is already specified in the not_ignored list.
pub fn parse_ignore(ignore: &[&str]) -> (Vec<String>, Vec<String>) {
    let mut ignored = vec![];
    let mut not_ignored = vec![];
    for pattern in ignore {
        let as_posix = pattern.replace('\\', "/");
        let mut pat = as_posix.as_str();
        let is_ignored = !pat.starts_with('!');
        if !is_ignored {
            pat = &pat[1..];
        }
        if pat.starts_with("./") {
            pat = &pat[2..];
        }
        let is_hidden = pat.starts_with('.');
        if is_hidden || is_ignored {
            ignored.push(format!("./{pat}"));
        } else {
            not_ignored.push(format!("./{pat}"));
        }
    }

    if let Ok(read_buf) = fs::read_to_string(".gitmodules") {
        for line in read_buf.split('\n') {
            if line.trim_start().starts_with("path") {
                assert!(line.find('=').unwrap() > 0);
                let submodule = String::from("./") + line.split('=').last().unwrap().trim();
                log::debug!("Found submodule: {submodule}");
                let mut is_ignored = true;
                for pat in &not_ignored {
                    if pat == &submodule {
                        is_ignored = false;
                        break;
                    }
                }
                if is_ignored && !ignored.contains(&submodule) {
                    ignored.push(submodule);
                }
            }
        }
    }

    if !ignored.is_empty() {
        log::info!("Ignored:");
        for pattern in &ignored {
            log::info!("  {pattern}");
        }
    }
    if !not_ignored.is_empty() {
        log::info!("Not Ignored:");
        for pattern in &not_ignored {
            log::info!("  {pattern}");
        }
    }
    (ignored, not_ignored)
}

/// Converts the parsed value of the `--extra-arg` option into an optional vector of strings.
///
/// This is for adapting to 2 scenarios where `--extra-arg` is either
///
/// - specified multiple times
///     - each val is appended to a [`Vec`] (by clap crate)
/// - specified once with multiple space-separated values
///     - resulting [`Vec`] is made from splitting at the spaces between
/// - not specified at all (returns [`None`])
///
/// It is preferred that the values specified in either situation do not contain spaces and are
/// quoted:
/// ```shell
/// --extra-arg="-std=c++17" --extra-arg="-Wall"
/// # or equivalently
/// --extra-arg="-std=c++17 -Wall"
/// ```
/// The cpp-linter-action (for Github CI workflows) can only use 1 `extra-arg` input option, so
/// the value will be split at spaces.
pub fn convert_extra_arg_val(args: &ArgMatches) -> Option<Vec<&str>> {
    let raw_val = if let Ok(extra_args) = args.try_get_many::<String>("extra-arg") {
        extra_args.map(|extras| extras.map(|val| val.as_str()).collect::<Vec<_>>())
    } else {
        None
    };
    if let Some(val) = raw_val {
        if val.len() == 1 {
            // specified once; split and return result
            Some(
                val[0]
                    .trim_matches('\'')
                    .trim_matches('"')
                    .split(' ')
                    .collect(),
            )
        } else {
            // specified multiple times; just return
            Some(val)
        }
    } else {
        // no value specified; just return
        None
    }
}

#[cfg(test)]
mod test {
    use clap::ArgMatches;

    use super::{convert_extra_arg_val, get_arg_parser};

    fn parser_args(input: Vec<&str>) -> ArgMatches {
        let arg_parser = get_arg_parser();
        arg_parser.get_matches_from(input)
    }

    #[test]
    fn extra_arg_0() {
        let args = parser_args(vec!["cpp-linter"]);
        let extras = convert_extra_arg_val(&args);
        assert!(extras.is_none());
    }

    #[test]
    fn extra_arg_1() {
        let args = parser_args(vec!["cpp-linter", "--extra-arg='-std=c++17 -Wall'"]);
        let extras = convert_extra_arg_val(&args);
        assert!(extras.is_some());
        if let Some(extra_args) = extras {
            assert_eq!(extra_args.len(), 2);
            assert_eq!(extra_args, ["-std=c++17", "-Wall"])
        }
    }

    #[test]
    fn extra_arg_2() {
        let args = parser_args(vec![
            "cpp-linter",
            "--extra-arg=-std=c++17",
            "--extra-arg=-Wall",
        ]);
        let extras = convert_extra_arg_val(&args);
        assert!(extras.is_some());
        if let Some(extra_args) = extras {
            assert_eq!(extra_args.len(), 2);
            assert_eq!(extra_args, ["-std=c++17", "-Wall"])
        }
    }
}
