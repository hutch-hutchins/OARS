use egui::text::LayoutJob;
use egui::{Color32, FontId, Stroke, TextFormat};

// ─── Palette ──────────────────────────────────────────────────────────────────
const C_DEFAULT: Color32 = Color32::from_rgb(204, 204, 204);
const C_COMMENT: Color32 = Color32::from_rgb(106, 153, 85);
const C_DIRECTIVE: Color32 = Color32::from_rgb(220, 150, 80);
const C_REGISTER: Color32 = Color32::from_rgb(100, 200, 100);
const C_LABEL_DEF: Color32 = Color32::from_rgb(255, 200, 80);
const C_MNEMONIC: Color32 = Color32::from_rgb(100, 180, 255);
const C_STRING: Color32 = Color32::from_rgb(206, 145, 120);
const C_NUMBER: Color32 = Color32::from_rgb(180, 220, 255);
const C_ERR_UNDERLINE: Color32 = Color32::from_rgb(255, 80, 80);

const FONT: FontId = FontId::monospace(13.0);

fn fmt(color: Color32, bg: Color32) -> TextFormat {
    TextFormat {
        font_id: FONT,
        color,
        background: bg,
        ..Default::default()
    }
}

fn fmt_err(color: Color32, bg: Color32) -> TextFormat {
    TextFormat {
        font_id: FONT,
        color,
        background: bg,
        underline: Stroke::new(1.5, C_ERR_UNDERLINE),
        ..Default::default()
    }
}

/// Produce a syntax-highlighted `LayoutJob` for RISC-V assembly source.
/// `error` is `Some((line, col))` where both are 1-based; that token gets
/// a red background and an underline starting at the error column.
pub fn highlight(text: &str, error: Option<(u32, u32)>) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;

    let error_line = error.map(|(l, _)| l);
    let error_col = error.map(|(_, c)| c);

    for (line_idx, line) in text.split('\n').enumerate() {
        let line_no = (line_idx + 1) as u32;
        let err_bg = if error_line == Some(line_no) {
            Color32::from_rgb(90, 20, 20)
        } else {
            Color32::TRANSPARENT
        };

        // Convert 1-based column to 0-based byte offset within this line.
        let err_byte: Option<usize> = if error_line == Some(line_no) {
            error_col.map(|c| (c as usize).saturating_sub(1))
        } else {
            None
        };

        highlight_line(&mut job, line, err_bg, err_byte);

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

fn highlight_line(job: &mut LayoutJob, line: &str, bg: Color32, err_byte: Option<usize>) {
    let mut chars = line.char_indices().peekable();

    // Leading whitespace — always default colour
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
        highlight_operands(job, after, bg, rest_start + tok.len(), err_byte);
        return;
    }

    // Label definition
    let (first_tok, after_first) = split_token(rest);
    if first_tok.ends_with(':') {
        job.append(first_tok, 0.0, fmt(C_LABEL_DEF, bg));
        let after_offset = rest_start + first_tok.len();
        highlight_line_after_label(job, after_first, bg, after_offset, err_byte);
        return;
    }

    // Mnemonic
    if !first_tok.is_empty() {
        let tok_fmt = pick_fmt(C_MNEMONIC, bg, rest_start, first_tok.len(), err_byte);
        job.append(first_tok, 0.0, tok_fmt);
        let after_offset = rest_start + first_tok.len();
        highlight_operands(job, after_first, bg, after_offset, err_byte);
    }
}

fn highlight_line_after_label(
    job: &mut LayoutJob,
    s: &str,
    bg: Color32,
    byte_offset: usize,
    err_byte: Option<usize>,
) {
    let ws_end = s.len() - s.trim_start().len();
    if ws_end > 0 {
        job.append(&s[..ws_end], 0.0, fmt(C_DEFAULT, bg));
    }
    let rest = s[ws_end..].trim_start();
    if rest.is_empty() {
        return;
    }
    let rest_offset = byte_offset + ws_end;
    if rest.starts_with('#') || rest.starts_with(';') {
        job.append(rest, 0.0, fmt(C_COMMENT, bg));
        return;
    }
    let (tok, after) = split_token(rest);
    if !tok.is_empty() {
        let tok_fmt = pick_fmt(C_MNEMONIC, bg, rest_offset, tok.len(), err_byte);
        job.append(tok, 0.0, tok_fmt);
        highlight_operands(job, after, bg, rest_offset + tok.len(), err_byte);
    }
}

/// Colour operand tokens: registers, numbers, strings, labels, commas.
fn highlight_operands(
    job: &mut LayoutJob,
    mut s: &str,
    bg: Color32,
    mut byte_offset: usize,
    err_byte: Option<usize>,
) {
    while !s.is_empty() {
        if s.starts_with([' ', '\t']) {
            let n = s.find(|c: char| c != ' ' && c != '\t').unwrap_or(s.len());
            job.append(&s[..n], 0.0, fmt(C_DEFAULT, bg));
            s = &s[n..];
            byte_offset += n;
            continue;
        }
        if s.starts_with('#') || s.starts_with(';') {
            job.append(s, 0.0, fmt(C_COMMENT, bg));
            return;
        }
        if s.starts_with('"') {
            let end = find_string_end(s);
            let f = pick_fmt(C_STRING, bg, byte_offset, end, err_byte);
            job.append(&s[..end], 0.0, f);
            byte_offset += end;
            s = &s[end..];
            continue;
        }
        if s.starts_with('\'') {
            let end = s[1..].find('\'').map(|i| i + 2).unwrap_or(s.len());
            let f = pick_fmt(C_STRING, bg, byte_offset, end, err_byte);
            job.append(&s[..end], 0.0, f);
            byte_offset += end;
            s = &s[end..];
            continue;
        }
        if s.starts_with("0x")
            || s.starts_with("0X")
            || s.starts_with("0b")
            || s.starts_with("0B")
            || s.starts_with(|c: char| c.is_ascii_digit())
            || (s.starts_with('-') && s[1..].starts_with(|c: char| c.is_ascii_digit()))
        {
            let n = s.find([',', ' ', '\t', '#', ';', ')']).unwrap_or(s.len());
            let f = pick_fmt(C_NUMBER, bg, byte_offset, n, err_byte);
            job.append(&s[..n], 0.0, f);
            byte_offset += n;
            s = &s[n..];
            continue;
        }
        if s.starts_with([',', '(', ')']) {
            job.append(&s[..1], 0.0, fmt(C_DEFAULT, bg));
            byte_offset += 1;
            s = &s[1..];
            continue;
        }
        let (tok, rest) = split_token(s);
        if tok.is_empty() {
            let n = s.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            job.append(&s[..n], 0.0, fmt(C_DEFAULT, bg));
            byte_offset += n;
            s = &s[n..];
            continue;
        }
        let base_color = if is_register(tok) {
            C_REGISTER
        } else {
            C_DEFAULT
        };
        let f = pick_fmt(base_color, bg, byte_offset, tok.len(), err_byte);
        job.append(tok, 0.0, f);
        byte_offset += tok.len();
        s = rest;
    }
}

/// Return `fmt_err` if this token starts at or after the error byte, else `fmt`.
#[inline]
fn pick_fmt(
    color: Color32,
    bg: Color32,
    tok_start: usize,
    _tok_len: usize,
    err_byte: Option<usize>,
) -> TextFormat {
    match err_byte {
        Some(eb) if tok_start >= eb => fmt_err(color, bg),
        _ => fmt(color, bg),
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn split_token(s: &str) -> (&str, &str) {
    let end = s.find([' ', '\t', ',', '(', ')']).unwrap_or(s.len());
    (&s[..end], &s[end..])
}

fn find_string_end(s: &str) -> usize {
    let mut i = 1;
    let b = s.as_bytes();
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
        } else if b[i] == b'"' {
            return i + 1;
        } else {
            i += 1;
        }
    }
    s.len()
}

fn is_register(tok: &str) -> bool {
    const ABI: &[&str] = &[
        "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "t3", "t4", "t5", "t6", "s0", "s1", "s2",
        "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "a0", "a1", "a2", "a3", "a4", "a5",
        "a6", "a7", "fp", "ft0", "ft1", "ft2", "ft3", "ft4", "ft5", "ft6", "ft7", "ft8", "ft9",
        "ft10", "ft11", "fa0", "fa1", "fa2", "fa3", "fa4", "fa5", "fa6", "fa7", "fs0", "fs1",
        "fs2", "fs3", "fs4", "fs5", "fs6", "fs7", "fs8", "fs9", "fs10", "fs11", "fflags", "frm",
        "fcsr", "cycle", "instret",
    ];
    if ABI.contains(&tok) {
        return true;
    }
    if let Some(rest) = tok.strip_prefix('x').or_else(|| tok.strip_prefix('f')) {
        if let Ok(n) = rest.parse::<u32>() {
            return n < 32;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_underline(job: &LayoutJob) -> bool {
        job.sections.iter().any(|s| s.format.underline.width > 0.0)
    }

    fn underlined_text(job: &LayoutJob) -> String {
        job.sections
            .iter()
            .filter(|s| s.format.underline.width > 0.0)
            .map(|s| &job.text[s.byte_range.clone()])
            .collect()
    }

    #[test]
    fn no_error_no_underline() {
        let job = highlight("add a0, a1, a2", None);
        assert!(!has_underline(&job));
    }

    #[test]
    fn error_line_gets_red_background() {
        let job = highlight("add a0, a1, a2", Some((1, 1)));
        let has_red = job
            .sections
            .iter()
            .any(|s| s.format.background == Color32::from_rgb(90, 20, 20));
        assert!(has_red);
    }

    #[test]
    fn error_col_underlines_at_column() {
        // "add a0, a1, a2"
        //  0123456789...
        // col 5 = 'a' of "a0" — first operand
        let job = highlight("add a0, a1, a2", Some((1, 5)));
        assert!(has_underline(&job));
        let ul = underlined_text(&job);
        // a0 starts at col 5, so it should be underlined
        assert!(
            ul.contains("a0"),
            "expected a0 in underlined text, got: {ul:?}"
        );
    }

    #[test]
    fn error_on_different_line_no_underline_on_first() {
        let src = "add a0, a1, a2\nbad_token";
        // Error on line 2
        let job = highlight(src, Some((2, 1)));
        // line 1 should have no underline
        let underlined: String = job
            .sections
            .iter()
            .filter(|s| s.format.underline.width > 0.0)
            .map(|s| &job.text[s.byte_range.clone()])
            .collect();
        assert!(
            !underlined.contains("add"),
            "line 1 should not be underlined, got: {underlined:?}"
        );
    }
}
