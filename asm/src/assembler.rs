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

#[derive(Clone, Debug)]
enum MacroToken {
    Token(Token),
    Argument(usize),
}

impl From<Token> for MacroToken {
    fn from(tok: Token) -> Self {
        Self::Token(tok)
    }
}

struct Macro {
    loc: SourceLoc,
    args: Vec<(SourceLoc, StrRef)>,
    tokens: Vec<(SourceLoc, MacroToken)>,
    arg_indices: FxHashMap<StrRef, usize>,
}

#[derive(Copy, Clone, Debug)]
enum State {
    Initial,
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct AssemblerError(String);

pub struct Assembler<R> {
    path_interner: Rc<RefCell<PathInterner>>,
    str_interner: Rc<RefCell<StrInterner>>,
    lexer_factory: Box<dyn LexerFactory<R>>,
    lexers: Vec<Lexer<R>>,
    macros: FxHashMap<StrRef, Macro>,
    pass_index: usize,

    stash: Option<(SourceLoc, MacroToken)>,
    pc: isize,
    state: Vec<State>,
    active_macro: Option<StrRef>,
}

impl<R: Read> Assembler<R> {
    pub fn new(lexer_factory: Box<dyn LexerFactory<R>>) -> Self {
        Self {
            path_interner: Rc::new(RefCell::new(PathInterner::new())),
            str_interner: Rc::new(RefCell::new(StrInterner::new())),
            lexer_factory,
            lexers: Vec::new(),
            macros: FxHashMap::default(),
            pass_index: 0,

            stash: None,
            pc: 0,
            state: vec![State::Initial],
            active_macro: None,
        }
    }

    pub fn assemble<P: AsRef<Path>>(
        mut self,
        path: P,
        bin_writer: &mut dyn Write,
    ) -> Result<(), AssemblerError> {
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

        self.pass(bin_writer)?;
        self.pass(bin_writer)
    }

    fn pass(&mut self, bin_writer: &mut dyn Write) -> Result<(), AssemblerError> {
        let result = self.pass_real(bin_writer);
        self.pass_index += 1;
        result
    }

    fn pass_real(&mut self, bin_writer: &mut dyn Write) -> Result<(), AssemblerError> {
        self.pc = 0;

        write!(bin_writer, "test").map_err(|e| AssemblerError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Ref,
        io::{self, Cursor, ErrorKind},
        path::PathBuf,
    };

    use fxhash::FxHashMap;

    use super::*;
    use crate::intern::PathRef;

    fn assembler<P: AsRef<Path>>(files: &[(P, &str)]) -> Assembler<Cursor<String>> {
        struct StringLexerFactory {
            files: FxHashMap<PathBuf, Cursor<String>>,
        }

        impl LexerFactory<Cursor<String>> for StringLexerFactory {
            fn create(
                &self,
                path_interner: Ref<PathInterner>,
                str_interner: Rc<RefCell<StrInterner>>,
                pathref: PathRef,
            ) -> io::Result<Lexer<Cursor<String>>> {
                let path = path_interner.get(pathref).unwrap();
                match self.files.get(path) {
                    Some(reader) => Ok(Lexer::new(str_interner, pathref, reader.clone())),
                    None => Err(io::Error::new(ErrorKind::NotFound, "File not found")),
                }
            }
        }

        let mut map = FxHashMap::default();
        for (path, string) in files {
            map.insert(path.as_ref().into(), Cursor::new(string.to_string()));
        }

        Assembler::new(Box::new(StringLexerFactory { files: map }))
    }

    #[test]
    fn sanity() {
        let assembler = assembler(&[(
            "test.asm",
            r#"
            "#,
        )]);

        let mut binary = Vec::new();
        assert!(assembler.assemble("test.asm", &mut binary).is_ok());

        assert_eq!("testtest", String::from_utf8(binary).unwrap());
    }
}
