use {
    super::{
        ast::{Span, Symbol},
        error::ParseError,
        lexer::TokenKind,
    },
    std::collections::HashSet,
};

/// Parser configuration describing the primitive vocabulary.
#[derive(Debug, Clone)]
pub struct KatParserConfig {
    actions: HashSet<Symbol>,
    tests: HashSet<Symbol>,
}

impl KatParserConfig {
    pub fn new<A, T>(actions: A, tests: T) -> Result<Self, ParseError>
    where
        A: IntoIterator,
        A::Item: Into<Symbol>,
        T: IntoIterator,
        T::Item: Into<Symbol>,
    {
        let actions: HashSet<Symbol> = actions.into_iter().map(Into::into).collect();
        let tests: HashSet<Symbol> = tests.into_iter().map(Into::into).collect();

        if let Some(sym) = actions.iter().find(|s| tests.contains(*s)) {
            return Err(ParseError::new(
                format!("symbol `{}` cannot be both action and test", sym),
                None,
            ));
        }

        Ok(Self { actions, tests })
    }

    pub(crate) fn classify(&self, symbol: &str, span: Span) -> Result<TokenKind, ParseError> {
        if let Some(sym) = self.actions.get(symbol) {
            return Ok(TokenKind::Action(sym.clone()));
        }
        if let Some(sym) = self.tests.get(symbol) {
            return Ok(TokenKind::Test(sym.clone()));
        }
        Err(ParseError::new(
            format!("unknown symbol `{}`", symbol),
            Some(span),
        ))
    }
}
