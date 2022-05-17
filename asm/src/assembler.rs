use std::{
    cell::RefCell,
    io::{Read, Write},
    path::Path,
    rc::Rc,
};

use fxhash::FxHashMap;

use crate::{
    intern::StrRef,
    lexer::{Lexer, LexerFactory, SourceLoc, Token},
    PathInterner, StrInterner,
};

enum MacroToken {
    Token(Token),
    Argument(usize),
}

struct Macro {
    args: Vec<(SourceLoc, StrRef)>,
    tokens: Vec<(SourceLoc, MacroToken)>,
    arg_indices: FxHashMap<StrRef, usize>,
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct AssemblerError(String);

pub struct Assembler<R> {
    path_interner: Rc<RefCell<PathInterner>>,
    str_interner: Rc<RefCell<StrInterner>>,
    lexer_factory: Box<dyn LexerFactory<R>>,
    lexers: Vec<Lexer<R>>,
    bin_writer: Box<dyn Write>,
    macros: FxHashMap<StrRef, Macro>,
    stash: Option<Token>,
    pass_count: usize,
}

impl<R: Read> Assembler<R> {
    pub fn new(lexer_factory: Box<dyn LexerFactory<R>>, bin_writer: Box<dyn Write>) -> Self {
        Self {
            path_interner: Rc::new(RefCell::new(PathInterner::new())),
            str_interner: Rc::new(RefCell::new(StrInterner::new())),
            lexer_factory,
            lexers: Vec::new(),
            bin_writer,
            macros: FxHashMap::default(),
            stash: None,
            pass_count: 0,
        }
    }

    pub fn assemble<P: AsRef<Path>>(&mut self, path: P) -> Result<(), AssemblerError> {
        // TODO: We need a file manager that can resolve working dirs and include paths!
        let path = path.as_ref();
        let pathref = self.path_interner.borrow_mut().intern(path.clone());

        let lexer = self.lexer_factory.create(
            self.path_interner.borrow(),
            self.str_interner.clone(),
            pathref,
        );

        self.lexers.push(lexer.map_err(|e| {
            AssemblerError(format!(
                "Failed to open \"{}\" for reading: {e}",
                path.display()
            ))
        })?);

        self.pass()?;
        self.pass()
    }

    fn pass(&mut self) -> Result<(), AssemblerError> {
        let result = self.pass_real();
        self.pass_count += 1;
        result
    }

    fn pass_real(&mut self) -> Result<(), AssemblerError> {
        todo!()
    }
}
