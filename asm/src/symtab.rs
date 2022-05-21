use fxhash::FxHashMap;

use crate::{expr::Expr, intern::StrRef};

#[derive(Clone, Debug)]
pub enum Symbol {
    Expr(Expr),
    Value(u16),
}

pub struct Symtab {
    inner: FxHashMap<StrRef, Symbol>,
}

impl Symtab {
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: FxHashMap::default(),
        }
    }

    #[inline]
    pub fn insert(&mut self, key: StrRef, value: Symbol) -> Option<Symbol> {
        self.inner.insert(key, value)
    }

    #[inline]
    pub fn get(&self, key: StrRef) -> Option<&Symbol> {
        self.inner.get(&key)
    }
}
