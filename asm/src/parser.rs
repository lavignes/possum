use std::{cell::RefCell, collections::HashMap, io::Read, rc::Rc};

use crate::{
    expr::Expr,
    intern::StrRef,
    lexer::{Lexer, SourceLoc, Token},
    PathInterner, StrInterner,
};

enum Node {
    Org(Expr),
}

struct SourceNode {
    loc: SourceLoc,
    node: Node,
}

struct Macro {}

#[derive(thiserror::Error, Debug)]
#[error("{msg}")]
pub struct ParseError {
    msg: String,
}

pub struct Parser<R: Read> {
    path_interner: Rc<RefCell<PathInterner>>,
    str_interner: Rc<RefCell<StrInterner>>,
    macros: HashMap<StrRef, Macro>,
    lexers: Vec<Lexer<R>>,
    stash: Option<Token>,
}

impl<R: Read> Parser<R> {
    fn parse(&mut self) -> Result<Vec<Node>, ParseError> {
        let nodes = Vec::new();
        Ok(nodes)
    }
}
