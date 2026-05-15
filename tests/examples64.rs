/// Integration tests for the RV64I example programs in examples/64bit/.
///
/// Each test assembles a .s file through the shared OARS assembler, loads the
/// image into a CpuState64, runs it with the 64-bit engine, and checks the
/// expected output and exit code.
use oars::assembler::{codegen, parser};
use oars::cli::RunOpts;
use oars::hardware::memory::TEXT_BASE;
use oars::simulator::engine64::{self, CpuState64};
use std::io::Cursor;
use std::path::Path;

fn run64(name: &str) -> (String, i32) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/64bit")
        .join(name);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("cannot read {name}"));
    let stmts = parser::parse(&src).unwrap_or_else(|e| panic!("parse error in {name}: {e}"));
    let mut cpu = CpuState64::new(TEXT_BASE);
    let asm_out = codegen::assemble(&stmts, &mut cpu.mem)
        .unwrap_or_else(|e| panic!("assemble error in {name}: {e}"));
    cpu.pc = asm_out.entry as u64;
    let mut stdout = Vec::<u8>::new();
    let mut stdin = Cursor::new(b"");
    let opts = RunOpts::default();
    let telem = engine64::run64(&mut cpu, &opts, &mut stdout, &mut stdin)
        .unwrap_or_else(|e| panic!("runtime error in {name}: {e}"));
    (String::from_utf8(stdout).unwrap(), telem.exit_code)
}

// ── Hello World ───────────────────────────────────────────────────────────────

#[test]
fn example64_hello() {
    let (out, code) = run64("hello64.s");
    assert_eq!(code, 0);
    assert_eq!(out, "Hello, 64-bit World!\n", "wrong output: {out:?}");
}

// ── Fibonacci using 64-bit registers ─────────────────────────────────────────

#[test]
fn example64_fibonacci() {
    let (out, code) = run64("fibonacci64.s");
    assert_eq!(code, 0);
    assert_eq!(
        out, "0\n1\n1\n2\n3\n5\n8\n13\n21\n34\n",
        "wrong Fibonacci output: {out:?}"
    );
}

// ── LD/SD: sum doubleword array ───────────────────────────────────────────────

#[test]
fn example64_array() {
    let (out, code) = run64("array64.s");
    assert_eq!(code, 0);
    // [1,2,3,4,5,6,7,8] sum = 36
    assert_eq!(out, "36\n", "expected array sum 36: {out:?}");
}

// ── W-suffix instructions: ADDIW / ADDW overflow behaviour ───────────────────

#[test]
fn example64_word_ops() {
    let (out, code) = run64("word_ops64.s");
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 4, "expected 4 output lines: {out:?}");
    // addi  INT32_MAX, 1  → 2,147,483,648 (64-bit, printed unsigned)
    assert_eq!(lines[0], "2147483648", "addi result wrong: {out:?}");
    // addiw INT32_MAX, 1  → -2,147,483,648 (32-bit overflow, sign-extended)
    assert_eq!(lines[1], "-2147483648", "addiw result wrong: {out:?}");
    // addw  INT32_MAX, INT32_MAX → -2 (32-bit truncation, sign-extended)
    assert_eq!(lines[2], "-2", "addw result wrong: {out:?}");
    // add   INT32_MAX, INT32_MAX → 4,294,967,294 (no truncation)
    assert_eq!(lines[3], "4294967294", "add result wrong: {out:?}");
}

// ── 64-bit MUL: 10^6 × 10^6 ÷ 10^9 = 1000 ───────────────────────────────────

#[test]
fn example64_big_mul() {
    let (out, code) = run64("big_mul64.s");
    assert_eq!(code, 0);
    // 1,000,000 * 1,000,000 = 10^12; 10^12 / 10^9 = 1000
    assert_eq!(out, "1000\n", "expected 1000 from 64-bit mul/div: {out:?}");
}

// ── F(50) > 2^32: split print via div/rem by 10^9 ────────────────────────────

#[test]
fn example64_fibonacci_big() {
    let (out, code) = run64("fibonacci_big64.s");
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "expected 2 output lines: {out:?}");
    assert_eq!(lines[0], "12", "F(50)/10^9 wrong: {out:?}");
    assert_eq!(lines[1], "586269025", "F(50)%10^9 wrong: {out:?}");
}

// ── MULHU: upper 64 bits of (5×10^9)² overflows a single 64-bit register ──────

#[test]
fn example64_mulhi() {
    let (out, code) = run64("mulhi64.s");
    assert_eq!(code, 0);
    assert!(
        out.contains("upper half = 1"),
        "expected 'upper half = 1': {out:?}"
    );
}

// ── Gauss sum 1..100,000 = 5,000,050,000 > 2^32 ─────────────────────────────

#[test]
fn example64_gauss_sum() {
    let (out, code) = run64("gauss_sum64.s");
    assert_eq!(code, 0);
    assert_eq!(out, "MATCH\n", "loop and formula disagree: {out:?}");
}

// ── Popcount of 0x1_FFFF_FFFF (2^33-1) = 33 set bits ─────────────────────────

#[test]
fn example64_count_bits() {
    let (out, code) = run64("count_bits64.s");
    assert_eq!(code, 0);
    assert_eq!(out, "33\n", "expected 33 set bits: {out:?}");
}

// ── Sum of cubes 1^3..1000^3 = 250,500,250,000 > 2^37 ────────────────────────

#[test]
fn example64_sum_cubes() {
    let (out, code) = run64("sum_cubes64.s");
    assert_eq!(code, 0);
    assert_eq!(out, "MATCH\n", "loop sum and formula disagree: {out:?}");
}
