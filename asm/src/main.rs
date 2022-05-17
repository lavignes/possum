mod assembler;
mod charreader;
mod expr;
mod intern;
mod lexer;
mod symtab;

use std::{fs::File, io, io::Write, path::PathBuf, process::ExitCode};

use clap::Parser;

use crate::{
    assembler::Assembler,
    intern::{PathInterner, StrInterner},
    lexer::FileLexerFactory,
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to input assembly file
    #[clap(parse(from_os_str), value_name = "INPUT")]
    input: PathBuf,

    /// Path to output binary file. (Default: stdout)
    #[clap(parse(from_os_str), short, long)]
    output: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let output: Box<dyn Write> = if let Some(path) = args.output {
        let result = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path.clone());
        match result {
            Err(e) => {
                eprintln!("Cannot open file \"{}\" for writing: {e}", path.display());
                return ExitCode::FAILURE;
            }
            Ok(file) => Box::new(file),
        }
    } else {
        Box::new(io::stdout())
    };

    let mut assembler = Assembler::new(Box::new(FileLexerFactory::new()), output);
    match assembler.assemble(args.input) {
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
        _ => ExitCode::SUCCESS,
    }
}
