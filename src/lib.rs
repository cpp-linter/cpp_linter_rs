#![doc(html_logo_url = "https://github.com/cpp-linter/cpp-linter/raw/main/docs/_static/logo.png")]
#![doc(
    html_favicon_url = "https://github.com/cpp-linter/cpp-linter/raw/main/docs/_static/favicon.ico"
)]
//! The root module for the cpp_linter package when compiled as a library.
//! This module mainly holds the declarations of this package's other modules.
//!
//! The python binding is also defined here, and it is exposed in python as
//! `cpp_linter.cpp_linter` in the python path.

use pyo3::prelude::*;

// project specific modules/crates
pub mod clang_tools;
pub mod cli;
pub mod common_fs;
pub mod git;
pub mod rest_api;
pub use rest_api::github_api;
pub mod logger;
pub mod run;

/// The python binding for the cpp_linter package. It only exposes a submodule named
/// ``cpp_linter.run`` whose only exposed function is used as the entrypoint script.
/// See the pure python sources in this repo's cpp_linter folder (located at repo root).
#[pymodule]
fn cpp_linter(_py: Python, m: &PyModule) -> PyResult<()> {
    let run_submodule = PyModule::new(_py, "run")?;
    run_submodule.add_function(wrap_pyfunction!(run::main, m)?)?;
    m.add_submodule(run_submodule)?;
    Ok(())
}
