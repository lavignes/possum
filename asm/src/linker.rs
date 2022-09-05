use std::{cell::RefCell, io::Write, rc::Rc};

use crate::{
    expr::Expr,
    fileman::{FileManager, FileSystem},
    intern::{StrInterner, StrRef},
    lexer::SourceLoc,
    symtab::{Symbol, Symtab},
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
    Assert {
        loc: SourceLoc,
        msg: Option<StrRef>,
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

    #[inline]
    pub fn assert(loc: SourceLoc, msg: Option<StrRef>, expr: Expr) -> Self {
        Self::Assert { loc, msg, expr }
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
        for (strref, loc) in self.symtab.references() {
            let interner = self.str_interner.as_ref().borrow();
            let label = interner.get(*strref).unwrap();
            let path = self.file_manager.path(loc.pathref).unwrap();
            match self.symtab.get(*strref) {
                None => {
                    return Err(LinkerError(format!(
                        "In \"{}\"\n\n{}:{}:{}: Undefined symbol: \"{label}\"",
                        path.display(),
                        path.file_name().unwrap().to_str().unwrap(),
                        loc.line,
                        loc.column
                    )));
                }
                Some(Symbol::Expr(expr)) => {
                    if expr.evaluate(&self.symtab).is_none() {
                        return Err(LinkerError(format!(
                            "In \"{}\"\n\n{}:{}:{}: Undefined symbol: \"{label}\"",
                            path.display(),
                            path.file_name().unwrap().to_str().unwrap(),
                            loc.line,
                            loc.column
                        )));
                    }
                }
                _ => {}
            }
        }

        for link in &self.links {
            match link {
                Link::Byte {
                    loc, offset, expr, ..
                } => {
                    if let Some(value) = expr.evaluate(&self.symtab) {
                        if (value as u32) > (u8::MAX as u32) {
                            let path = self.file_manager.path(loc.pathref).unwrap();
                            return Err(LinkerError(format!(
                                "In \"{}\"\n\n{}:{}:{}: Expression result ({value}) does not fit in a byte",
                                path.display(),
                                path.file_name().unwrap().to_str().unwrap(),
                                loc.line,
                                loc.column
                            )));
                        }
                        self.data[*offset] = value as u8;
                    } else {
                        let path = self.file_manager.path(loc.pathref).unwrap();
                        return Err(LinkerError(format!(
                            "In \"{}\"\n\n{}:{}:{}: Expression could not be solved",
                            path.display(),
                            path.file_name().unwrap().to_str().unwrap(),
                            loc.line,
                            loc.column
                        )));
                    }
                }
                Link::SignedByte {
                    loc, offset, expr, ..
                } => {
                    if let Some(value) = expr.evaluate(&self.symtab) {
                        if (value < (i8::MIN as i32)) || (value > (i8::MAX as i32)) {
                            let path = self.file_manager.path(loc.pathref).unwrap();
                            return Err(LinkerError(format!(
                                "In \"{}\"\n\n{}:{}:{}: Expression result ({value}) does not fit in a byte",
                                path.display(),
                                path.file_name().unwrap().to_str().unwrap(),
                                loc.line,
                                loc.column
                            )));
                        }
                        self.data[*offset] = value as u8;
                    } else {
                        let path = self.file_manager.path(loc.pathref).unwrap();
                        return Err(LinkerError(format!(
                            "In \"{}\"\n\n{}:{}:{}: Expression could not be solved",
                            path.display(),
                            path.file_name().unwrap().to_str().unwrap(),
                            loc.line,
                            loc.column
                        )));
                    }
                }
                Link::Word {
                    loc, offset, expr, ..
                } => {
                    if let Some(value) = expr.evaluate(&self.symtab) {
                        if (value as u32) > (u16::MAX as u32) {
                            let path = self.file_manager.path(loc.pathref).unwrap();
                            return Err(LinkerError(format!(
                                "In \"{}\"\n\n{}:{}:{}: Expression result ({value}) does not fit in a word",
                                path.display(),
                                path.file_name().unwrap().to_str().unwrap(),
                                loc.line,
                                loc.column
                            )));
                        }
                        let bytes = (value as u16).to_le_bytes();
                        self.data[*offset] = bytes[0];
                        self.data[*offset + 1] = bytes[1];
                    } else {
                        let path = self.file_manager.path(loc.pathref).unwrap();
                        return Err(LinkerError(format!(
                            "In \"{}\"\n\n{}:{}:{}: Expression could not be solved",
                            path.display(),
                            path.file_name().unwrap().to_str().unwrap(),
                            loc.line,
                            loc.column
                        )));
                    }
                }
                Link::Space {
                    loc,
                    offset,
                    len,
                    expr,
                    ..
                } => {
                    if let Some(value) = expr.evaluate(&self.symtab) {
                        if (value as u32) > (u8::MAX as u32) {
                            let path = self.file_manager.path(loc.pathref).unwrap();
                            return Err(LinkerError(format!(
                                "In \"{}\"\n\n{}:{}:{}: Expression result ({value}) does not fit in a byte",
                                path.display(),
                                path.file_name().unwrap().to_str().unwrap(),
                                loc.line,
                                loc.column
                            )));
                        }
                        for i in *offset..*offset + *len {
                            self.data[i] = value as u8;
                        }
                    } else {
                        let path = self.file_manager.path(loc.pathref).unwrap();
                        return Err(LinkerError(format!(
                            "In \"{}\"\n\n{}:{}:{}: Expression could not be solved",
                            path.display(),
                            path.file_name().unwrap().to_str().unwrap(),
                            loc.line,
                            loc.column
                        )));
                    }
                }
                Link::Assert { loc, msg, expr, .. } => {
                    if let Some(value) = expr.evaluate(&self.symtab) {
                        if value == 0 {
                            let path = self.file_manager.path(loc.pathref).unwrap();
                            if let Some(msg) = msg {
                                let interner = self.str_interner.as_ref().borrow();
                                let msg = interner.get(*msg).unwrap();
                                return Err(LinkerError(format!(
                                    "In \"{}\"\n\n{}:{}:{}: Assertion failed: {msg}",
                                    path.display(),
                                    path.file_name().unwrap().to_str().unwrap(),
                                    loc.line,
                                    loc.column
                                )));
                            }
                            return Err(LinkerError(format!(
                                "In \"{}\"\n\n{}:{}:{}: Assertion failed",
                                path.display(),
                                path.file_name().unwrap().to_str().unwrap(),
                                loc.line,
                                loc.column
                            )));
                        }
                    } else {
                        let path = self.file_manager.path(loc.pathref).unwrap();
                        return Err(LinkerError(format!(
                            "In \"{}\"\n\n{}:{}:{}: Expression could not be solved",
                            path.display(),
                            path.file_name().unwrap().to_str().unwrap(),
                            loc.line,
                            loc.column
                        )));
                    }
                }
            }
        }

        writer
            .write_all(&self.data)
            .map_err(|e| LinkerError(format!("Failed to write output: {e}")))
    }
}
