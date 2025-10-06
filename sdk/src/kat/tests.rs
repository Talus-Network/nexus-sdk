use super::*;

fn config() -> KatParserConfig {
    KatParserConfig::new(["a", "b"], ["p", "q", "r"]).unwrap()
}

fn sym(name: &str) -> Symbol {
    Symbol::from(name)
}

#[test]
fn parses_action_sequence_with_star_and_choice() {
    let cfg = config();
    let expr = parse_kat_expr("a b* + 1", &cfg).unwrap();
    let expected = KatExpr::Choice(
        Box::new(KatExpr::Sequence(
            Box::new(KatExpr::Action(sym("a"))),
            Box::new(KatExpr::Star(Box::new(KatExpr::Action(sym("b"))))),
        )),
        Box::new(KatExpr::One),
    );
    assert_eq!(expr, expected);
}

#[test]
fn parses_boolean_with_negation_and_join() {
    let cfg = config();
    let expr = parse_kat_expr("!(p | q) & r", &cfg).unwrap();
    let expected = KatExpr::Test(TestExpr::And(
        Box::new(TestExpr::Not(Box::new(TestExpr::Or(
            Box::new(TestExpr::Atom(sym("p"))),
            Box::new(TestExpr::Atom(sym("q"))),
        )))),
        Box::new(TestExpr::Atom(sym("r"))),
    ));
    assert_eq!(expr, expected);
}

#[test]
fn respects_concatenation_precedence() {
    let cfg = config();
    let expr = parse_kat_expr("p(q + r)", &cfg).unwrap();
    let expected = KatExpr::Sequence(
        Box::new(KatExpr::Test(TestExpr::Atom(sym("p")))),
        Box::new(KatExpr::Choice(
            Box::new(KatExpr::Test(TestExpr::Atom(sym("q")))),
            Box::new(KatExpr::Test(TestExpr::Atom(sym("r")))),
        )),
    );
    assert_eq!(expr, expected);
}

#[test]
fn parses_plus_as_choice_at_top_level() {
    let cfg = config();
    let expr = parse_kat_expr("p + q", &cfg).unwrap();
    let expected = KatExpr::Choice(
        Box::new(KatExpr::Test(TestExpr::Atom(sym("p")))),
        Box::new(KatExpr::Test(TestExpr::Atom(sym("q")))),
    );
    assert_eq!(expr, expected);
}

#[test]
fn unknown_symbol_is_reported() {
    let cfg = config();
    let err = parse_kat_expr("c", &cfg).unwrap_err();
    assert!(err.message.contains("unknown symbol"));
}

#[test]
fn complement_allows_inner_choice() {
    let cfg = config();
    let expr = parse_kat_expr("!(p + q)", &cfg).unwrap();
    let expected = KatExpr::Test(TestExpr::Not(Box::new(TestExpr::Or(
        Box::new(TestExpr::Atom(sym("p"))),
        Box::new(TestExpr::Atom(sym("q"))),
    ))));
    assert_eq!(expr, expected);
}
