use std::{
    cell::RefCell,
    io::{self, Read, Write},
    path::Path,
    rc::Rc,
};

use fxhash::FxHashMap;

use crate::{
    fileman::{FileManager, FileSystem},
    intern::StrRef,
    lexer::{Lexer, SourceLoc, Token},
    StrInterner,
};

#[derive(Clone, Debug)]
enum MacroToken {
    Token(Token),
    Argument(usize),
}

impl From<Token> for MacroToken {
    fn from(tok: Token) -> Self {
        Self::Token(tok)
    }
}

struct Macro {
    loc: SourceLoc,
    args: Vec<(SourceLoc, StrRef)>,
    tokens: Vec<(SourceLoc, MacroToken)>,
    arg_indices: FxHashMap<StrRef, usize>,
}

#[derive(Copy, Clone, Debug)]
enum State {
    Initial,
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct AssemblerError(String);

pub struct Assembler<S, R> {
    file_manager: FileManager<S>,
    str_interner: Rc<RefCell<StrInterner>>,
    lexers: Vec<Lexer<R>>,
    macros: FxHashMap<StrRef, Macro>,
    pass_index: usize,

    stash: Option<(SourceLoc, MacroToken)>,
    pc: isize,
    state: Vec<State>,
    active_macro: Option<StrRef>,
}

impl<S: FileSystem<Reader = R>, R: Read> Assembler<S, R> {
    pub fn new(file_system: S) -> Self {
        Self {
            file_manager: FileManager::new(file_system),
            str_interner: Rc::new(RefCell::new(StrInterner::new())),
            lexers: Vec::new(),
            macros: FxHashMap::default(),
            pass_index: 0,

            stash: None,
            pc: 0,
            state: vec![State::Initial],
            active_macro: None,
        }
    }

    pub fn add_search_path<C: AsRef<Path>, P: AsRef<Path>>(
        &mut self,
        cwd: C,
        path: P,
    ) -> io::Result<()> {
        self.file_manager.add_search_path(cwd, path)?;
        Ok(())
    }

    pub fn assemble<C: AsRef<Path>, P: AsRef<Path>>(
        mut self,
        cwd: C,
        path: P,
        bin_writer: &mut dyn Write,
    ) -> Result<(), AssemblerError> {
        let (pathref, reader) = match self.file_manager.reader(cwd, path.as_ref()) {
            Ok(Some(tup)) => tup,
            Ok(None) => {
                return Err(AssemblerError(format!(
                    "File not found: \"{}\"",
                    path.as_ref().display()
                )))
            }
            Err(e) => {
                return Err(AssemblerError(format!(
                    "Failed to open \"{}\" for reading: {e}",
                    path.as_ref().display()
                )))
            }
        };

        let lexer = Lexer::new(self.str_interner.clone(), pathref, reader);
        self.lexers.push(lexer);

        self.pass(bin_writer)?;
        self.pass(bin_writer)
    }

    fn pass(&mut self, bin_writer: &mut dyn Write) -> Result<(), AssemblerError> {
        let result = self.pass_real(bin_writer);
        self.pass_index += 1;
        result
    }

    fn pass_real(&mut self, bin_writer: &mut dyn Write) -> Result<(), AssemblerError> {
        self.pc = 0;

        write!(bin_writer, "test").map_err(|e| AssemblerError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Cursor},
        path::PathBuf,
    };

    use fxhash::FxHashMap;

    use super::*;

    struct StringFileSystem {
        files: FxHashMap<PathBuf, String>,
    }

    impl StringFileSystem {
        #[inline]
        fn new<P: AsRef<Path>>(files: &[(P, &str)]) -> Self {
            let mut map = FxHashMap::default();
            for (path, s) in files {
                map.insert(path.as_ref().to_path_buf(), s.to_string());
            }
            Self { files: map }
        }
    }

    impl FileSystem for StringFileSystem {
        type Reader = Cursor<String>;

        #[inline]
        fn is_dir(&self, _: &Path) -> io::Result<bool> {
            Ok(true)
        }

        #[inline]
        fn is_file(&self, path: &Path) -> io::Result<bool> {
            Ok(self.files.contains_key(path))
        }

        #[inline]
        fn open_read(&self, path: &Path) -> io::Result<Self::Reader> {
            Ok(Cursor::new(self.files.get(path).unwrap().clone()))
        }
    }

    fn assembler<P: AsRef<Path>>(
        files: &[(P, &str)],
    ) -> Assembler<StringFileSystem, Cursor<String>> {
        Assembler::new(StringFileSystem::new(files))
    }

    #[test]
    fn sanity() {
        let assembler = assembler(&[(
            "/test.asm",
            r#"
            "#,
        )]);

        let mut binary = Vec::new();
        assert!(assembler.assemble("/", "test.asm", &mut binary).is_ok());

        assert_eq!("testtest", String::from_utf8(binary).unwrap());
    }
}
