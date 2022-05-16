use std::{collections::HashMap, io::Read};

use crate::{
    charreader::CharReaderError,
    expr::Expr,
    fileinfo::{FileInfo, SourceLoc},
    lexer::{Lexer, Token},
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
pub enum PassOneError {
    #[error("{0}")]
    ReadError(#[from] CharReaderError),
}

pub struct Parser<R: Read> {
    file_info: FileInfo,
    macros: HashMap<String, Macro>,
    lexers: Vec<Lexer<R>>,
    stash: Option<Token>,
}

impl<R: Read> Parser<R> {
    // fn peek(&mut self) -> Option<Token> {
    //     if self.stash.is_none() {
    //         self.stash = self.next();
    //     }
    //     self.stash.as_ref()
    // }
    //
    // fn next(&mut self) -> Option<Result<Token, LexerError>> {
    //     if self.stash.is_some() {
    //         return self.stash.take();
    //     }
    //     if let Some(next) = self.lexers.last_mut()?.next() {
    //         return Some(next);
    //     }
    //     self.lexers.pop();
    //     self.next()
    // }

    fn parse(&mut self) -> Result<Vec<Node>, PassOneError> {
        let nodes = Vec::new();
        Ok(nodes)
    }
}
