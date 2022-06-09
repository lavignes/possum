mod assembler;
mod charreader;
mod expr;
mod fileman;
mod intern;
mod lexer;
mod linker;
mod symtab;

use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use crate::{assembler::Assembler, fileman::RealFileSystem, intern::StrInterner};

#[derive(clap::Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to input assembly file
    #[clap(parse(from_os_str), value_name = "FILE")]
    file: PathBuf,

    /// Path to output binary file [default: stdout]
    #[clap(parse(from_os_str), short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Paths to search for included files [repeatable]
    #[clap(parse(from_os_str), short = 'I', long, value_name = "DIRECTORY")]
    include: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let args = <Args as clap::Parser>::parse();

    let mut output: Box<dyn Write> = if let Some(path) = args.output {
        let result = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path.as_path());
        match result {
            Err(e) => {
                eprintln!(
                    "Cannot open output file \"{}\" for writing: {e}",
                    path.display()
                );
                return ExitCode::FAILURE;
            }
            Ok(file) => Box::new(file),
        }
    } else {
        Box::new(io::stdout())
    };

    let cwd = env::current_dir().unwrap();
    let full_cwd = fs::canonicalize(cwd).unwrap();
    let file_system = RealFileSystem::new();
    let mut assembler = Assembler::new(file_system);

    for path in &args.include {
        if let Err(e) = assembler.add_search_path(full_cwd.as_path(), path) {
            eprintln!("[ERROR]: {e}");
            return ExitCode::FAILURE;
        }
    }

    let module = match assembler.assemble(full_cwd.as_path(), args.file) {
        Ok(module) => module,
        Err(e) => {
            eprintln!("[ERROR]: {e}");
            return ExitCode::FAILURE;
        }
    };

    match module.link(&mut output) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[ERROR]: {e}");
            ExitCode::FAILURE
        }
    }
}
