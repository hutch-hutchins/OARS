use crate::assembler::lexer::{LexError, Lexer, Span, Token};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("lex error: {0}")]
    Lex(#[from] LexError),
    #[error("{line}:{col}: {msg}")]
    Parse { line: u32, col: u32, msg: String },
}

impl ParseError {
    fn at(span: &Span, msg: impl Into<String>) -> Self {
        Self::Parse {
            line: span.line,
            col: span.col,
            msg: msg.into(),
        }
    }
}

// ─── AST ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Operand {
    Reg(usize),
    FpReg(usize),
    Imm(i32),
    Label(String),
    MemOff(i32, usize), // offset(base_reg)
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub mnemonic: String,
    pub ops: Vec<Operand>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DataItem {
    Byte(i8),
    Half(i16),
    Word(i32),
    Dword(i64),
    Float(f32),
    Double(f64),
    String(String), // null-terminated (.string / .asciiz)
    Ascii(String),  // no null terminator (.ascii)
    Space(u32),
    Align(u32),
    // Multi-value variants (e.g. .word 1, 2, 3)
    Words(Vec<i32>),
    Halfs(Vec<i16>),
    Bytes(Vec<i8>),
    Dwords(Vec<i64>),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Statement {
    Label(String, Span),
    Instr(Instruction),
    Data(DataItem, Span),
    Segment(Seg, Span),
    Globl(String),
    Equ(String, i32),
    /// Resolved by `assembler::include::resolve` before codegen; never reaches the assembler.
    Include(std::path::PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Text,
    Data,
}

// ─── Parser ──────────────────────────────────────────────────────────────────

pub fn parse(src: &str) -> Result<Vec<Statement>, ParseError> {
    let tokens = Lexer::tokenize(src)?;
    let mut p = Parser { tokens, pos: 0 };
    p.parse_program()
}

struct Parser {
    tokens: Vec<crate::assembler::lexer::Spanned>,
    pos: usize,
}

impl Parser {
    fn peek_token(&self) -> &Token {
        &self.tokens[self.pos].token
    }

    fn peek_span(&self) -> &Span {
        &self.tokens[self.pos].span
    }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos].token;
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek_token(), Token::Newline) {
            self.advance();
        }
    }

    fn expect_newline_or_eof(&mut self) -> Result<(), ParseError> {
        match self.peek_token() {
            Token::Newline | Token::Eof => {
                self.advance();
                Ok(())
            }
            _ => Err(ParseError::at(self.peek_span(), "expected end of line")),
        }
    }

    fn parse_program(&mut self) -> Result<Vec<Statement>, ParseError> {
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek_token(), Token::Eof) {
                break;
            }
            if let Some(s) = self.parse_statement()? {
                stmts.push(s);
            }
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Option<Statement>, ParseError> {
        let span = self.peek_span().clone();
        match self.peek_token().clone() {
            Token::Label(name) => {
                self.advance();
                Ok(Some(Statement::Label(name, span)))
            }

            Token::Directive(dir) => {
                self.advance();
                let stmt = self.parse_directive(&dir, &span)?;
                Ok(Some(stmt))
            }

            Token::Ident(mnemonic) => {
                self.advance();
                let ops = self.parse_operands()?;
                self.expect_newline_or_eof()?;
                Ok(Some(Statement::Instr(Instruction {
                    mnemonic,
                    ops,
                    span,
                })))
            }

            Token::Newline | Token::Eof => Ok(None),

            tok => Err(ParseError::at(&span, format!("unexpected token {tok:?}"))),
        }
    }

    fn parse_directive(&mut self, dir: &str, span: &Span) -> Result<Statement, ParseError> {
        Ok(match dir {
            "text" => {
                self.expect_newline_or_eof()?;
                Statement::Segment(Seg::Text, span.clone())
            }
            "data" => {
                self.expect_newline_or_eof()?;
                Statement::Segment(Seg::Data, span.clone())
            }

            "globl" | "global" => {
                let name = self.expect_ident()?;
                self.expect_newline_or_eof()?;
                Statement::Globl(name)
            }

            "word" => {
                let vals = self.parse_int_list()?;
                self.expect_newline_or_eof()?;
                if vals.len() == 1 {
                    Statement::Data(DataItem::Word(vals[0] as i32), span.clone())
                } else {
                    Statement::Data(
                        DataItem::Words(vals.iter().map(|v| *v as i32).collect()),
                        span.clone(),
                    )
                }
            }

            "byte" => {
                let vals = self.parse_int_list()?;
                self.expect_newline_or_eof()?;
                if vals.len() == 1 {
                    Statement::Data(DataItem::Byte(vals[0] as i8), span.clone())
                } else {
                    Statement::Data(
                        DataItem::Bytes(vals.iter().map(|v| *v as i8).collect()),
                        span.clone(),
                    )
                }
            }

            "half" | "short" => {
                let vals = self.parse_int_list()?;
                self.expect_newline_or_eof()?;
                if vals.len() == 1 {
                    Statement::Data(DataItem::Half(vals[0] as i16), span.clone())
                } else {
                    Statement::Data(
                        DataItem::Halfs(vals.iter().map(|v| *v as i16).collect()),
                        span.clone(),
                    )
                }
            }

            "float" => {
                let v = self.expect_float()?;
                self.expect_newline_or_eof()?;
                Statement::Data(DataItem::Float(v as f32), span.clone())
            }

            "double" => {
                let v = self.expect_float()?;
                self.expect_newline_or_eof()?;
                Statement::Data(DataItem::Double(v), span.clone())
            }

            "string" | "asciiz" | "asciz" => {
                let s = self.expect_string()?;
                self.expect_newline_or_eof()?;
                Statement::Data(DataItem::String(s), span.clone())
            }

            "ascii" => {
                let s = self.expect_string()?;
                self.expect_newline_or_eof()?;
                Statement::Data(DataItem::Ascii(s), span.clone())
            }

            "space" => {
                let n = self.expect_int()? as u32;
                self.expect_newline_or_eof()?;
                Statement::Data(DataItem::Space(n), span.clone())
            }

            "align" => {
                let n = self.expect_int()? as u32;
                self.expect_newline_or_eof()?;
                Statement::Data(DataItem::Align(n), span.clone())
            }

            "dword" | "quad" => {
                let vals = self.parse_int_list()?;
                self.expect_newline_or_eof()?;
                if vals.len() == 1 {
                    Statement::Data(DataItem::Dword(vals[0]), span.clone())
                } else {
                    Statement::Data(DataItem::Dwords(vals), span.clone())
                }
            }

            "include" => {
                let path = self.expect_string()?;
                self.expect_newline_or_eof()?;
                Statement::Include(std::path::PathBuf::from(path))
            }

            "equ" | "set" => {
                let name = self.expect_ident()?;
                if matches!(self.peek_token(), Token::Comma) {
                    self.advance();
                }
                let val = self.expect_int()?;
                self.expect_newline_or_eof()?;
                Statement::Equ(name, val as i32)
            }

            // Silently ignore unknown directives (e.g. .option, .size)
            _ => {
                while !matches!(self.peek_token(), Token::Newline | Token::Eof) {
                    self.advance();
                }
                self.expect_newline_or_eof()?;
                return Ok(Statement::Globl(String::new())); // placeholder no-op
            }
        })
    }

    fn parse_operands(&mut self) -> Result<Vec<Operand>, ParseError> {
        let mut ops = Vec::new();
        loop {
            match self.peek_token() {
                Token::Newline | Token::Eof => break,
                Token::Comma => {
                    self.advance();
                }
                _ => {
                    let op = self.parse_one_operand()?;
                    ops.push(op);
                }
            }
        }
        Ok(ops)
    }

    fn parse_one_operand(&mut self) -> Result<Operand, ParseError> {
        let span = self.peek_span().clone();
        match self.peek_token().clone() {
            Token::Ident(name) => {
                self.advance();
                if let Some(idx) = crate::hardware::registers::parse_reg(&name) {
                    return Ok(Operand::Reg(idx));
                }
                if let Some(idx) = crate::hardware::fp_registers::parse_fp_reg(&name) {
                    return Ok(Operand::FpReg(idx));
                }
                Ok(Operand::Label(name))
            }

            Token::CharLit(c) => {
                self.advance();
                Ok(Operand::Imm(c as i32))
            }

            Token::Integer(v) => {
                self.advance();
                let imm = v as i32;
                // Check for (reg) suffix → MemOff
                if matches!(self.peek_token(), Token::LParen) {
                    self.advance(); // consume '('
                    let reg = self.expect_reg()?;
                    self.expect_rparen()?;
                    return Ok(Operand::MemOff(imm, reg));
                }
                Ok(Operand::Imm(imm))
            }

            // Negative immediate: handled as unary minus by lex producing negative int
            // But if a label like %hi(foo) comes up, handle as label
            Token::Directive(name) => {
                // e.g. %hi(foo) or %lo(foo) — treat whole thing as a label modifier
                self.advance();
                if matches!(self.peek_token(), Token::LParen) {
                    self.advance();
                    let lbl = self.expect_ident()?;
                    self.expect_rparen()?;
                    return Ok(Operand::Label(format!("%{name}({lbl})")));
                }
                Ok(Operand::Label(format!(".{name}")))
            }

            tok => Err(ParseError::at(
                &span,
                format!("expected operand, got {tok:?}"),
            )),
        }
    }

    fn expect_reg(&mut self) -> Result<usize, ParseError> {
        let span = self.peek_span().clone();
        match self.peek_token().clone() {
            Token::Ident(name) => {
                self.advance();
                crate::hardware::registers::parse_reg(&name)
                    .ok_or_else(|| ParseError::at(&span, format!("not a register: {name}")))
            }
            tok => Err(ParseError::at(
                &span,
                format!("expected register, got {tok:?}"),
            )),
        }
    }

    fn expect_int(&mut self) -> Result<i64, ParseError> {
        let span = self.peek_span().clone();
        match self.peek_token().clone() {
            Token::Integer(v) => {
                self.advance();
                Ok(v)
            }
            tok => Err(ParseError::at(
                &span,
                format!("expected integer, got {tok:?}"),
            )),
        }
    }

    fn parse_int_list(&mut self) -> Result<Vec<i64>, ParseError> {
        let mut vals = vec![self.expect_int()?];
        while matches!(self.peek_token(), Token::Comma) {
            self.advance();
            vals.push(self.expect_int()?);
        }
        Ok(vals)
    }

    fn expect_string(&mut self) -> Result<String, ParseError> {
        let span = self.peek_span().clone();
        match self.peek_token().clone() {
            Token::StringLit(s) => {
                self.advance();
                Ok(s)
            }
            tok => Err(ParseError::at(
                &span,
                format!("expected string, got {tok:?}"),
            )),
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let span = self.peek_span().clone();
        match self.peek_token().clone() {
            Token::Ident(s) => {
                self.advance();
                Ok(s)
            }
            tok => Err(ParseError::at(
                &span,
                format!("expected identifier, got {tok:?}"),
            )),
        }
    }

    fn expect_float(&mut self) -> Result<f64, ParseError> {
        let span = self.peek_span().clone();
        match self.peek_token().clone() {
            Token::Float(v) => {
                self.advance();
                Ok(v)
            }
            Token::Integer(v) => {
                self.advance();
                Ok(v as f64)
            }
            tok => Err(ParseError::at(
                &span,
                format!("expected float, got {tok:?}"),
            )),
        }
    }

    fn expect_rparen(&mut self) -> Result<(), ParseError> {
        let span = self.peek_span().clone();
        match self.peek_token() {
            Token::RParen => {
                self.advance();
                Ok(())
            }
            tok => Err(ParseError::at(&span, format!("expected ')', got {tok:?}"))),
        }
    }
}
