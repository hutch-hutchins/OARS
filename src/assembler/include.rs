use crate::assembler::parser::{self, Statement};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Recursively resolve all `.include` directives by inlining the referenced
/// files in place.  `base_dir` is the directory from which relative include
/// paths are resolved (usually the directory of the top-level source file).
///
/// Circular includes are detected via a DFS stack: if file A includes B which
/// includes A again, an error is returned before any infinite recursion occurs.
pub fn resolve(stmts: Vec<Statement>, base_dir: &Path) -> anyhow::Result<Vec<Statement>> {
    let mut stack = HashSet::new();
    resolve_inner(stmts, base_dir, &mut stack)
}

fn resolve_inner(
    stmts: Vec<Statement>,
    base_dir: &Path,
    stack: &mut HashSet<PathBuf>,
) -> anyhow::Result<Vec<Statement>> {
    let mut out = Vec::with_capacity(stmts.len());
    for stmt in stmts {
        match stmt {
            Statement::Include(rel_path) => {
                let abs = base_dir
                    .join(&rel_path)
                    .canonicalize()
                    .map_err(|e| anyhow::anyhow!(".include {:?}: {}", rel_path, e))?;

                if !stack.insert(abs.clone()) {
                    anyhow::bail!("circular .include: {}", abs.display());
                }

                let src = std::fs::read_to_string(&abs)
                    .map_err(|e| anyhow::anyhow!("cannot read {}: {}", abs.display(), e))?;

                let included =
                    parser::parse(&src).map_err(|e| anyhow::anyhow!("{}: {}", abs.display(), e))?;

                let child_dir = abs.parent().unwrap_or(Path::new("."));
                let resolved = resolve_inner(included, child_dir, stack)?;
                out.extend(resolved);

                stack.remove(&abs);
            }
            other => out.push(other),
        }
    }
    Ok(out)
}
