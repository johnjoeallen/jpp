use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: Option<PathBuf>,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    pub fn new(file: Option<PathBuf>, line: usize, column: usize) -> Self {
        Self { file, line, column }
    }

    pub fn at_offset(file: Option<&Path>, source: &str, offset: usize) -> Self {
        let mut line = 1;
        let mut column = 1;

        for (index, ch) in source.char_indices() {
            if index >= offset {
                break;
            }

            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }

        Self {
            file: file.map(Path::to_path_buf),
            line,
            column,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub suggestion: Option<String>,
    pub location: SourceLocation,
}

impl Diagnostic {
    pub fn new(code: &'static str, message: impl Into<String>, location: SourceLocation) -> Self {
        Self {
            code,
            message: message.into(),
            suggestion: None,
            location,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(file) = &self.location.file {
            writeln!(
                f,
                "{}:{}:{}",
                file.display(),
                self.location.line,
                self.location.column
            )?;
        } else {
            writeln!(f, "{}:{}", self.location.line, self.location.column)?;
        }

        writeln!(f)?;
        writeln!(f, "{}:", self.code)?;
        writeln!(f, "{}", self.message)?;

        if let Some(suggestion) = &self.suggestion {
            writeln!(f)?;
            writeln!(f, "Suggestion: {suggestion}")?;
        }

        Ok(())
    }
}

pub type JppResult<T> = Result<T, Diagnostic>;
