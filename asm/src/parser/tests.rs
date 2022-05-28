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

fn parser<P: AsRef<Path>>(files: &[(P, &str)]) -> Parser<StringFileSystem, Cursor<String>> {
    Parser::new(StringFileSystem::new(files))
}

#[test]
fn adc() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            adc a, a
            adc a, b
            adc a, c
            adc a, d
            adc a, e
            adc a, h
            adc a, l
            adc a, ixh
            adc a, ixl
            adc a, iyh
            adc a, iyl
            adc a, (hl)
            adc a, $42
            adc a, (ix+1)
            adc a, (iy+1)
            adc hl, bc
            adc hl, de
            adc hl, hl
            adc hl, sp
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser.parse("/", "test.asm").unwrap().assemble(&mut data);

    #[rustfmt::skip]
    assert_eq!(vec![
        0x8F,
        0x88,
        0x89,
        0x8A,
        0x8B,
        0x8C,
        0x8D,
        0xDD, 0x8C,
        0xDD, 0x8D,
        0xFD, 0x8C,
        0xFD, 0x8D,
        0x8E,
        0xCE, 0x42,
        0xDD, 0x8E, 0x01,
        0xFD, 0x8E, 0x01,
        0xED, 0x4A,
        0xED, 0x5A,
        0xED, 0x6A,
        0xED, 0x7A,
        32, 00
    ], data);
}

#[test]
fn add() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            add a, a
            add a, b
            add a, c
            add a, d
            add a, e
            add a, h
            add a, l
            add a, ixh
            add a, ixl
            add a, iyh
            add a, iyl
            add a, (hl)
            add a, $42
            add a, (ix+1)
            add a, (iy+1)
            add hl, bc
            add hl, de
            add hl, hl
            add hl, sp
            add ix, bc
            add ix, de
            add ix, ix
            add ix, sp
            add iy, bc
            add iy, de
            add iy, iy
            add iy, sp
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser.parse("/", "test.asm").unwrap().assemble(&mut data);

    #[rustfmt::skip]
    assert_eq!(vec![
        0x87,
        0x80,
        0x81,
        0x82,
        0x83,
        0x84,
        0x85,
        0xDD, 0x84,
        0xDD, 0x85,
        0xFD, 0x84,
        0xFD, 0x85,
        0x86,
        0xC6, 0x42,
        0xDD, 0x86, 0x01,
        0xFD, 0x86, 0x01,
        0x09,
        0x19,
        0x29,
        0x39,
        0xDD, 0x09,
        0xDD, 0x19,
        0xDD, 0x29,
        0xDD, 0x39,
        0xFD, 0x09,
        0xFD, 0x19,
        0xFD, 0x29,
        0xFD, 0x39,
        44, 00
    ], data);
}

fn and() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            and a, a
            and a, b
            and a, c
            and a, d
            and a, e
            and a, h
            and a, l
            and a, ixh
            and a, ixl
            and a, iyh
            and a, iyl
            and a, (hl)
            and a, $42
            and a, (ix+1)
            and a, (iy+1)
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser.parse("/", "test.asm").unwrap().assemble(&mut data);

    #[rustfmt::skip]
    assert_eq!(vec![
        0xA7,
        0xA0,
        0xA1,
        0xA2,
        0xA3,
        0xA4,
        0xA5,
        0xDD, 0xA4,
        0xDD, 0xA5,
        0xFD, 0xA4,
        0xFD, 0xA5,
        0xA6,
        0xE6, 0x42,
        0xDD, 0xA6, 0x01,
        0xFD, 0xA6, 0x01,
        24, 00
    ], data);
}
