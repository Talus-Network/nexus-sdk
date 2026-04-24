//! Compile-fail harness for the strict DSL layer.
//!
//! Each `.rs` file under `tests/compile_fail/` is a program that MUST NOT
//! compile. If it starts compiling, the strict-layer type invariant that
//! program was guarding has silently regressed. Each file's comment header
//! states what invariant it guards.

#[test]
fn strict_port_types_rejected_at_compile_time() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
