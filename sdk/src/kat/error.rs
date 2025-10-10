use {super::ast::Span, std::fmt};

/// Errors that can occur while configuring or running the parser.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Option<Span>,
}

impl ParseError {
    pub(crate) fn new<S: Into<String>>(message: S, span: Option<Span>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.span {
            Some(span) => write!(f, "{} (at {}..{})", self.message, span.start, span.end),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ParseError {}
