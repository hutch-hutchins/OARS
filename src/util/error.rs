use thiserror::Error;

/// A source location within a .s file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLoc {
    pub file: String,
    pub line: u32,
    pub col: u32,
}

impl std::fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.col)
    }
}

/// Top-level simulator error type.
#[derive(Debug, Error)]
pub enum OarsError {
    #[error("{loc}: lex error: {msg}")]
    Lex { loc: SourceLoc, msg: String },

    #[error("{loc}: parse error: {msg}")]
    Parse { loc: SourceLoc, msg: String },

    #[error("{loc}: assembler error: {msg}")]
    Assemble { loc: SourceLoc, msg: String },

    #[error("runtime error at PC {pc:#010x}: {msg}")]
    Runtime { pc: u32, msg: String },

    #[error("syscall {number}: {msg}")]
    Syscall { number: u32, msg: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
