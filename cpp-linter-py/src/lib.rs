use pyo3::prelude::*;

use cpp_linter_lib::run::run_main;

/// A wrapper for the cpp_linter_lib::run::run_main()
#[pyfunction]
fn main(args: Vec<String>) -> PyResult<i32> {
    Ok(run_main(args))
}

/// The python binding for the cpp_linter package. It only exposes a submodule named
/// ``cpp_linter.run`` whose only exposed function is used as the entrypoint script.
/// See the pure python sources in this repo's cpp_linter folder (located at repo root).
#[pymodule]
fn cpp_linter(_py: Python, m: &PyModule) -> PyResult<()> {
    let run_submodule = PyModule::new(_py, "run")?;
    run_submodule.add_function(wrap_pyfunction!(main, m)?)?;
    m.add_submodule(run_submodule)?;
    Ok(())
}
