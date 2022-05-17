mod charreader;
mod expr;
mod intern;
mod lexer;
mod parser;

use std::{cell::RefCell, fs::File, io, path::PathBuf, rc::Rc};

use clap::Parser;

use crate::{
    intern::{PathInterner, StrInterner},
    lexer::Lexer,
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to input assembly file
    #[clap(parse(from_os_str), value_name = "INPUT")]
    input: PathBuf,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let path_interner = Rc::new(RefCell::new(PathInterner::new()));
    let str_interner = Rc::new(RefCell::new(StrInterner::new()));
    let file = path_interner.borrow_mut().intern(args.input.clone());
    let reader = File::open(args.input)?;
    let lexer = Lexer::new(path_interner, str_interner, file, reader);

    Ok(())
}
