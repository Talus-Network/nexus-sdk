use super::{
    ast::{KatExpr, Span, TestExpr},
    config::KatParserConfig,
    error::ParseError,
    lexer::{Lexer, Token, TokenKind},
};

pub(crate) fn parse_kat_expr(input: &str, config: &KatParserConfig) -> Result<KatExpr, ParseError> {
    let lexer = Lexer::new(input, config);
    let stream = TokenStream::new(lexer);
    Parser::new(stream).parse()
}

struct TokenStream<'a> {
    lexer: Lexer<'a>,
    lookahead: Option<Token>,
}

impl<'a> TokenStream<'a> {
    fn new(lexer: Lexer<'a>) -> Self {
        Self {
            lexer,
            lookahead: None,
        }
    }

    fn peek(&mut self) -> Result<&Token, ParseError> {
        if self.lookahead.is_none() {
            self.lookahead = Some(self.lexer.next_token()?);
        }
        self.lookahead
            .as_ref()
            .ok_or_else(|| ParseError::new("unexpected end of input", Some(Span::new(0, 0))))
    }

    fn peek_kind(&mut self) -> Result<&TokenKind, ParseError> {
        Ok(&self.peek()?.kind)
    }

    fn bump(&mut self) -> Result<Token, ParseError> {
        if let Some(token) = self.lookahead.take() {
            Ok(token)
        } else {
            self.lexer.next_token()
        }
    }

    fn take_if<F>(&mut self, predicate: F) -> Result<Option<Token>, ParseError>
    where
        F: FnOnce(&TokenKind) -> bool,
    {
        if predicate(self.peek_kind()?) {
            self.bump().map(Some)
        } else {
            Ok(None)
        }
    }

    fn expect<F>(&mut self, predicate: F, expected: &str) -> Result<Token, ParseError>
    where
        F: FnOnce(&TokenKind) -> bool,
    {
        if predicate(self.peek_kind()?) {
            self.bump()
        } else {
            let token = self.peek()?.clone();
            Err(ParseError::new(
                format!("expected {}, found {:?}", expected, token.kind),
                Some(token.span),
            ))
        }
    }
}

struct Parser<'a> {
    tokens: TokenStream<'a>,
}

impl<'a> Parser<'a> {
    fn new(tokens: TokenStream<'a>) -> Self {
        Self { tokens }
    }

    fn parse(mut self) -> Result<KatExpr, ParseError> {
        let expr = self.parse_choice()?;
        match self.tokens.bump()? {
            Token {
                kind: TokenKind::End,
                ..
            } => Ok(expr),
            token => Err(ParseError::new(
                "unexpected token after end of expression",
                Some(token.span),
            )),
        }
    }

    fn parse_choice(&mut self) -> Result<KatExpr, ParseError> {
        let mut expr = self.parse_concat()?;
        while self
            .tokens
            .take_if(|k| matches!(k, TokenKind::Plus))?
            .is_some()
        {
            let rhs = self.parse_concat()?;
            expr = KatExpr::Choice(Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    fn parse_concat(&mut self) -> Result<KatExpr, ParseError> {
        let mut expr = self.parse_unary()?;
        loop {
            if self
                .tokens
                .take_if(|k| matches!(k, TokenKind::Semicolon))?
                .is_some()
            {
                let rhs = self.parse_unary()?;
                expr = KatExpr::Sequence(Box::new(expr), Box::new(rhs));
                continue;
            }

            if self.next_begins_unary()? {
                let rhs = self.parse_unary()?;
                expr = KatExpr::Sequence(Box::new(expr), Box::new(rhs));
                continue;
            }

            break Ok(expr);
        }
    }

    fn next_begins_unary(&mut self) -> Result<bool, ParseError> {
        Ok(matches!(
            self.tokens.peek_kind()?,
            TokenKind::Action(_)
                | TokenKind::Test(_)
                | TokenKind::Zero
                | TokenKind::One
                | TokenKind::LParen
                | TokenKind::Bang
        ))
    }

    fn parse_unary(&mut self) -> Result<KatExpr, ParseError> {
        let mut expr = self.parse_primary()?;
        while self
            .tokens
            .take_if(|k| matches!(k, TokenKind::Star))?
            .is_some()
        {
            expr = KatExpr::Star(Box::new(expr));
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<KatExpr, ParseError> {
        let span = self.tokens.peek()?.span;
        match self.tokens.peek_kind()?.clone() {
            TokenKind::Action(_) => {
                let token = self.tokens.bump()?;
                Ok(KatExpr::Action(token.expect_action()))
            }
            TokenKind::Test(_) => {
                let test = self.parse_test_expr(true)?;
                Ok(KatExpr::Test(test))
            }
            TokenKind::Bang => {
                let test = self.parse_test_expr(false)?;
                Ok(KatExpr::Test(test))
            }
            TokenKind::Zero => {
                self.tokens.bump()?;
                Ok(KatExpr::Zero)
            }
            TokenKind::One => {
                self.tokens.bump()?;
                Ok(KatExpr::One)
            }
            TokenKind::LParen => {
                self.tokens.bump()?;
                let expr = self.parse_choice()?;
                self.tokens
                    .expect(|k| matches!(k, TokenKind::RParen), "`)`")?;
                Ok(expr)
            }
            other => Err(ParseError::new(
                format!("unexpected token {:?} in expression", other),
                Some(span),
            )),
        }
    }

    fn parse_test_expr(&mut self, stop_on_choice: bool) -> Result<TestExpr, ParseError> {
        self.parse_test_disjunction(stop_on_choice)
    }

    fn parse_test_disjunction(&mut self, stop_on_choice: bool) -> Result<TestExpr, ParseError> {
        let mut expr = self.parse_test_conjunction(stop_on_choice)?;
        loop {
            let is_join = match self.tokens.peek_kind()? {
                TokenKind::Pipe => true,
                TokenKind::Plus if !stop_on_choice => true,
                _ => false,
            };
            if !is_join {
                break;
            }
            self.tokens.bump()?;
            let rhs = self.parse_test_conjunction(stop_on_choice)?;
            expr = TestExpr::Or(Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    fn parse_test_conjunction(&mut self, stop_on_choice: bool) -> Result<TestExpr, ParseError> {
        let mut expr = self.parse_test_negation(stop_on_choice)?;
        while self
            .tokens
            .take_if(|k| matches!(k, TokenKind::Ampersand))?
            .is_some()
        {
            let rhs = self.parse_test_negation(stop_on_choice)?;
            expr = TestExpr::And(Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    fn parse_test_negation(&mut self, stop_on_choice: bool) -> Result<TestExpr, ParseError> {
        if self
            .tokens
            .take_if(|k| matches!(k, TokenKind::Bang))?
            .is_some()
        {
            let expr = self.parse_test_negation(false)?;
            Ok(TestExpr::Not(Box::new(expr)))
        } else {
            self.parse_test_atom(stop_on_choice)
        }
    }

    fn parse_test_atom(&mut self, stop_on_choice: bool) -> Result<TestExpr, ParseError> {
        let span = self.tokens.peek()?.span;
        match self.tokens.peek_kind()?.clone() {
            TokenKind::Test(_) => {
                let token = self.tokens.bump()?;
                Ok(TestExpr::Atom(token.expect_test()))
            }
            TokenKind::Zero => {
                self.tokens.bump()?;
                Ok(TestExpr::Zero)
            }
            TokenKind::One => {
                self.tokens.bump()?;
                Ok(TestExpr::One)
            }
            TokenKind::LParen => {
                self.tokens.bump()?;
                let expr = self.parse_test_expr(false)?;
                self.tokens
                    .expect(|k| matches!(k, TokenKind::RParen), "`)`")?;
                Ok(expr)
            }
            TokenKind::Bang => self.parse_test_negation(stop_on_choice),
            other => Err(ParseError::new(
                format!("unexpected token {:?} in test expression", other),
                Some(span),
            )),
        }
    }
}
