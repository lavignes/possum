use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    path::{Path, PathBuf},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct FileId(pub usize);

#[derive(Default)]
pub struct FileInfo {
    next_raw_id: usize,
    id_to_source: HashMap<FileId, SourceFile>,
    path_to_id: HashMap<PathBuf, FileId>,
}

impl FileInfo {
    pub fn new() -> Self {
        Self::default()
    }
}

impl FileInfo {
    pub fn insert<P: AsRef<Path>>(&mut self, path: P) -> FileId {
        let path = path.as_ref();
        if let Some(id) = self.path_to_id.get(path) {
            return *id;
        }
        let id = FileId(self.next_raw_id);
        let path = path.to_path_buf();
        self.path_to_id.insert(path.clone(), id);
        self.id_to_source.insert(id, SourceFile { path });
        self.next_raw_id += 1;
        id
    }

    pub fn get(&self, file: FileId) -> Option<&SourceFile> {
        self.id_to_source.get(&file)
    }
}

pub struct SourceFile {
    path: PathBuf,
}

#[derive(Copy, Clone, Debug)]
pub struct SourceLoc {
    pub file: FileId,
    pub line: usize,
    pub column: usize,
}

impl Display for SourceLoc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { file, line, column } = self;
        write!(f, "<{file:?}>:{line}:{column}")
    }
}
