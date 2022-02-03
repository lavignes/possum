use std::{
    fs::File,
    io,
    ops::{Index, IndexMut},
};

use memmap2::MmapMut;
use possum_emu::MemoryMap;

pub struct MemoryMapWrapper(MmapMut);

impl MemoryMapWrapper {
    pub fn new(file: File) -> io::Result<Self> {
        Ok(Self(unsafe { MmapMut::map_mut(&file) }?))
    }
}

impl Index<usize> for MemoryMapWrapper {
    type Output = u8;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.0.index(index)
    }
}

impl IndexMut<usize> for MemoryMapWrapper {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.0.index_mut(index)
    }
}

impl MemoryMap for MemoryMapWrapper {
    type Error = io::Error;

    #[inline]
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.0.flush()
    }

    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}
