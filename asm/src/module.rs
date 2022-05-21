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
    items: Vec<(SourceLoc, Item)>,
}

pub enum Item {
    Bytes { data: Vec<u8> },
}

impl<S: FileSystem> Module<S> {
    #[inline]
    pub fn new(
        str_interner: Rc<RefCell<StrInterner>>,
        file_manager: FileManager<S>,
        symtab: Symtab,
        items: Vec<(SourceLoc, Item)>,
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
