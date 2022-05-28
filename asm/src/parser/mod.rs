use std::{borrow::Borrow, cell::RefCell, fmt, io::Read, iter, path::Path, rc::Rc};

use fxhash::FxHashMap;

use crate::{
    expr::{Expr, ExprNode},
    fileman::{FileManager, FileSystem},
    intern::StrRef,
    lexer::{
        DirectiveName, LabelKind, Lexer, LexerError, OperationName, RegisterName, SourceLoc,
        SymbolName, Token,
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
            Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::DoublePipe,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::DoubleAmpersand,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::Pipe,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::Caret,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::Ampersand,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::Ampersand,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::Equal,
                ..
            }) => {
                while let Some(&Token::Symbol {
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

            Some(&Token::Symbol {
                name: SymbolName::NotEqual,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::LessThan,
                ..
            }) => {
                while let Some(&Token::Symbol {
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

            Some(&Token::Symbol {
                name: SymbolName::LessEqual,
                ..
            }) => {
                while let Some(&Token::Symbol {
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

            Some(&Token::Symbol {
                name: SymbolName::GreaterThan,
                ..
            }) => {
                while let Some(&Token::Symbol {
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

            Some(&Token::Symbol {
                name: SymbolName::GreaterEqual,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::ShiftLeft,
                ..
            }) => {
                while let Some(&Token::Symbol {
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

            Some(&Token::Symbol {
                name: SymbolName::ShiftRight,
                ..
            }) => {
                while let Some(&Token::Symbol {
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

            Some(&Token::Symbol {
                name: SymbolName::ShiftLeftLogical,
                ..
            }) => {
                while let Some(&Token::Symbol {
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

            Some(&Token::Symbol {
                name: SymbolName::ShiftRightLogical,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::Plus,
                ..
            }) => {
                while let Some(&Token::Symbol {
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

            Some(&Token::Symbol {
                name: SymbolName::Minus,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
            Some(&Token::Symbol {
                name: SymbolName::Star,
                ..
            }) => {
                while let Some(&Token::Symbol {
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

            Some(&Token::Symbol {
                name: SymbolName::Div,
                ..
            }) => {
                while let Some(&Token::Symbol {
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

            Some(&Token::Symbol {
                name: SymbolName::Mod,
                ..
            }) => {
                while let Some(&Token::Symbol {
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
                        continue;
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
                        continue;
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
                        continue;
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

                        continue;
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

                        continue;
                    }

                    DirectiveName::Ds => {
                        self.next()?;

                        let size = match self.const_expr()? {
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
                            (loc, None) => {
                                return Err((
                                    loc,
                                    ParserError(format!(
                                    "The size of a \"@ds\" directive must be immediately solvable"
                                )),
                                ))
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
                                loc,
                                name: RegisterName::A,
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
                                loc,
                                name: RegisterName::A,
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
                                loc,
                                name: RegisterName::A,
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

                    OperationName::Bit => {}

                    OperationName::Nop => {
                        self.next()?;
                        self.here += 1;
                        self.data.push(0);
                    }

                    _ => todo!(),
                },

                Some(_) => todo!(),

                None => return Ok(()),
            }
        }
    }
}
