use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
};

use crate::{
    expr::Expr,
    fileman::{FileManager, FileSystem},
    intern::StrInterner,
    lexer::SourceLoc,
    symtab::Symtab,
};

pub enum Hole {
    Byte {
        loc: SourceLoc,
        offset: usize,
        expr: Expr,
    },
    Word {
        loc: SourceLoc,
        offset: usize,
        expr: Expr,
    },
    Space {
        loc: SourceLoc,
        offset: usize,
        len: usize,
        expr: Expr,
    },
}

impl Hole {
    #[inline]
    pub fn byte(loc: SourceLoc, offset: usize, expr: Expr) -> Self {
        Self::Byte { loc, offset, expr }
    }

    #[inline]
    pub fn word(loc: SourceLoc, offset: usize, expr: Expr) -> Self {
        Self::Word { loc, offset, expr }
    }

    #[inline]
    pub fn space(loc: SourceLoc, offset: usize, len: usize, expr: Expr) -> Self {
        Self::Space {
            loc,
            offset,
            len,
            expr,
        }
    }
}

pub struct Module<S> {
    str_interner: Rc<RefCell<StrInterner>>,
    file_manager: FileManager<S>,
    symtab: Symtab,
    data: Vec<u8>,
    holes: Vec<Hole>,
}

impl<S: FileSystem> Module<S> {
    #[inline]
    pub fn new(
        str_interner: Rc<RefCell<StrInterner>>,
        file_manager: FileManager<S>,
        symtab: Symtab,
        data: Vec<u8>,
        holes: Vec<Hole>,
    ) -> Self {
        Self {
            str_interner,
            file_manager,
            symtab,
            data,
            holes,
        }
    }

    pub fn assemble(&self, writer: &mut dyn Write) -> io::Result<()> {
        // TODO: fill holes
        writer.write(&self.data)?;
        Ok(())
    }
}
