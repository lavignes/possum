use std::{borrow::Borrow, cell::RefCell, fmt, io::Read, iter, path::Path, rc::Rc};

use fxhash::FxHashMap;

use crate::{
    expr::{Expr, ExprNode},
    fileman::{FileManager, FileSystem},
    intern::StrRef,
    lexer::{
        DirectiveName, FlagName, LabelKind, Lexer, LexerError, OperationName, RegisterName,
        SourceLoc, SymbolName, Token,
    },
    module::{Hole, Module},
    symtab::{Symbol, Symtab},
    StrInterner,
};

#[cfg(test)]
mod tests;

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
    data: Vec<u8>,
    holes: Vec<Hole>,

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
            data: Vec::new(),
            holes: Vec::new(),

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

        if let Err((loc, e)) = self.parse_all() {
            return Err(self.trace_error(loc, e));
        }

        let Self {
            str_interner,
            file_manager,
            symtab,
            data,
            holes,
            ..
        } = self;
        Ok(Module::new(str_interner, file_manager, symtab, data, holes))
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
                Some(Token::Comment { .. }) => {
                    self.stash = None;
                }

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
        writeln!(fmt_msg, "In \"{}\"", path.display()).unwrap();

        let mut included_from = self.lexer.as_ref().unwrap().included_from();
        for lexer in self.lexers.iter().rev() {
            let loc = included_from.unwrap();
            let path = self.file_manager.borrow().path(loc.pathref).unwrap();
            writeln!(
                fmt_msg,
                "\tIncluded from {}:{}:{}",
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

    fn end_of_input_err<T>(&mut self) -> Result<T, (SourceLoc, ParserError)> {
        Err((self.loc(), ParserError(format!("Unexpected end of input"))))
    }

    #[inline]
    #[must_use]
    fn expect_symbol(&mut self, sym: SymbolName) -> Result<(), (SourceLoc, ParserError)> {
        match self.next()? {
            Some(Token::Symbol { loc, name }) => {
                if name != sym {
                    Err((
                        loc,
                        ParserError(format!("Unexpected symbol: \"{name}\", expected \"{sym}\"")),
                    ))
                } else {
                    Ok(())
                }
            }
            Some(tok) => Err((
                tok.loc(),
                ParserError(format!(
                    "Unexpected \"{}\", expected the symbol \"{sym}\"",
                    tok.as_display(&self.str_interner)
                )),
            )),
            None => self.end_of_input_err(),
        }
    }

    #[inline]
    #[must_use]
    fn expect_register(&mut self, reg: RegisterName) -> Result<(), (SourceLoc, ParserError)> {
        match self.next()? {
            Some(Token::Register { loc, name }) => {
                if name != reg {
                    Err((
                        loc,
                        ParserError(format!(
                            "Unexpected register: \"{name}\", expected the register \"{reg}\""
                        )),
                    ))
                } else {
                    Ok(())
                }
            }
            Some(tok) => Err((
                tok.loc(),
                ParserError(format!(
                    "Unexpected \"{}\", expected the register \"{reg}\"",
                    tok.as_display(&self.str_interner)
                )),
            )),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn const_expr(&mut self) -> Result<(SourceLoc, Option<i32>), (SourceLoc, ParserError)> {
        self.expr()
            .map(|(loc, expr)| (loc, expr.evaluate(&self.symtab)))
    }

    #[must_use]
    fn expr(&mut self) -> Result<(SourceLoc, Expr), (SourceLoc, ParserError)> {
        let mut nodes = Vec::new();
        let loc = self.expr_prec_0(&mut nodes)?;
        Ok((loc, Expr::new(nodes)))
    }

    #[must_use]
    fn expr_prec_0(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_1(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::Question,
                ..
            }) => {
                self.next()?;
                self.expr_prec_1(nodes)?;
                if self.peeked_symbol(SymbolName::Colon)?.is_none() {
                    return Err((
                        self.loc(),
                        ParserError(format!("Expected a \":\" in ternary expression")),
                    ));
                }
                self.next()?;
                self.expr_prec_1(nodes)?;
                nodes.push(ExprNode::Ternary);
                Ok(loc)
            }
            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_1(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_2(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::DoublePipe,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::DoublePipe,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_2(nodes)?;
                    nodes.push(ExprNode::OrLogical);
                }
                Ok(loc)
            }
            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_2(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_3(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::DoubleAmpersand,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::DoubleAmpersand,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_3(nodes)?;
                    nodes.push(ExprNode::AndLogical);
                }
                Ok(loc)
            }
            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_3(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_4(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::Pipe,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::Pipe,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_4(nodes)?;
                    nodes.push(ExprNode::Or);
                }
                Ok(loc)
            }
            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_4(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_5(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::Caret,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::Caret,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_5(nodes)?;
                    nodes.push(ExprNode::Xor);
                }
                Ok(loc)
            }
            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_5(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_6(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::Ampersand,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::Caret,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_6(nodes)?;
                    nodes.push(ExprNode::And);
                }
                Ok(loc)
            }
            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_6(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_7(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::Ampersand,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::Ampersand,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_7(nodes)?;
                    nodes.push(ExprNode::And);
                }
                Ok(loc)
            }
            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_7(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_8(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::Equal,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::Equal,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_8(nodes)?;
                    nodes.push(ExprNode::Equal);
                }
                Ok(loc)
            }

            Some(Token::Symbol {
                name: SymbolName::NotEqual,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::NotEqual,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_8(nodes)?;
                    nodes.push(ExprNode::NotEqual);
                }
                Ok(loc)
            }

            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_8(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_9(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::LessThan,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::LessThan,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_9(nodes)?;
                    nodes.push(ExprNode::LessThan);
                }
                Ok(loc)
            }

            Some(Token::Symbol {
                name: SymbolName::LessEqual,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::LessEqual,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_9(nodes)?;
                    nodes.push(ExprNode::LessThanEqual);
                }
                Ok(loc)
            }

            Some(Token::Symbol {
                name: SymbolName::GreaterThan,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::GreaterThan,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_9(nodes)?;
                    nodes.push(ExprNode::GreaterThan);
                }
                Ok(loc)
            }

            Some(Token::Symbol {
                name: SymbolName::GreaterEqual,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::GreaterEqual,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_9(nodes)?;
                    nodes.push(ExprNode::GreaterThanEqual);
                }
                Ok(loc)
            }

            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_9(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_10(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::ShiftLeft,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::ShiftLeft,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_10(nodes)?;
                    nodes.push(ExprNode::ShiftLeft);
                }
                Ok(loc)
            }

            Some(Token::Symbol {
                name: SymbolName::ShiftRight,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::ShiftRight,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_10(nodes)?;
                    nodes.push(ExprNode::ShiftRight);
                }
                Ok(loc)
            }

            Some(Token::Symbol {
                name: SymbolName::ShiftLeftLogical,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::ShiftLeftLogical,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_10(nodes)?;
                    nodes.push(ExprNode::ShiftLeftLogical);
                }
                Ok(loc)
            }

            Some(Token::Symbol {
                name: SymbolName::ShiftRightLogical,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::ShiftRightLogical,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_10(nodes)?;
                    nodes.push(ExprNode::ShiftRightLogical);
                }
                Ok(loc)
            }

            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_10(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_11(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::Plus,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::Plus,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_11(nodes)?;
                    nodes.push(ExprNode::Add);
                }
                Ok(loc)
            }

            Some(Token::Symbol {
                name: SymbolName::Minus,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::Minus,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_11(nodes)?;
                    nodes.push(ExprNode::Sub);
                }
                Ok(loc)
            }

            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_11(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        let loc = self.expr_prec_12(nodes)?;

        match self.peek()? {
            Some(Token::Symbol {
                name: SymbolName::Star,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::Star,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_12(nodes)?;
                    nodes.push(ExprNode::Mul);
                }
                Ok(loc)
            }

            Some(Token::Symbol {
                name: SymbolName::Div,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::Div,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_12(nodes)?;
                    nodes.push(ExprNode::Div);
                }
                Ok(loc)
            }

            Some(Token::Symbol {
                name: SymbolName::Mod,
                ..
            }) => {
                while let Some(Token::Symbol {
                    name: SymbolName::Mod,
                    ..
                }) = self.peek()?
                {
                    self.next()?;
                    self.expr_prec_12(nodes)?;
                    nodes.push(ExprNode::Mod);
                }
                Ok(loc)
            }

            Some(_) => Ok(loc),
            None => self.end_of_input_err(),
        }
    }

    #[must_use]
    fn expr_prec_12(
        &mut self,
        nodes: &mut Vec<ExprNode>,
    ) -> Result<SourceLoc, (SourceLoc, ParserError)> {
        match self.peek()? {
            Some(&Token::Symbol {
                loc,
                name: SymbolName::Minus,
            }) => {
                self.next()?;
                self.expr_prec_12(nodes)?;
                nodes.push(ExprNode::Neg);
                Ok(loc)
            }

            Some(&Token::Symbol {
                loc,
                name: SymbolName::Bang,
            }) => {
                self.next()?;
                self.expr_prec_12(nodes)?;
                nodes.push(ExprNode::Not);
                Ok(loc)
            }

            Some(&Token::Symbol {
                loc,
                name: SymbolName::Tilde,
            }) => {
                self.next()?;
                self.expr_prec_12(nodes)?;
                nodes.push(ExprNode::Invert);
                Ok(loc)
            }

            Some(&Token::Symbol {
                loc,
                name: SymbolName::ParenOpen,
            }) => {
                self.next()?;
                self.expr_prec_0(nodes)?;
                if self.peeked_symbol(SymbolName::ParenClose)?.is_none() {
                    return Err((
                        self.loc(),
                        ParserError(format!("Expected a \")\" to close expression")),
                    ));
                }
                self.next()?;
                Ok(loc)
            }

            Some(&Token::Number { loc, value }) => {
                self.next()?;
                nodes.push(ExprNode::Value(value as i32));
                Ok(loc)
            }

            Some(&Token::Directive { loc, name }) => match name {
                DirectiveName::Here => {
                    self.next()?;
                    nodes.push(ExprNode::Value(self.here as i32));
                    Ok(loc)
                }
                _ => Err((
                    loc,
                    ParserError(format!(
                        "Only \"@here\" directives are allowed in expressions"
                    )),
                )),
            },

            Some(&Token::Label { loc, kind, value }) => {
                self.next()?;
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
                    match sym {
                        Symbol::Value(value) => {
                            nodes.push(ExprNode::Value(*value));
                        }
                        Symbol::Expr(expr) => {
                            if let Some(value) = expr.evaluate(&self.symtab) {
                                nodes.push(ExprNode::Value(value));
                            } else {
                                nodes.push(ExprNode::Label(value));
                            }
                        }
                    }
                } else {
                    nodes.push(ExprNode::Label(value));
                }
                Ok(loc)
            }

            Some(&tok) => Err((
                tok.loc(),
                ParserError(format!(
                    "Unexpected {} in expression",
                    tok.as_display(&self.str_interner)
                )),
            )),

            None => self.end_of_input_err(),
        }
    }

    #[inline]
    #[must_use]
    fn peeked_symbol(
        &mut self,
        sym: SymbolName,
    ) -> Result<Option<Token>, (SourceLoc, ParserError)> {
        match self.peek()? {
            Some(&tok @ Token::Symbol { name, .. }) if name == sym => Ok(Some(tok)),
            _ => Ok(None),
        }
    }

    #[must_use]
    fn parse_all(&mut self) -> Result<(), (SourceLoc, ParserError)> {
        loop {
            match self.peek()? {
                Some(Token::NewLine { .. }) => {
                    self.next()?;
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
                }

                Some(&Token::Directive { loc, name }) => match name {
                    DirectiveName::Org => {
                        self.next()?;

                        self.here = match self.const_expr()? {
                            (loc, Some(value)) => {
                                if (value as u32) > (u16::MAX as u32) {
                                    return Err((
                                        loc,
                                        ParserError(format!(
                                            "\"@org\" expression result ({}) is not a valid address", value
                                        )),
                                    ));
                                }
                                value as u16
                            },
                            (loc, None) => return Err((loc, ParserError(format!("The expression following an \"@org\" directive must be immediately solvable")))),
                        };
                    }

                    DirectiveName::Echo => {
                        self.next()?;

                        match self.peek()? {
                            Some(&Token::String { value, ..  }) => {
                                self.next()?;
                                let interner = self.str_interner.as_ref().borrow_mut();
                                let value = interner.get(value).unwrap();
                                println!("{value}");
                            }

                            Some(_) => {
                                match self.const_expr()? {
                                    (_, Some(value)) => {
                                        println!("{value}");
                                    },
                                    (loc, None) => return Err((loc, ParserError(format!("An expression following an \"@echo\" directive must be immediately solvable")))),
                                }
                            }

                            None => return self.end_of_input_err()
                        }
                    }

                    DirectiveName::Die => {
                        self.next()?;

                        match self.peek()? {
                            Some(&Token::String { value, ..  }) => {
                                self.next()?;
                                let interner = self.str_interner.as_ref().borrow_mut();
                                let value = interner.get(value).unwrap();
                                return Err((loc, ParserError(format!("{value}"))));
                            }

                            Some(_) => {
                                match self.const_expr()? {
                                    (_, Some(value)) => return Err((loc, ParserError(format!("{value}")))),
                                    (loc, None) => return Err((loc, ParserError(format!("An expression following an \"@die\" directive must be immediately solvable")))),
                                }
                            }

                            None => return self.end_of_input_err()
                        }
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
                                ParserError(format!("The symbol \"{label}\" was already defined")),
                            ));
                        }
                        self.expect_symbol(SymbolName::Comma)?;

                        let (_, expr) = self.expr()?;
                        self.symtab.insert(direct, Symbol::Expr(expr));
                    }

                    DirectiveName::Db => {
                        self.next()?;

                        loop {
                            match self.peek()? {
                                Some(&Token::String { value, .. }) => {
                                    self.next()?;
                                    let interner = self.str_interner.as_ref().borrow();
                                    let bytes = interner.get(value).unwrap().as_bytes();

                                    if (self.here as usize) + bytes.len() > (u16::MAX as usize) {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "\"@db\" bytes extend past address $FFFF"
                                            )),
                                        ));
                                    }
                                    self.here += bytes.len() as u16;
                                    self.data.extend_from_slice(bytes);
                                }

                                _ => {
                                    let (loc, expr) = self.expr()?;
                                    self.here += 1;
                                    if let Some(value) = expr.evaluate(&self.symtab) {
                                        if (value as u32) > (u8::MAX as u32) {
                                            return Err((
                                                    loc,
                                                    ParserError(format!(
                                                        "\"@db\" expression result ({value}) will not fit in a byte"
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
                                        self.data.push(value as u8);
                                    } else {
                                        self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                        self.data.push(0);
                                    }
                                }
                            }

                            if self.peeked_symbol(SymbolName::Comma)?.is_some() {
                                self.next()?;
                                continue;
                            }
                            break;
                        }
                    }

                    DirectiveName::Dw => {
                        self.next()?;

                        loop {
                            match self.peek()? {
                                _ => {
                                    let (loc, expr) = self.expr()?;
                                    self.here += 2;
                                    if let Some(value) = expr.evaluate(&self.symtab) {
                                        if (value as u32) > (u16::MAX as u32) {
                                            return Err((
                                                    loc,
                                                    ParserError(format!(
                                                        "\"@dw\" expression result ({value}) will not fit in a word"
                                                    )),
                                                ));
                                        }
                                        if (self.here as usize) + 1 > (u16::MAX as usize) {
                                            return Err((
                                                loc,
                                                ParserError(format!(
                                                    "\"@dw\" bytes extend past address $FFFF"
                                                )),
                                            ));
                                        }
                                        self.data.extend_from_slice(&(value as u16).to_le_bytes());
                                    } else {
                                        self.holes.push(Hole::word(loc, self.data.len(), expr));
                                        self.data.push(0);
                                        self.data.push(0);
                                    }
                                }
                            }

                            if self.peeked_symbol(SymbolName::Comma)?.is_some() {
                                self.next()?;
                                continue;
                            }
                            break;
                        }
                    }

                    DirectiveName::Ds => {
                        self.next()?;

                        let size = match self.const_expr()? {
                            (loc, None) => {
                                return Err((
                                    loc,
                                    ParserError(format!(
                                    "The size of a \"@ds\" directive must be immediately solvable"
                                )),
                                ))
                            }
                            (loc, Some(size)) => {
                                if (size as u32) > (u16::MAX as u32) {
                                    return Err((
                                            loc,
                                            ParserError(format!(
                                                "\"@ds\" size expression result ({size}) will not fit in a word"
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
                                size as usize
                            }
                        };

                        let value = if self.peeked_symbol(SymbolName::Comma)?.is_some() {
                            self.next()?;
                            let (loc, expr) = self.expr()?;
                            if let Some(value) = expr.evaluate(&self.symtab) {
                                if (value as u32) > (u8::MAX as u32) {
                                    return Err((
                                            loc,
                                            ParserError(format!(
                                                "\"@ds\" value expression result ({value}) will not fit in a byte"
                                            )),
                                        ));
                                }
                                value as u8
                            } else {
                                self.holes
                                    .push(Hole::space(loc, self.data.len(), size, expr));
                                0
                            }
                        } else {
                            0
                        };
                        self.data.extend(iter::repeat(value).take(size));
                    }

                    _ => todo!(),
                },

                Some(Token::Operation { loc, name }) => match name {
                    OperationName::Adc => {
                        self.next()?;
                        match self.next()? {
                            Some(Token::Register {
                                name: RegisterName::A,
                                ..
                            }) => {
                                self.expect_symbol(SymbolName::Comma)?;

                                match self.peek()? {
                                    Some(Token::Register {
                                        name: RegisterName::A,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x8F);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::B,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x88);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::C,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x89);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::D,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x8A);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::E,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x8B);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::H,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x8C);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::L,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x8D);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IXH,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0x8C);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IXL,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0x8D);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IYH,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0x8C);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IYL,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0x8D);
                                    }

                                    Some(Token::Symbol {
                                        name: SymbolName::ParenOpen,
                                        ..
                                    }) => {
                                        self.next()?;
                                        match self.next()? {
                                            Some(Token::Register {
                                                name: RegisterName::HL,
                                                ..
                                            }) => {
                                                self.here += 1;
                                                self.data.push(0x8E);
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(Token::Register {
                                                name: RegisterName::IX,
                                                ..
                                            }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 3;
                                                self.data.push(0xDD);
                                                self.data.push(0x8E);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                        loc,
                                                        ParserError(format!(
                                                            "Expression result ({value}) will not fit in a byte"
                                                        )),
                                                    ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(Token::Register {
                                                name: RegisterName::IY,
                                                ..
                                            }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 3;
                                                self.data.push(0xFD);
                                                self.data.push(0x8E);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                        loc,
                                                        ParserError(format!(
                                                            "Expression result ({value}) will not fit in a byte"
                                                        )),
                                                    ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(_) => {
                                                self.here += 2;
                                                self.data.push(0xCE);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                        loc,
                                                        ParserError(format!(
                                                            "Expression result ({value}) will not fit in a byte"
                                                        )),
                                                    ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }
                                            None => return self.end_of_input_err(),
                                        }
                                    }

                                    Some(_) => {
                                        self.here += 2;
                                        self.data.push(0xCE);
                                        let (loc, expr) = self.expr()?;
                                        if let Some(value) = expr.evaluate(&self.symtab) {
                                            if (value as u32) > (u8::MAX as u32) {
                                                return Err((
                                                    loc,
                                                    ParserError(format!(
                                                        "Expression result ({value}) will not fit in a byte"
                                                    )),
                                                ));
                                            }
                                            self.data.push(value as u8);
                                        } else {
                                            self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                            self.data.push(0);
                                        }
                                    }
                                    None => return self.end_of_input_err(),
                                }
                            }

                            Some(Token::Register {
                                loc,
                                name: RegisterName::HL,
                            }) => {
                                self.expect_symbol(SymbolName::Comma)?;
                                match self.next()? {
                                    Some(Token::Register {
                                             name: RegisterName::BC,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xED);
                                        self.data.push(0x4A);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::DE,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xED);
                                        self.data.push(0x5A);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::HL,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xED);
                                        self.data.push(0x6A);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::SP,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xED);
                                        self.data.push(0x7A);
                                    }

                                    Some(tok) => {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "Unexpected {}, expected register \"bc\", \"de\", \"hl\" or \"sp\"",
                                                tok.as_display(&self.str_interner)
                                            )),
                                        ))
                                    }
                                    None => return self.end_of_input_err(),
                                }
                            }

                            Some(tok) => {
                                return Err((
                                    self.loc(),
                                    ParserError(format!(
                                        "Unexpected {}, expected register \"a\" or \"hl\"",
                                        tok.as_display(&self.str_interner)
                                    )),
                                ))
                            }

                            _ => return self.end_of_input_err(),
                        }
                    }

                    OperationName::Add => {
                        self.next()?;
                        match self.next()? {
                            Some(Token::Register {
                                name: RegisterName::A,
                                ..
                            }) => {
                                self.expect_symbol(SymbolName::Comma)?;

                                match self.peek()? {
                                    Some(Token::Register {
                                        name: RegisterName::A,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x87);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::B,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x80);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::C,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x81);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::D,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x82);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::E,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x83);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::H,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x84);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::L,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0x85);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IXH,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0x84);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IXL,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0x85);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IYH,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0x84);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IYL,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0x85);
                                    }

                                    Some(Token::Symbol {
                                        name: SymbolName::ParenOpen,
                                        ..
                                    }) => {
                                        self.next()?;
                                        match self.next()? {
                                            Some(Token::Register {
                                                name: RegisterName::HL,
                                                ..
                                            }) => {
                                                self.here += 1;
                                                self.data.push(0x86);
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(Token::Register {
                                                name: RegisterName::IX,
                                                ..
                                            }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 3;
                                                self.data.push(0xDD);
                                                self.data.push(0x86);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                        loc,
                                                        ParserError(format!(
                                                            "Expression result ({value}) will not fit in a byte"
                                                        )),
                                                    ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(Token::Register {
                                                name: RegisterName::IY,
                                                ..
                                            }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 3;
                                                self.data.push(0xFD);
                                                self.data.push(0x86);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                        loc,
                                                        ParserError(format!(
                                                            "Expression result ({value}) will not fit in a byte"
                                                        )),
                                                    ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(_) => {
                                                self.here += 2;
                                                self.data.push(0xC6);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                        loc,
                                                        ParserError(format!(
                                                            "Expression result ({value}) will not fit in a byte"
                                                        )),
                                                    ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }
                                            None => return self.end_of_input_err(),
                                        }
                                    }

                                    Some(_) => {
                                        self.here += 2;
                                        self.data.push(0xC6);
                                        let (loc, expr) = self.expr()?;
                                        if let Some(value) = expr.evaluate(&self.symtab) {
                                            if (value as u32) > (u8::MAX as u32) {
                                                return Err((
                                                    loc,
                                                    ParserError(format!(
                                                        "Expression result ({value}) will not fit in a byte"
                                                    )),
                                                ));
                                            }
                                            self.data.push(value as u8);
                                        } else {
                                            self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                            self.data.push(0);
                                        }
                                    }
                                    None => return self.end_of_input_err(),
                                }
                            }

                            Some(Token::Register {
                                loc,
                                name: RegisterName::HL,
                            }) => {
                                self.expect_symbol(SymbolName::Comma)?;
                                match self.next()? {
                                    Some(Token::Register {
                                             name: RegisterName::BC,
                                             ..
                                         }) => {
                                        self.here += 1;
                                        self.data.push(0x09);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::DE,
                                             ..
                                         }) => {
                                        self.here += 1;
                                        self.data.push(0x19);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::HL,
                                             ..
                                         }) => {
                                        self.here += 1;
                                        self.data.push(0x29);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::SP,
                                             ..
                                         }) => {
                                        self.here += 1;
                                        self.data.push(0x39);
                                    }

                                    Some(tok) => {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "Unexpected {}, expected register \"bc\", \"de\", \"hl\" or \"sp\"",
                                                tok.as_display(&self.str_interner)
                                            )),
                                        ))
                                    }
                                    None => return self.end_of_input_err(),
                                }
                            }

                            Some(Token::Register {
                                loc,
                                name: RegisterName::IX,
                            }) => {
                                self.expect_symbol(SymbolName::Comma)?;
                                match self.next()? {
                                    Some(Token::Register {
                                             name: RegisterName::BC,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0x09);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::DE,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0x19);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::IX,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0x29);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::SP,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0x39);
                                    }

                                    Some(tok) => {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "Unexpected {}, expected register \"bc\", \"de\", \"ix\" or \"sp\"",
                                                tok.as_display(&self.str_interner)
                                            )),
                                        ))
                                    }
                                    None => return self.end_of_input_err(),
                                }
                            }

                            Some(Token::Register {
                                loc,
                                name: RegisterName::IY,
                            }) => {
                                self.expect_symbol(SymbolName::Comma)?;
                                match self.next()? {
                                    Some(Token::Register {
                                             name: RegisterName::BC,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0x09);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::DE,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0x19);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::IY,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0x29);
                                    }

                                    Some(Token::Register {
                                             name: RegisterName::SP,
                                             ..
                                         }) => {
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0x39);
                                    }

                                    Some(tok) => {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "Unexpected {}, expected register \"bc\", \"de\", \"iy\" or \"sp\"",
                                                tok.as_display(&self.str_interner)
                                            )),
                                        ))
                                    }
                                    None => return self.end_of_input_err(),
                                }
                            }

                            Some(tok) => {
                                return Err((
                                    self.loc(),
                                    ParserError(format!(
                                        "Unexpected {}, expected register \"a\" or \"hl\"",
                                        tok.as_display(&self.str_interner)
                                    )),
                                ))
                            }

                            _ => return self.end_of_input_err(),
                        }
                    }

                    OperationName::And => {
                        self.next()?;
                        match self.next()? {
                            Some(Token::Register {
                                name: RegisterName::A,
                                ..
                            }) => {
                                self.expect_symbol(SymbolName::Comma)?;

                                match self.peek()? {
                                    Some(Token::Register {
                                        name: RegisterName::A,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xA7);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::B,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xA0);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::C,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xA1);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::D,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xA2);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::E,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xA3);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::H,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xA4);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::L,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xA5);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IXH,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0xA4);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IXL,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0xA5);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IYH,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0xA4);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IYL,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0xA5);
                                    }

                                    Some(Token::Symbol {
                                        name: SymbolName::ParenOpen,
                                        ..
                                    }) => {
                                        self.next()?;
                                        match self.next()? {
                                            Some(Token::Register {
                                                name: RegisterName::HL,
                                                ..
                                            }) => {
                                                self.here += 1;
                                                self.data.push(0xA6);
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(Token::Register {
                                                name: RegisterName::IX,
                                                ..
                                            }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 3;
                                                self.data.push(0xDD);
                                                self.data.push(0xA6);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                            loc,
                                                            ParserError(format!(
                                                                "Expression result ({value}) will not fit in a byte"
                                                            )),
                                                        ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(Token::Register {
                                                name: RegisterName::IY,
                                                ..
                                            }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 3;
                                                self.data.push(0xFD);
                                                self.data.push(0xA6);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                            loc,
                                                            ParserError(format!(
                                                                "Expression result ({value}) will not fit in a byte"
                                                            )),
                                                        ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(_) => {
                                                self.here += 2;
                                                self.data.push(0xE6);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                            loc,
                                                            ParserError(format!(
                                                                "Expression result ({value}) will not fit in a byte"
                                                            )),
                                                        ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }
                                            None => return self.end_of_input_err(),
                                        }
                                    }

                                    Some(_) => {
                                        self.here += 2;
                                        self.data.push(0xE6);
                                        let (loc, expr) = self.expr()?;
                                        if let Some(value) = expr.evaluate(&self.symtab) {
                                            if (value as u32) > (u8::MAX as u32) {
                                                return Err((
                                                    loc,
                                                    ParserError(format!(
                                                        "Expression result ({value}) will not fit in a byte"
                                                    )),
                                                ));
                                            }
                                            self.data.push(value as u8);
                                        } else {
                                            self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                            self.data.push(0);
                                        }
                                    }
                                    None => return self.end_of_input_err(),
                                }
                            }

                            Some(tok) => {
                                return Err((
                                    self.loc(),
                                    ParserError(format!(
                                        "Unexpected {}, expected register \"a\"",
                                        tok.as_display(&self.str_interner)
                                    )),
                                ))
                            }

                            _ => return self.end_of_input_err(),
                        }
                    }

                    OperationName::Bit => {
                        self.next()?;
                        match self.const_expr()? {
                            (loc, None) => {
                                return Err((
                                    loc,
                                    ParserError(format!("Bit index must be immediately solvable")),
                                ))
                            }
                            (loc, Some(value)) => {
                                if value < 0 || value > 7 {
                                    return Err((
                                        loc,
                                        ParserError(format!(
                                            "Bit index ({value}) must be between 0 and 7"
                                        )),
                                    ));
                                }

                                self.expect_symbol(SymbolName::Comma)?;

                                match self.next()? {
                                    Some(Token::Register {
                                        name: RegisterName::A,
                                        ..
                                    }) => {
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x47,
                                            1 => 0x4F,
                                            2 => 0x57,
                                            3 => 0x5F,
                                            4 => 0x67,
                                            5 => 0x6F,
                                            6 => 0x77,
                                            7 => 0x7F,
                                            _ => unreachable!(),
                                        });
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::B,
                                        ..
                                    }) => {
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x40,
                                            1 => 0x48,
                                            2 => 0x50,
                                            3 => 0x58,
                                            4 => 0x60,
                                            5 => 0x68,
                                            6 => 0x70,
                                            7 => 0x78,
                                            _ => unreachable!(),
                                        });
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::C,
                                        ..
                                    }) => {
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x41,
                                            1 => 0x49,
                                            2 => 0x51,
                                            3 => 0x59,
                                            4 => 0x61,
                                            5 => 0x69,
                                            6 => 0x71,
                                            7 => 0x79,
                                            _ => unreachable!(),
                                        });
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::D,
                                        ..
                                    }) => {
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x42,
                                            1 => 0x4A,
                                            2 => 0x52,
                                            3 => 0x5A,
                                            4 => 0x62,
                                            5 => 0x6A,
                                            6 => 0x72,
                                            7 => 0x7A,
                                            _ => unreachable!(),
                                        });
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::E,
                                        ..
                                    }) => {
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x43,
                                            1 => 0x4B,
                                            2 => 0x53,
                                            3 => 0x5B,
                                            4 => 0x63,
                                            5 => 0x6B,
                                            6 => 0x73,
                                            7 => 0x7B,
                                            _ => unreachable!(),
                                        });
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::H,
                                        ..
                                    }) => {
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x44,
                                            1 => 0x4C,
                                            2 => 0x54,
                                            3 => 0x5C,
                                            4 => 0x64,
                                            5 => 0x6C,
                                            6 => 0x74,
                                            7 => 0x7C,
                                            _ => unreachable!(),
                                        });
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::L,
                                        ..
                                    }) => {
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x45,
                                            1 => 0x4D,
                                            2 => 0x55,
                                            3 => 0x5D,
                                            4 => 0x65,
                                            5 => 0x6D,
                                            6 => 0x75,
                                            7 => 0x7D,
                                            _ => unreachable!(),
                                        });
                                    }

                                    Some(Token::Symbol {
                                        name: SymbolName::ParenOpen,
                                        ..
                                    }) => {
                                        match self.next()? {
                                            Some(Token::Register { name: RegisterName::HL, .. }) => {
                                                self.here += 2;
                                                self.data.push(0xCB);
                                                self.data.push(match value {
                                                    0 => 0x46,
                                                    1 => 0x4E,
                                                    2 => 0x56,
                                                    3 => 0x5E,
                                                    4 => 0x66,
                                                    5 => 0x6E,
                                                    6 => 0x76,
                                                    7 => 0x7E,
                                                    _ => unreachable!(),
                                                });
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(Token::Register { name: RegisterName::IX, .. }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 4;
                                                self.data.push(0xDD);
                                                self.data.push(0xCB);
                                                self.data.push(match value {
                                                    0 => 0x46,
                                                    1 => 0x4E,
                                                    2 => 0x56,
                                                    3 => 0x5E,
                                                    4 => 0x66,
                                                    5 => 0x6E,
                                                    6 => 0x76,
                                                    7 => 0x7E,
                                                    _ => unreachable!(),
                                                });
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((loc, ParserError(format!("Expression result ({value}) will not fit in a byte"))));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(Token::Register { name: RegisterName::IY, .. }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 4;
                                                self.data.push(0xFD);
                                                self.data.push(0xCB);
                                                self.data.push(match value {
                                                    0 => 0x46,
                                                    1 => 0x4E,
                                                    2 => 0x56,
                                                    3 => 0x5E,
                                                    4 => 0x66,
                                                    5 => 0x6E,
                                                    6 => 0x76,
                                                    7 => 0x7E,
                                                    _ => unreachable!(),
                                                });
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((loc, ParserError(format!("Expression result ({value}) will not fit in a byte"))));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(tok) => return Err((loc, ParserError(format!("Unexpected {}, expected register \"hl\", \"ix\", or \"iy\"", tok.as_display(&self.str_interner))))),
                                            None => return self.end_of_input_err(),
                                        }
                                    }

                                    Some(tok) => {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "Unexpected {}, expected a register",
                                                tok.as_display(&self.str_interner)
                                            )),
                                        ))
                                    }
                                    None => return self.end_of_input_err(),
                                }
                            }
                        }
                    }

                    OperationName::Call => {
                        self.next()?;
                        self.here += 3;
                        match self.peek()? {
                            Some(Token::Flag {
                                name: FlagName::Zero,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0xCC);
                                self.expect_symbol(SymbolName::Comma)?;
                            }
                            Some(Token::Flag {
                                name: FlagName::NotZero,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0xC4);
                                self.expect_symbol(SymbolName::Comma)?;
                            }
                            Some(Token::Register {
                                name: RegisterName::C,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0xDC);
                                self.expect_symbol(SymbolName::Comma)?;
                            }
                            Some(Token::Flag {
                                name: FlagName::NotCarry,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0xD4);
                                self.expect_symbol(SymbolName::Comma)?;
                            }
                            Some(Token::Flag {
                                name: FlagName::ParityEven,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0xEC);
                                self.expect_symbol(SymbolName::Comma)?;
                            }
                            Some(Token::Flag {
                                name: FlagName::ParityOdd,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0xE4);
                                self.expect_symbol(SymbolName::Comma)?;
                            }
                            Some(Token::Flag {
                                name: FlagName::Positive,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0xF4);
                                self.expect_symbol(SymbolName::Comma)?;
                            }
                            Some(Token::Flag {
                                name: FlagName::Negative,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0xFC);
                                self.expect_symbol(SymbolName::Comma)?;
                            }
                            Some(_) => {
                                self.data.push(0xCD);
                            }
                            None => return self.end_of_input_err(),
                        }
                        let (loc, expr) = self.expr()?;
                        if let Some(value) = expr.evaluate(&self.symtab) {
                            if (value as u32) > (u16::MAX as u32) {
                                return Err((
                                    loc,
                                    ParserError(format!(
                                        "Expression result ({value}) will not fit in a word"
                                    )),
                                ));
                            }
                            self.data.extend_from_slice(&(value as u16).to_le_bytes());
                        } else {
                            self.data.push(0);
                            self.data.push(0);
                        }
                    }

                    OperationName::Ccf => {
                        self.next()?;
                        self.here += 1;
                        self.data.push(0x3F);
                    }

                    OperationName::Cp => {
                        self.next()?;
                        match self.next()? {
                            Some(Token::Register {
                                name: RegisterName::A,
                                ..
                            }) => {
                                self.expect_symbol(SymbolName::Comma)?;

                                match self.peek()? {
                                    Some(Token::Register {
                                        name: RegisterName::A,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xBF);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::B,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xB8);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::C,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xB9);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::D,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xBA);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::E,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xBB);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::H,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xBC);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::L,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 1;
                                        self.data.push(0xBD);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IXH,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0xBC);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IXL,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0xBD);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IYH,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0xBC);
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::IYL,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0xBD);
                                    }

                                    Some(Token::Symbol {
                                        name: SymbolName::ParenOpen,
                                        ..
                                    }) => {
                                        self.next()?;
                                        match self.next()? {
                                            Some(Token::Register {
                                                name: RegisterName::HL,
                                                ..
                                            }) => {
                                                self.here += 1;
                                                self.data.push(0xBE);
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(Token::Register {
                                                name: RegisterName::IX,
                                                ..
                                            }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 3;
                                                self.data.push(0xDD);
                                                self.data.push(0xBE);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                            loc,
                                                            ParserError(format!(
                                                                "Expression result ({value}) will not fit in a byte"
                                                            )),
                                                        ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(Token::Register {
                                                name: RegisterName::IY,
                                                ..
                                            }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 3;
                                                self.data.push(0xFD);
                                                self.data.push(0xBE);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                            loc,
                                                            ParserError(format!(
                                                                "Expression result ({value}) will not fit in a byte"
                                                            )),
                                                        ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }

                                            Some(_) => {
                                                self.here += 2;
                                                self.data.push(0xFE);
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((
                                                            loc,
                                                            ParserError(format!(
                                                                "Expression result ({value}) will not fit in a byte"
                                                            )),
                                                        ));
                                                    }
                                                    self.data.push(value as u8);
                                                } else {
                                                    self.holes.push(Hole::byte(
                                                        loc,
                                                        self.data.len(),
                                                        expr,
                                                    ));
                                                    self.data.push(0);
                                                }
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }
                                            None => return self.end_of_input_err(),
                                        }
                                    }

                                    Some(_) => {
                                        self.here += 2;
                                        self.data.push(0xFE);
                                        let (loc, expr) = self.expr()?;
                                        if let Some(value) = expr.evaluate(&self.symtab) {
                                            if (value as u32) > (u8::MAX as u32) {
                                                return Err((
                                                    loc,
                                                    ParserError(format!(
                                                        "Expression result ({value}) will not fit in a byte"
                                                    )),
                                                ));
                                            }
                                            self.data.push(value as u8);
                                        } else {
                                            self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                            self.data.push(0);
                                        }
                                    }
                                    None => return self.end_of_input_err(),
                                }
                            }

                            Some(tok) => {
                                return Err((
                                    self.loc(),
                                    ParserError(format!(
                                        "Unexpected {}, expected register \"a\"",
                                        tok.as_display(&self.str_interner)
                                    )),
                                ))
                            }

                            _ => return self.end_of_input_err(),
                        }
                    }

                    OperationName::Cpd => {
                        self.next()?;
                        self.here += 2;
                        self.data.push(0xED);
                        self.data.push(0xA9);
                    }

                    OperationName::Cpdr => {
                        self.next()?;
                        self.here += 2;
                        self.data.push(0xED);
                        self.data.push(0xB9);
                    }

                    OperationName::Cpi => {
                        self.next()?;
                        self.here += 2;
                        self.data.push(0xED);
                        self.data.push(0xA1);
                    }

                    OperationName::Cpir => {
                        self.next()?;
                        self.here += 2;
                        self.data.push(0xED);
                        self.data.push(0xB1);
                    }

                    OperationName::Cpl => {
                        self.next()?;
                        self.here += 1;
                        self.data.push(0x2F);
                    }

                    OperationName::Daa => {
                        self.next()?;
                        self.here += 1;
                        self.data.push(0x27);
                    }

                    OperationName::Dec => {
                        self.next()?;
                        match self.next()? {
                            Some(Token::Register { name: RegisterName::A, .. }) => {
                                self.here += 1;
                                self.data.push(0x3D);
                            }

                            Some(Token::Register { name: RegisterName::B, .. }) => {
                                self.here += 1;
                                self.data.push(0x05);
                            }

                            Some(Token::Register { name: RegisterName::C, .. }) => {
                                self.here += 1;
                                self.data.push(0x0D);
                            }

                            Some(Token::Register { name: RegisterName::D, .. }) => {
                                self.here += 1;
                                self.data.push(0x15);
                            }

                            Some(Token::Register { name: RegisterName::E, .. }) => {
                                self.here += 1;
                                self.data.push(0x1D);
                            }
                            
                            Some(Token::Register { name: RegisterName::H, .. }) => {
                                self.here += 1;
                                self.data.push(0x25);
                            }

                            Some(Token::Register { name: RegisterName::L, .. }) => {
                                self.here += 1;
                                self.data.push(0x2D);
                            }

                            Some(Token::Register { name: RegisterName::IXH, .. }) => {
                                self.here += 2;
                                self.data.push(0xDD);
                                self.data.push(0x25);
                            }

                            Some(Token::Register { name: RegisterName::IXL, .. }) => {
                                self.here += 2;
                                self.data.push(0xDD);
                                self.data.push(0x2D);
                            }

                            Some(Token::Register { name: RegisterName::IYH, .. }) => {
                                self.here += 2;
                                self.data.push(0xFD);
                                self.data.push(0x25);
                            }

                            Some(Token::Register { name: RegisterName::IYL, .. }) => {
                                self.here += 2;
                                self.data.push(0xFD);
                                self.data.push(0x2D);
                            }
                            
                            Some(Token::Register { name: RegisterName::BC, .. }) => {
                                self.here += 1;
                                self.data.push(0x0B);
                            }

                            Some(Token::Register { name: RegisterName::DE, .. }) => {
                                self.here += 1;
                                self.data.push(0x1B);
                            }

                            Some(Token::Register { name: RegisterName::HL, .. }) => {
                                self.here += 1;
                                self.data.push(0x2B);
                            }

                            Some(Token::Register { name: RegisterName::SP, .. }) => {
                                self.here += 1;
                                self.data.push(0x3B);
                            }

                            Some(Token::Register { name: RegisterName::IX, .. }) => {
                                self.here += 2;
                                self.data.push(0xDD);
                                self.data.push(0x2B);
                            }

                            Some(Token::Register { name: RegisterName::IY, .. }) => {
                                self.here += 2;
                                self.data.push(0xFD);
                                self.data.push(0x2B);
                            }

                            Some(Token::Symbol { name: SymbolName::ParenOpen, .. }) => {
                                match self.next()? {
                                    None => return self.end_of_input_err(),

                                    Some(Token::Register { name: RegisterName::HL, .. }) => {
                                        self.here += 1;
                                        self.data.push(0x35);
                                        self.expect_symbol(SymbolName::ParenClose)?;
                                    }

                                    Some(Token::Register { name: RegisterName::IX, .. }) => {
                                        self.expect_symbol(SymbolName::Plus)?;
                                        self.here += 3;
                                        self.data.push(0xDD);
                                        self.data.push(0x35);
                                        let (loc, expr) = self.expr()?;
                                        if let Some(value) = expr.evaluate(&self.symtab) {
                                            if (value as u32) > (u8::MAX as u32) {
                                                return Err((loc, ParserError(format!("Expression result ({value}) will not fit in a byte"))));
                                            }
                                            self.data.push(value as u8);
                                        } else {
                                            self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                            self.data.push(0);
                                        }
                                        self.expect_symbol(SymbolName::ParenClose)?;
                                    }
                                    
                                    Some(Token::Register { name: RegisterName::IY, .. }) => {
                                        self.expect_symbol(SymbolName::Plus)?;
                                        self.here += 3;
                                        self.data.push(0xFD);
                                        self.data.push(0x35);
                                        let (loc, expr) = self.expr()?;
                                        if let Some(value) = expr.evaluate(&self.symtab) {
                                            if (value as u32) > (u8::MAX as u32) {
                                                return Err((loc, ParserError(format!("Expression result ({value}) will not fit in a byte"))));
                                            }
                                            self.data.push(value as u8);
                                        } else {
                                            self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                            self.data.push(0);
                                        }
                                        self.expect_symbol(SymbolName::ParenClose)?;
                                    }

                                    Some(tok) => return Err((tok.loc(), ParserError(format!("Unexpected {}, expected registers \"hl\", \"ix\", or \"iy\"", tok.as_display(&self.str_interner))))),
                                }
                            }

                            Some(tok) => return Err((tok.loc(), ParserError(format!("Unexpected {}, expected a register", tok.as_display(&self.str_interner))))),
                            None => return self.end_of_input_err(),
                        }
                    }

                    OperationName::Di => {
                        self.next()?;
                        self.here += 1;
                        self.data.push(0xF3);
                    }

                    OperationName::Djnz => {
                        self.next()?;
                        self.here += 2;
                        self.data.push(0x10);
                        let (loc, mut expr) = self.expr()?;
                        // Make the expression relative to @here
                        expr.push(ExprNode::Value(self.here as i32));
                        expr.push(ExprNode::Sub);
                        if let Some(value) = expr.evaluate(&self.symtab) {
                            if (value < (i8::MIN as i32)) || (value > (i8::MAX as i32)) {
                                return Err((
                                    loc,
                                    ParserError(format!(
                                        "Jump distance ({value}) will not fit in a byte"
                                    )),
                                ));
                            }
                            self.data.push(value as u8);
                        } else {
                            self.holes
                                .push(Hole::signed_byte(loc, self.data.len(), expr));
                            self.data.push(0);
                        }
                    }

                    OperationName::Ei => {
                        self.next()?;
                        self.here += 1;
                        self.data.push(0xFB);
                    }

                    OperationName::Ex => {
                        self.next()?;
                        match self.next()? {
                            None => return self.end_of_input_err(),

                            Some(Token::Register { name: RegisterName::AF, .. }) => {
                                self.expect_symbol(SymbolName::Comma)?;
                                self.expect_register(RegisterName::AFPrime)?;
                                self.here += 1;
                                self.data.push(0x08);
                            }
                            
                            Some(Token::Register { name: RegisterName::DE, .. }) => {
                                self.expect_symbol(SymbolName::Comma)?;
                                self.expect_register(RegisterName::HL)?;
                                self.here += 1;
                                self.data.push(0xEB);
                            }

                            Some(Token::Symbol { name: SymbolName::ParenOpen, .. }) => {
                                self.expect_register(RegisterName::SP)?;
                                self.expect_symbol(SymbolName::ParenClose)?;
                                self.expect_symbol(SymbolName::Comma)?;

                                match self.next()? {
                                    None => return self.end_of_input_err(),

                                    Some(Token::Register { name: RegisterName::HL, .. }) => {
                                        self.here += 1;
                                        self.data.push(0xE3);
                                    }
                                    
                                    Some(Token::Register { name: RegisterName::IX, .. }) => {
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0xE3);
                                    }
                                    
                                    Some(Token::Register { name: RegisterName::IY, .. }) => {
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0xE3);
                                    }

                                    Some(tok) => return Err((tok.loc(), ParserError(format!("Unexpected {}, expected the registers \"hl\", \"ix\", or \"iy\"", tok.as_display(&self.str_interner))))),
                                }
                            }

                            Some(tok) => return Err((tok.loc(), ParserError(format!("Unexpected {}, expected the registers \"af\", \"de\", or \"(sp)\"", tok.as_display(&self.str_interner))))),
                        }
                    }

                    OperationName::Exx => {
                        self.next()?;
                        self.here += 1;
                        self.data.push(0xD9);
                    }

                    OperationName::Halt => {
                        self.next()?;
                        self.here += 1;
                        self.data.push(0x76);
                    }

                    OperationName::Im => {
                        self.next()?;
                        self.here += 2;
                        self.data.push(0xED);
                        match self.next()? {
                            None => return self.end_of_input_err(),

                            Some(Token::Number { value: 0, .. }) => {
                                self.data.push(0x46);
                            }

                            Some(Token::Number { value: 1, .. }) => {
                                self.data.push(0x56);
                            }

                            Some(Token::Number { value: 2, .. }) => {
                                self.data.push(0x5E);
                            }

                            Some(tok) => {
                                return Err((
                                    tok.loc(),
                                    ParserError(format!(
                                        "Unexpected {}, expected the numbers 0, 1, or 2",
                                        tok.as_display(&self.str_interner)
                                    )),
                                ))
                            }
                        }
                    }

                    OperationName::In => {
                        self.next()?;
                        self.here += 2;
                        match self.next()? {
                            None => return self.end_of_input_err(),

                            Some(Token::Register {
                                name: RegisterName::A,
                                ..
                            }) => {
                                self.expect_symbol(SymbolName::Comma)?;
                                self.expect_symbol(SymbolName::ParenOpen)?;
                                match self.peek()? {
                                    None => return self.end_of_input_err(),

                                    Some(Token::Register {
                                        name: RegisterName::C,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.expect_symbol(SymbolName::ParenClose)?;
                                        self.data.push(0xED);
                                        self.data.push(0x78);
                                    }

                                    Some(_) => {
                                        self.data.push(0xDB);
                                        let (loc, expr) = self.expr()?;
                                        if let Some(value) = expr.evaluate(&self.symtab) {
                                            if (value as u32) > (u8::MAX as u32) {
                                                return Err((loc, ParserError(format!("Expression result ({value}) will not fit in a byte"))));
                                            }
                                            self.data.push(value as u8);
                                        } else {
                                            self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                            self.data.push(0);
                                        }
                                        self.expect_symbol(SymbolName::ParenClose)?;
                                    }
                                }
                            }

                            Some(Token::Register {
                                name: RegisterName::B,
                                ..
                            }) => {
                                self.data.push(0xED);
                                self.data.push(0x40);
                                self.expect_symbol(SymbolName::Comma)?;
                                self.expect_symbol(SymbolName::ParenOpen)?;
                                self.expect_register(RegisterName::C)?;
                                self.expect_symbol(SymbolName::ParenClose)?;
                            }

                            Some(Token::Register {
                                name: RegisterName::C,
                                ..
                            }) => {
                                self.data.push(0xED);
                                self.data.push(0x48);
                                self.expect_symbol(SymbolName::Comma)?;
                                self.expect_symbol(SymbolName::ParenOpen)?;
                                self.expect_register(RegisterName::C)?;
                                self.expect_symbol(SymbolName::ParenClose)?;
                            }

                            Some(Token::Register {
                                name: RegisterName::D,
                                ..
                            }) => {
                                self.data.push(0xED);
                                self.data.push(0x50);
                                self.expect_symbol(SymbolName::Comma)?;
                                self.expect_symbol(SymbolName::ParenOpen)?;
                                self.expect_register(RegisterName::C)?;
                                self.expect_symbol(SymbolName::ParenClose)?;
                            }

                            Some(Token::Register {
                                name: RegisterName::E,
                                ..
                            }) => {
                                self.data.push(0xED);
                                self.data.push(0x58);
                                self.expect_symbol(SymbolName::Comma)?;
                                self.expect_symbol(SymbolName::ParenOpen)?;
                                self.expect_register(RegisterName::C)?;
                                self.expect_symbol(SymbolName::ParenClose)?;
                            }

                            Some(Token::Register {
                                name: RegisterName::H,
                                ..
                            }) => {
                                self.data.push(0xED);
                                self.data.push(0x60);
                                self.expect_symbol(SymbolName::Comma)?;
                                self.expect_symbol(SymbolName::ParenOpen)?;
                                self.expect_register(RegisterName::C)?;
                                self.expect_symbol(SymbolName::ParenClose)?;
                            }

                            Some(Token::Register {
                                name: RegisterName::L,
                                ..
                            }) => {
                                self.data.push(0xED);
                                self.data.push(0x68);
                                self.expect_symbol(SymbolName::Comma)?;
                                self.expect_symbol(SymbolName::ParenOpen)?;
                                self.expect_register(RegisterName::C)?;
                                self.expect_symbol(SymbolName::ParenClose)?;
                            }

                            Some(tok) => {
                                return Err((
                                    tok.loc(),
                                    ParserError(format!(
                                        "Unexpected {}, expected a register",
                                        tok.as_display(&self.str_interner)
                                    )),
                                ))
                            }
                        }
                    }

                    OperationName::Inc => {
                        self.next()?;
                        match self.next()? {
                            Some(Token::Register { name: RegisterName::A, .. }) => {
                                self.here += 1;
                                self.data.push(0x3C);
                            }

                            Some(Token::Register { name: RegisterName::B, .. }) => {
                                self.here += 1;
                                self.data.push(0x04);
                            }

                            Some(Token::Register { name: RegisterName::C, .. }) => {
                                self.here += 1;
                                self.data.push(0x0C);
                            }

                            Some(Token::Register { name: RegisterName::D, .. }) => {
                                self.here += 1;
                                self.data.push(0x14);
                            }

                            Some(Token::Register { name: RegisterName::E, .. }) => {
                                self.here += 1;
                                self.data.push(0x1C);
                            }
                            
                            Some(Token::Register { name: RegisterName::H, .. }) => {
                                self.here += 1;
                                self.data.push(0x24);
                            }

                            Some(Token::Register { name: RegisterName::L, .. }) => {
                                self.here += 1;
                                self.data.push(0x2C);
                            }

                            Some(Token::Register { name: RegisterName::IXH, .. }) => {
                                self.here += 2;
                                self.data.push(0xDD);
                                self.data.push(0x24);
                            }

                            Some(Token::Register { name: RegisterName::IXL, .. }) => {
                                self.here += 2;
                                self.data.push(0xDD);
                                self.data.push(0x2C);
                            }

                            Some(Token::Register { name: RegisterName::IYH, .. }) => {
                                self.here += 2;
                                self.data.push(0xFD);
                                self.data.push(0x24);
                            }

                            Some(Token::Register { name: RegisterName::IYL, .. }) => {
                                self.here += 2;
                                self.data.push(0xFD);
                                self.data.push(0x2C);
                            }
                            
                            Some(Token::Register { name: RegisterName::BC, .. }) => {
                                self.here += 1;
                                self.data.push(0x03);
                            }

                            Some(Token::Register { name: RegisterName::DE, .. }) => {
                                self.here += 1;
                                self.data.push(0x13);
                            }

                            Some(Token::Register { name: RegisterName::HL, .. }) => {
                                self.here += 1;
                                self.data.push(0x23);
                            }

                            Some(Token::Register { name: RegisterName::SP, .. }) => {
                                self.here += 1;
                                self.data.push(0x33);
                            }

                            Some(Token::Register { name: RegisterName::IX, .. }) => {
                                self.here += 2;
                                self.data.push(0xDD);
                                self.data.push(0x23);
                            }

                            Some(Token::Register { name: RegisterName::IY, .. }) => {
                                self.here += 2;
                                self.data.push(0xFD);
                                self.data.push(0x23);
                            }

                            Some(Token::Symbol { name: SymbolName::ParenOpen, .. }) => {
                                match self.next()? {
                                    None => return self.end_of_input_err(),

                                    Some(Token::Register { name: RegisterName::HL, .. }) => {
                                        self.here += 1;
                                        self.data.push(0x34);
                                        self.expect_symbol(SymbolName::ParenClose)?;
                                    }

                                    Some(Token::Register { name: RegisterName::IX, .. }) => {
                                        self.expect_symbol(SymbolName::Plus)?;
                                        self.here += 3;
                                        self.data.push(0xDD);
                                        self.data.push(0x34);
                                        let (loc, expr) = self.expr()?;
                                        if let Some(value) = expr.evaluate(&self.symtab) {
                                            if (value as u32) > (u8::MAX as u32) {
                                                return Err((loc, ParserError(format!("Expression result ({value}) will not fit in a byte"))));
                                            }
                                            self.data.push(value as u8);
                                        } else {
                                            self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                            self.data.push(0);
                                        }
                                        self.expect_symbol(SymbolName::ParenClose)?;
                                    }
                                    
                                    Some(Token::Register { name: RegisterName::IY, .. }) => {
                                        self.expect_symbol(SymbolName::Plus)?;
                                        self.here += 3;
                                        self.data.push(0xFD);
                                        self.data.push(0x34);
                                        let (loc, expr) = self.expr()?;
                                        if let Some(value) = expr.evaluate(&self.symtab) {
                                            if (value as u32) > (u8::MAX as u32) {
                                                return Err((loc, ParserError(format!("Expression result ({value}) will not fit in a byte"))));
                                            }
                                            self.data.push(value as u8);
                                        } else {
                                            self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                            self.data.push(0);
                                        }
                                        self.expect_symbol(SymbolName::ParenClose)?;
                                    }

                                    Some(tok) => return Err((tok.loc(), ParserError(format!("Unexpected {}, expected registers \"hl\", \"ix\", or \"iy\"", tok.as_display(&self.str_interner))))),
                                }
                            }

                            Some(tok) => return Err((tok.loc(), ParserError(format!("Unexpected {}, expected a register", tok.as_display(&self.str_interner))))),
                            None => return self.end_of_input_err(),
                        }
                    }

                    OperationName::Ind => {
                        self.next()?;
                        self.here += 2;
                        self.data.push(0xED);
                        self.data.push(0xAA);
                    }

                    OperationName::Indr => {
                        self.next()?;
                        self.here += 2;
                        self.data.push(0xED);
                        self.data.push(0xBA);
                    }

                    OperationName::Ini => {
                        self.next()?;
                        self.here += 2;
                        self.data.push(0xED);
                        self.data.push(0xA2);
                    }

                    OperationName::Inir => {
                        self.next()?;
                        self.here += 2;
                        self.data.push(0xED);
                        self.data.push(0xB2);
                    }

                    OperationName::Jp => {
                        self.next()?;
                        match self.peek()? {
                            None => return self.end_of_input_err(),

                            Some(Token::Symbol {
                                name: SymbolName::ParenOpen,
                                ..
                            }) => {
                                self.next()?;
                                match self.next()? {
                                    None => return self.end_of_input_err(),

                                    Some(Token::Register { name: RegisterName::HL, .. }) => {
                                        self.here += 1;
                                        self.data.push(0xE9);
                                    }
                                    
                                    Some(Token::Register { name: RegisterName::IX, .. }) => {
                                        self.here += 2;
                                        self.data.push(0xDD);
                                        self.data.push(0xE9);
                                    }
                                    
                                    Some(Token::Register { name: RegisterName::IY, .. }) => {
                                        self.here += 2;
                                        self.data.push(0xFD);
                                        self.data.push(0xE9);
                                    }

                                    Some(tok) => return Err((tok.loc(), ParserError(format!("Unexpected {}, expected register \"hl\", \"ix\", or \"iy\"", tok.as_display(&self.str_interner))))),
                                }
                                self.expect_symbol(SymbolName::ParenClose)?;
                            }

                            Some(_) => {
                                self.here += 3;
                                match self.peek()? {
                                    None => return self.end_of_input_err(),
                                    Some(Token::Flag {
                                        name: FlagName::Zero,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.data.push(0xCA);
                                        self.expect_symbol(SymbolName::Comma)?;
                                    }
                                    Some(Token::Flag {
                                        name: FlagName::NotZero,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.data.push(0xC2);
                                        self.expect_symbol(SymbolName::Comma)?;
                                    }
                                    Some(Token::Register {
                                        name: RegisterName::C,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.data.push(0xDA);
                                        self.expect_symbol(SymbolName::Comma)?;
                                    }
                                    Some(Token::Flag {
                                        name: FlagName::NotCarry,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.data.push(0xD2);
                                        self.expect_symbol(SymbolName::Comma)?;
                                    }
                                    Some(Token::Flag {
                                        name: FlagName::ParityEven,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.data.push(0xEA);
                                        self.expect_symbol(SymbolName::Comma)?;
                                    }
                                    Some(Token::Flag {
                                        name: FlagName::ParityOdd,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.data.push(0xE2);
                                        self.expect_symbol(SymbolName::Comma)?;
                                    }
                                    Some(Token::Flag {
                                        name: FlagName::Positive,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.data.push(0xF2);
                                        self.expect_symbol(SymbolName::Comma)?;
                                    }
                                    Some(Token::Flag {
                                        name: FlagName::Negative,
                                        ..
                                    }) => {
                                        self.next()?;
                                        self.data.push(0xFA);
                                        self.expect_symbol(SymbolName::Comma)?;
                                    }
                                    Some(_) => {
                                        self.data.push(0xC3);
                                    }
                                }
                                let (loc, expr) = self.expr()?;
                                if let Some(value) = expr.evaluate(&self.symtab) {
                                    if (value as u32) > (u16::MAX as u32) {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "Expression result ({value}) will not fit in a word"
                                            )),
                                        ));
                                    }
                                    self.data.extend_from_slice(&(value as u16).to_le_bytes());
                                } else {
                                    self.data.push(0);
                                    self.data.push(0);
                                }
                            }
                        }
                    }

                    OperationName::Jr => {
                        self.next()?;
                        self.here += 2;
                        match self.peek()? {
                            None => return self.end_of_input_err(),

                            Some(Token::Flag {
                                name: FlagName::NotZero,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0x20);
                                self.expect_symbol(SymbolName::Comma)?;
                            }

                            Some(Token::Flag {
                                name: FlagName::Zero,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0x28);
                                self.expect_symbol(SymbolName::Comma)?;
                            }

                            Some(Token::Flag {
                                name: FlagName::NotCarry,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0x30);
                                self.expect_symbol(SymbolName::Comma)?;
                            }

                            Some(Token::Register {
                                name: RegisterName::C,
                                ..
                            }) => {
                                self.next()?;
                                self.data.push(0x38);
                                self.expect_symbol(SymbolName::Comma)?;
                            }

                            Some(_) => {
                                self.data.push(0x18);
                            }
                        }
                        let (loc, mut expr) = self.expr()?;
                        // Make the expression relative to @here
                        expr.push(ExprNode::Value(self.here as i32));
                        expr.push(ExprNode::Sub);
                        if let Some(value) = expr.evaluate(&self.symtab) {
                            if (value < (i8::MIN as i32)) || (value > (i8::MAX as i32)) {
                                return Err((
                                    loc,
                                    ParserError(format!(
                                        "Jump distance ({value}) will not fit in a byte"
                                    )),
                                ));
                            }
                            self.data.push(value as u8);
                        } else {
                            self.holes
                                .push(Hole::signed_byte(loc, self.data.len(), expr));
                            self.data.push(0);
                        }
                    }

                    OperationName::Nop => {
                        self.next()?;
                        self.here += 1;
                        self.data.push(0x00);
                    }

                    OperationName::Res => {
                        self.next()?;
                        match self.const_expr()? {
                            (loc, None) => {
                                return Err((
                                    loc,
                                    ParserError(format!("Bit index must be immediately solvable")),
                                ))
                            }
                            (loc, Some(value)) => {
                                if value < 0 || value > 7 {
                                    return Err((
                                        loc,
                                        ParserError(format!(
                                            "Bit index ({value}) must be between 0 and 7"
                                        )),
                                    ));
                                }

                                self.expect_symbol(SymbolName::Comma)?;

                                match self.next()? {
                                    Some(Token::Register {
                                        name: RegisterName::A,
                                        ..
                                    }) => {
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x87,
                                            1 => 0x8F,
                                            2 => 0x97,
                                            3 => 0x9F,
                                            4 => 0xA7,
                                            5 => 0xAF,
                                            6 => 0xB7,
                                            7 => 0xBF,
                                            _ => unreachable!(),
                                        });
                                    }

                                    Some(Token::Register {
                                        name: RegisterName::B,
                                        ..
                                    }) => {
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x80,
                                            1 => 0x88,
                                            2 => 0x90,
                                            3 => 0x98,
                                            4 => 0xA0,
                                            5 => 0xA8,
                                            6 => 0xB0,
                                            7 => 0xB8,
                                            _ => unreachable!(),
                                        });  
                                    }  
  
                                    Some(Token::Register {
                                        name: RegisterName::C,
                                        ..  
                                    }) => {  
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x81,
                                            1 => 0x89,
                                            2 => 0x91,
                                            3 => 0x99,
                                            4 => 0xA1,
                                            5 => 0xA9,
                                            6 => 0xB1,
                                            7 => 0xB9,
                                            _ => unreachable!(),
                                        });  
                                    }  
  
                                    Some(Token::Register {
                                        name: RegisterName::D,
                                        ..  
                                    }) => {  
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x82,
                                            1 => 0x8A,
                                            2 => 0x92,
                                            3 => 0x9A,
                                            4 => 0xA2,
                                            5 => 0xAA,
                                            6 => 0xB2,
                                            7 => 0xBA,
                                            _ => unreachable!(),
                                        });  
                                    }  
  
                                    Some(Token::Register {
                                        name: RegisterName::E,
                                        ..  
                                    }) => {  
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x83,
                                            1 => 0x8B,
                                            2 => 0x93,
                                            3 => 0x9B,
                                            4 => 0xA3,
                                            5 => 0xAB,
                                            6 => 0xB3,
                                            7 => 0xBB,
                                            _ => unreachable!(),
                                        });  
                                    }  
  
                                    Some(Token::Register {
                                        name: RegisterName::H,
                                        ..  
                                    }) => {  
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x84,
                                            1 => 0x8C,
                                            2 => 0x94,
                                            3 => 0x9C,
                                            4 => 0xA4,
                                            5 => 0xAC,
                                            6 => 0xB4,
                                            7 => 0xBC,
                                            _ => unreachable!(),
                                        });  
                                    }  
  
                                    Some(Token::Register {
                                        name: RegisterName::L,
                                        ..  
                                    }) => {  
                                        self.here += 2;
                                        self.data.push(0xCB);
                                        self.data.push(match value {
                                            0 => 0x85,
                                            1 => 0x8D,
                                            2 => 0x95,
                                            3 => 0x9D,
                                            4 => 0xA5,
                                            5 => 0xAD,
                                            6 => 0xB5,
                                            7 => 0xBD,
                                            _ => unreachable!(),
                                        });
                                    }

                                    Some(Token::Symbol {
                                        name: SymbolName::ParenOpen,
                                        ..
                                    }) => {
                                        match self.next()? {
                                            Some(Token::Register { name: RegisterName::HL, .. }) => {
                                                self.here += 2;
                                                self.data.push(0xCB);
                                                self.data.push(match value {
                                                    0 => 0x86,
                                                    1 => 0x8E,
                                                    2 => 0x96,
                                                    3 => 0x9E,
                                                    4 => 0xA6,
                                                    5 => 0xAE,
                                                    6 => 0xB6,
                                                    7 => 0xBE,
                                                    _ => unreachable!(),
                                                });  
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }  
  
                                            Some(Token::Register { name: RegisterName::IX, .. }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 4;
                                                self.data.push(0xDD);
                                                self.data.push(0xCB);
                                                self.data.push(match value {
                                                    0 => 0x86,
                                                    1 => 0x8E, 
                                                    2 => 0x96, 
                                                    3 => 0x9E, 
                                                    4 => 0xA6,
                                                    5 => 0xAE,
                                                    6 => 0xB6,
                                                    7 => 0xBE,
                                                    _ => unreachable!(),
                                                });  
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((loc, ParserError(format!("Expression result ({value}) will not fit in byte"))));
                                                    }  
                                                    self.data.push(value as u8);
                                                } else {  
                                                    self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                                    self.data.push(0);
                                                }  
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }  
  
                                            Some(Token::Register { name: RegisterName::IY, .. }) => {
                                                self.expect_symbol(SymbolName::Plus)?;
                                                self.here += 4;
                                                self.data.push(0xFD);
                                                self.data.push(0xCB);
                                                self.data.push(match value {
                                                    0 => 0x86,
                                                    1 => 0x8E,
                                                    2 => 0x96,
                                                    3 => 0x9E,
                                                    4 => 0xA6,
                                                    5 => 0xAE,
                                                    6 => 0xB6,
                                                    7 => 0xBE,
                                                    _ => unreachable!(),
                                                });  
                                                let (loc, expr) = self.expr()?;
                                                if let Some(value) = expr.evaluate(&self.symtab) {
                                                    if (value as u32) > (u8::MAX as u32) {
                                                        return Err((loc, ParserError(format!("Expression result ({value}) will not fit in byte"))));
                                                    }  
                                                    self.data.push(value as u8);
                                                } else {  
                                                    self.holes.push(Hole::byte(loc, self.data.len(), expr));
                                                    self.data.push(0);
                                                }  
                                                self.expect_symbol(SymbolName::ParenClose)?;
                                            }  
  
                                            Some(tok) => return Err((loc, ParserError(format!("Unexpected {}, expected register \"hl\", \"ix\", or \"iy\"", tok.as_display(&self.str_interner))))),
                                            None => return self.end_of_input_err(),
                                        }
                                    }

                                    Some(tok) => {
                                        return Err((
                                            loc,
                                            ParserError(format!(
                                                "Unexpected {}, expected a register",
                                                tok.as_display(&self.str_interner)
                                            )),
                                        ))
                                    }
                                    None => return self.end_of_input_err(),
                                }
                            }
                        }
                    }

                    _ => todo!(),
                },

                Some(&tok) => todo!("{}", tok.as_display(&self.str_interner)),

                None => return Ok(()),
            }
        }
    }
}
