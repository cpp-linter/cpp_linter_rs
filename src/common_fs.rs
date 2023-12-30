//! A module to hold all common file system functionality.

use std::io::Read;
use std::path::{Component, Path};
use std::{fs, io};
use std::{ops::RangeInclusive, path::PathBuf};

/// A structure to represent a file's path and line changes.
#[derive(Debug)]
pub struct FileObj {
    /// The path to the file.
    pub name: PathBuf,

    /// The list of lines with additions.
    pub added_lines: Vec<u32>,

    /// The list of ranges that span only lines with additions.
    pub added_ranges: Vec<RangeInclusive<u32>>,

    /// The list of ranges that span the lines present in diff chunks.
    pub diff_chunks: Vec<RangeInclusive<u32>>,
}

impl FileObj {
    /// Instantiate a rudimentary object with only file name information.
    ///
    /// To instantiate an object with line information, use [FileObj::from].
    pub fn new(name: PathBuf) -> Self {
        FileObj {
            name,
            added_lines: Vec::<u32>::new(),
            added_ranges: Vec::<RangeInclusive<u32>>::new(),
            diff_chunks: Vec::<RangeInclusive<u32>>::new(),
        }
    }

    /// Instantiate an object with file name and changed lines information.
    pub fn from(
        name: PathBuf,
        added_lines: Vec<u32>,
        diff_chunks: Vec<RangeInclusive<u32>>,
    ) -> Self {
        let added_ranges = FileObj::consolidate_numbers_to_ranges(&added_lines);
        FileObj {
            name,
            added_lines,
            added_ranges,
            diff_chunks,
        }
    }

    /// A helper function to consolidate a [Vec<u32>] of line numbers into a
    /// [Vec<RangeInclusive<u32>>] in which each range describes the beginning and
    /// ending of a group of consecutive line numbers.
    fn consolidate_numbers_to_ranges(lines: &Vec<u32>) -> Vec<RangeInclusive<u32>> {
        let mut range_start = None;
        let mut ranges: Vec<RangeInclusive<u32>> = Vec::new();
        for (index, number) in lines.iter().enumerate() {
            if index == 0 {
                range_start = Some(*number);
            } else if number - 1 != lines[index - 1] {
                ranges.push(RangeInclusive::new(range_start.unwrap(), lines[index - 1]));
                range_start = Some(*number);
            }
            if index == lines.len() - 1 {
                ranges.push(RangeInclusive::new(range_start.unwrap(), *number));
            }
        }
        ranges
    }
}

/// Describes if a specified `file_name` is contained within the given `set` of paths.
///
/// The `set` of paths is used as domains, so the specified `file_name` can be a direct
/// or distant descendant of any given paths in the `set`.
pub fn is_file_in_list(file_name: &Path, set: &[String], prompt: String) -> bool {
    for pattern in set {
        let pat = Path::new(pattern);
        if pat.is_file() {
            if file_name == pat {
                log::debug!(
                    "{} is {prompt} as specified via {:?}",
                    file_name.to_string_lossy().replace('\\', "/"),
                    pat
                );
                return true;
            }
        } else if pat.is_dir() && file_name.starts_with(pat) {
            log::debug!(
                "{} is {prompt} as specified in domain {:?}",
                file_name.to_string_lossy().replace('\\', "/"),
                pat
            );
            return true;
        }
        // else file doesn't exist; return false
    }
    false
}

/// A helper function that checks if `entry` satisfies the following conditions (in
/// ordered priority):
///
/// - Does `entry`'s path use at least 1 of the listed file `extensions`? (takes
///   precedence)
/// - Is `entry` *not* specified in list of `ignored` paths?
/// - Is `entry` specified in the list of explicitly `not_ignored` paths? (supersedes
///   specified `ignored` paths)
pub fn is_source_or_ignored(
    entry: &Path,
    extensions: &[&str],
    ignored: &[String],
    not_ignored: &[String],
) -> bool {
    let extension = entry.extension();
    if extension.is_none() {
        return false;
    }
    let mut is_ignored = true;
    for ext in extensions {
        if ext == &extension.unwrap().to_os_string().into_string().unwrap() {
            is_ignored = false;
            break;
        }
    }
    if !is_ignored {
        log::debug!(
            "{} is a source file",
            entry.to_string_lossy().replace('\\', "/")
        );
        let is_in_ignored = is_file_in_list(entry, ignored, String::from("ignored"));
        let is_in_not_ignored = is_file_in_list(entry, not_ignored, String::from("not ignored"));
        if !is_in_ignored || is_in_not_ignored {
            return true;
        }
    }
    false
}

/// Walks a given `root_path` recursively and returns a [`Vec<FileObj>`] that
///
/// - uses at least 1 of the `extensions`
/// - is not specified in the given list of `ignored` paths
/// - is specified in the given list `not_ignored` paths (which supersedes `ignored` paths)
pub fn list_source_files(
    extensions: &[&str],
    ignored: &[String],
    not_ignored: &[String],
    root_path: &str,
) -> Vec<FileObj> {
    let mut files: Vec<FileObj> = Vec::new();
    let entries = fs::read_dir(root_path)
        .expect("repo root-path should exist")
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()
        .unwrap();
    for entry in entries {
        if entry.is_dir() {
            let mut is_hidden = false;
            let parent = entry.components().last().expect("parent not known");
            if parent.as_os_str().to_str().unwrap().starts_with('.') {
                is_hidden = true;
            }
            if !is_hidden {
                files.extend(list_source_files(
                    extensions,
                    ignored,
                    not_ignored,
                    &entry.into_os_string().into_string().unwrap(),
                ));
            }
        } else {
            let is_valid_src = is_source_or_ignored(&entry, extensions, ignored, not_ignored);
            if is_valid_src {
                files.push(FileObj::new(
                    entry.clone().strip_prefix("./").unwrap().to_path_buf(),
                ));
            }
        }
    }
    files
}

/// Gets the line and column number from a given `offset` (of bytes) for given
/// `file_path`.
///
/// This computes the line and column numbers from a buffer of bytes read from the
/// `file_path`. In non-UTF-8 encoded files, this does not guarantee that a word
/// boundary exists at the returned column number. However, the `offset` given to this
/// function is expected to originate from diagnostic information provided by
/// clang-format or clang-tidy.
pub fn get_line_cols_from_offset(file_path: &PathBuf, offset: usize) -> (usize, usize) {
    let mut file_buf = vec![0; offset];
    fs::File::open(file_path)
        .unwrap()
        .read_exact(&mut file_buf)
        .unwrap();
    let lines = file_buf.split(|byte| byte == &b'\n');
    let line_count = lines.clone().count();
    let column_count = lines.last().unwrap_or(&[]).len() + 1; // +1 because not a 0 based count
    (line_count, column_count)
}

/// This was copied from [cargo source code](https://github.com/rust-lang/cargo/blob/fede83ccf973457de319ba6fa0e36ead454d2e20/src/cargo/util/paths.rs#L61).
///
/// NOTE: Rust [std::path] crate has no native functionality equivalent to this.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                ret.pop();
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
}

#[cfg(test)]
mod test {

    // *********************** tests for normalized paths
    use super::{list_source_files, normalize_path};
    use std::env::current_dir;
    use std::path::PathBuf;

    #[test]
    fn normalize_redirects() {
        let mut src = current_dir().unwrap();
        src.push("..");
        src.push(
            current_dir()
                .unwrap()
                .strip_prefix(current_dir().unwrap().parent().unwrap())
                .unwrap(),
        );
        println!("relative path = {}", src.to_str().unwrap());
        assert_eq!(normalize_path(&src), current_dir().unwrap());
    }

    #[test]
    fn normalize_no_root() {
        let src = PathBuf::from("../cpp_linter_rs");
        let mut cur_dir = current_dir().unwrap();
        cur_dir = cur_dir
            .strip_prefix(current_dir().unwrap().parent().unwrap())
            .unwrap()
            .to_path_buf();
        println!("relative path = {}", src.to_str().unwrap());
        assert_eq!(normalize_path(&src), cur_dir);
    }

    #[test]
    fn normalize_current_redirect() {
        let src = PathBuf::from("tests/./ignored_paths");
        println!("relative path = {}", src.to_str().unwrap());
        assert_eq!(normalize_path(&src), PathBuf::from("tests/ignored_paths"));
    }

    // ************* tests for ignored paths
    use crate::cli::{get_arg_parser, parse_ignore};
    use crate::common_fs::is_file_in_list;
    use std::env::set_current_dir;

    fn setup_ignore(input: &str) -> (Vec<String>, Vec<String>) {
        let arg_parser = get_arg_parser();
        let args = arg_parser.get_matches_from(vec!["cpp-linter", "-i", input]);
        let ignore_arg = args
            .get_many::<String>("ignore")
            .unwrap()
            .map(|s| s.as_str())
            .collect::<Vec<_>>();
        let (ignored, not_ignored) = parse_ignore(&ignore_arg);
        println!("ignored = {:?}", ignored);
        println!("not ignored = {:?}", not_ignored);
        (ignored, not_ignored)
    }

    #[test]
    fn ignore_src() {
        let (ignored, not_ignored) = setup_ignore("src");
        assert!(is_file_in_list(
            &PathBuf::from("./src/lib.rs"),
            &ignored,
            "ignored".to_string()
        ));
        assert!(!is_file_in_list(
            &PathBuf::from("./src/lib.rs"),
            &not_ignored,
            "not_ignored".to_string()
        ));
    }

    #[test]
    fn ignore_root() {
        let (ignored, not_ignored) = setup_ignore("!src/lib.rs|./");
        assert!(is_file_in_list(
            &PathBuf::from("./cargo.toml"),
            &ignored,
            "ignored".to_string()
        ));
        assert!(is_file_in_list(
            &PathBuf::from("./src/lib.rs"),
            &not_ignored,
            "not_ignored".to_string()
        ));
    }

    #[test]
    fn ignore_root_implicit() {
        let (ignored, not_ignored) = setup_ignore("!src|");
        assert!(is_file_in_list(
            &PathBuf::from("./cargo.toml"),
            &ignored,
            "ignored".to_string()
        ));
        assert!(is_file_in_list(
            &PathBuf::from("./src/lib.rs"),
            &not_ignored,
            "not_ignored".to_string()
        ));
    }

    #[test]
    fn ignore_submodules() {
        set_current_dir("tests/ignored_paths").unwrap();
        let (ignored, not_ignored) = setup_ignore("!pybind11");

        // using Vec::contains() because these files don't actually exist in project files
        for ignored_submodule in ["./RF24", "./RF24Network", "./RF24Mesh"] {
            assert!(ignored.contains(&ignored_submodule.to_string()));
            assert!(!is_file_in_list(
                &PathBuf::from(ignored_submodule.to_string() + "/some_src.cpp"),
                &ignored,
                "ignored".to_string()
            ));
        }
        assert!(not_ignored.contains(&"./pybind11".to_string()));
        assert!(!is_file_in_list(
            &PathBuf::from("./pybind11/some_src.cpp"),
            &not_ignored,
            "not ignored".to_string()
        ));
    }

    #[test]
    fn walk_dir_recursively() {
        let (ignored, not_ignored) = setup_ignore("target");
        let extensions = vec!["cpp", "hpp"];
        let files = list_source_files(&extensions, &ignored, &not_ignored, ".");
        assert!(!files.is_empty());
        for file in files {
            assert!(extensions.contains(
                &file
                    .name
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
                    .as_str()
            ));
        }
    }

    use super::get_line_cols_from_offset;
    #[test]
    fn translate_byte_offset() {
        let (lines, cols) = get_line_cols_from_offset(&PathBuf::from("tests/demo/demo.cpp"), 144);
        println!("lines: {lines}, cols: {cols}");
        assert_eq!(lines, 13);
        assert_eq!(cols, 5);
    }
}
