use std::{
    fmt::{self, Display, Formatter},
    io::Read,
};

use crate::{
    charreader::{CharReader, CharReaderError},
    fileinfo::{FileId, SourceLoc},
};

#[derive(thiserror::Error, Debug)]
pub enum LexerError {
    #[error("read error: {source}")]
    ReadError {
        loc: SourceLoc,
        source: CharReaderError,
    },

    #[error("unrecognized string escape: `{escape}`")]
    UnrecognizedStringEscape { loc: SourceLoc, escape: String },

    #[error("malformed binary number: `{value}`")]
    MalformedBinaryNumber { loc: SourceLoc, value: String },

    #[error("malformed decimal number: `{value}`")]
    MalformedDecimalNumber { loc: SourceLoc, value: String },

    #[error("malformed hexadecimal number: `{value}`")]
    MalformedHexidecimalNumber { loc: SourceLoc, value: String },

    #[error("unrecognized input: `{value}`")]
    UnrecognizedInput { loc: SourceLoc, value: String },

    #[error("malformed label: `{value}`")]
    MalformedLabel { loc: SourceLoc, value: String },
}

#[derive(Debug, Copy, Clone)]
pub enum OperationName {
    Adc,
    Add,
    And,
    Bit,
    Call,
    Ccf,
    Cp,
}

impl OperationName {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "adc" | "ADC" => Some(Self::Adc),
            "add" | "ADD" => Some(Self::Add),
            "and" | "AND" => Some(Self::And),
            "bit" | "BIT" => Some(Self::Bit),
            "call" | "CALL" => Some(Self::Call),
            "ccf" | "CCF" => Some(Self::Ccf),
            "cp" | "CP" => Some(Self::Cp),
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
                Self::Adc => "adc",
                Self::Add => "add",
                Self::And => "and",
                Self::Bit => "bit",
                Self::Call => "call",
                Self::Ccf => "ccf",
                Self::Cp => "cp",
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
            "=" => Some(Self::Equal),
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
                Self::Equal => "=",
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
}

impl DirectiveName {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "org" | "ORG" => Some(Self::Org),
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
                Self::Org => "org",
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
    String(String),
    Number(usize),
    Operation(OperationName),
    Directive(DirectiveName),
    Register(RegisterName),
    Symbol(Symbol),
    Label(LabelType, String),
}

pub struct Lexer<R: Read> {
    loc: SourceLoc,
    included_from: Option<SourceLoc>,
    inner: CharReader<R>,
    stash: Option<char>,
    state: State,
    buffer: String,
}

impl<R: Read> Lexer<R> {
    pub fn new(file: FileId, reader: R) -> Self {
        Self {
            loc: SourceLoc {
                file,
                line: 1,
                column: 1,
            },
            included_from: None,
            inner: CharReader::new(reader),
            stash: None,
            state: State::Initial,
            buffer: String::new(),
        }
    }

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
    type Item = Result<(SourceLoc, Token), LexerError>;

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
                        return Some(Err(LexerError::ReadError {
                            loc: self.loc,
                            source: e,
                        }));
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
                    }

                    '"' => {
                        self.state = State::InString;
                        self.buffer.clear();
                    }

                    '%' => {
                        self.state = State::InNumberBase2;
                        self.buffer.clear();
                    }

                    '0'..='9' => {
                        self.state = State::InNumberBase10;
                        self.buffer.clear();
                        self.buffer.push(c);
                    }

                    '$' => {
                        self.state = State::InNumberBase16;
                        self.buffer.clear();
                    }

                    _ if self.is_symbol_start(c) => {
                        self.state = State::InSymbol;
                        self.buffer.clear();
                        self.buffer.push(c);
                    }

                    _ if c.is_alphanumeric() || c == '_' || c == '.' => {
                        self.state = State::InIdentifier;
                        self.buffer.clear();
                        self.buffer.push(c);
                    }

                    _ => {
                        return Some(Err(LexerError::UnrecognizedInput {
                            loc: self.loc,
                            value: format!("{c}"),
                        }))
                    }
                },

                State::InComment => match c {
                    '\n' => {
                        self.state = State::Initial;
                        return Some(Ok((self.loc, Token::Comment)));
                    }

                    _ => {}
                },

                State::InString => match c {
                    '"' => {
                        self.state = State::Initial;
                        return Some(Ok((self.loc, Token::String(self.buffer.clone()))));
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
                        return Some(Err(LexerError::UnrecognizedStringEscape {
                            loc: self.loc,
                            escape: format!("\\{c}"),
                        }))
                    }
                },

                State::InHexStringEscape1 => match c {
                    '0'..='9' | 'a'..='f' | 'A'..='F' => {
                        self.state = State::InHexStringEscape2;
                        self.buffer.push(c.to_ascii_lowercase());
                    }

                    _ => {
                        return Some(Err(LexerError::UnrecognizedStringEscape {
                            loc: self.loc,
                            escape: format!("\\${c}"),
                        }))
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
                        return Some(Err(LexerError::UnrecognizedStringEscape {
                            loc: self.loc,
                            escape: format!("\\${}{}", self.buffer.chars().last().unwrap(), c),
                        }))
                    }
                },

                State::InNumberBase2 => match c {
                    _ if c.is_whitespace() || self.is_value_terminator(c) => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        // Instead of treating this as a binary number, this must
                        // be the mod symbol
                        if self.buffer.is_empty() {
                            return Some(Ok((self.loc, Token::Symbol(Symbol::Mod))));
                        }

                        let value = usize::from_str_radix(&self.buffer, 2).unwrap();
                        return Some(Ok((self.loc, Token::Number(value))));
                    }

                    '0' | '1' => self.buffer.push(c),

                    _ => {
                        return Some(Err(LexerError::MalformedBinaryNumber {
                            loc: self.loc,
                            value: format!("%{}{}", self.buffer, c),
                        }))
                    }
                },

                State::InNumberBase10 => match c {
                    _ if c.is_whitespace() || self.is_value_terminator(c) => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        let value = usize::from_str_radix(&self.buffer, 10).unwrap();
                        return Some(Ok((self.loc, Token::Number(value))));
                    }

                    '0'..='9' => self.buffer.push(c),

                    _ => {
                        return Some(Err(LexerError::MalformedDecimalNumber {
                            loc: self.loc,
                            value: format!("{}{}", self.buffer, c),
                        }))
                    }
                },

                State::InNumberBase16 => match c {
                    _ if c.is_whitespace() || self.is_value_terminator(c) => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        // Instead of treating this as a hex number, this must
                        // be the dollar symbol
                        if self.buffer.is_empty() {
                            return Some(Ok((self.loc, Token::Symbol(Symbol::Dollar))));
                        }

                        let value = usize::from_str_radix(&self.buffer, 16).unwrap();
                        return Some(Ok((self.loc, Token::Number(value))));
                    }

                    '0'..='9' | 'a'..='f' | 'A'..='F' => self.buffer.push(c.to_ascii_lowercase()),

                    _ => {
                        return Some(Err(LexerError::MalformedHexidecimalNumber {
                            loc: self.loc,
                            value: format!("${}{}", self.buffer, c),
                        }))
                    }
                },

                State::InSymbol => {
                    self.state = State::Initial;
                    self.buffer.push(c);

                    // first try and parse a 2 char symbol
                    if let Some(symbol) = Symbol::parse(&self.buffer) {
                        return Some(Ok((self.loc, Token::Symbol(symbol))));
                    }

                    // try again for 1 char (stash the other char)
                    self.stash = self.buffer.pop();
                    return Some(Ok((
                        self.loc,
                        Token::Symbol(Symbol::parse(&self.buffer).unwrap()),
                    )));
                }

                State::InIdentifier => match c {
                    _ if c.is_alphanumeric() || c == '_' || c == '.' => {
                        self.buffer.push(c);
                    }

                    '\'' => {
                        self.state = State::Initial;
                        if self.buffer == "af" {
                            return Some(Ok((self.loc, Token::Register(RegisterName::AFPrime))));
                        }
                    }

                    _ => {
                        self.state = State::Initial;

                        if let Some(op) = OperationName::parse(&self.buffer) {
                            return Some(Ok((self.loc, Token::Operation(op))));
                        }

                        if let Some(dir) = DirectiveName::parse(&self.buffer) {
                            return Some(Ok((self.loc, Token::Directive(dir))));
                        }

                        if let Some(reg) = RegisterName::parse(&self.buffer) {
                            return Some(Ok((self.loc, Token::Register(reg))));
                        }

                        return match self.buffer.chars().filter(|c| *c == '.').count() {
                            0 => Some(Ok((
                                self.loc,
                                Token::Label(LabelType::Global, self.buffer.clone()),
                            ))),
                            1 => {
                                if self.buffer.starts_with('.') {
                                    Some(Ok((
                                        self.loc,
                                        Token::Label(LabelType::Local, self.buffer.clone()),
                                    )))
                                } else {
                                    Some(Ok((
                                        self.loc,
                                        Token::Label(LabelType::Direct, self.buffer.clone()),
                                    )))
                                }
                            }
                            _ => Some(Err(LexerError::MalformedLabel {
                                loc: self.loc,
                                value: self.buffer.clone(),
                            })),
                        };
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{
        fileinfo::FileId,
        lexer::{LabelType, RegisterName, Symbol, Token},
        Lexer,
    };

    const FILE: FileId = FileId(0);

    #[test]
    fn comment() {
        let text = r#"
            ; comment
        "#;
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        assert!(matches!(lexer.next(), Some(Ok((_, Token::Comment)))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn string() {
        let text = r#"
            "test"
        "#;
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        assert!(matches!(lexer.next(), Some(Ok((_, Token::String(s)))) if s == "test"));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn string_escape() {
        let text = r#"
            "test\n"
        "#;
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        assert!(matches!(lexer.next(), Some(Ok((_, Token::String(s)))) if s == "test\n"));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn string_escape_raw() {
        let text = r#"
            "test\$7f"
        "#;
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        assert!(matches!(lexer.next(), Some(Ok((_, Token::String(s)))) if s == "test\x7f"));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn base2() {
        let text = r#"
            %010101
        "#;
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        assert!(matches!(lexer.next(), Some(Ok((_, Token::Number(n)))) if n == 0b010101));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn base10() {
        let text = r#"
            123456
        "#;
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        assert!(matches!(lexer.next(), Some(Ok((_, Token::Number(n)))) if n == 123456));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn base16() {
        let text = r#"
            $cafebabe
        "#;
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        assert!(matches!(lexer.next(), Some(Ok((_, Token::Number(n)))) if n == 0xcafebabe));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn symbols() {
        let text = r#"
            ~ ! $ % ^ & && * ( ) - = + | || : , < > <= >= << >> / ?
        "#;
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Tilde))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Bang))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Dollar))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Mod))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Caret))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Ampersand))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::DoubleAmpersand))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Star))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::ParenOpen))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::ParenClose))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Minus))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Equal))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Plus))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Pipe))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::DoublePipe))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Colon))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Comma))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::LessThan))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::GreaterThan))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::LessEqual))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::GreaterEqual))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::ShiftLeft))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::ShiftRight))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Div))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Symbol(Symbol::Question))))
        ));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn regnames() {
        let text = r#"
            a b c d e h l af bc de hl ix iy ixl ixh iyl iyh sp pc af'
        "#;
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::A))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::B))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::C))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::D))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::E))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::H))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::L))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::AF))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::BC))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::DE))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::HL))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::IX))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::IY))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::IXL))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::IXH))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::IYL))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::IYH))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::SP))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::PC))))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Register(RegisterName::AFPrime))))
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
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Label(LabelType::Global, s)))) if s == "global_label"
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Label(LabelType::Local, s)))) if s == ".local_label"
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok((_, Token::Label(LabelType::Direct, s)))) if s == "direct.label"
        ));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn sanity_test() {
        let text = r#"
            org $0000
            
            ; comment
            add a, b
        "#;
        let mut lexer = Lexer::new(FILE, Cursor::new(text));
        while let Some(elem) = lexer.next() {
            assert!(matches!(elem, Ok(elem)));
        }
    }
}
