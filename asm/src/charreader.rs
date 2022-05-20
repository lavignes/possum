use std::{
    io::{self, Read},
    str,
};

#[derive(thiserror::Error, Debug)]
pub enum CharReaderError {
    #[error("{0}")]
    IoError(#[from] io::Error),

    #[error("{0}")]
    Utf8Error(#[from] str::Utf8Error),
}

pub struct CharReader<R> {
    inner: R,
    buf: [u8; 4],
    buf_len: usize,
}

impl<R> CharReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buf: [0; 4],
            buf_len: 0,
        }
    }
}

impl<R: Read> Iterator for CharReader<R> {
    type Item = Result<char, CharReaderError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buf_len == 0 {
            self.buf_len = match self.inner.read(&mut self.buf) {
                Ok(len) => len,
                Err(e) => return Some(Err(e.into())),
            }
        }

        if self.buf_len == 0 {
            return None;
        }

        let s = match str::from_utf8(&self.buf[0..self.buf_len]) {
            Ok(s) => s,
            Err(e) => {
                let valid_len = e.valid_up_to();
                if valid_len == 0 {
                    return Some(Err(e.into()));
                }
                // Safety: We already checked up to `valid_len`
                unsafe { str::from_utf8_unchecked(&self.buf[0..valid_len]) }
            }
        };

        let c = s.chars().next().unwrap();
        let char_len = c.len_utf8();
        self.buf.rotate_left(char_len);
        self.buf_len -= char_len;
        Some(Ok(c))
    }
}
