mod charreader;
mod expr;
mod fileinfo;
mod lexer;
mod parser;

use std::{fs::File, io, path::PathBuf};

use clap::Parser;

use crate::{fileinfo::FileInfo, lexer::Lexer};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to input assembly file
    #[clap(parse(from_os_str), value_name = "INPUT")]
    input: PathBuf,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let mut file_info = FileInfo::new();
    let file = file_info.insert(&args.input);
    let reader = File::open(&args.input)?;
    let lexer = Lexer::new(file, reader);

    Ok(())
}
