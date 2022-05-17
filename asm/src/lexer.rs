use std::{
    cell::{Ref, RefCell, RefMut},
    fmt::{self, Display, Formatter},
    fs::File,
    io::{self, Read},
    rc::Rc,
};

use crate::{
    charreader::{CharReader, CharReaderError},
    intern::{PathRef, StrRef},
    PathInterner, StrInterner,
};

#[derive(Copy, Clone, Debug)]
pub struct SourceLoc {
    pub path: PathRef,
    pub line: usize,
    pub column: usize,
}

impl Display for SourceLoc {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self { path, line, column } = self;
        write!(f, "<pathref {path:?}>:{line}:{column}")
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LexerError {
    #[error("read error: {0}")]
    ReadError(CharReaderError),

    #[error("unrecognized string escape: `{0}`")]
    UnrecognizedStringEscape(String),

    #[error("malformed binary number: `{0}`")]
    MalformedBinaryNumber(String),

    #[error("malformed decimal number: `{0}`")]
    MalformedDecimalNumber(String),

    #[error("malformed hexadecimal number: `{0}`")]
    MalformedHexidecimalNumber(String),

    #[error("unrecognized input: `{0}`")]
    UnrecognizedInput(String),

    #[error("unknown directive: `{0}`")]
    UnknownDirective(String),

    #[error("malformed label: `{0}`")]
    MalformedLabel(String),
}

#[derive(Debug, Copy, Clone)]
pub enum OperationName {
    Nop,
    Ld,
    Inc,
    Dec,
    Rlca,
    Ex,
    Add,
    Rrca,
    Djnz,
    Rla,
    Jr,
    Rra,
    Daa,
    Cpl,
    Scf,
    Ccf,
    Halt,
    Adc,
    Sub,
    Sbc,
    And,
    Xor,
    Or,
    Cp,
    Ret,
    Pop,
    Jp,
    Call,
    Push,
    Rst,
    Out,
    Exx,
    In,
    Di,
    Ei,
    Neg,
    Retn,
    Im,
    Reti,
    Ldi,
    Cpi,
    Ini,
    Outi,
    Ldd,
    Cpd,
    Ind,
    Outd,
    Ldir,
    Cpir,
    Inir,
    Otir,
    Lddr,
    Cpdr,
    Indr,
    Otdr,
    Rlc,
    Rrc,
    Rl,
    Rr,
    Sla,
    Sra,
    Sll,
    Srl,
    Bit,
    Res,
    Set,
}

impl OperationName {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "nop" | "NOP" => Some(Self::Nop),
            "ld" | "LD" => Some(Self::Ld),
            "inc" | "INC" => Some(Self::Inc),
            "dec" | "DEC" => Some(Self::Dec),
            "rlca" | "RLCA" => Some(Self::Rlca),
            "ex" | "EX" => Some(Self::Ex),
            "add" | "ADD" => Some(Self::Add),
            "rrca" | "RRCA" => Some(Self::Rrca),
            "djnz" | "DJNZ" => Some(Self::Djnz),
            "rla" | "RLA" => Some(Self::Rla),
            "jr" | "JR" => Some(Self::Jr),
            "rra" | "RRA" => Some(Self::Rra),
            "daa" | "DAA" => Some(Self::Daa),
            "cpl" | "CPL" => Some(Self::Cpl),
            "scf" | "SCF" => Some(Self::Scf),
            "ccf" | "CCF" => Some(Self::Ccf),
            "halt" | "HALT" => Some(Self::Halt),
            "adc" | "ADC" => Some(Self::Adc),
            "sub" | "SUB" => Some(Self::Sub),
            "sbc" | "SBC" => Some(Self::Sbc),
            "and" | "AND" => Some(Self::And),
            "xor" | "XOR" => Some(Self::Xor),
            "or" | "OR" => Some(Self::Or),
            "cp" | "CP" => Some(Self::Cp),
            "ret" | "RET" => Some(Self::Ret),
            "pop" | "POP" => Some(Self::Pop),
            "jp" | "JP" => Some(Self::Jp),
            "call" | "CALL" => Some(Self::Call),
            "push" | "PUSH" => Some(Self::Push),
            "rst" | "RST" => Some(Self::Rst),
            "out" | "OUT" => Some(Self::Out),
            "exx" | "EXX" => Some(Self::Exx),
            "in" | "IN" => Some(Self::In),
            "di" | "DI" => Some(Self::Di),
            "ei" | "EI" => Some(Self::Ei),
            "neg" | "NEG" => Some(Self::Neg),
            "retn" | "RETN" => Some(Self::Retn),
            "im" | "IM" => Some(Self::Im),
            "reti" | "RETI" => Some(Self::Reti),
            "ldi" | "LDI" => Some(Self::Ldi),
            "cpi" | "CPI" => Some(Self::Cpi),
            "ini" | "INI" => Some(Self::Ini),
            "outi" | "OUTI" => Some(Self::Outi),
            "ldd" | "LDD" => Some(Self::Ldd),
            "cpd" | "CPD" => Some(Self::Cpd),
            "ind" | "IND" => Some(Self::Ind),
            "outd" | "OUTD" => Some(Self::Outd),
            "ldir" | "LDIR" => Some(Self::Ldir),
            "cpir" | "CPIR" => Some(Self::Cpir),
            "inir" | "INIR" => Some(Self::Inir),
            "otir" | "OTIR" => Some(Self::Otir),
            "lddr" | "LDDR" => Some(Self::Lddr),
            "cpdr" | "CPDR" => Some(Self::Cpdr),
            "indr" | "INDR" => Some(Self::Indr),
            "otdr" | "OTDR" => Some(Self::Otdr),
            "rlc" | "RLC" => Some(Self::Rlc),
            "rrc" | "RRC" => Some(Self::Rrc),
            "rl" | "RL" => Some(Self::Rl),
            "rr" | "RR" => Some(Self::Rr),
            "sla" | "SLA" => Some(Self::Sla),
            "sra" | "SRA" => Some(Self::Sra),
            "sll" | "SLL" => Some(Self::Sll),
            "srl" | "SRL" => Some(Self::Srl),
            "bit" | "BIT" => Some(Self::Bit),
            "res" | "RES" => Some(Self::Res),
            "set" | "SET" => Some(Self::Set),
            _ => None,
        }
    }
}

impl Display for OperationName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Nop => "nop",
                Self::Ld => "ld",
                Self::Inc => "inc",
                Self::Dec => "dec",
                Self::Rlca => "rlca",
                Self::Ex => "ex",
                Self::Add => "add",
                Self::Rrca => "rrca",
                Self::Djnz => "djnz",
                Self::Rla => "rla",
                Self::Jr => "jr",
                Self::Rra => "rra",
                Self::Daa => "daa",
                Self::Cpl => "cpl",
                Self::Scf => "scf",
                Self::Ccf => "ccf",
                Self::Halt => "halt",
                Self::Adc => "adc",
                Self::Sub => "sub",
                Self::Sbc => "sbc",
                Self::And => "and",
                Self::Xor => "xor",
                Self::Or => "or",
                Self::Cp => "cp",
                Self::Ret => "ret",
                Self::Pop => "pop",
                Self::Jp => "jp",
                Self::Call => "call",
                Self::Push => "push",
                Self::Rst => "rst",
                Self::Out => "out",
                Self::Exx => "exx",
                Self::In => "in",
                Self::Di => "di",
                Self::Ei => "ei",
                Self::Neg => "neg",
                Self::Retn => "retn",
                Self::Im => "im",
                Self::Reti => "reti",
                Self::Ldi => "ldi",
                Self::Cpi => "cpi",
                Self::Ini => "ini",
                Self::Outi => "outi",
                Self::Ldd => "ldd",
                Self::Cpd => "cpd",
                Self::Ind => "ind",
                Self::Outd => "outd",
                Self::Ldir => "ldir",
                Self::Cpir => "cpir",
                Self::Inir => "inir",
                Self::Otir => "otir",
                Self::Lddr => "lddr",
                Self::Cpdr => "cpdr",
                Self::Indr => "indr",
                Self::Otdr => "otdr",
                Self::Rlc => "rlc",
                Self::Rrc => "rrc",
                Self::Rl => "rl",
                Self::Rr => "rr",
                Self::Sla => "sla",
                Self::Sra => "sra",
                Self::Sll => "sll",
                Self::Srl => "srl",
                Self::Bit => "bit",
                Self::Res => "res",
                Self::Set => "set",
            }
        )
    }
}

#[derive(Debug, Copy, Clone)]
pub enum RegisterName {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    AF,
    BC,
    DE,
    HL,
    IX,
    IY,
    IXL,
    IXH,
    IYL,
    IYH,
    PC,
    SP,
    AFPrime,
}

impl RegisterName {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "a" | "A" => Some(Self::A),
            "b" | "B" => Some(Self::B),
            "c" | "C" => Some(Self::C),
            "d" | "D" => Some(Self::D),
            "e" | "E" => Some(Self::E),
            "h" | "H" => Some(Self::H),
            "l" | "L" => Some(Self::L),
            "af" | "AF" => Some(Self::AF),
            "bc" | "BC" => Some(Self::BC),
            "de" | "DE" => Some(Self::DE),
            "hl" | "HL" => Some(Self::HL),
            "ix" | "IX" => Some(Self::IX),
            "iy" | "IY" => Some(Self::IY),
            "ixl" | "IXL" => Some(Self::IXL),
            "ixh" | "IXH" => Some(Self::IXH),
            "iyl" | "IYL" => Some(Self::IYL),
            "iyh" | "IYH" => Some(Self::IYH),
            "pc" | "PC" => Some(Self::PC),
            "sp" | "SP" => Some(Self::SP),
            "af'" | "AF'" => Some(Self::AFPrime),
            _ => None,
        }
    }
}

impl Display for RegisterName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::A => "a",
                Self::B => "b",
                Self::C => "c",
                Self::D => "d",
                Self::E => "e",
                Self::H => "h",
                Self::L => "l",
                Self::AF => "af",
                Self::BC => "bc",
                Self::DE => "de",
                Self::HL => "hl",
                Self::IX => "ix",
                Self::IY => "iy",
                Self::IXL => "ixl",
                Self::IXH => "ixh",
                Self::IYL => "iyl",
                Self::IYH => "iyh",
                Self::PC => "pc",
                Self::SP => "sp",
                Self::AFPrime => "af'",
            }
        )
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Symbol {
    Tilde,
    Bang,
    Dollar,
    Mod,
    Caret,
    Ampersand,
    DoubleAmpersand,
    Star,
    ParenOpen,
    ParenClose,
    Minus,
    Equal,
    NotEqual,
    Plus,
    Pipe,
    DoublePipe,
    Colon,
    Comma,
    LessThan,
    GreaterThan,
    LessEqual,
    GreaterEqual,
    ShiftLeft,
    ShiftRight,
    Div,
    Question,
}

impl Symbol {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "~" => Some(Self::Tilde),
            "!" => Some(Self::Bang),
            "$" => Some(Self::Dollar),
            "%" => Some(Self::Mod),
            "^" => Some(Self::Caret),
            "&" => Some(Self::Ampersand),
            "&&" => Some(Self::DoubleAmpersand),
            "*" => Some(Self::Star),
            "(" => Some(Self::ParenOpen),
            ")" => Some(Self::ParenClose),
            "-" => Some(Self::Minus),
            "==" => Some(Self::Equal),
            "!=" => Some(Self::NotEqual),
            "+" => Some(Self::Plus),
            "|" => Some(Self::Pipe),
            "||" => Some(Self::DoublePipe),
            ":" => Some(Self::Colon),
            "," => Some(Self::Comma),
            "<" => Some(Self::LessThan),
            ">" => Some(Self::GreaterThan),
            "<=" => Some(Self::LessEqual),
            ">=" => Some(Self::GreaterEqual),
            "<<" => Some(Self::ShiftLeft),
            ">>" => Some(Self::ShiftRight),
            "/" => Some(Self::Div),
            "?" => Some(Self::Question),
            _ => None,
        }
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Tilde => "~",
                Self::Bang => "!",
                Self::Dollar => "$",
                Self::Mod => " %",
                Self::Caret => "^",
                Self::Ampersand => "&",
                Self::DoubleAmpersand => "&&",
                Self::Star => "*",
                Self::ParenOpen => "(",
                Self::ParenClose => ")",
                Self::Minus => "-",
                Self::Equal => "==",
                Self::NotEqual => "!=",
                Self::Plus => "+",
                Self::Pipe => "|",
                Self::DoublePipe => "||",
                Self::Colon => ":",
                Self::Comma => ",",
                Self::LessThan => "<",
                Self::GreaterThan => ">",
                Self::LessEqual => "<=",
                Self::GreaterEqual => ">=",
                Self::ShiftLeft => "<<",
                Self::ShiftRight => ">>",
                Self::Div => "/",
                Self::Question => "?",
            }
        )
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DirectiveName {
    Org,
    Macro,
    Enum,
    Struct,
    Define,
    If,
    Ifdef,
    Ifndef,
    Else,
    End,
    Echo,
    Db,
    Dw,
    Ds,
    Include,
    Incbin,
    Sizeof,
}

impl DirectiveName {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "#org" | "#ORG" => Some(Self::Org),
            "#macro" | "#MACRO" => Some(Self::Macro),
            "#enum" | "#ENUM" => Some(Self::Enum),
            "#struct" | "#STRUCT" => Some(Self::Struct),
            "#define" | "#DEFINE" => Some(Self::Define),
            "#if" | "#IF" => Some(Self::If),
            "#ifdef" | "#IFDEF" => Some(Self::Ifdef),
            "#ifndef" | "#IFNDEF" => Some(Self::Ifndef),
            "#else" | "#ELSE" => Some(Self::Else),
            "#end" | "#END" => Some(Self::End),
            "#echo" | "#ECHO" => Some(Self::Echo),
            "#db" | "#DB" => Some(Self::Db),
            "#dw" | "#DW" => Some(Self::Dw),
            "#ds" | "#DS" => Some(Self::Ds),
            "#include" | "#INCLUDE" => Some(Self::Include),
            "#incbin" | "#INCBIN" => Some(Self::Incbin),
            "#sizeof" | "#SIZEOF" => Some(Self::Sizeof),
            _ => None,
        }
    }
}

impl Display for DirectiveName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Org => "#org",
                Self::Macro => "#macro",
                Self::Enum => "#enum",
                Self::Struct => "#struct",
                Self::Define => "#define",
                Self::If => "#if",
                Self::Ifdef => "#ifdef",
                Self::Ifndef => "#ifndef",
                Self::Else => "#else",
                Self::End => "#end",
                Self::Echo => "#echo",
                Self::Db => "#db",
                Self::Dw => "#dw",
                Self::Ds => "#ds",
                Self::Include => "#include",
                Self::Incbin => "#incbin",
                Self::Sizeof => "#sizeof",
            }
        )
    }
}

enum State {
    Initial,
    InComment,
    InString,
    InStringEscape,
    InHexStringEscape1,
    InHexStringEscape2,
    InNumberBase2,
    InNumberBase10,
    InNumberBase16,
    InSymbol,
    InIdentifier,
    InDirective,
}

#[derive(Debug, Copy, Clone)]
pub enum LabelType {
    Global,
    Local,
    Direct,
}

#[derive(Debug, Clone)]
pub enum Token {
    Comment,
    String(StrRef),
    Number(usize),
    Operation(OperationName),
    Directive(DirectiveName),
    Register(RegisterName),
    Symbol(Symbol),
    Label(LabelType, StrRef),
}

pub trait LexerFactory<R> {
    fn create(
        &self,
        path_interner: Ref<PathInterner>,
        str_interner: Rc<RefCell<StrInterner>>,
        path: PathRef,
    ) -> io::Result<Lexer<R>>;
}

pub struct FileLexerFactory {}

impl FileLexerFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl LexerFactory<File> for FileLexerFactory {
    fn create(
        &self,
        path_interner: Ref<PathInterner>,
        str_interner: Rc<RefCell<StrInterner>>,
        path: PathRef,
    ) -> io::Result<Lexer<File>> {
        // TODO: Need to pass the file manager!
        let reader = File::open(path_interner.get(path).unwrap())?;
        Ok(Lexer::new(str_interner, path, reader))
    }
}

pub struct Lexer<R> {
    str_interner: Rc<RefCell<StrInterner>>,
    loc: SourceLoc,
    tok_loc: SourceLoc,
    included_from: Option<SourceLoc>,
    inner: CharReader<R>,
    stash: Option<char>,
    state: State,
    buffer: String,
}

impl<R: Read> Lexer<R> {
    #[inline]
    pub fn new(str_interner: Rc<RefCell<StrInterner>>, path: PathRef, reader: R) -> Self {
        let loc = SourceLoc {
            path,
            line: 1,
            column: 1,
        };
        Self {
            str_interner,
            loc,
            tok_loc: loc,
            included_from: None,
            inner: CharReader::new(reader),
            stash: None,
            state: State::Initial,
            buffer: String::new(),
        }
    }

    #[inline]
    pub fn str_interner(&self) -> Ref<StrInterner> {
        self.str_interner.as_ref().borrow()
    }

    #[inline]
    pub fn str_interner_mut(&self) -> RefMut<StrInterner> {
        self.str_interner.borrow_mut()
    }

    #[inline]
    fn is_value_terminator(&self, c: char) -> bool {
        // Basically any char that could reasonably mark the end of a label or number
        matches!(
            c,
            '~' | '!'
                | '%'
                | '^'
                | '&'
                | '*'
                | '-'
                | '+'
                | '>'
                | '<'
                | '='
                | '?'
                | '/'
                | ':'
                | '|'
                | ';'
                | ','
        )
    }

    #[inline]
    fn is_symbol_start(&self, c: char) -> bool {
        matches!(
            c,
            '~' | '!'
                | '$'
                | '%'
                | '^'
                | '&'
                | '*'
                | '('
                | ')'
                | '-'
                | '='
                | '+'
                | '|'
                | ':'
                | ','
                | '<'
                | '>'
                | '?'
                | '/'
        )
    }
}

impl<R: Read> Iterator for Lexer<R> {
    type Item = (SourceLoc, Result<Token, LexerError>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let c = match self.stash {
                Some(c) => {
                    self.stash = None;
                    c
                }
                None => match self.inner.next() {
                    None => return None,
                    Some(Err(e)) => {
                        return Some((self.loc, Err(LexerError::ReadError(e))));
                    }
                    Some(Ok(c)) => {
                        self.loc.column += 1;
                        if c == '\n' {
                            self.loc.line += 1;
                            self.loc.column = 1;
                        }
                        c
                    }
                },
            };

            match self.state {
                State::Initial => match c {
                    _ if c.is_whitespace() => continue,

                    ';' => {
                        self.state = State::InComment;
                        self.tok_loc = self.loc;
                    }

                    '"' => {
                        self.state = State::InString;
                        self.tok_loc = self.loc;
                        self.buffer.clear();
                    }

                    '%' => {
                        self.state = State::InNumberBase2;
                        self.tok_loc = self.loc;
                        self.buffer.clear();
                    }

                    '0'..='9' => {
                        self.state = State::InNumberBase10;
                        self.tok_loc = self.loc;
                        self.buffer.clear();
                        self.buffer.push(c);
                    }

                    '$' => {
                        self.state = State::InNumberBase16;
                        self.tok_loc = self.loc;
                        self.buffer.clear();
                    }

                    _ if self.is_symbol_start(c) => {
                        self.state = State::InSymbol;
                        self.tok_loc = self.loc;
                        self.buffer.clear();
                        self.buffer.push(c);
                    }

                    '#' => {
                        self.state = State::InDirective;
                        self.tok_loc = self.loc;
                        self.buffer.clear();
                        self.buffer.push(c);
                    }

                    _ if c.is_alphanumeric() || c == '_' || c == '.' => {
                        self.state = State::InIdentifier;
                        self.tok_loc = self.loc;
                        self.buffer.clear();
                        self.buffer.push(c);
                    }

                    _ => {
                        return Some((
                            self.loc,
                            Err(LexerError::UnrecognizedInput(format!("{c}"))),
                        ));
                    }
                },

                State::InComment => match c {
                    '\n' => {
                        self.state = State::Initial;
                        return Some((self.tok_loc, Ok(Token::Comment)));
                    }

                    _ => {}
                },

                State::InString => match c {
                    '"' => {
                        self.state = State::Initial;
                        let string = self.str_interner.borrow_mut().intern(&self.buffer);
                        return Some((self.tok_loc, Ok(Token::String(string))));
                    }

                    '\\' => {
                        self.state = State::InStringEscape;
                    }

                    _ => self.buffer.push(c),
                },

                State::InStringEscape => match c {
                    'n' => {
                        self.state = State::InString;
                        self.buffer.push('\n');
                    }

                    'r' => {
                        self.state = State::InString;
                        self.buffer.push('\r');
                    }

                    't' => {
                        self.state = State::InString;
                        self.buffer.push('\t');
                    }

                    '\\' => {
                        self.state = State::InString;
                        self.buffer.push('\\');
                    }

                    '0' => {
                        self.state = State::InString;
                        self.buffer.push('\0');
                    }

                    '\"' => {
                        self.state = State::InString;
                        self.buffer.push('\"');
                    }

                    '$' => {
                        self.state = State::InHexStringEscape1;
                    }

                    _ => {
                        return Some((
                            self.loc,
                            Err(LexerError::UnrecognizedStringEscape(format!("\\{c}"))),
                        ));
                    }
                },

                State::InHexStringEscape1 => match c {
                    '0'..='9' | 'a'..='f' | 'A'..='F' => {
                        self.state = State::InHexStringEscape2;
                        self.buffer.push(c.to_ascii_lowercase());
                    }

                    _ => {
                        return Some((
                            self.loc,
                            Err(LexerError::UnrecognizedStringEscape(format!("\\${c}"))),
                        ));
                    }
                },

                State::InHexStringEscape2 => match c {
                    '0'..='9' | 'a'..='f' | 'A'..='F' => {
                        // Its kinda janky, but we pushed both bytes onto the buffer
                        // so we can parse them, then we overwrite the bytes with the char
                        // value.
                        self.state = State::InString;
                        self.buffer.push(c.to_ascii_lowercase());

                        let last2 = &self.buffer[self.buffer.len() - 2..self.buffer.len()];
                        let byte = u8::from_str_radix(last2, 16).unwrap();

                        self.buffer.truncate(self.buffer.len() - 2);
                        self.buffer.push(byte as char);
                    }

                    _ => {
                        return Some((
                            self.loc,
                            Err(LexerError::UnrecognizedStringEscape(format!(
                                "\\${}{}",
                                self.buffer.chars().last().unwrap(),
                                c
                            ))),
                        ));
                    }
                },

                State::InNumberBase2 => match c {
                    _ if c.is_whitespace() || self.is_value_terminator(c) => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        // Instead of treating this as a binary number, this must
                        // be the mod symbol
                        if self.buffer.is_empty() {
                            return Some((self.tok_loc, Ok(Token::Symbol(Symbol::Mod))));
                        }

                        let value = usize::from_str_radix(&self.buffer, 2).unwrap();
                        return Some((self.tok_loc, Ok(Token::Number(value))));
                    }

                    '0' | '1' => self.buffer.push(c),

                    _ => {
                        return Some((
                            self.tok_loc,
                            Err(LexerError::MalformedBinaryNumber(format!(
                                "%{}{}",
                                self.buffer, c
                            ))),
                        ));
                    }
                },

                State::InNumberBase10 => match c {
                    _ if c.is_whitespace() || self.is_value_terminator(c) => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        let value = usize::from_str_radix(&self.buffer, 10).unwrap();
                        return Some((self.tok_loc, Ok(Token::Number(value))));
                    }

                    '0'..='9' => self.buffer.push(c),

                    _ => {
                        return Some((
                            self.tok_loc,
                            Err(LexerError::MalformedDecimalNumber(format!(
                                "{}{}",
                                self.buffer, c
                            ))),
                        ));
                    }
                },

                State::InNumberBase16 => match c {
                    _ if c.is_whitespace() || self.is_value_terminator(c) => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        // Instead of treating this as a hex number, this must
                        // be the dollar symbol
                        if self.buffer.is_empty() {
                            return Some((self.tok_loc, Ok(Token::Symbol(Symbol::Dollar))));
                        }

                        let value = usize::from_str_radix(&self.buffer, 16).unwrap();
                        return Some((self.tok_loc, Ok(Token::Number(value))));
                    }

                    '0'..='9' | 'a'..='f' | 'A'..='F' => self.buffer.push(c.to_ascii_lowercase()),

                    _ => {
                        return Some((
                            self.tok_loc,
                            Err(LexerError::MalformedHexidecimalNumber(format!(
                                "${}{}",
                                self.buffer, c
                            ))),
                        ));
                    }
                },

                State::InSymbol => {
                    self.state = State::Initial;
                    self.buffer.push(c);

                    // first try and parse a 2 char symbol
                    if let Some(symbol) = Symbol::parse(&self.buffer) {
                        return Some((self.tok_loc, Ok(Token::Symbol(symbol))));
                    }

                    // It must be 1 char (stash the other char)
                    self.stash = self.buffer.pop();
                    return Some((
                        self.tok_loc,
                        Ok(Token::Symbol(Symbol::parse(&self.buffer).unwrap())),
                    ));
                }

                State::InDirective => match c {
                    _ if c.is_alphanumeric() || c == '_' => {
                        self.buffer.push(c);
                    }

                    _ => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        if let Some(dir) = DirectiveName::parse(&self.buffer) {
                            return Some((self.tok_loc, Ok(Token::Directive(dir))));
                        }

                        return Some((
                            self.tok_loc,
                            Err(LexerError::UnknownDirective(self.buffer.clone())),
                        ));
                    }
                },

                State::InIdentifier => match c {
                    _ if c.is_alphanumeric() || c == '_' || c == '.' => {
                        self.buffer.push(c);
                    }

                    '\'' => {
                        self.state = State::Initial;
                        if self.buffer == "af" {
                            return Some((
                                self.tok_loc,
                                Ok(Token::Register(RegisterName::AFPrime)),
                            ));
                        }
                    }

                    _ => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        if let Some(op) = OperationName::parse(&self.buffer) {
                            return Some((self.tok_loc, Ok(Token::Operation(op))));
                        }

                        if let Some(reg) = RegisterName::parse(&self.buffer) {
                            return Some((self.tok_loc, Ok(Token::Register(reg))));
                        }

                        let string = self.str_interner.borrow_mut().intern(&self.buffer);
                        return match self.buffer.chars().filter(|c| *c == '.').count() {
                            0 => Some((self.tok_loc, Ok(Token::Label(LabelType::Global, string)))),
                            1 => {
                                if self.buffer.starts_with('.') {
                                    Some((self.tok_loc, Ok(Token::Label(LabelType::Local, string))))
                                } else {
                                    Some((
                                        self.tok_loc,
                                        Ok(Token::Label(LabelType::Direct, string)),
                                    ))
                                }
                            }
                            _ => Some((
                                self.tok_loc,
                                Err(LexerError::MalformedLabel(self.buffer.clone())),
                            )),
                        };
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, io::Cursor, rc::Rc};

    use crate::{
        lexer::{LabelType, RegisterName, Symbol, Token},
        Lexer, PathInterner, StrInterner,
    };

    fn lexer(text: &str) -> Lexer<Cursor<&str>> {
        let path_interner = Rc::new(RefCell::new(PathInterner::new()));
        let str_interner = Rc::new(RefCell::new(StrInterner::new()));
        let path = path_interner.borrow_mut().intern("file.test");
        Lexer::new(str_interner, path, Cursor::new(text))
    }

    #[test]
    fn comment() {
        let text = r#"
            ; comment
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(lexer.next(), Some((_, Ok(Token::Comment)))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn string() {
        let text = r#"
            "test"
        "#;
        let mut lexer = lexer(text);
        assert!(
            matches!(lexer.next(), Some((_, Ok(Token::String(s)))) if lexer.str_interner().eq_some("test", s))
        );
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn string_escape() {
        let text = r#"
            "test\n"
        "#;
        let mut lexer = lexer(text);
        assert!(
            matches!(lexer.next(), Some((_, Ok(Token::String(s)))) if lexer.str_interner().eq_some("test\n", s))
        );
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn string_escape_raw() {
        let text = r#"
            "test\$7f"
        "#;
        let mut lexer = lexer(text);
        assert!(
            matches!(lexer.next(), Some((_, Ok(Token::String(s)))) if lexer.str_interner().eq_some("test\x7f", s))
        );
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn base2() {
        let text = r#"
            %010101
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(lexer.next(), Some((_, Ok(Token::Number(n)))) if n == 0b010101));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn base10() {
        let text = r#"
            123456
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(lexer.next(), Some((_, Ok(Token::Number(n)))) if n == 123456));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn base16() {
        let text = r#"
            $cafebabe
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(lexer.next(), Some((_, Ok(Token::Number(n)))) if n == 0xcafebabe));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn symbols() {
        let text = r#"
            ~ ! $ % ^ & && * ( ) - = + | || : , < > <= >= << >> / ?
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Tilde))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Bang))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Dollar))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Mod))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Caret))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Ampersand))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::DoubleAmpersand))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Star))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::ParenOpen))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::ParenClose))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Minus))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Equal))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Plus))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Pipe))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::DoublePipe))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Colon))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Comma))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::LessThan))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::GreaterThan))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::LessEqual))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::GreaterEqual))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::ShiftLeft))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::ShiftRight))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Div))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Symbol(Symbol::Question))))
        ));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn regnames() {
        let text = r#"
            a b c d e h l af bc de hl ix iy ixl ixh iyl iyh sp pc af'
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::A))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::B))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::C))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::D))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::E))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::H))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::L))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::AF))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::BC))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::DE))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::HL))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::IX))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::IY))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::IXL))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::IXH))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::IYL))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::IYH))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::SP))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::PC))))
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Register(RegisterName::AFPrime))))
        ));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn labels() {
        let text = r#"
            global_label
            .local_label
            direct.label
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Label(LabelType::Global, s)))) if lexer.str_interner().eq_some("global_label", s)
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Label(LabelType::Local, s)))) if lexer.str_interner().eq_some(".local_label", s)
        ));
        assert!(matches!(
            lexer.next(),
            Some((_, Ok(Token::Label(LabelType::Direct, s)))) if lexer.str_interner().eq_some("direct.label", s)
        ));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn sanity_test() {
        let text = r#"
            #org $0000
            
            #macro foo arg1, arg2
                add arg1, arg2
            #end
            
            ; comment
            foo a, b
        "#;
        let mut lexer = lexer(text);
        while let Some((loc, res)) = lexer.next() {
            dbg!((loc, &res));
            assert!(res.is_ok());
        }
    }
}
