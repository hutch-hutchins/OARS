/// Integration tests for the example programs in examples/asm/.
///
/// Each test assembles a .s file through the OARS assembler, runs it with the
/// simulator engine, and checks the expected output.
use oars::assembler::{codegen, parser};
use oars::cli::RunOpts;
use oars::hardware::memory::TEXT_BASE;
use oars::simulator::engine::{self, CpuState};
use std::io::Cursor;
use std::path::Path;

fn run_example(name: &str) -> (String, i32) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/asm")
        .join(name);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("cannot read {name}"));
    let stmts = parser::parse(&src).unwrap_or_else(|e| panic!("parse error in {name}: {e}"));
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

// ── RV32M: rem — Euclidean GCD ────────────────────────────────────────────────

#[test]
fn example_gcd() {
    let (out, code) = run_example("gcd.s");
    assert_eq!(code, 0);
    assert!(out.contains("GCD(48, 18) = "), "missing header: {out:?}");
    assert!(out.contains("= 6"), "expected GCD = 6, got: {out:?}");
}

// ── RV32M: mul / mulh / mulhu — exponentiation by squaring ───────────────────

#[test]
fn example_integer_power() {
    let (out, code) = run_example("integer_power.s");
    assert_eq!(code, 0);
    assert!(out.contains("2^10 = 1024"), "wrong power result: {out:?}");
    assert!(
        out.contains("upper half = 0"),
        "wrong mulhu result: {out:?}"
    );
}

// ── RV32F: fmul.s / fsub.s / fsqrt.s / fdiv.s — quadratic formula ────────────

#[test]
fn example_quadratic() {
    let (out, code) = run_example("quadratic.s");
    assert_eq!(code, 0);
    assert!(
        !out.contains("No real roots"),
        "discriminant should be positive: {out:?}"
    );
    // Roots of x^2 - 5x + 6 = 0  are 3.0 and 2.0.
    // Rust f32 Display prints 3.0 as "3" and 2.0 as "2".
    assert!(out.contains("x1 = 3"), "expected x1 = 3, got: {out:?}");
    assert!(out.contains("x2 = 2"), "expected x2 = 2, got: {out:?}");
}

// ── RV32F: fmadd.s — dot product with fused multiply-add ─────────────────────

#[test]
fn example_dot_product() {
    let (out, code) = run_example("dot_product.s");
    assert_eq!(code, 0);
    // [1,2,3,4] · [4,3,2,1] = 4+6+6+4 = 20.  Rust f32 Display: "20"
    assert!(
        out.contains("u · v = 20"),
        "expected dot product 20, got: {out:?}"
    );
}

// ── Zicsr: csrr instret — instruction-count benchmarking ─────────────────────

#[test]
fn example_csr_benchmark() {
    let (out, code) = run_example("csr_benchmark.s");
    assert_eq!(code, 0);
    // Both the loop and the Gauss formula compute sum 1..=100 = 5050.
    assert!(out.contains("Loop sum"), "missing loop section: {out:?}");
    assert!(
        out.contains("Formula sum"),
        "missing formula section: {out:?}"
    );
    let count_5050 = out.matches("5050").count();
    assert!(
        count_5050 >= 2,
        "expected 5050 from both methods, found {count_5050} in: {out:?}"
    );
    // The loop instruction count should be much larger than the formula's.
    // We don't check the exact number but verify the program ran both paths.
    assert!(
        out.contains("loop instrs:"),
        "missing loop instr count: {out:?}"
    );
    assert!(
        out.contains("formula instrs:"),
        "missing formula instr count: {out:?}"
    );
}

// ── RV32D: fmul.d / fdiv.d / fabs.d / fsqrt.d — Newton-Raphson sqrt ──────────

#[test]
fn example_newton_sqrt() {
    let (out, code) = run_example("newton_sqrt.s");
    assert_eq!(code, 0);
    // Newton-Raphson converges to sqrt(2); check first 7 digits are correct.
    assert!(
        out.contains("Newton-Raphson sqrt(2) = 1.41421"),
        "NR sqrt wrong: {out:?}"
    );
    // Hardware fsqrt.d gives the correctly-rounded IEEE 754 result.
    assert!(
        out.contains("Hardware   fsqrt.d(2) = 1.4142135623730951"),
        "hardware sqrt wrong: {out:?}"
    );
}

// ── Hello World ───────────────────────────────────────────────────────────────

#[test]
fn example_hello() {
    let (out, code) = run_example("hello.s");
    assert_eq!(code, 0);
    assert_eq!(out, "Hello, World!\n", "wrong output: {out:?}");
}

// ── Fibonacci sequence (first 10 terms) ───────────────────────────────────────

#[test]
fn example_fibonacci() {
    let (out, code) = run_example("fibonacci.s");
    assert_eq!(code, 0);
    assert_eq!(
        out, "0\n1\n1\n2\n3\n5\n8\n13\n21\n34\n",
        "wrong output: {out:?}"
    );
}

// ── Floating-point smoke test: (3.0 + 4.0) * 0.5 = 3.5 ───────────────────────

#[test]
fn example_fp_test() {
    let (out, code) = run_example("fp_test.s");
    assert_eq!(code, 0);
    assert_eq!(out, "FP result: 3.5\n", "wrong output: {out:?}");
}

// ── Bubble sort: [5,3,1,4,2] → "1 2 3 4 5 " ─────────────────────────────────

#[test]
fn example_bubble_sort() {
    let (out, code) = run_example("bubble_sort.s");
    assert_eq!(code, 0);
    assert!(
        out.contains("1 2 3 4 5"),
        "expected sorted sequence: {out:?}"
    );
}

// ── Stack frames: sum_of_squares(5) = 55 ─────────────────────────────────────

#[test]
fn example_stack_frame() {
    let (out, code) = run_example("stack_frame.s");
    assert_eq!(code, 0);
    assert!(
        out.contains("sum_of_squares(5) = "),
        "missing label: {out:?}"
    );
    assert!(out.contains("55"), "expected 55: {out:?}");
}

// ── Recursive factorial: 10! = 3628800 ───────────────────────────────────────

#[test]
fn example_factorial() {
    let (out, code) = run_example("factorial.s");
    assert_eq!(code, 0);
    assert!(out.contains("10! = "), "missing label: {out:?}");
    assert!(out.contains("3628800"), "expected 3628800: {out:?}");
}

// ── Heap allocation via sbrk: prints "1 2 3 4 5 6 7 8" ───────────────────────

#[test]
fn example_heap_alloc() {
    let (out, code) = run_example("heap_alloc.s");
    assert_eq!(code, 0);
    assert!(
        out.contains("1 2 3 4 5 6 7 8"),
        "expected heap array: {out:?}"
    );
}

// ── Linked list via heap allocation ───────────────────────────────────────────

#[test]
fn example_linked_list() {
    let (out, code) = run_example("linked_list.s");
    assert_eq!(code, 0);
    assert_eq!(out, "10\n20\n30\n", "wrong linked-list output: {out:?}");
}

// ── String operations: strlen + str_reverse ───────────────────────────────────

#[test]
fn example_string_ops() {
    let (out, code) = run_example("string_ops.s");
    assert_eq!(code, 0);
    assert!(out.contains("Length: 5"), "missing length: {out:?}");
    assert!(out.contains("olleh"), "missing reversed string: {out:?}");
}

// ── Selection sort: {64,25,12,22,11} → "11 12 22 25 64 " ─────────────────────

#[test]
fn example_selection_sort() {
    let (out, code) = run_example("selection_sort.s");
    assert_eq!(code, 0);
    assert!(
        out.contains("11 12 22 25 64"),
        "expected sorted output: {out:?}"
    );
}

// ── .equ constants: SIZE=8, sum 1..=8 = 36 ───────────────────────────────────

#[test]
fn example_constants() {
    let (out, code) = run_example("constants.s");
    assert_eq!(code, 0);
    assert!(out.contains("SIZE = 8"), "missing SIZE: {out:?}");
    assert!(out.contains("SUM  = 36"), "expected sum 36: {out:?}");
}
