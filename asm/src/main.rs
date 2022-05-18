mod assembler;
mod charreader;
mod expr;
mod fileman;
mod intern;
mod lexer;
mod symtab;

use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::Parser;

use crate::{
    assembler::Assembler,
    fileman::RealFileSystem,
    intern::{PathInterner, StrInterner},
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to input assembly file
    #[clap(parse(from_os_str), value_name = "FILE")]
    file: PathBuf,

    /// Path to output binary file (Default: stdout)
    #[clap(parse(from_os_str), short, long)]
    output: Option<PathBuf>,

    /// Paths to search for included files
    #[clap(parse(from_os_str), short = 'I', long)]
    include: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let mut output: Box<dyn Write> = if let Some(path) = args.output {
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

    let cwd = std::env::current_dir().unwrap();
    let file_system = RealFileSystem::new();
    let mut assembler = Assembler::new(file_system);

    for path in &args.include {
        if let Err(e) = assembler.add_search_path(cwd.clone(), path) {
            eprintln!(
                "Could not read include directory \"{}\": {e}",
                path.display()
            );
            return ExitCode::FAILURE;
        }
    }

    if let Err(e) = assembler.assemble(cwd, args.file, &mut output) {
        eprintln!("{e}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
