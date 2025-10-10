//! Streaming parser for Kleene Algebra with Tests (KAT).
//! TODO: Move to Concurrent version of the KAT parser
//! The parser follows KAT's algebraic structure directly. Actions and
//! primitive tests are supplied up-front and the parser consumes a token
//! stream on demand, never buffering more than necessary. Parsing produces an
//! abstract syntax tree that separates KAT expressions from Boolean test
//! expressions while keeping their interaction faithful to the algebra.

pub mod ast;
mod automaton;
mod config;
mod error;
mod lexer;
mod parser;

pub use {
    ast::{KatExpr, Span, Symbol, TestExpr},
    automaton::{
        DeterministicFiniteAutomaton,
        DfaState,
        DfaStateId,
        DfaTransition,
        EpsilonNfa,
        StateId,
        Transition,
        TransitionLabel,
    },
    config::KatParserConfig,
    error::ParseError,
};

/// Parse a KAT expression from `input` using the given vocabulary configuration.
pub fn parse_kat_expr(input: &str, config: &KatParserConfig) -> Result<KatExpr, ParseError> {
    parser::parse_kat_expr(input, config)
}

#[cfg(test)]
mod tests;
