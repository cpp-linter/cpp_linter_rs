use std::{fs::OpenOptions, io::Write};

use cpp_linter::cli;

pub fn main() -> std::io::Result<()> {
    let command = cli::get_arg_parser();
    let doc_file = OpenOptions::new()
        .truncate(true)
        .create(true)
        .write(true)
        .open("docs/cli_args.rst")?;
    let title = "Command Line Interface Options".to_string();
    writeln!(&doc_file, "{}", &title)?;
    for _ in title.chars() {
        write!(&doc_file, "=")?;
    }
    write!(&doc_file, "\n\n")?;
    for arg in command.get_arguments() {
        writeln!(
            &doc_file,
            ".. std:option:: -{}, --{}\n",
            &arg.get_short().unwrap(),
            &arg.get_long().unwrap()
        )?;
        for line in arg.get_long_help().unwrap().to_string().split('\n') {
            writeln!(&doc_file, "    {}", &line)?;
        }
        writeln!(&doc_file)?;
        let default = arg.get_default_values();
        if !default.is_empty() {
            writeln!(&doc_file, "    :Default:")?;
            if default.len() < 2 {
                writeln!(&doc_file, "        ``{:?}``", default.first().unwrap())?;
            } else {
                for val in default {
                    writeln!(&doc_file, "        - ``{:?}``", val)?;
                }
            }
        }
    }
    println!("docs/cli_args.rst generated!");
    Ok(())
}
