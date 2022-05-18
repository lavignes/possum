use std::{fs, fs::File, io, io::Read, iter, path::Path};

use crate::intern::{AbsPathInterner, PathRef};

pub trait FileSystem {
    type Reader: Read;

    fn is_dir(&self, path: &Path) -> io::Result<bool>;
    fn is_file(&self, path: &Path) -> io::Result<bool>;
    fn open_read(&self, path: &Path) -> io::Result<Self::Reader>;
}

pub struct RealFileSystem;

impl RealFileSystem {
    #[inline]
    pub fn new() -> Self {
        Self {}
    }
}

impl FileSystem for RealFileSystem {
    type Reader = File;

    #[inline]
    fn is_dir(&self, path: &Path) -> io::Result<bool> {
        fs::read_dir(path)?;
        Ok(true)
    }

    #[inline]
    fn is_file(&self, path: &Path) -> io::Result<bool> {
        Ok(fs::metadata(path)?.is_file())
    }

    #[inline]
    fn open_read(&self, path: &Path) -> io::Result<Self::Reader> {
        File::open(path)
    }
}

pub struct FileManager<S> {
    file_system: S,
    path_interner: AbsPathInterner,
    search_paths: Vec<PathRef>,
}

impl<S: FileSystem> FileManager<S> {
    #[inline]
    pub fn new(file_system: S) -> Self {
        Self {
            file_system,
            path_interner: AbsPathInterner::new(),
            search_paths: Vec::new(),
        }
    }

    pub fn add_search_path<C: AsRef<Path>, P: AsRef<Path>>(
        &mut self,
        cwd: C,
        path: P,
    ) -> io::Result<PathRef> {
        self.file_system.is_dir(path.as_ref())?;
        Ok(self.path_interner.intern(cwd, path))
    }

    pub fn reader<C: AsRef<Path>, P: AsRef<Path>>(
        &mut self,
        cwd: C,
        path: P,
    ) -> io::Result<Option<(PathRef, S::Reader)>> {
        if let Some(pathref) = self.search(cwd, path)? {
            let path = self.path_interner.get(pathref).unwrap();
            return Ok(Some((pathref, self.file_system.open_read(path)?)));
        }
        Ok(None)
    }

    fn search<C: AsRef<Path>, P: AsRef<Path>>(
        &mut self,
        cwd: C,
        path: P,
    ) -> io::Result<Option<PathRef>> {
        let cwd = self.path_interner.intern(cwd, ".");
        for dir in iter::once(&cwd).chain(&self.search_paths) {
            let dir = self.path_interner.get(*dir).unwrap().to_path_buf();
            let path = dir.join(path.as_ref());
            if self.file_system.is_file(path.as_ref())? {
                return Ok(Some(self.path_interner.intern(dir, path)));
            }
        }
        Ok(None)
    }
}
