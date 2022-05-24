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
    loc: Option<SourceLoc>,
    here: u16,
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
            loc: None,
            here: 0,
            active_macro: None,
            active_namespace: None,
        }
    }

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

    #[inline]
    fn loc(&mut self) -> SourceLoc {
        self.loc.unwrap()
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
                        self.loc = Some(lexer.loc());
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

    #[must_use]
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

        for item in &items {
            dbg!(item);
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
    #[must_use]
    fn const_expr(&mut self) -> Result<Option<i32>, (SourceLoc, ParserError)> {
        let (loc, expr) = self.expr()?;
        Ok(expr.evaluate(&self.symtab))
    }

    #[must_use]
    fn expr(&mut self) -> Result<(SourceLoc, Expr), (SourceLoc, ParserError)> {
        // match self.peek()? {
        //     Some(&Token::Symbol {
        //         loc,
        //         name:
        //             SymbolName::ParenOpen | SymbolName::Bang | SymbolName::Minus | SymbolName::Tilde,
        //     })
        //     | Some(&Token::Label { loc, .. }) => self.expr_prec_0(),
        //     _ => Ok(),
        // }
        let loc = self.next()?.unwrap().loc();
        Ok((loc, Expr::value(42)))
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
    fn item_opt(&mut self) -> Result<Option<Item>, (SourceLoc, ParserError)> {
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

                    if self.symtab.get(direct).is_some() {
                        let interner = self.str_interner.as_ref().borrow();
                        let label = interner.get(direct).unwrap();
                        return Err((
                            loc,
                            ParserError(format!("The label \"{label}\" was already defined")),
                        ));
                    }
                    self.symtab.insert(direct, Symbol::Value(self.here as i32));
                    self.next()?;

                    if self.peeked_symbol(SymbolName::Colon)?.is_some() {
                        self.next()?;
                    }
                    continue;
                }

                Some(&Token::Directive { loc, name }) => {
                    match name {
                        DirectiveName::Org => {
                            self.next()?;

                            self.here = match self.const_expr()? {
                                Some(value) => {
                                    if (value as u32) > 0xFFFF {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "\"@org\" expression result ({}) is not a valid address", value
                                            )),
                                        ));
                                    }
                                    value as u16
                                },
                                None => return Err((loc, ParserError(format!("The expression following an \"@org\" directive must be immediately solvable")))),
                            };
                            continue;
                        }

                        DirectiveName::Symbol => {
                            self.next()?;

                            let direct = match self.peek()? {
                                Some(&Token::Label { loc, value, kind }) => match kind {
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
                                            return Err((loc, ParserError(format!("The local symbol \"{label}\" is being defined but there was no global label defined before it"))));
                                        }
                                    }
                                },
                                _ => {
                                    return Err((
                                        loc,
                                        ParserError(format!("A symbol name is required")),
                                    ))
                                }
                            };
                            self.next()?;

                            if self.symtab.get(direct).is_some() {
                                let interner = self.str_interner.as_ref().borrow();
                                let label = interner.get(direct).unwrap();
                                return Err((
                                    loc,
                                    ParserError(format!(
                                        "The symbol \"{label}\" was already defined"
                                    )),
                                ));
                            }

                            if self.peeked_symbol(SymbolName::Comma)?.is_none() {
                                return Err((
                                    loc,
                                    ParserError(format!(
                                        "Expected a comma between the name and value of a \"@symbol\" directive"
                                    )),
                                ));
                            }
                            self.next()?;

                            let (_, expr) = self.expr()?;
                            self.symtab.insert(direct, Symbol::Expr(expr));
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

                                        if (self.here as usize) + bytes.len() > (u16::MAX as usize)
                                        {
                                            return Err((
                                                loc,
                                                ParserError(format!(
                                                    "\"@db\" bytes extend past address $FFFF"
                                                )),
                                            ));
                                        }
                                        self.here += bytes.len() as u16;
                                        data.extend_from_slice(bytes);
                                    }

                                    _ => {
                                        match self.const_expr()? {
                                            Some(value) => {
                                                if (value as u32) > 0xFF {
                                                    return Err((
                                                        loc,
                                                        ParserError(format!(
                                                            "\"@db\" expression result ({}) will not fit in a byte", value
                                                        )),
                                                    ));
                                                }
                                                if (self.here as usize) + 1 > (u16::MAX as usize) {
                                                    return Err((
                                                        loc,
                                                        ParserError(format!(
                                                            "\"@db\" bytes extend past address $FFFF"
                                                        )),
                                                    ));
                                                }
                                                self.here += 1;
                                                data.push(value as u8);
                                            },
                                            None => return Err((loc, ParserError(format!("Every expression following a \"@db\" directive must be immediately solvable")))),
                                        };
                                    }
                                }

                                if self.peeked_symbol(SymbolName::Comma)?.is_some() {
                                    self.next()?;
                                    continue;
                                }
                                break;
                            }
                            return Ok(Some(Item::Bytes { loc, data }));
                        }

                        DirectiveName::Dw => {
                            self.next()?;

                            let mut data = Vec::new();
                            loop {
                                match self.peek()? {
                                    _ => {
                                        match self.const_expr()? {
                                            Some(value) => {
                                                if (value as u32) > 0xFFFF {
                                                    return Err((
                                                        loc,
                                                        ParserError(format!(
                                                            "\"@dw\" expression result ({}) will not fit in a word", value
                                                        )),
                                                    ));
                                                }
                                                if (self.here as usize) + 2 > (u16::MAX as usize) {
                                                    return Err((
                                                        loc,
                                                        ParserError(format!(
                                                            "\"@dw\" words extend past address $FFFF"
                                                        )),
                                                    ));
                                                }
                                                self.here += 2;
                                                data.push(value as u16);
                                            },
                                            None => return Err((loc, ParserError(format!("Every expression following a \"@dw\" directive must be immediately solvable")))),
                                        };
                                    }
                                }

                                if self.peeked_symbol(SymbolName::Comma)?.is_some() {
                                    self.next()?;
                                    continue;
                                }
                                break;
                            }
                            return Ok(Some(Item::Words { loc, data }));
                        }

                        DirectiveName::Ds => {
                            self.next()?;

                            let size = match self.const_expr()? {
                                Some(size) => {
                                    if (size as u32) > 0xFFFF {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "\"@ds\" size expression result ({}) will not fit in a word", size
                                            )),
                                        ));
                                    }
                                    if (self.here as usize) + (size as usize) > (u16::MAX as usize) {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "\"@ds\" size extends past address $FFFF"
                                            )),
                                        ));
                                    }
                                    self.here += size as u16;
                                    size as u16
                                },
                                None => return Err((loc, ParserError(format!("The size of a \"@ds\" directive must be immediately solvable")))),
                            };

                            let value = if self.peeked_symbol(SymbolName::Comma)?.is_some() {
                                self.next()?;
                                match self.const_expr()? {
                                    Some(value) => {
                                        if (value as u32) > 0xFF {
                                            return Err((
                                                loc,
                                                ParserError(format!(
                                                    "\"@ds\" value expression result ({}) will not fit in a byte", value
                                                )),
                                            ));
                                        }
                                        value as u8
                                    },
                                    None => return Err((loc, ParserError(format!("The value of a \"@ds\" directive must be immediately solvable")))),
                                }
                            } else {
                                0
                            };
                            return Ok(Some(Item::Space { loc, size, value }));
                        }

                        _ => todo!(),
                    }
                }

                Some(Token::Operation { .. }) => {}

                Some(_) => todo!(),

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
                @symbol test, @here

                ; @db "Hello World!", 42, "this", "is", "a test!"
                ; @dw 18
            "#,
        )]);

        parser
            .parse("/", "test.asm")
            .inspect_err(|e| eprintln!("{e}"))
            .unwrap();
    }
}
