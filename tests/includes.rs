/// Integration tests for multi-file programs in examples/includes/.
///
/// Each test assembles a main .s file that uses `.include` to pull in a shared
/// library, runs the result, and verifies the expected output and exit code.
use oars::assembler::{codegen, include, parser};
use oars::cli::RunOpts;
use oars::hardware::memory::TEXT_BASE;
use oars::simulator::engine::{self, CpuState};
use std::io::Cursor;
use std::path::Path;

fn run_include(name: &str) -> (String, i32) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/includes")
        .join(name);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("cannot read {name}"));
    let stmts = parser::parse(&src).unwrap_or_else(|e| panic!("parse error in {name}: {e}"));
    let base_dir = path.parent().expect("path has no parent");
    let stmts = include::resolve(stmts, base_dir)
        .unwrap_or_else(|e| panic!("include error in {name}: {e}"));
    let mut cpu = CpuState::new(TEXT_BASE);
    let asm_out = codegen::assemble(&stmts, &mut cpu.mem)
        .unwrap_or_else(|e| panic!("assemble error in {name}: {e}"));
    cpu.pc = asm_out.entry;
    let mut stdout = Vec::<u8>::new();
    let mut stdin = Cursor::new(b"");
    let opts = RunOpts::default();
    let telem = engine::run(&mut cpu, &opts, &mut stdout, &mut stdin)
        .unwrap_or_else(|e| panic!("runtime error in {name}: {e}"));
    (String::from_utf8(stdout).unwrap(), telem.exit_code)
}

// ── math_lib.s: gcd and int_power via .include ───────────────────────────────

#[test]
fn include_demo_math() {
    let (out, code) = run_include("demo_math.s");
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 4, "expected 4 output lines: {out:?}");
    assert_eq!(lines[0], "6", "gcd(48,18) wrong: {out:?}");
    assert_eq!(lines[1], "25", "gcd(100,75) wrong: {out:?}");
    assert_eq!(lines[2], "1024", "2^10 wrong: {out:?}");
    assert_eq!(lines[3], "243", "3^5 wrong: {out:?}");
}

// ── string_lib.s: str_len and str_upper via .include ─────────────────────────

#[test]
fn include_demo_strings() {
    let (out, code) = run_include("demo_strings.s");
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "expected 2 output lines: {out:?}");
    assert_eq!(lines[0], "13", "strlen wrong: {out:?}");
    assert_eq!(lines[1], "HELLO, WORLD!", "str_upper wrong: {out:?}");
}
