use std::env;

use cpp_linter_lib::run::run_main;

pub fn main() {
    run_main(env::args().collect::<Vec<String>>());
}
