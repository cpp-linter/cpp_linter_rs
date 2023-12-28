//! This is the cpp_linter package's primary binary executable's source code.
//!
//! Notice how similar this is compared to the python binding's
//! cpp_linter/entry_point.py script.

use std::env;

// project specific modules/crates
use cpp_linter::run::main as run_main;

/// This takes the CLI arguments and passes them to [`cpp_linter::run::main`].
pub fn main() {
    run_main(env::args().collect::<Vec<String>>());
}
