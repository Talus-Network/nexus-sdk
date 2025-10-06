use super::{
    ast::{Span, Symbol},
    config::KatParserConfig,
    error::ParseError,
};

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) span: Span,
}

impl Token {
    pub(crate) fn expect_action(self) -> Symbol {
        match self.kind {
            TokenKind::Action(symbol) => symbol,
            _ => unreachable!("expected action token"),
        }
    }

    pub(crate) fn expect_test(self) -> Symbol {
        match self.kind {
            TokenKind::Test(symbol) => symbol,
            _ => unreachable!("expected test token"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum TokenKind {
    Action(Symbol),
    Test(Symbol),
    Zero,
    One,
    Plus,
    Star,
    Semicolon,
    LParen,
    RParen,
    Bang,
    Ampersand,
    Pipe,
    End,
}

pub(crate) struct Lexer<'a> {
    input: &'a str,
    chars: std::str::CharIndices<'a>,
    peeked: Option<(usize, char)>,
    config: &'a KatParserConfig,
    finished: bool,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(input: &'a str, config: &'a KatParserConfig) -> Self {
        Self {
            input,
            chars: input.char_indices(),
            peeked: None,
            config,
            finished: false,
        }
    }

    pub(crate) fn next_token(&mut self) -> Result<Token, ParseError> {
        if self.finished {
            return Ok(Token {
                kind: TokenKind::End,
                span: Span::new(self.input.len(), self.input.len()),
            });
        }

        self.skip_whitespace();

        let (start, ch) = match self.bump_char() {
            Some(pair) => pair,
            None => {
                self.finished = true;
                return Ok(Token {
                    kind: TokenKind::End,
                    span: Span::new(self.input.len(), self.input.len()),
                });
            }
        };

        let token = match ch {
            '+' => Token {
                kind: TokenKind::Plus,
                span: Span::new(start, start + 1),
            },
            '*' => Token {
                kind: TokenKind::Star,
                span: Span::new(start, start + 1),
            },
            ';' => Token {
                kind: TokenKind::Semicolon,
                span: Span::new(start, start + 1),
            },
            '(' => Token {
                kind: TokenKind::LParen,
                span: Span::new(start, start + 1),
            },
            ')' => Token {
                kind: TokenKind::RParen,
                span: Span::new(start, start + 1),
            },
            '!' => Token {
                kind: TokenKind::Bang,
                span: Span::new(start, start + 1),
            },
            '&' => Token {
                kind: TokenKind::Ampersand,
                span: Span::new(start, start + 1),
            },
            '|' => Token {
                kind: TokenKind::Pipe,
                span: Span::new(start, start + 1),
            },
            '0' => Token {
                kind: TokenKind::Zero,
                span: Span::new(start, start + 1),
            },
            '1' => Token {
                kind: TokenKind::One,
                span: Span::new(start, start + 1),
            },
            c if is_identifier_start(c) => {
                let mut end = start + c.len_utf8();
                let mut name = String::new();
                name.push(c);
                while let Some((_idx, next)) = self.peek_char() {
                    if is_identifier_continue(next) {
                        let (idx, ch) = self.bump_char().expect("peeked character missing");
                        end = idx + ch.len_utf8();
                        name.push(ch);
                    } else {
                        break;
                    }
                }

                match self.config.classify(&name, Span::new(start, end)) {
                    Ok(kind) => Token {
                        kind,
                        span: Span::new(start, end),
                    },
                    Err(err) => return Err(err),
                }
            }
            _ => {
                return Err(ParseError::new(
                    format!("unexpected character `{}`", ch),
                    Some(Span::new(start, start + ch.len_utf8())),
                ));
            }
        };

        Ok(token)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek_char(), Some((_, ch)) if ch.is_whitespace()) {
            self.bump_char();
        }
    }

    fn peek_char(&mut self) -> Option<(usize, char)> {
        if self.peeked.is_none() {
            self.peeked = self.chars.next();
        }
        self.peeked
    }

    fn bump_char(&mut self) -> Option<(usize, char)> {
        if let Some(peeked) = self.peeked.take() {
            Some(peeked)
        } else {
            self.chars.next()
        }
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
