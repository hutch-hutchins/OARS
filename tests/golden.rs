/// Golden-file integration tests.
///
/// Each test assembles a .s file from tests/programs/, runs it through OARS,
/// and compares stdout to the corresponding tests/golden/*.txt file.
///
/// All tests are `#[ignore]` until Phase 1 produces a working simulator.
/// To generate golden files from RARS (requires JVM):
///
///   java -jar reference/rars/rars.jar nc me sm tests/programs/hello.s \
///        > tests/golden/hello.txt
///
/// To run once Phase 1 is done:
///
///   cargo test --test golden -- --include-ignored
use std::path::Path;
use std::process::Command;

fn oars_binary() -> std::path::PathBuf {
    // Use the debug build during testing
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push("debug");
    p.push(if cfg!(windows) { "oars.exe" } else { "oars" });
    p
}

fn run_program(program: &str) -> String {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/programs")
        .join(program);

    let out = Command::new(oars_binary())
        .arg(&src)
        .output()
        .expect("failed to run oars binary — did you `cargo build` first?");

    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "golden file not found: {} — run RARS headless to generate it",
            path.display()
        )
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "Phase 1 not implemented yet"]
fn hello_world() {
    assert_eq!(run_program("hello.s"), golden("hello.txt"));
}

#[test]
#[ignore = "Phase 1 not implemented yet"]
fn fibonacci() {
    assert_eq!(run_program("fibonacci.s"), golden("fibonacci.txt"));
}

#[test]
#[ignore = "Phase 1 not implemented yet"]
fn bubble_sort() {
    assert_eq!(run_program("bubble_sort.s"), golden("bubble_sort.txt"));
}
