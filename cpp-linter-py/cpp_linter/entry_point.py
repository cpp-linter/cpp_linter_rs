"""
This module is the python frontend of the cpp-linter package written in Rust.
It exposes a single function: `main()`.

The idea here is that all functionality is implemented in Rust. However, passing
command line arguments is done differently in Python or Rust.

- In python, the ``sys.argv`` list is passed from the ``cpp_linter.entry_point.main()``
  function to rust via the ``cpp_linter.run.main()`` binding.
- In rust, the ``std::env::args`` is passed to ``run::main()`` in the binary driver
  source `bin.rs`.

This is done because of the way the python entry point is invoked. If ``std::env::args``
is used instead of python's ``sys.argv``, then the list of strings includes the entry
point alias ("path/to/cpp-linter.exe"). Thus, the parser in `cli.rs` will halt on an
error because it is not configured to handle positional arguments.
"""
import sys

# Using relative import to load binary lib with same name as root package.
# This is just how pyo3 builds python bindings from rust.
from .cpp_linter import run


def main():
    """The main entrypoint for the python frontend. See our rust docs for more info on
    the backend (implemented in rust)"""
    sys.exit(run.main(sys.argv))
