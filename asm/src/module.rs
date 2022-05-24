use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
};

use crate::{
    fileman::{FileManager, FileSystem},
    intern::StrInterner,
    lexer::SourceLoc,
    symtab::Symtab,
};

pub struct Module<S> {
    str_interner: Rc<RefCell<StrInterner>>,
    file_manager: FileManager<S>,
    symtab: Symtab,
    items: Vec<Item>,
}

#[derive(Debug)]
pub enum Item {
    Bytes {
        loc: SourceLoc,
        data: Vec<u8>,
    },
    Words {
        loc: SourceLoc,
        data: Vec<u16>,
    },
    Space {
        loc: SourceLoc,
        size: u16,
        value: u8,
    },
}

impl<S: FileSystem> Module<S> {
    #[inline]
    pub fn new(
        str_interner: Rc<RefCell<StrInterner>>,
        file_manager: FileManager<S>,
        symtab: Symtab,
        items: Vec<Item>,
    ) -> Self {
        Self {
            str_interner,
            file_manager,
            symtab,
            items,
        }
    }

    pub fn assemble(&self, writer: &mut dyn Write) -> io::Result<()> {
        Ok(())
    }
}
