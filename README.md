# C/C++ Linting Package

A Python and Rust package for linting C/C++ code with clang-tidy and/or clang-format to collect feedback provided in the form of thread comments, step summary, or file annotations.

## Usage

For usage in a CI workflow, see [the cpp-linter/cpp-linter-action repository](https://github.com/cpp-linter/cpp-linter-action).

For the description of supported Command Line Interface options, see [the CLI documentation](https://cpp-linter.github.io/cpp-linter/cli_args.html).

## Have question or feedback?

To provide feedback (requesting a feature or reporting a bug) please post to [issues](https://github.com/cpp-linter/cpp-linter/issues).

## License

The scripts and documentation in this project are released under the [MIT][MIT].

Dependencies (that are redistributed by us in binary form) have the following license agreements:

- [clap](https://crates.io/crates/clap): Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].
- [git2](https://crates.io/crates/git2): Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].

  The following are conditionally included in binaries (using the `openssl-vendored` feature on a
  case-by-case basis) because it is a dependency of git2:

  - [openssl](https://crates.io/crates/openssl): Licensed under [Apache 2.0][Apache2]
  - [openssl-probe](https://crates.io/crates/openssl-probe) : Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].

- [lenient_semver](https://crates.io/crates/lenient_semver): Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].
- [log](https://crates.io/crates/log): Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].
- [regex](https://crates.io/crates/regex): Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].
- [reqwest](https://crates.io/crates/reqwest): Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].
- [semver](https://crates.io/crates/semver): Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].
- [serde](https://crates.io/crates/serde): Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].
- [serde-xml-rs](https://crates.io/crates/serde-xml-rs): Licensed under [MIT][MIT].
- [serde_json](https://crates.io/crates/serde_json): Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].
- [which](https://crates.io/crates/which): Licensed under [MIT][MIT].

The python binding uses

- [pyo3](https://crates.io/crates/pyo3): Dual-licensed under [Apache 2.0][Apache2] or [MIT][MIT].

[MIT]: https://choosealicense.com/licenses/mit
[Apache2]: https://github.com/clap-rs/clap/blob/HEAD/LICENSE-APACHE
