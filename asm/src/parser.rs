use std::{borrow::Borrow, cell::RefCell, fmt, io::Read, path::Path, rc::Rc};

use fxhash::FxHashMap;

use crate::{
    expr::Expr,
    fileman::{FileManager, FileSystem},
    intern::StrRef,
    lexer::{DirectiveName, LabelKind, Lexer, LexerError, SourceLoc, SymbolName, Token},
    module::{Item, Module},
    symtab::{Symbol, Symtab},
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
    tokens: Vec<MacroToken>,
    arg_indices: FxHashMap<StrRef, usize>,
}

#[derive(Copy, Clone, Debug)]
enum State {
    Initial,
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct ParserError(String);

impl From<LexerError> for (SourceLoc, ParserError) {
    fn from(e: LexerError) -> Self {
        (e.loc(), ParserError(format!("{e}")))
    }
}

pub struct Parser<S, R> {
    file_manager: FileManager<S>,
    str_interner: Rc<RefCell<StrInterner>>,
    lexers: Vec<Lexer<R>>,
    lexer: Option<Lexer<R>>,
    macros: FxHashMap<StrRef, Macro>,
    symtab: Symtab,

    stash: Option<Token>,
    here: u16,
    state: Vec<State>,
    active_macro: Option<StrRef>,
    active_namespace: Option<StrRef>,
}

impl<S, R> Parser<S, R>
where
    S: FileSystem<Reader = R>,
    R: Read,
{
    pub fn new(file_system: S) -> Self {
        Self {
            file_manager: FileManager::new(file_system),
            str_interner: Rc::new(RefCell::new(StrInterner::new())),
            lexers: Vec::new(),
            lexer: None,
            macros: FxHashMap::default(),
            symtab: Symtab::new(),

            stash: None,
            here: 0,
            state: vec![State::Initial],
            active_macro: None,
            active_namespace: None,
        }
    }

    #[must_use]
    pub fn add_search_path<C: AsRef<Path>, P: AsRef<Path>>(
        &mut self,
        cwd: C,
        path: P,
    ) -> Result<(), ParserError> {
        let path = path.as_ref();
        self.file_manager.add_search_path(cwd, path).map_err(|e| {
            ParserError(format!(
                "Failed to find include path \"{}\": {e}",
                path.display()
            ))
        })?;
        Ok(())
    }

    #[must_use]
    pub fn parse<C: AsRef<Path>, P: AsRef<Path>>(
        mut self,
        cwd: C,
        path: P,
    ) -> Result<Module<S>, ParserError> {
        let path = path.as_ref();
        let (pathref, reader) = match self.file_manager.reader(cwd, path) {
            Ok(Some(tup)) => tup,
            Ok(None) => {
                return Err(ParserError(format!(
                    "File not found: \"{}\"",
                    path.display()
                )))
            }
            Err(e) => {
                return Err(ParserError(format!(
                    "Failed to open \"{}\" for reading: {e}",
                    path.display()
                )))
            }
        };

        self.lexer = Some(Lexer::new(self.str_interner.clone(), pathref, reader));
        self.module()
    }

    #[must_use]
    fn peek(&mut self) -> Result<Option<&Token>, LexerError> {
        loop {
            if self.lexer.is_none() {
                self.lexer = self.lexers.pop();
            }
            match self.stash {
                // Skip comment tokens
                Some(Token::Comment { .. }) => self.stash = None,

                Some(_) => return Ok(self.stash.as_ref()),

                None => match &mut self.lexer {
                    Some(lexer) => {
                        self.stash = lexer.next().transpose()?;
                        if self.stash.is_none() {
                            self.lexer = None;
                        }
                    }
                    None => return Ok(None),
                },
            }
        }
    }

    #[must_use]
    fn next(&mut self) -> Result<Option<Token>, LexerError> {
        self.peek()?;
        Ok(self.stash.take())
    }

    fn trace_error(&self, loc: SourceLoc, e: ParserError) -> ParserError {
        let mut msg = String::new();
        let fmt_msg = &mut msg as &mut dyn fmt::Write;

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
        ParserError(msg)
    }

    fn module(mut self) -> Result<Module<S>, ParserError> {
        self.here = 0;
        let mut items = Vec::new();

        loop {
            match self.item_opt() {
                Err((loc, e)) => return Err(self.trace_error(loc, e)),
                Ok(Some(item)) => items.push(item),
                Ok(None) => break,
            }
        }

        let Self {
            str_interner,
            file_manager,
            symtab,
            ..
        } = self;
        Ok(Module::new(str_interner, file_manager, symtab, items))
    }

    #[inline]
    fn eval_expr(&mut self) -> Result<(SourceLoc, Option<i32>), (SourceLoc, ParserError)> {
        match self
            .expr()
            .map(|(loc, expr)| (loc, expr.evaluate(&self.symtab)))?
        {
            (loc, Ok(value)) => Ok((loc, value)),
            _ => todo!(),
        }
    }

    fn expr(&mut self) -> Result<(SourceLoc, Expr), (SourceLoc, ParserError)> {
        let loc = self.next()?.unwrap().loc();
        Ok((loc, 42.into()))
    }

    #[inline]
    #[must_use]
    fn peeked_symbol(
        &mut self,
        sym: SymbolName,
    ) -> Result<Option<Token>, (SourceLoc, ParserError)> {
        if let Some(&tok @ Token::Symbol { name: sym, .. }) = self.peek()? {
            Ok(Some(tok))
        } else {
            Ok(None)
        }
    }

    #[must_use]
    fn item_opt(&mut self) -> Result<Option<(SourceLoc, Item)>, (SourceLoc, ParserError)> {
        loop {
            match self.peek()? {
                Some(Token::NewLine { .. }) => {
                    self.next()?;
                    continue;
                }

                Some(&Token::Label { loc, value, kind }) => {
                    let direct = match kind {
                        LabelKind::Global | LabelKind::Direct => value,

                        LabelKind::Local => {
                            let interner = self.str_interner.as_ref().borrow_mut();
                            let label = interner.get(value).unwrap();
                            if let Some(namespace) = self.active_namespace {
                                let global = interner.get(namespace).unwrap();
                                self.str_interner
                                    .borrow_mut()
                                    .intern(format!("{global}{label}"))
                            } else {
                                return Err((loc, ParserError(format!("The local label \"{label}\" is being defined but there was no global label defined before it"))));
                            }
                        }
                    };

                    if let Some(sym) = self.symtab.get(direct) {
                        let interner = self.str_interner.as_ref().borrow();
                        let label = interner.get(direct).unwrap();
                        return Err((
                            loc,
                            ParserError(format!("The label \"{label}\" is already defined")),
                        ));
                    }
                    self.symtab.insert(value, Symbol::Value(self.here));

                    self.next()?;
                    continue;
                }

                Some(&Token::Directive { loc, name }) => match name {
                    DirectiveName::Org => {
                        self.next()?;

                        self.here = match self.eval_expr()? {
                            // TODO: Check for truncation error
                            (_, Some(value)) => value as u16,
                            (_, None) => return Err((loc, ParserError(format!("The expression following an \"@org\" directive must be immediately solvable")))),
                        };
                        continue;
                    }

                    DirectiveName::Db => {
                        self.next()?;

                        let mut data = Vec::new();
                        loop {
                            match self.peek()? {
                                Some(&Token::String { value, .. }) => {
                                    self.next()?;
                                    let interner = self.str_interner.as_ref().borrow();
                                    let bytes = interner.get(value).unwrap().as_bytes();
                                    // TODO: Check for truncation error and wrapping!
                                    self.here += bytes.len() as u16;
                                    data.extend_from_slice(bytes);
                                }

                                _ => {
                                    match self.eval_expr()? {
                                        // TODO: Check for truncation error
                                        (_, Some(value)) => {
                                            self.here += 1;
                                            data.push(value as u8);
                                        },
                                        (loc, None) => return Err((loc, ParserError(format!("Every expression following a \"@db\" directive must be immediately solvable")))),
                                    };
                                }
                            }

                            if self.peeked_symbol(SymbolName::Comma)?.is_some() {
                                self.next()?;
                                continue;
                            }

                            break;
                        }

                        return Ok(Some((loc, Item::Bytes { data })));
                    }

                    _ => todo!(),
                },

                Some(Token::Operation { .. }) => {}

                Some(_) => {}

                None => return Ok(None),
            }
        }
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

    fn parser<P: AsRef<Path>>(files: &[(P, &str)]) -> Parser<StringFileSystem, Cursor<String>> {
        Parser::new(StringFileSystem::new(files))
    }

    #[test]
    fn sanity() {
        let parser = parser(&[(
            "/test.asm",
            r#"
                @org 42
                @db "Hello World!", 42, "this", "is", "a test!"
            "#,
        )]);

        parser
            .parse("/", "test.asm")
            .inspect_err(|e| eprintln!("{e}"))
            .unwrap();
    }
}
