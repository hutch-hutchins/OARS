/// Source position (1-based line and column).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub col: u32,
}

/// Every token the assembler can encounter in a .s file.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A label definition: `foo:`
    Label(String),
    /// An assembler directive: `.text`, `.data`, `.word`
    Directive(String),
    /// An identifier (opcode, pseudo-op, register name, or label reference).
    /// The parser distinguishes registers from opcodes from references.
    Ident(String),
    /// An integer literal: `42`, `-1`, `0xFF`, `0b1010`
    Integer(i64),
    /// A floating-point literal: `3.14`, `-1.0`
    Float(f64),
    /// A string literal: `"hello\n"`
    StringLit(String),
    /// A character literal: `'A'`
    CharLit(u8),
    Comma,
    LParen,
    RParen,
    /// End of a logical line (newline or \r\n; blank lines emit one token)
    Newline,
    Eof,
}

/// A token together with where it appeared in the source.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub span: Span,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LexError {
    #[error("{line}:{col}: unexpected character {ch:?}")]
    UnexpectedChar { line: u32, col: u32, ch: char },

    #[error("{line}:{col}: unterminated string literal")]
    UnterminatedString { line: u32, col: u32 },

    #[error("{line}:{col}: invalid integer literal {src:?}")]
    BadInteger { line: u32, col: u32, src: String },
}

// ─── Lexer ────────────────────────────────────────────────────────────────────

pub struct Lexer<'src> {
    src: &'src str,
    pos: usize,
    line: u32,
    col: u32,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self {
            src,
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    /// Tokenize an entire source string.
    pub fn tokenize(src: &str) -> Result<Vec<Spanned>, LexError> {
        Lexer::new(src).collect_tokens()
    }

    fn collect_tokens(&mut self) -> Result<Vec<Spanned>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_horizontal_whitespace();

            if self.is_at_end() {
                tokens.push(self.spanned(Token::Eof));
                break;
            }

            match self.peek() {
                '\n' => {
                    tokens.push(self.spanned(Token::Newline));
                    self.bump_newline();
                }
                '\r' => {
                    self.advance();
                    if !self.is_at_end() && self.peek() == '\n' {
                        self.advance();
                    }
                    tokens.push(self.spanned(Token::Newline));
                    self.newline();
                }
                '#' | ';' => self.skip_line_comment(),
                ',' => {
                    tokens.push(self.spanned(Token::Comma));
                    self.advance();
                }
                '(' => {
                    tokens.push(self.spanned(Token::LParen));
                    self.advance();
                }
                ')' => {
                    tokens.push(self.spanned(Token::RParen));
                    self.advance();
                }
                '"' => tokens.push(self.lex_string()?),
                '\'' => tokens.push(self.lex_char()?),
                '.' => tokens.push(self.lex_directive()),
                '-' => tokens.push(self.lex_number()?),
                '0'..='9' => tokens.push(self.lex_number()?),
                c if is_ident_start(c) => tokens.push(self.lex_ident_or_label()),
                c => {
                    return Err(LexError::UnexpectedChar {
                        line: self.line,
                        col: self.col,
                        ch: c,
                    });
                }
            }
        }
        Ok(tokens)
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn peek(&self) -> char {
        self.src[self.pos..].chars().next().unwrap_or('\0')
    }

    fn peek2(&self) -> char {
        let mut chars = self.src[self.pos..].chars();
        chars.next();
        chars.next().unwrap_or('\0')
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            let ch = self.peek();
            self.pos += ch.len_utf8();
            self.col += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn spanned(&self, token: Token) -> Spanned {
        Spanned {
            token,
            span: Span {
                line: self.line,
                col: self.col,
            },
        }
    }

    fn bump_newline(&mut self) {
        self.advance();
        self.newline();
    }

    fn newline(&mut self) {
        self.line += 1;
        self.col = 1;
    }

    fn skip_horizontal_whitespace(&mut self) {
        while !self.is_at_end() && matches!(self.peek(), ' ' | '\t') {
            self.advance();
        }
    }

    fn skip_line_comment(&mut self) {
        while !self.is_at_end() && self.peek() != '\n' {
            self.advance();
        }
    }

    fn read_while(&mut self, pred: impl Fn(char) -> bool) -> &str {
        let start = self.pos;
        while !self.is_at_end() && pred(self.peek()) {
            self.advance();
        }
        &self.src[start..self.pos]
    }

    // ── Token constructors ────────────────────────────────────────────────────

    fn lex_directive(&mut self) -> Spanned {
        let span = Span {
            line: self.line,
            col: self.col,
        };
        self.advance(); // consume '.'
        let name = self.read_while(is_ident_continue).to_owned();
        Spanned {
            token: Token::Directive(name),
            span,
        }
    }

    fn lex_ident_or_label(&mut self) -> Spanned {
        let span = Span {
            line: self.line,
            col: self.col,
        };
        let name = self.read_while(is_ident_continue).to_owned();
        if !self.is_at_end() && self.peek() == ':' {
            self.advance(); // consume ':'
            Spanned {
                token: Token::Label(name),
                span,
            }
        } else {
            Spanned {
                token: Token::Ident(name),
                span,
            }
        }
    }

    fn lex_number(&mut self) -> Result<Spanned, LexError> {
        let span = Span {
            line: self.line,
            col: self.col,
        };
        let start = self.pos;

        // Optional leading minus
        if self.peek() == '-' {
            self.advance();
        }

        // Hex / binary prefix detection
        if !self.is_at_end() && self.peek() == '0' {
            match self.peek2() {
                'x' | 'X' => {
                    self.advance(); // '0'
                    self.advance(); // 'x'
                    self.read_while(|c| c.is_ascii_hexdigit());
                    let raw = &self.src[start..self.pos];
                    return parse_prefixed_int(raw, 16, &span);
                }
                'b' | 'B' => {
                    self.advance(); // '0'
                    self.advance(); // 'b'
                    self.read_while(|c| c == '0' || c == '1');
                    let raw = &self.src[start..self.pos];
                    return parse_prefixed_int(raw, 2, &span);
                }
                _ => {}
            }
        }

        // Decimal integer or float
        self.read_while(|c| c.is_ascii_digit());
        if !self.is_at_end() && self.peek() == '.' && self.peek2().is_ascii_digit() {
            self.advance(); // '.'
            self.read_while(|c| c.is_ascii_digit());
            let raw = &self.src[start..self.pos];
            let val: f64 = raw.parse().map_err(|_| LexError::BadInteger {
                line: span.line,
                col: span.col,
                src: raw.to_owned(),
            })?;
            return Ok(Spanned {
                token: Token::Float(val),
                span,
            });
        }

        let raw = &self.src[start..self.pos];
        let val: i64 = raw.parse().map_err(|_| LexError::BadInteger {
            line: span.line,
            col: span.col,
            src: raw.to_owned(),
        })?;
        Ok(Spanned {
            token: Token::Integer(val),
            span,
        })
    }

    fn lex_string(&mut self) -> Result<Spanned, LexError> {
        let span = Span {
            line: self.line,
            col: self.col,
        };
        self.advance(); // consume opening '"'
        let mut s = String::new();
        loop {
            if self.is_at_end() || self.peek() == '\n' {
                return Err(LexError::UnterminatedString {
                    line: span.line,
                    col: span.col,
                });
            }
            let c = self.peek();
            self.advance();
            if c == '"' {
                break;
            }
            if c == '\\' {
                let esc = self.peek();
                self.advance();
                match esc {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    'r' => s.push('\r'),
                    '0' => s.push('\0'),
                    '\\' => s.push('\\'),
                    '"' => s.push('"'),
                    other => {
                        s.push('\\');
                        s.push(other);
                    }
                }
            } else {
                s.push(c);
            }
        }
        Ok(Spanned {
            token: Token::StringLit(s),
            span,
        })
    }

    fn lex_char(&mut self) -> Result<Spanned, LexError> {
        let span = Span {
            line: self.line,
            col: self.col,
        };
        self.advance(); // consume opening '\''
        let c = self.peek();
        self.advance();
        let byte = if c == '\\' {
            let esc = self.peek();
            self.advance();
            match esc {
                'n' => b'\n',
                't' => b'\t',
                'r' => b'\r',
                '0' => b'\0',
                '\\' => b'\\',
                '\'' => b'\'',
                other => other as u8,
            }
        } else {
            c as u8
        };
        if !self.is_at_end() && self.peek() == '\'' {
            self.advance();
        }
        Ok(Spanned {
            token: Token::CharLit(byte),
            span,
        })
    }
}

// ─── Character classification ─────────────────────────────────────────────────

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.'
}

// ─── Numeric helpers ──────────────────────────────────────────────────────────

fn parse_prefixed_int(raw: &str, radix: u32, span: &Span) -> Result<Spanned, LexError> {
    let (neg, digits) = if raw.starts_with('-') {
        (true, &raw[3..]) // skip "-0x" or "-0b"
    } else {
        (false, &raw[2..]) // skip "0x" or "0b"
    };
    let val = i64::from_str_radix(digits, radix).map_err(|_| LexError::BadInteger {
        line: span.line,
        col: span.col,
        src: raw.to_owned(),
    })?;
    Ok(Spanned {
        token: Token::Integer(if neg { -val } else { val }),
        span: span.clone(),
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(src: &str) -> Vec<Token> {
        Lexer::tokenize(src)
            .expect("lex failed")
            .into_iter()
            .map(|s| s.token)
            .filter(|t| !matches!(t, Token::Newline | Token::Eof))
            .collect()
    }

    fn token_kinds(src: &str) -> Result<Vec<Token>, LexError> {
        Ok(Lexer::tokenize(src)?
            .into_iter()
            .map(|s| s.token)
            .filter(|t| !matches!(t, Token::Newline | Token::Eof))
            .collect())
    }

    // ── Directives ──────────────────────────────────────────────────────────

    #[test]
    fn directive_text() {
        assert_eq!(tokens(".text"), vec![Token::Directive("text".into())]);
    }

    #[test]
    fn directive_data() {
        assert_eq!(tokens(".data"), vec![Token::Directive("data".into())]);
    }

    #[test]
    fn directive_word() {
        assert_eq!(
            tokens(".word 42"),
            vec![Token::Directive("word".into()), Token::Integer(42)]
        );
    }

    // ── Labels ──────────────────────────────────────────────────────────────

    #[test]
    fn label_simple() {
        assert_eq!(tokens("main:"), vec![Token::Label("main".into())]);
    }

    #[test]
    fn label_followed_by_opcode() {
        assert_eq!(
            tokens("loop: addi"),
            vec![Token::Label("loop".into()), Token::Ident("addi".into())]
        );
    }

    // ── Identifiers / opcodes ────────────────────────────────────────────────

    #[test]
    fn ident_opcode() {
        assert_eq!(tokens("add"), vec![Token::Ident("add".into())]);
    }

    #[test]
    fn ident_register() {
        // Registers are just identifiers at the lex stage; parser resolves them
        assert_eq!(tokens("a0"), vec![Token::Ident("a0".into())]);
        assert_eq!(tokens("zero"), vec![Token::Ident("zero".into())]);
        assert_eq!(tokens("x31"), vec![Token::Ident("x31".into())]);
    }

    // ── Integer literals ─────────────────────────────────────────────────────

    #[test]
    fn integer_decimal() {
        assert_eq!(tokens("42"), vec![Token::Integer(42)]);
    }

    #[test]
    fn integer_negative() {
        assert_eq!(tokens("-1"), vec![Token::Integer(-1)]);
    }

    #[test]
    fn integer_hex() {
        assert_eq!(tokens("0xFF"), vec![Token::Integer(255)]);
        assert_eq!(tokens("0x10"), vec![Token::Integer(16)]);
    }

    #[test]
    fn integer_binary() {
        assert_eq!(tokens("0b1010"), vec![Token::Integer(10)]);
    }

    // ── Float literals ───────────────────────────────────────────────────────

    #[test]
    fn float_literal() {
        let toks = tokens("2.5");
        assert!(matches!(toks[0], Token::Float(f) if (f - 2.5).abs() < 1e-10));
    }

    // ── String literals ──────────────────────────────────────────────────────

    #[test]
    fn string_simple() {
        assert_eq!(tokens(r#""hello""#), vec![Token::StringLit("hello".into())]);
    }

    #[test]
    fn string_escape_newline() {
        assert_eq!(
            tokens(r#""hello\nworld""#),
            vec![Token::StringLit("hello\nworld".into())]
        );
    }

    #[test]
    fn string_unterminated() {
        let err = Lexer::tokenize("\"oops").unwrap_err();
        assert!(matches!(err, LexError::UnterminatedString { .. }));
    }

    // ── Character literals ───────────────────────────────────────────────────

    #[test]
    fn char_literal() {
        assert_eq!(tokens("'A'"), vec![Token::CharLit(b'A')]);
    }

    #[test]
    fn char_escape_newline() {
        assert_eq!(tokens(r"'\n'"), vec![Token::CharLit(b'\n')]);
    }

    // ── Punctuation ──────────────────────────────────────────────────────────

    #[test]
    fn comma() {
        assert_eq!(tokens(","), vec![Token::Comma]);
    }

    #[test]
    fn parens() {
        assert_eq!(tokens("()"), vec![Token::LParen, Token::RParen]);
    }

    // ── Comments ─────────────────────────────────────────────────────────────

    #[test]
    fn comment_hash_ignored() {
        assert_eq!(
            tokens("add # this is a comment"),
            vec![Token::Ident("add".into())]
        );
    }

    #[test]
    fn comment_semicolon_ignored() {
        assert_eq!(
            tokens("add ; semicolon comment"),
            vec![Token::Ident("add".into())]
        );
    }

    // ── Full instruction lines ────────────────────────────────────────────────

    #[test]
    fn addi_instruction() {
        assert_eq!(
            tokens("addi a0, a1, -4"),
            vec![
                Token::Ident("addi".into()),
                Token::Ident("a0".into()),
                Token::Comma,
                Token::Ident("a1".into()),
                Token::Comma,
                Token::Integer(-4),
            ]
        );
    }

    #[test]
    fn load_with_offset() {
        // lw a0, 0(sp)
        assert_eq!(
            tokens("lw a0, 0(sp)"),
            vec![
                Token::Ident("lw".into()),
                Token::Ident("a0".into()),
                Token::Comma,
                Token::Integer(0),
                Token::LParen,
                Token::Ident("sp".into()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn newline_emitted() {
        let toks: Vec<_> = Lexer::tokenize("add\nsub")
            .unwrap()
            .into_iter()
            .map(|s| s.token)
            .collect();
        assert!(toks.contains(&Token::Newline));
    }

    #[test]
    fn span_tracks_line_and_col() {
        let spanned = Lexer::tokenize("add\nsub").unwrap();
        let sub = spanned
            .iter()
            .find(|s| s.token == Token::Ident("sub".into()))
            .unwrap();
        assert_eq!(sub.span.line, 2);
        assert_eq!(sub.span.col, 1);
    }

    // ── Error cases ──────────────────────────────────────────────────────────

    #[test]
    fn unexpected_char_at() {
        let err = Lexer::tokenize("@foo").unwrap_err();
        assert!(matches!(err, LexError::UnexpectedChar { ch: '@', .. }));
    }

    #[test]
    fn empty_input() {
        let toks = token_kinds("").unwrap();
        assert!(toks.is_empty());
    }
}
