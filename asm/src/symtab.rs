use std::collections::hash_map::Iter;

use fxhash::FxHashMap;

use crate::{expr::Expr, intern::StrRef, lexer::SourceLoc};

#[derive(Clone, Debug)]
pub enum Symbol {
    Expr(Expr),
    Value(i32),
}

pub struct Symtab {
    inner: FxHashMap<StrRef, Symbol>,
    hits: FxHashMap<StrRef, SourceLoc>,
}

impl Symtab {
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: FxHashMap::default(),
            hits: FxHashMap::default(),
        }
    }

    #[inline]
    pub fn insert(&mut self, key: StrRef, value: Symbol) -> Option<Symbol> {
        self.inner.insert(key, value)
    }

    #[inline]
    pub fn touch(&mut self, key: StrRef, loc: SourceLoc) {
        if !self.hits.contains_key(&key) {
            self.hits.insert(key, loc);
        }
    }

    #[inline]
    pub fn first_reference(&self, key: StrRef) -> Option<&SourceLoc> {
        self.hits.get(&key)
    }

    #[inline]
    pub fn get(&self, key: StrRef) -> Option<&Symbol> {
        self.inner.get(&key)
    }

    #[inline]
    pub fn references(&self) -> SymtabRefIter<'_> {
        SymtabRefIter {
            inner: self.hits.iter(),
        }
    }
}

impl<'a> IntoIterator for &'a Symtab {
    type IntoIter = SymtabIter<'a>;
    type Item = (&'a StrRef, &'a Symbol);

    fn into_iter(self) -> Self::IntoIter {
        SymtabIter {
            inner: self.inner.iter(),
        }
    }
}

pub struct SymtabIter<'a> {
    inner: Iter<'a, StrRef, Symbol>,
}

impl<'a> Iterator for SymtabIter<'a> {
    type Item = (&'a StrRef, &'a Symbol);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct SymtabRefIter<'a> {
    inner: Iter<'a, StrRef, SourceLoc>,
}

impl<'a> Iterator for SymtabRefIter<'a> {
    type Item = (&'a StrRef, &'a SourceLoc);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
