use std::{
    borrow::Borrow,
    cell::RefCell,
    fmt,
    io::{Read, Write},
    path::Path,
    rc::Rc,
};

use fxhash::FxHashMap;

use crate::{
    fileman::{FileManager, FileSystem},
    intern::StrRef,
    lexer::{Lexer, LexerError, SourceLoc, Token},
    StrInterner,
};

#[derive(Copy, Clone, Debug)]
enum MacroToken {
    Token(Token),
    Argument(usize),
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

struct Context<'a> {
    bin_writer: &'a dyn Write,
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct AssemblerError(String);

pub struct Assembler<S, R> {
    file_manager: FileManager<S>,
    str_interner: Rc<RefCell<StrInterner>>,
    lexers: Vec<Lexer<R>>,
    lexer: Option<Lexer<R>>,
    macros: FxHashMap<StrRef, Macro>,
    pass_index: usize,

    stash: Option<(SourceLoc, Result<MacroToken, LexerError>)>,
    pc: isize,
    state: Vec<State>,
    active_macro: Option<StrRef>,
}

impl<S: FileSystem<Reader = R>, R: Read> Assembler<S, R> {
    pub fn new(file_system: S) -> Self {
        Self {
            file_manager: FileManager::new(file_system),
            str_interner: Rc::new(RefCell::new(StrInterner::new())),
            lexers: Vec::new(),
            lexer: None,
            macros: FxHashMap::default(),
            pass_index: 0,

            stash: None,
            pc: 0,
            state: vec![State::Initial],
            active_macro: None,
        }
    }

    pub fn add_search_path<C: AsRef<Path>, P: AsRef<Path>>(
        &mut self,
        cwd: C,
        path: P,
    ) -> Result<(), AssemblerError> {
        let path = path.as_ref();
        self.file_manager.add_search_path(cwd, path).map_err(|e| {
            AssemblerError(format!(
                "Failed to find include path \"{}\": {e}",
                path.display()
            ))
        })?;
        Ok(())
    }

    pub fn assemble<C: AsRef<Path>, P: AsRef<Path>>(
        mut self,
        cwd: C,
        path: P,
        bin_writer: &mut dyn Write,
    ) -> Result<(), AssemblerError> {
        let path = path.as_ref();
        let (pathref, reader) = match self.file_manager.reader(cwd, path) {
            Ok(Some(tup)) => tup,
            Ok(None) => {
                return Err(AssemblerError(format!(
                    "File not found: \"{}\"",
                    path.display()
                )))
            }
            Err(e) => {
                return Err(AssemblerError(format!(
                    "Failed to open \"{}\" for reading: {e}",
                    path.display()
                )))
            }
        };

        self.lexer = Some(Lexer::new(self.str_interner.clone(), pathref, reader));

        let mut ctx = Context { bin_writer };
        self.pass(&mut ctx)?;
        self.pass(&mut ctx)
    }

    fn pass(&mut self, ctx: &mut Context) -> Result<(), AssemblerError> {
        let result = self.pass_real(ctx);
        self.pass_index += 1;
        result
    }

    fn peek(&mut self) -> Option<&(SourceLoc, Result<MacroToken, LexerError>)> {
        loop {
            // Skip comment tokens
            if let Some((_, Ok(MacroToken::Token(Token::Comment)))) = self.stash {
                self.stash = None;
            } else {
                return self.stash.as_ref();
            }
            if self.lexer.is_none() {
                self.lexer = self.lexers.pop();
            }
            match &mut self.lexer {
                None => return None,
                Some(lexer) => {
                    self.stash = lexer
                        .next()
                        .map(|(loc, result)| (loc, result.map(MacroToken::Token)));
                    continue;
                }
            }
        }
    }

    fn next(&mut self) -> Option<(SourceLoc, Result<MacroToken, LexerError>)> {
        self.peek();
        self.stash.take()
    }

    fn make_full_error(&self, loc: SourceLoc, e: AssemblerError) -> AssemblerError {
        let mut msg = String::new();
        let mut fmt_msg = &mut msg as &mut dyn fmt::Write;

        let path = self.file_manager.borrow().path(loc.pathref).unwrap();
        writeln!(fmt_msg, "From \"{}\"", path.display()).unwrap();

        let mut included_from = self.lexer.as_ref().unwrap().included_from();
        for lexer in self.lexers.iter().rev() {
            let loc = included_from.unwrap();
            let path = self.file_manager.borrow().path(loc.pathref).unwrap();
            writeln!(
                fmt_msg,
                "\tIncluded at {}:{}:{}",
                path.display(),
                loc.line,
                loc.column
            )
            .unwrap();
            included_from = lexer.included_from();
        }
        writeln!(
            fmt_msg,
            "\n{}:{}:{}:",
            path.file_name().unwrap().to_str().unwrap(),
            loc.line,
            loc.column
        )
        .unwrap();
        writeln!(fmt_msg, "{e}").unwrap();
        AssemblerError(msg)
    }

    fn pass_real(&mut self, ctx: &mut Context) -> Result<(), AssemblerError> {
        self.pc = 0;
        loop {
            match self.item_opt(ctx) {
                Some((loc, Err(e))) => Err(self.make_full_error(loc, e))?,
                Some(_) => {}
                None => break,
            }
        }
        Ok(())
    }

    fn item_opt(&mut self, ctx: &mut Context) -> Option<(SourceLoc, Result<(), AssemblerError>)> {
        /*
        match self.peek() {
            Some()

            None => None,
        }
        */

        None

        // if let Some(directive) = self.directive_opt(ctx) {
        //     return Some(value);
        // }
        // if let Some(value) = self.directive_opt(ctx) {
        //     return Some(value);
        // }
        // return None;
    }

    fn directive_opt(
        &mut self,
        ctx: &mut Context,
    ) -> Option<(SourceLoc, Result<MacroToken, AssemblerError>)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Cursor},
        path::PathBuf,
    };

    use fxhash::FxHashMap;

    use super::*;

    struct StringFileSystem {
        files: FxHashMap<PathBuf, String>,
    }

    impl StringFileSystem {
        #[inline]
        fn new<P: AsRef<Path>>(files: &[(P, &str)]) -> Self {
            let mut map = FxHashMap::default();
            for (path, s) in files {
                map.insert(path.as_ref().to_path_buf(), s.to_string());
            }
            Self { files: map }
        }
    }

    impl FileSystem for StringFileSystem {
        type Reader = Cursor<String>;

        #[inline]
        fn is_dir(&self, _: &Path) -> io::Result<bool> {
            Ok(true)
        }

        #[inline]
        fn is_file(&self, path: &Path) -> io::Result<bool> {
            Ok(self.files.contains_key(path))
        }

        #[inline]
        fn open_read(&self, path: &Path) -> io::Result<Self::Reader> {
            Ok(Cursor::new(self.files.get(path).unwrap().clone()))
        }
    }

    fn assembler<P: AsRef<Path>>(
        files: &[(P, &str)],
    ) -> Assembler<StringFileSystem, Cursor<String>> {
        Assembler::new(StringFileSystem::new(files))
    }

    #[test]
    fn sanity() {
        let assembler = assembler(&[(
            "/test.asm",
            r#"
                
            "#,
        )]);

        let mut binary = Vec::new();
        assembler.assemble("/", "test.asm", &mut binary).unwrap();

        // assert_eq!("testtest", String::from_utf8(binary).unwrap());
    }
}
