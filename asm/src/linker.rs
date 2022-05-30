use std::{cell::RefCell, io::Write, rc::Rc};

use crate::{
    expr::Expr,
    fileman::{FileManager, FileSystem},
    intern::StrInterner,
    lexer::SourceLoc,
    symtab::Symtab,
};

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct LinkerError(String);

pub enum Link {
    Byte {
        loc: SourceLoc,
        offset: usize,
        expr: Expr,
    },
    SignedByte {
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

impl Link {
    #[inline]
    pub fn byte(loc: SourceLoc, offset: usize, expr: Expr) -> Self {
        Self::Byte { loc, offset, expr }
    }

    #[inline]
    pub fn signed_byte(loc: SourceLoc, offset: usize, expr: Expr) -> Self {
        Self::SignedByte { loc, offset, expr }
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
    links: Vec<Link>,
}

impl<S: FileSystem> Module<S> {
    #[inline]
    pub fn new(
        str_interner: Rc<RefCell<StrInterner>>,
        file_manager: FileManager<S>,
        symtab: Symtab,
        data: Vec<u8>,
        links: Vec<Link>,
    ) -> Self {
        Self {
            str_interner,
            file_manager,
            symtab,
            data,
            links,
        }
    }

    pub fn link(mut self, writer: &mut dyn Write) -> Result<(), LinkerError> {
        // TODO: Need to scan for unresolved symbol and report them nicely
        for link in &self.links {
            match link {
                Link::Byte { offset, expr, .. } => {
                    if let Some(value) = expr.evaluate(&self.symtab) {
                        if (value as u32) > (u8::MAX as u32) {
                            return Err(LinkerError(format!(
                                "Expression ({value}) does not fit in a byte"
                            )));
                        }
                        self.data[*offset] = value as u8;
                    } else {
                        return Err(LinkerError(format!("Expression could not be solved")));
                    }
                }
                Link::SignedByte { offset, expr, .. } => {
                    if let Some(value) = expr.evaluate(&self.symtab) {
                        if (value < (i8::MIN as i32)) || (value > (i8::MAX as i32)) {
                            return Err(LinkerError(format!(
                                "Expression ({value}) does not fit in a byte"
                            )));
                        }
                        self.data[*offset] = value as u8;
                    } else {
                        return Err(LinkerError(format!("Expression could not be solved")));
                    }
                }
                Link::Word { offset, expr, .. } => {
                    if let Some(value) = expr.evaluate(&self.symtab) {
                        if (value as u32) > (u16::MAX as u32) {
                            return Err(LinkerError(format!(
                                "Expression ({value}) does not fit in a word"
                            )));
                        }
                        let bytes = (value as u16).to_le_bytes();
                        self.data[*offset] = bytes[0];
                        self.data[*offset + 1] = bytes[1];
                    } else {
                        return Err(LinkerError(format!("Expression could not be solved")));
                    }
                }
                Link::Space {
                    offset, len, expr, ..
                } => {
                    if let Some(value) = expr.evaluate(&self.symtab) {
                        if (value as u32) > (u8::MAX as u32) {
                            return Err(LinkerError(format!(
                                "Expression ({value}) does not fit in a byte"
                            )));
                        }
                        for i in *offset..*offset + *len {
                            self.data[i] = value as u8;
                        }
                    } else {
                        return Err(LinkerError(format!("Expression could not be solved")));
                    }
                }
            }
        }

        writer
            .write(&self.data)
            .map(|_| {})
            .map_err(|e| LinkerError(format!("Failed to write output: {e}")))
    }
}
