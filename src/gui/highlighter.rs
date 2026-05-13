use egui::text::LayoutJob;
use egui::{Color32, FontId, TextFormat};

// ─── Palette ──────────────────────────────────────────────────────────────────
const C_DEFAULT: Color32 = Color32::from_rgb(204, 204, 204);
const C_COMMENT: Color32 = Color32::from_rgb(106, 153, 85); // green-grey
const C_DIRECTIVE: Color32 = Color32::from_rgb(220, 150, 80); // orange
const C_REGISTER: Color32 = Color32::from_rgb(100, 200, 100); // green
const C_LABEL_DEF: Color32 = Color32::from_rgb(255, 200, 80); // amber
const C_MNEMONIC: Color32 = Color32::from_rgb(100, 180, 255); // sky blue
const C_STRING: Color32 = Color32::from_rgb(206, 145, 120); // salmon
const C_NUMBER: Color32 = Color32::from_rgb(180, 220, 255); // pale blue

const FONT: FontId = FontId::monospace(13.0);

fn fmt(color: Color32, bg: Color32) -> TextFormat {
    TextFormat {
        font_id: FONT,
        color,
        background: bg,
        ..Default::default()
    }
}

/// Produce a syntax-highlighted `LayoutJob` for RISC-V assembly source.
/// `error_line` is a 1-based line number to highlight with a red background.
pub fn highlight(text: &str, error_line: Option<u32>) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;

    for (line_idx, line) in text.split('\n').enumerate() {
        let line_no = (line_idx + 1) as u32;
        let err_bg = if error_line == Some(line_no) {
            Color32::from_rgb(90, 20, 20)
        } else {
            Color32::TRANSPARENT
        };

        highlight_line(&mut job, line, err_bg);

        // Re-add the newline stripped by split (except after the last line)
        job.append(
            "\n",
            0.0,
            TextFormat {
                font_id: FONT,
                ..Default::default()
            },
        );
    }

    job
}

fn highlight_line(job: &mut LayoutJob, line: &str, bg: Color32) {
    let mut chars = line.char_indices().peekable();

    // Skip leading whitespace (preserve it as default-colored)
    while let Some(&(i, ch)) = chars.peek() {
        if ch == ' ' || ch == '\t' {
            chars.next();
            job.append(&line[i..i + ch.len_utf8()], 0.0, fmt(C_DEFAULT, bg));
        } else {
            break;
        }
    }

    if chars.peek().is_none() {
        return;
    }

    let rest_start = chars.peek().map(|&(i, _)| i).unwrap_or(line.len());
    let rest = &line[rest_start..];

    // Comment
    if rest.starts_with('#') || rest.starts_with(';') {
        job.append(rest, 0.0, fmt(C_COMMENT, bg));
        return;
    }

    // Directive (.text, .data, .word, …)
    if rest.starts_with('.') {
        let (tok, after) = split_token(rest);
        job.append(tok, 0.0, fmt(C_DIRECTIVE, bg));
        highlight_operands(job, after, bg);
        return;
    }

    // Label definition (token ending with ':')
    let (first_tok, after_first) = split_token(rest);
    if first_tok.ends_with(':') {
        job.append(first_tok, 0.0, fmt(C_LABEL_DEF, bg));
        // Remainder of the line (may have an instr after the label)
        highlight_line_after_label(job, after_first, bg);
        return;
    }

    // Mnemonic (first token on the line)
    if !first_tok.is_empty() {
        job.append(first_tok, 0.0, fmt(C_MNEMONIC, bg));
        highlight_operands(job, after_first, bg);
    }
}

/// Handle the rest of a line after a label definition.
fn highlight_line_after_label(job: &mut LayoutJob, s: &str, bg: Color32) {
    // Leading whitespace
    let ws_end = s.len() - s.trim_start().len();
    if ws_end > 0 {
        job.append(&s[..ws_end], 0.0, fmt(C_DEFAULT, bg));
    }
    let rest = s[ws_end..].trim_start();
    if rest.is_empty() {
        return;
    }
    // Comment after label?
    if rest.starts_with('#') || rest.starts_with(';') {
        job.append(rest, 0.0, fmt(C_COMMENT, bg));
        return;
    }
    // Mnemonic after label on the same line
    let (tok, after) = split_token(rest);
    if !tok.is_empty() {
        job.append(tok, 0.0, fmt(C_MNEMONIC, bg));
        highlight_operands(job, after, bg);
    }
}

/// Colour operand tokens: registers, numbers, strings, labels, commas.
fn highlight_operands(job: &mut LayoutJob, mut s: &str, bg: Color32) {
    while !s.is_empty() {
        // Whitespace
        if s.starts_with([' ', '\t']) {
            let n = s.find(|c: char| c != ' ' && c != '\t').unwrap_or(s.len());
            job.append(&s[..n], 0.0, fmt(C_DEFAULT, bg));
            s = &s[n..];
            continue;
        }
        // Comment
        if s.starts_with('#') || s.starts_with(';') {
            job.append(s, 0.0, fmt(C_COMMENT, bg));
            return;
        }
        // String literal
        if s.starts_with('"') {
            let end = find_string_end(s);
            job.append(&s[..end], 0.0, fmt(C_STRING, bg));
            s = &s[end..];
            continue;
        }
        // Char literal
        if s.starts_with('\'') {
            let end = s[1..].find('\'').map(|i| i + 2).unwrap_or(s.len());
            job.append(&s[..end], 0.0, fmt(C_STRING, bg));
            s = &s[end..];
            continue;
        }
        // Number: hex, binary, or decimal (possibly negative)
        if s.starts_with("0x")
            || s.starts_with("0X")
            || s.starts_with("0b")
            || s.starts_with("0B")
            || s.starts_with(|c: char| c.is_ascii_digit())
            || (s.starts_with('-') && s[1..].starts_with(|c: char| c.is_ascii_digit()))
        {
            let n = s.find([',', ' ', '\t', '#', ';', ')']).unwrap_or(s.len());
            job.append(&s[..n], 0.0, fmt(C_NUMBER, bg));
            s = &s[n..];
            continue;
        }
        // Comma / paren — default colour
        if s.starts_with([',', '(', ')']) {
            job.append(&s[..1], 0.0, fmt(C_DEFAULT, bg));
            s = &s[1..];
            continue;
        }
        // Token: register or label reference
        let (tok, rest) = split_token(s);
        if tok.is_empty() {
            // Unknown single char — emit and advance
            let n = s.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            job.append(&s[..n], 0.0, fmt(C_DEFAULT, bg));
            s = &s[n..];
            continue;
        }
        let color = if is_register(tok) {
            C_REGISTER
        } else {
            C_DEFAULT
        };
        job.append(tok, 0.0, fmt(color, bg));
        s = rest;
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Split off the first whitespace/comma/paren-delimited token.
fn split_token(s: &str) -> (&str, &str) {
    let end = s.find([' ', '\t', ',', '(', ')']).unwrap_or(s.len());
    (&s[..end], &s[end..])
}

/// Find the end index (exclusive) of a `"..."` string literal.
fn find_string_end(s: &str) -> usize {
    let mut i = 1; // skip opening quote
    let b = s.as_bytes();
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2; // skip escape
        } else if b[i] == b'"' {
            return i + 1;
        } else {
            i += 1;
        }
    }
    s.len()
}

fn is_register(tok: &str) -> bool {
    // ABI names
    const ABI: &[&str] = &[
        "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "t3", "t4", "t5", "t6", "s0", "s1", "s2",
        "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "a0", "a1", "a2", "a3", "a4", "a5",
        "a6", "a7", "fp", "ft0", "ft1", "ft2", "ft3", "ft4", "ft5", "ft6", "ft7", "ft8", "ft9",
        "ft10", "ft11", "fa0", "fa1", "fa2", "fa3", "fa4", "fa5", "fa6", "fa7", "fs0", "fs1",
        "fs2", "fs3", "fs4", "fs5", "fs6", "fs7", "fs8", "fs9", "fs10", "fs11",
        // CSR names
        "fflags", "frm", "fcsr", "cycle", "instret",
    ];
    if ABI.contains(&tok) {
        return true;
    }
    // Numeric: x0–x31, f0–f31
    if let Some(rest) = tok.strip_prefix('x').or_else(|| tok.strip_prefix('f')) {
        if let Ok(n) = rest.parse::<u32>() {
            return n < 32;
        }
    }
    false
}
