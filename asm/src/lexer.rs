use std::{
    cell::{Ref, RefCell, RefMut},
    fmt::{self, Display, Formatter},
    io::Read,
    rc::Rc,
};

use crate::{
    charreader::{CharReader, CharReaderError},
    intern::{PathRef, StrRef},
    StrInterner,
};

#[derive(Copy, Clone, Debug)]
pub struct SourceLoc {
    pub pathref: PathRef,
    pub line: usize,
    pub column: usize,
}

#[derive(thiserror::Error, Debug)]
pub enum LexerError {
    #[error("read error: {source}")]
    ReadError {
        loc: SourceLoc,
        source: CharReaderError,
    },

    #[error("unexpected line break")]
    UnexpectedLineBreak { loc: SourceLoc },

    #[error("unrecognized string escape: `{msg}`")]
    UnrecognizedStringEscape { loc: SourceLoc, msg: String },

    #[error("malformed binary number: `{msg}`")]
    MalformedBinaryNumber { loc: SourceLoc, msg: String },

    #[error("malformed decimal number: `{msg}`")]
    MalformedDecimalNumber { loc: SourceLoc, msg: String },

    #[error("malformed hexadecimal number: `{msg}`")]
    MalformedHexidecimalNumber { loc: SourceLoc, msg: String },

    #[error("unrecognized input: `{msg}`")]
    UnrecognizedInput { loc: SourceLoc, msg: String },

    #[error("unknown directive: `{msg}`")]
    UnknownDirective { loc: SourceLoc, msg: String },

    #[error("malformed label: `{msg}`")]
    MalformedLabel { loc: SourceLoc, msg: String },
}

impl LexerError {
    #[inline]
    pub fn loc(&self) -> SourceLoc {
        match self {
            Self::ReadError { loc, .. } => *loc,
            Self::UnexpectedLineBreak { loc } => *loc,
            Self::UnrecognizedStringEscape { loc, .. } => *loc,
            Self::MalformedBinaryNumber { loc, .. } => *loc,
            Self::MalformedDecimalNumber { loc, .. } => *loc,
            Self::MalformedHexidecimalNumber { loc, .. } => *loc,
            Self::UnrecognizedInput { loc, .. } => *loc,
            Self::UnknownDirective { loc, .. } => *loc,
            Self::MalformedLabel { loc, .. } => *loc,
        }
    }
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
    Cpd,
    Cpdr,
    Cpi,
    Cpir,
    Cpl,
    Daa,
    Dec,
    Di,
    Djnz,
    Ei,
    Ex,
    Exx,
    Halt,
    Im,
    In,
    Inc,
    Ind,
    Indr,
    Ini,
    Inir,
    Jp,
    Jr,
    Ld,
    Ldd,
    Lddr,
    Ldi,
    Ldir,
    Neg,
    Nop,
    Or,
    Otdr,
    Otir,
    Out,
    Outd,
    Outi,
    Pop,
    Push,
    Res,
    Ret,
    Reti,
    Retn,
    Rl,
    Rla,
    Rlc,
    Rlca,
    Rr,
    Rra,
    Rrc,
    Rrca,
    Rst,
    Sbc,
    Scf,
    Set,
    Sla,
    Sll,
    Sra,
    Srl,
    Sub,
    Xor,
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
            "cpd" | "CPD" => Some(Self::Cpd),
            "cpdr" | "CPDR" => Some(Self::Cpdr),
            "cpi" | "CPI" => Some(Self::Cpi),
            "cpir" | "CPIR" => Some(Self::Cpir),
            "cpl" | "CPL" => Some(Self::Cpl),
            "daa" | "DAA" => Some(Self::Daa),
            "dec" | "DEC" => Some(Self::Dec),
            "di" | "DI" => Some(Self::Di),
            "djnz" | "DJNZ" => Some(Self::Djnz),
            "ei" | "EI" => Some(Self::Ei),
            "ex" | "EX" => Some(Self::Ex),
            "exx" | "EXX" => Some(Self::Exx),
            "halt" | "HALT" => Some(Self::Halt),
            "im" | "IM" => Some(Self::Im),
            "in" | "IN" => Some(Self::In),
            "inc" | "INC" => Some(Self::Inc),
            "ind" | "IND" => Some(Self::Ind),
            "indr" | "INDR" => Some(Self::Indr),
            "ini" | "INI" => Some(Self::Ini),
            "inir" | "INIR" => Some(Self::Inir),
            "jp" | "JP" => Some(Self::Jp),
            "jr" | "JR" => Some(Self::Jr),
            "ld" | "LD" => Some(Self::Ld),
            "ldd" | "LDD" => Some(Self::Ldd),
            "lddr" | "LDDR" => Some(Self::Lddr),
            "ldi" | "LDI" => Some(Self::Ldi),
            "ldir" | "LDIR" => Some(Self::Ldir),
            "neg" | "NEG" => Some(Self::Neg),
            "nop" | "NOP" => Some(Self::Nop),
            "or" | "OR" => Some(Self::Or),
            "otdr" | "OTDR" => Some(Self::Otdr),
            "otir" | "OTIR" => Some(Self::Otir),
            "out" | "OUT" => Some(Self::Out),
            "outd" | "OUTD" => Some(Self::Outd),
            "outi" | "OUTI" => Some(Self::Outi),
            "pop" | "POP" => Some(Self::Pop),
            "push" | "PUSH" => Some(Self::Push),
            "res" | "RES" => Some(Self::Res),
            "ret" | "RET" => Some(Self::Ret),
            "reti" | "RETI" => Some(Self::Reti),
            "retn" | "RETN" => Some(Self::Retn),
            "rl" | "RL" => Some(Self::Rl),
            "rla" | "RLA" => Some(Self::Rla),
            "rlc" | "RLC" => Some(Self::Rlc),
            "rlca" | "RLCA" => Some(Self::Rlca),
            "rr" | "RR" => Some(Self::Rr),
            "rra" | "RRA" => Some(Self::Rra),
            "rrc" | "RRC" => Some(Self::Rrc),
            "rrca" | "RRCA" => Some(Self::Rrca),
            "rst" | "RST" => Some(Self::Rst),
            "sbc" | "SBC" => Some(Self::Sbc),
            "scf" | "SCF" => Some(Self::Scf),
            "set" | "SET" => Some(Self::Set),
            "sla" | "SLA" => Some(Self::Sla),
            "sll" | "SLL" => Some(Self::Sll),
            "sra" | "SRA" => Some(Self::Sra),
            "srl" | "SRL" => Some(Self::Srl),
            "sub" | "SUB" => Some(Self::Sub),
            "xor" | "XOR" => Some(Self::Xor),
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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
    I,
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
            "i" | "I" => Some(Self::I),
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
                Self::I => "i",
            }
        )
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SymbolName {
    Tilde,
    Bang,
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
    ShiftLeftLogical,
    ShiftRightLogical,
    Div,
    Question,
}

impl SymbolName {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "~" => Some(Self::Tilde),
            "!" => Some(Self::Bang),
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
            "<:" => Some(Self::ShiftLeftLogical),
            ":>" => Some(Self::ShiftRightLogical),
            "/" => Some(Self::Div),
            "?" => Some(Self::Question),
            _ => None,
        }
    }
}

impl Display for SymbolName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Tilde => "~",
                Self::Bang => "!",
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
                Self::ShiftLeftLogical => "<:",
                Self::ShiftRightLogical => ":>",
                Self::Div => "/",
                Self::Question => "?",
            }
        )
    }
}

#[derive(Debug, Copy, Clone)]
pub enum FlagName {
    Zero,
    NotZero,
    NotCarry,
    ParityEven,
    ParityOdd,
    Positive,
    Negative,
}

impl FlagName {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "z" | "Z" => Some(Self::Zero),
            "nz" | "NZ" => Some(Self::NotZero),
            "nc" | "NC" => Some(Self::NotCarry),
            "pe" | "PE" => Some(Self::ParityEven),
            "po" | "PO" => Some(Self::ParityOdd),
            "p" | "P" => Some(Self::Positive),
            "m" | "M" => Some(Self::Negative),
            _ => None,
        }
    }
}

impl Display for FlagName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Zero => "zero flag",
                Self::NotZero => "not zero flag",
                Self::NotCarry => "not carry flag",
                Self::ParityEven => "parity even flag",
                Self::ParityOdd => "parity odd flag",
                Self::Positive => "positive flag",
                Self::Negative => "negative/minus flag",
            }
        )
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DirectiveName {
    Org,
    Here,
    Macro,
    Enum,
    Struct,
    Symbol,
    If,
    Ifdef,
    Ifndef,
    Else,
    End,
    Echo,
    Die,
    Db,
    Dw,
    Ds,
    Include,
    Incbin,
}

impl DirectiveName {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "@org" | "@ORG" => Some(Self::Org),
            "@here" | "@HERE" => Some(Self::Here),
            "@macro" | "@MACRO" => Some(Self::Macro),
            "@enum" | "@ENUM" => Some(Self::Enum),
            "@struct" | "@STRUCT" => Some(Self::Struct),
            "@def" | "@DEF" => Some(Self::Symbol),
            "@if" | "@IF" => Some(Self::If),
            "@ifdef" | "@IFDEF" => Some(Self::Ifdef),
            "@ifndef" | "@IFNDEF" => Some(Self::Ifndef),
            "@else" | "@ELSE" => Some(Self::Else),
            "@end" | "@END" => Some(Self::End),
            "@echo" | "@ECHO" => Some(Self::Echo),
            "@die" | "@DIE" => Some(Self::Die),
            "@db" | "@DB" => Some(Self::Db),
            "@dw" | "@DW" => Some(Self::Dw),
            "@ds" | "@DS" => Some(Self::Ds),
            "@include" | "@INCLUDE" => Some(Self::Include),
            "@incbin" | "@INCBIN" => Some(Self::Incbin),
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
                Self::Org => "@org",
                Self::Here => "@here",
                Self::Macro => "@macro",
                Self::Enum => "@enum",
                Self::Struct => "@struct",
                Self::Symbol => "@def",
                Self::If => "@if",
                Self::Ifdef => "@ifdef",
                Self::Ifndef => "@ifndef",
                Self::Else => "@else",
                Self::End => "@end",
                Self::Echo => "@echo",
                Self::Die => "@die",
                Self::Db => "@db",
                Self::Dw => "@dw",
                Self::Ds => "@ds",
                Self::Include => "@include",
                Self::Incbin => "@incbin",
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
pub enum LabelKind {
    Global,
    Local,
    Direct,
}

#[derive(Debug, Copy, Clone)]
pub enum Token {
    Comment {
        loc: SourceLoc,
    },
    NewLine {
        loc: SourceLoc,
    },
    String {
        loc: SourceLoc,
        value: StrRef,
    },
    Number {
        loc: SourceLoc,
        value: u32,
    },
    Operation {
        loc: SourceLoc,
        name: OperationName,
    },
    Directive {
        loc: SourceLoc,
        name: DirectiveName,
    },
    Register {
        loc: SourceLoc,
        name: RegisterName,
    },
    Flag {
        loc: SourceLoc,
        name: FlagName,
    },
    Symbol {
        loc: SourceLoc,
        name: SymbolName,
    },
    Label {
        loc: SourceLoc,
        kind: LabelKind,
        value: StrRef,
    },
}

impl Token {
    #[inline]
    pub fn loc(&self) -> SourceLoc {
        match self {
            Self::NewLine { loc, .. } => *loc,
            Self::Comment { loc, .. } => *loc,
            Self::String { loc, .. } => *loc,
            Self::Number { loc, .. } => *loc,
            Self::Operation { loc, .. } => *loc,
            Self::Directive { loc, .. } => *loc,
            Self::Register { loc, .. } => *loc,
            Self::Flag { loc, .. } => *loc,
            Self::Symbol { loc, .. } => *loc,
            Self::Label { loc, .. } => *loc,
        }
    }

    #[inline]
    pub fn as_display<'a>(
        &'a self,
        str_interner: &'a Rc<RefCell<StrInterner>>,
    ) -> DisplayToken<'a> {
        DisplayToken {
            inner: self,
            str_interner,
        }
    }
}

pub struct DisplayToken<'a> {
    inner: &'a Token,
    str_interner: &'a Rc<RefCell<StrInterner>>,
}

impl<'a> Display for DisplayToken<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self {
            inner,
            str_interner,
        } = *self;
        match *inner {
            Token::NewLine { .. } => write!(f, "line break"),
            Token::Comment { .. } => write!(f, "comment"),
            Token::String { value, .. } => {
                let str_interner = str_interner.as_ref().borrow();
                let value = str_interner.get(value).unwrap();
                write!(f, "string: \"{value}\"")
            }
            Token::Number { value, .. } => write!(f, "number: {value}"),
            Token::Operation { name, .. } => write!(f, "operation: \"{name}\""),
            Token::Directive { name, .. } => write!(f, "directive: \"{name}\""),
            Token::Register { name, .. } => write!(f, "register: \"{name}\""),
            Token::Flag { name, .. } => write!(f, "{name}"),
            Token::Symbol { name, .. } => write!(f, "symbol: \"{name}\""),
            Token::Label { value, .. } => {
                let str_interner = str_interner.as_ref().borrow();
                let value = str_interner.get(value).unwrap();
                write!(f, "label: \"{value}\"")
            }
        }
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
    pub fn new(str_interner: Rc<RefCell<StrInterner>>, pathref: PathRef, reader: R) -> Self {
        let loc = SourceLoc {
            pathref,
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
    pub fn loc(&self) -> SourceLoc {
        self.loc
    }

    #[inline]
    pub fn included_from(&self) -> Option<SourceLoc> {
        self.included_from
    }

    #[inline]
    fn str_interner(&self) -> Ref<StrInterner> {
        self.str_interner.as_ref().borrow()
    }

    #[inline]
    fn str_interner_mut(&self) -> RefMut<StrInterner> {
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
                | '@'
                | ')'
                | '\n'
        )
    }

    #[inline]
    fn is_symbol_start(&self, c: char) -> bool {
        matches!(
            c,
            '~' | '!'
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
    type Item = Result<Token, LexerError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let c = match self.stash.take() {
                Some(c) => c,
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
                    '\n' => return Some(Ok(Token::NewLine { loc: self.loc })),

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

                    '@' => {
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
                        return Some(Err(LexerError::UnrecognizedInput {
                            loc: self.loc,
                            msg: format!("{c}"),
                        }));
                    }
                },

                State::InComment => match c {
                    '\n' => {
                        self.state = State::Initial;
                        self.stash = Some(c);
                        return Some(Ok(Token::Comment { loc: self.tok_loc }));
                    }

                    _ => {}
                },

                State::InString => match c {
                    '\n' => {
                        return Some(Err(LexerError::UnexpectedLineBreak { loc: self.loc }));
                    }

                    '"' => {
                        self.state = State::Initial;
                        let value = self.str_interner.borrow_mut().intern(&self.buffer);
                        return Some(Ok(Token::String {
                            loc: self.tok_loc,
                            value,
                        }));
                    }

                    '\\' => {
                        self.state = State::InStringEscape;
                    }

                    _ => self.buffer.push(c),
                },

                State::InStringEscape => match c {
                    '\n' => {
                        self.state = State::InString;
                    }

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
                            msg: format!("\\{c}"),
                        }));
                    }
                },

                State::InHexStringEscape1 => match c {
                    '\n' => {
                        return Some(Err(LexerError::UnexpectedLineBreak { loc: self.loc }));
                    }

                    '0'..='9' | 'a'..='f' | 'A'..='F' => {
                        self.state = State::InHexStringEscape2;
                        self.buffer.push(c.to_ascii_lowercase());
                    }

                    _ => {
                        return Some(Err(LexerError::UnrecognizedStringEscape {
                            loc: self.loc,
                            msg: format!("\\${c}"),
                        }));
                    }
                },

                State::InHexStringEscape2 => match c {
                    '\n' => {
                        return Some(Err(LexerError::UnexpectedLineBreak { loc: self.loc }));
                    }

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
                            loc: self.tok_loc,
                            msg: format!("\\${}{}", self.buffer.chars().last().unwrap(), c),
                        }));
                    }
                },

                State::InNumberBase2 => match c {
                    _ if c.is_whitespace() || self.is_value_terminator(c) => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        // Instead of treating this as a binary number, this must
                        // be the mod symbol
                        if self.buffer.is_empty() {
                            return Some(Ok(Token::Symbol {
                                loc: self.tok_loc,
                                name: SymbolName::Mod,
                            }));
                        }

                        let value = u32::from_str_radix(&self.buffer, 2).unwrap();
                        return Some(Ok(Token::Number {
                            loc: self.tok_loc,
                            value,
                        }));
                    }

                    '0' | '1' => self.buffer.push(c),

                    _ => {
                        return Some(Err(LexerError::MalformedBinaryNumber {
                            loc: self.tok_loc,
                            msg: format!("%{}{}", self.buffer, c),
                        }));
                    }
                },

                State::InNumberBase10 => match c {
                    _ if c.is_whitespace() || self.is_value_terminator(c) => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        let value = u32::from_str_radix(&self.buffer, 10).unwrap();
                        return Some(Ok(Token::Number {
                            loc: self.tok_loc,
                            value,
                        }));
                    }

                    '0'..='9' => self.buffer.push(c),

                    _ => {
                        return Some(Err(LexerError::MalformedDecimalNumber {
                            loc: self.tok_loc,
                            msg: format!("{}{}", self.buffer, c),
                        }));
                    }
                },

                State::InNumberBase16 => match c {
                    _ if c.is_whitespace() || self.is_value_terminator(c) => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        let value = u32::from_str_radix(&self.buffer, 16).unwrap();
                        return Some(Ok(Token::Number {
                            loc: self.tok_loc,
                            value,
                        }));
                    }

                    '0'..='9' | 'a'..='f' | 'A'..='F' => self.buffer.push(c.to_ascii_lowercase()),

                    _ => {
                        return Some(Err(LexerError::MalformedHexidecimalNumber {
                            loc: self.tok_loc,
                            msg: format!("${}{}", self.buffer, c),
                        }));
                    }
                },

                State::InSymbol => {
                    self.state = State::Initial;
                    self.buffer.push(c);

                    // first try and parse a 2 char symbol
                    if let Some(name) = SymbolName::parse(&self.buffer) {
                        return Some(Ok(Token::Symbol {
                            loc: self.tok_loc,
                            name,
                        }));
                    }

                    // It must be 1 char (stash the other char)
                    self.stash = self.buffer.pop();
                    let name = SymbolName::parse(&self.buffer).unwrap();
                    return Some(Ok(Token::Symbol {
                        loc: self.tok_loc,
                        name,
                    }));
                }

                State::InDirective => match c {
                    _ if c.is_alphanumeric() || c == '_' => {
                        self.buffer.push(c);
                    }

                    _ => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        if let Some(name) = DirectiveName::parse(&self.buffer) {
                            return Some(Ok(Token::Directive {
                                loc: self.tok_loc,
                                name,
                            }));
                        }

                        return Some(Err(LexerError::UnknownDirective {
                            loc: self.tok_loc,
                            msg: self.buffer.clone(),
                        }));
                    }
                },

                State::InIdentifier => match c {
                    _ if c.is_alphanumeric() || c == '_' || c == '.' => {
                        self.buffer.push(c);
                    }

                    '\'' => {
                        self.state = State::Initial;
                        if self.buffer == "af" {
                            return Some(Ok(Token::Register {
                                loc: self.tok_loc,
                                name: RegisterName::AFPrime,
                            }));
                        }
                    }

                    _ => {
                        self.state = State::Initial;
                        self.stash = Some(c);

                        if let Some(name) = OperationName::parse(&self.buffer) {
                            return Some(Ok(Token::Operation {
                                loc: self.tok_loc,
                                name,
                            }));
                        }

                        if let Some(name) = RegisterName::parse(&self.buffer) {
                            return Some(Ok(Token::Register {
                                loc: self.tok_loc,
                                name,
                            }));
                        }

                        if let Some(name) = FlagName::parse(&self.buffer) {
                            return Some(Ok(Token::Flag {
                                loc: self.tok_loc,
                                name,
                            }));
                        }

                        let value = self.str_interner.borrow_mut().intern(&self.buffer);
                        return match self.buffer.chars().filter(|c| *c == '.').count() {
                            0 => Some(Ok(Token::Label {
                                loc: self.tok_loc,
                                kind: LabelKind::Global,
                                value,
                            })),
                            1 => {
                                if self.buffer.starts_with('.') {
                                    Some(Ok(Token::Label {
                                        loc: self.tok_loc,
                                        kind: LabelKind::Local,
                                        value,
                                    }))
                                } else {
                                    Some(Ok(Token::Label {
                                        loc: self.tok_loc,
                                        kind: LabelKind::Direct,
                                        value,
                                    }))
                                }
                            }
                            _ => Some(Err(LexerError::MalformedLabel {
                                loc: self.tok_loc,
                                msg: self.buffer.clone(),
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
    use std::{cell::RefCell, io::Cursor, rc::Rc};

    use super::*;
    use crate::intern::PathInterner;

    fn lexer(text: &str) -> Lexer<Cursor<&str>> {
        let path_interner = Rc::new(RefCell::new(PathInterner::new()));
        let str_interner = Rc::new(RefCell::new(StrInterner::new()));
        let pathref = path_interner.borrow_mut().intern("file.test");
        Lexer::new(str_interner, pathref, Cursor::new(text))
    }

    #[test]
    fn comment() {
        let text = r#"
            ; comment
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(lexer.next(), Some(Ok(Token::Comment { .. }))));
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn string() {
        let text = r#"
            "test"
        "#;
        let mut lexer = lexer(text);
        let string = lexer.str_interner_mut().intern("test");
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::String { value, .. })) if value == string
        ));
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn string_escape() {
        let text = r#"
            "test\n"
        "#;
        let mut lexer = lexer(text);
        let string = lexer.str_interner_mut().intern("test\n");
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::String { value, .. })) if value == string
        ));
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn string_escape_raw() {
        let text = r#"
            "test\$7f"
        "#;
        let mut lexer = lexer(text);
        let string = lexer.str_interner_mut().intern("test\x7f");
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::String { value, .. })) if value == string
        ));
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn base2() {
        let text = r#"
            %010101
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Number {
                value: 0b010101,
                ..
            }))
        ));
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn base10() {
        let text = r#"
            123456
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Number { value: 123456, .. }))
        ));
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn base16() {
        let text = r#"
            $cafebabe
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Number {
                value: 0xcafebabe,
                ..
            }))
        ));
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn symbols() {
        let text = r#"
            ~ ! % ^ & && * ( ) - == + | || : , < > <= >= << >> <: :> / ?
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Tilde,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Bang,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Mod,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Caret,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Ampersand,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::DoubleAmpersand,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Star,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::ParenOpen,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::ParenClose,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Minus,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Equal,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Plus,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Pipe,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::DoublePipe,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Colon,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Comma,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::LessThan,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::GreaterThan,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::LessEqual,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::GreaterEqual,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::ShiftLeft,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::ShiftRight,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::ShiftLeftLogical,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::ShiftRightLogical,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Div,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Symbol {
                name: SymbolName::Question,
                ..
            }))
        ));
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn regnames() {
        let text = r#"
            a b c d e h l af bc de hl ix iy ixl ixh iyl iyh sp pc af' i
        "#;
        let mut lexer = lexer(text);
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::A,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::B,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::C,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::D,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::E,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::H,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::L,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::AF,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::BC,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::DE,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::HL,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::IX,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::IY,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::IXL,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::IXH,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::IYL,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::IYH,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::SP,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::PC,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::AFPrime,
                ..
            }))
        ));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Register {
                name: RegisterName::I,
                ..
            }))
        ));
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
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
        let label = lexer.str_interner_mut().intern("global_label");
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Label {
                kind: LabelKind::Global,
                value,
                ..
            })) if value == label
        ));
        let label = lexer.str_interner_mut().intern(".local_label");
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Label {
                kind: LabelKind::Local,
                value,
                ..
            })) if value == label
        ));
        let label = lexer.str_interner_mut().intern("direct.label");
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(
            lexer.next(),
            Some(Ok(Token::Label {
                kind: LabelKind::Direct,
                value,
                ..
            })) if value == label
        ));
        assert!(matches!(lexer.next(), Some(Ok(Token::NewLine { .. }))));
        assert!(matches!(lexer.next(), None));
    }

    #[test]
    fn sanity_test() {
        let text = r#"
            @org $0000
            
            @macro foo arg1, arg2
                add arg1, arg2
            @end
            
            @echo "hello"
            @die
            
            ; comment
            foo a, b
        "#;
        let mut lexer = lexer(text);
        while let Some(result) = lexer.next() {
            assert!(result.is_ok());
        }
    }
}
