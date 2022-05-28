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
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

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
        32, 0
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
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

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
        44, 0
    ], data);
}

#[test]
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
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

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
        24, 0
    ], data);
}

#[test]
fn bit() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            bit 0, a
            bit 1, b
            bit 2, c
            bit 3, d
            bit 4, e
            bit 5, h
            bit 6, l
            bit 7, (hl)
            bit 0, (ix+1)
            bit 1, (iy+1)
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0xCB, 0x47,
        0xCB, 0x48,
        0xCB, 0x51,
        0xCB, 0x5A,
        0xCB, 0x63,
        0xCB, 0x6C,
        0xCB, 0x75,
        0xCB, 0x7E,
        0xDD, 0xCB, 0x46, 0x01,
        0xFD, 0xCB, 0x4E, 0x01,
        24, 0
    ], data);
}

#[test]
fn call() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            @org $0100
            test: nop
            call test
            call nz, test
            call z, test
            call nc, test
            call c, test
            call po, test
            call pe, test
            call p, test
            call m, test
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0x00,
        0xCD, 0x00, 0x01,
        0xC4, 0x00, 0x01,
        0xCC, 0x00, 0x01,
        0xD4, 0x00, 0x01,
        0xDC, 0x00, 0x01,
        0xE4, 0x00, 0x01,
        0xEC, 0x00, 0x01,
        0xF4, 0x00, 0x01,
        0xFC, 0x00, 0x01,
        28, 0x01
    ], data);
}

#[test]
fn ccf() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            ccf
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0x3F,
        1, 0
    ], data);
}

#[test]
fn cp() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            cp a, a
            cp a, b
            cp a, c
            cp a, d
            cp a, e
            cp a, h
            cp a, l
            cp a, ixh
            cp a, ixl
            cp a, iyh
            cp a, iyl
            cp a, (hl)
            cp a, $42
            cp a, (ix+1)
            cp a, (iy+1)
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0xBF,
        0xB8,
        0xB9,
        0xBA,
        0xBB,
        0xBC,
        0xBD,
        0xDD, 0xBC,
        0xDD, 0xBD,
        0xFD, 0xBC,
        0xFD, 0xBD,
        0xBE,
        0xFE, 0x42,
        0xDD, 0xBE, 0x01,
        0xFD, 0xBE, 0x01,
        24, 0
    ], data);
}

#[test]
fn cpd() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            cpd
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0xED, 0xA9,
        2, 0
    ], data);
}

#[test]
fn cpdr() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            cpdr
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0xED, 0xB9,
        2, 0
    ], data);
}

#[test]
fn cpi() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            cpi
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0xED, 0xA1,
        2, 0
    ], data);
}

#[test]
fn cpir() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            cpir
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0xED, 0xB1,
        2, 0
    ], data);
}

#[test]
fn cpl() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            cpl
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0x2F,
        1, 0
    ], data);
}

#[test]
fn daa() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            daa
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0x27,
        1, 0
    ], data);
}

#[test]
fn res() {
    let parser = parser(&[(
        "/test.asm",
        r#"
            res 0, a
            res 1, b
            res 2, c
            res 3, d
            res 4, e
            res 5, h
            res 6, l
            res 7, (hl)
            res 0, (ix+1)
            res 1, (iy+1)
            @dw @here
        "#,
    )]);

    let mut data = Vec::new();
    parser
        .parse("/", "test.asm")
        .unwrap()
        .assemble(&mut data)
        .unwrap();

    #[rustfmt::skip]
    assert_eq!(vec![
        0xCB, 0x87,
        0xCB, 0x88,
        0xCB, 0x91,
        0xCB, 0x9A,
        0xCB, 0xA3,
        0xCB, 0xAC,
        0xCB, 0xB5,
        0xCB, 0xBE,
        0xDD, 0xCB, 0x86, 0x01,
        0xFD, 0xCB, 0x8E, 0x01,
        24, 0
    ], data);
}
