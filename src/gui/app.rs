use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use egui::RichText;
use egui_extras::{Column, TableBuilder};

use crate::assembler::{
    codegen::{self, AssemblyOutput},
    export, parser,
};
use crate::hardware::{
    csr::addr as csr_addr,
    fp_registers::FP_REG_NAMES,
    memory::{DATA_BASE, HEAP_BASE, STACK_TOP, TEXT_BASE},
    registers::REG_NAMES,
};
use crate::simulator::{
    backstepper::Backstepper,
    engine::{self, CpuState, StepOutcome},
};

// ─── State enums ─────────────────────────────────────────────────────────────

enum SimState {
    Idle,
    Ready,
    Running,
    Paused,
    WaitingInput,
    WaitingChar,
    StepOver(u32),      // running until PC == this address
    StepOut(u32),       // running until PC == saved ra
    WatchpointHit(u32), // paused after a write to this address
    Halted(i32),
    Error(String, Option<(u32, u32)>), // message, optional (1-based line, 1-based col)
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum MainTab {
    Editor,
    TextSegment,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum BottomTab {
    Console,
    Memory,
    Data,
    Stack,
    Watchpoints,
    CallStack,
    Breakpoints,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum RegisterTab {
    Integer,
    Float,
    Csr,
    Watches,
}

// ─── Watch panel ─────────────────────────────────────────────────────────────

#[derive(Clone)]
enum WatchTarget {
    IntReg(usize),
    FpReg(usize),
    Mem(u32),
}

#[derive(Clone)]
struct Watch {
    label: String,
    target: WatchTarget,
}

// ─── Autocomplete state ───────────────────────────────────────────────────────

#[derive(Clone)]
struct AcEntry {
    display: String,            // shown in the popup (e.g. "li rd, imm")
    insert: String,             // inserted into the editor
    first_ph: Option<(usize, usize)>, // byte range of first placeholder within `insert`
}

struct AcState {
    candidates: Vec<AcEntry>,
    selected: usize,
    anchor: egui::Pos2,
    word_start: usize, // byte offset of the typed prefix start in source
    word_end: usize,   // byte offset of the cursor (end of typed prefix)
}

/// True if `word` is a call instruction (jal/jalr with rd = ra = x1).
fn is_call_instr(word: u32) -> bool {
    let opc = word & 0x7F;
    let rd = (word >> 7) & 0x1F;
    (opc == 0x6F && rd == 1) || (opc == 0x67 && rd == 1 && ((word >> 12) & 0x7) == 0)
}

/// True if `word` is a return instruction (jalr x0, 0(x1)).
fn is_ret_instr(word: u32) -> bool {
    word == 0x0000_8067
}

/// Update call-frame tracking after executing the instruction at `pc`.
fn update_call_stack_frames(frames: &mut Vec<(u32, u32)>, pc: u32, word: u32) {
    if is_call_instr(word) {
        frames.push((pc, pc + 4));
    } else if is_ret_instr(word) {
        frames.pop();
    }
}

/// Parse a hex ("0x…") or decimal string into a u32 address.
fn parse_addr(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        let clean: String = hex.chars().filter(|&c| c != '_').collect();
        u32::from_str_radix(&clean, 16).ok()
    } else {
        s.parse::<u32>().ok()
    }
}

fn parse_watch_target(input: &str) -> Option<WatchTarget> {
    use crate::hardware::{fp_registers::FP_REG_NAMES, registers::REG_NAMES};
    let s = input.trim().to_lowercase();
    for (i, name) in REG_NAMES.iter().enumerate() {
        if s == *name {
            return Some(WatchTarget::IntReg(i));
        }
    }
    if let Some(rest) = s.strip_prefix('x') {
        if let Ok(n) = rest.parse::<usize>() {
            if n < 32 {
                return Some(WatchTarget::IntReg(n));
            }
        }
    }
    for (i, name) in FP_REG_NAMES.iter().enumerate() {
        if s == *name {
            return Some(WatchTarget::FpReg(i));
        }
    }
    if let Some(rest) = s.strip_prefix('f') {
        if let Ok(n) = rest.parse::<usize>() {
            if n < 32 {
                return Some(WatchTarget::FpReg(n));
            }
        }
    }
    if let Some(hex) = s.strip_prefix("0x") {
        let clean: String = hex.chars().filter(|&c| c != '_').collect();
        if let Ok(addr) = u32::from_str_radix(&clean, 16) {
            return Some(WatchTarget::Mem(addr));
        }
    }
    if let Ok(addr) = s.parse::<u32>() {
        return Some(WatchTarget::Mem(addr));
    }
    None
}

// ─── Per-tab state ────────────────────────────────────────────────────────────

struct Tab {
    source: String,
    file_path: Option<PathBuf>,
    cpu: Option<CpuState>,
    asm_out: Option<AssemblyOutput>,
    sim_state: SimState,
    backstepper: Backstepper,
    console_out: String,
    input_buf: String,
    input_queue: VecDeque<String>,
    // breakpoints: address → optional condition expression (None = unconditional)
    breakpoints: HashMap<u32, Option<String>>,
    watchpoint_addrs: HashSet<u32>,
    watchpoint_input: String,
    // Call stack: (call_site_pc, return_addr) pairs, innermost last
    call_frames: Vec<(u32, u32)>,
    // Conditional breakpoint editor inputs
    bp_cond_addr_input: String,
    bp_cond_expr_input: String,
    prev_int_regs: [u32; 32],
    prev_fp_regs: [u64; 32],
    main_tab: MainTab,
    // Find / replace
    show_find: bool,
    find_query: String,
    replace_query: String,
    find_case_sensitive: bool,
    // Editor autocomplete
    ac: Option<AcState>,
    // Char indices [start, end] to select in the editor on the next frame
    pending_cursor: Option<(usize, usize)>,
}

impl Tab {
    fn new() -> Self {
        Self {
            source: DEFAULT_SOURCE.to_owned(),
            file_path: None,
            cpu: None,
            asm_out: None,
            sim_state: SimState::Idle,
            backstepper: Backstepper::new(),
            console_out: String::new(),
            input_buf: String::new(),
            input_queue: VecDeque::new(),
            breakpoints: HashMap::new(),
            watchpoint_addrs: HashSet::new(),
            watchpoint_input: String::new(),
            call_frames: Vec::new(),
            bp_cond_addr_input: String::new(),
            bp_cond_expr_input: String::new(),
            prev_int_regs: [0u32; 32],
            prev_fp_regs: [0u64; 32],
            main_tab: MainTab::Editor,
            show_find: false,
            find_query: String::new(),
            replace_query: String::new(),
            find_case_sensitive: false,
            ac: None,
            pending_cursor: None,
        }
    }

    fn title(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled.s".to_owned())
    }

    // ── Register snapshot helpers ─────────────────────────────────────────────

    fn save_prev_regs(&mut self) {
        if let Some(cpu) = &self.cpu {
            self.prev_int_regs = cpu.regs.snapshot();
            self.prev_fp_regs = cpu.fp.snapshot();
        }
    }

    fn clear_prev_regs(&mut self) {
        self.prev_int_regs = [0u32; 32];
        self.prev_fp_regs = [0u64; 32];
    }

    // ── Actions ───────────────────────────────────────────────────────────────

    fn do_open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Assembly", &["s", "asm"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    self.source = text;
                    self.file_path = Some(path);
                    self.do_reset_state();
                }
                Err(e) => {
                    self.sim_state = SimState::Error(e.to_string(), None);
                }
            }
        }
    }

    fn do_save(&mut self) {
        let path = if let Some(p) = &self.file_path {
            Some(p.clone())
        } else {
            rfd::FileDialog::new()
                .add_filter("Assembly", &["s", "asm"])
                .set_file_name("program.s")
                .save_file()
        };
        if let Some(path) = path {
            if let Err(e) = std::fs::write(&path, &self.source) {
                self.sim_state = SimState::Error(e.to_string(), None);
            } else {
                self.file_path = Some(path);
            }
        }
    }

    fn do_export_binary(&mut self) {
        let Some(ref out) = self.asm_out else { return };
        let Some(ref cpu) = self.cpu else { return };
        let bytes = export::flat_binary(&cpu.mem, out.text_end);
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Binary", &["bin"])
            .set_file_name("program.bin")
            .save_file()
        {
            if let Err(e) = std::fs::write(&path, &bytes) {
                self.sim_state = SimState::Error(e.to_string(), None);
            }
        }
    }

    fn do_export_elf(&mut self) {
        let Some(ref out) = self.asm_out else { return };
        let Some(ref cpu) = self.cpu else { return };
        let bytes = export::elf32(&cpu.mem, out.entry, out.text_end, out.data_end);
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("ELF", &["elf"])
            .set_file_name("program.elf")
            .save_file()
        {
            if let Err(e) = std::fs::write(&path, &bytes) {
                self.sim_state = SimState::Error(e.to_string(), None);
            }
        }
    }

    fn do_assemble(&mut self) -> bool {
        self.do_reset_state();
        let stmts = match parser::parse(&self.source) {
            Ok(s) => s,
            Err(e) => {
                let msg = e.to_string();
                let pos = parse_error_position(&msg);
                self.sim_state = SimState::Error(msg, pos);
                return false;
            }
        };
        let mut cpu = CpuState::new(TEXT_BASE);
        match codegen::assemble(&stmts, &mut cpu.mem) {
            Err(e) => {
                let msg = e.to_string();
                let pos = parse_error_position(&msg);
                self.sim_state = SimState::Error(msg, pos);
                false
            }
            Ok(out) => {
                cpu.pc = out.entry;
                for &addr in &self.watchpoint_addrs {
                    cpu.mem.add_watchpoint(addr);
                }
                self.cpu = Some(cpu);
                self.asm_out = Some(out);
                self.sim_state = SimState::Ready;
                self.main_tab = MainTab::TextSegment;
                true
            }
        }
    }

    fn do_run(&mut self) {
        if self.cpu.is_some() {
            self.sim_state = SimState::Running;
        }
    }

    fn do_step(&mut self) {
        if let Some(ref mut cpu) = self.cpu {
            let saved_pc = cpu.pc;
            let saved_regs = cpu.regs.snapshot();
            let saved_fp = cpu.fp.snapshot();
            self.prev_int_regs = saved_regs;
            self.prev_fp_regs = saved_fp;
            let instr_word = cpu.mem.load_word(saved_pc);
            cpu.mem.begin_journal();
            let outcome = engine::step_one(cpu, &mut self.console_out, &mut self.input_queue);
            let (mem_undo, heap_ptr) = cpu.mem.end_journal();
            let wp_hit = cpu.mem.take_watchpoint_hit();
            self.backstepper
                .push(saved_pc, saved_regs, saved_fp, mem_undo, heap_ptr);
            self.apply_outcome(outcome);
            if matches!(self.sim_state, SimState::Running) {
                self.sim_state = SimState::Paused;
            }
            if let Some(addr) = wp_hit {
                if !matches!(self.sim_state, SimState::Halted(_) | SimState::Error(_, _)) {
                    self.sim_state = SimState::WatchpointHit(addr);
                }
            }
            update_call_stack_frames(&mut self.call_frames, saved_pc, instr_word);
        }
    }

    fn do_step_over(&mut self) {
        let call_target: Option<u32> = if let Some(ref cpu) = self.cpu {
            let word = cpu.mem.load_word(cpu.pc);
            if is_call_instr(word) {
                Some(cpu.pc + 4)
            } else {
                None
            }
        } else {
            return;
        };
        match call_target {
            Some(target) => self.sim_state = SimState::StepOver(target),
            None => self.do_step(),
        }
    }

    fn do_step_out(&mut self) {
        let return_addr = match self.cpu.as_ref() {
            Some(cpu) => cpu.regs.read(1), // ra = x1
            None => return,
        };
        self.sim_state = SimState::StepOut(return_addr);
    }

    fn do_backstep(&mut self) {
        if let Some(ref mut cpu) = self.cpu {
            self.prev_int_regs = cpu.regs.snapshot();
            self.prev_fp_regs = cpu.fp.snapshot();
            if self
                .backstepper
                .pop(&mut cpu.pc, &mut cpu.regs, &mut cpu.fp, &mut cpu.mem)
            {
                self.sim_state = SimState::Paused;
            }
        }
    }

    fn do_pause(&mut self) {
        self.sim_state = SimState::Paused;
    }

    fn do_reset(&mut self) {
        self.do_assemble();
    }

    fn do_reset_state(&mut self) {
        self.cpu = None;
        self.asm_out = None;
        self.sim_state = SimState::Idle;
        self.backstepper = Backstepper::new();
        self.console_out.clear();
        self.input_queue.clear();
        self.input_buf.clear();
        self.call_frames.clear();
        self.ac = None;
        self.clear_prev_regs();
    }

    fn pump_steps(&mut self, n: u32) {
        for _ in 0..n {
            if !matches!(
                self.sim_state,
                SimState::Running | SimState::StepOver(_) | SimState::StepOut(_)
            ) {
                return;
            }
            let target_pc: Option<u32> = match self.sim_state {
                SimState::StepOver(t) | SimState::StepOut(t) => Some(t),
                _ => None,
            };

            // Check stop conditions and capture pre-step info
            let (should_stop, saved_pc, instr_word) = if let Some(ref cpu) = self.cpu {
                let mut stop = false;
                if let Some(target) = target_pc {
                    if cpu.pc == target {
                        stop = true;
                    }
                }
                if !stop {
                    if let Some(cond) = self.breakpoints.get(&cpu.pc) {
                        let hit = match cond {
                            None => true,
                            Some(expr) => eval_condition(expr, cpu),
                        };
                        if hit {
                            stop = true;
                        }
                    }
                }
                (stop, cpu.pc, cpu.mem.load_word(cpu.pc))
            } else {
                (false, 0, 0)
            };

            if should_stop {
                self.sim_state = SimState::Paused;
                return;
            }

            if let Some(ref mut cpu) = self.cpu {
                let outcome = engine::step_one(cpu, &mut self.console_out, &mut self.input_queue);
                let wp_hit = cpu.mem.take_watchpoint_hit();
                self.apply_outcome(outcome);
                if let Some(addr) = wp_hit {
                    if !matches!(self.sim_state, SimState::Halted(_) | SimState::Error(_, _)) {
                        self.sim_state = SimState::WatchpointHit(addr);
                    }
                    return;
                }
            } else {
                self.sim_state = SimState::Idle;
                return;
            }

            update_call_stack_frames(&mut self.call_frames, saved_pc, instr_word);
        }
    }

    fn apply_outcome(&mut self, outcome: StepOutcome) {
        match outcome {
            StepOutcome::Continue => {}
            StepOutcome::NeedInput => {
                self.sim_state = SimState::WaitingInput;
            }
            StepOutcome::NeedChar => {
                self.sim_state = SimState::WaitingChar;
            }
            StepOutcome::Halted(c) => {
                self.sim_state = SimState::Halted(c);
            }
            StepOutcome::Faulted(m) => {
                self.sim_state = SimState::Error(m, None);
            }
        }
        const CAP: usize = 64 * 1024;
        if self.console_out.len() > CAP {
            let trim = self.console_out.len() - (CAP - 4 * 1024);
            if let Some(pos) = self.console_out[trim..].char_indices().next() {
                self.console_out = self.console_out[trim + pos.0..].to_owned();
            }
        }
    }

    fn submit_input(&mut self) {
        let line = format!("{}\n", self.input_buf.trim_end_matches('\n'));
        self.input_queue.push_back(line.clone());
        self.console_out.push_str(&line);
        self.input_buf.clear();
        self.sim_state = SimState::Running;
    }

    fn submit_char(&mut self, c: char) {
        self.console_out.push(c);
        self.input_queue.push_back(c.to_string());
        self.sim_state = SimState::Running;
    }

    // ── Status ────────────────────────────────────────────────────────────────

    fn status_text(&self) -> (String, egui::Color32) {
        match &self.sim_state {
            SimState::Idle => ("Not assembled".into(), egui::Color32::GRAY),
            SimState::Ready => ("Ready".into(), egui::Color32::GREEN),
            SimState::Running => ("Running...".into(), egui::Color32::YELLOW),
            SimState::Paused => ("Paused".into(), egui::Color32::WHITE),
            SimState::StepOver(_) => ("Step Over...".into(), egui::Color32::YELLOW),
            SimState::StepOut(_) => ("Step Out...".into(), egui::Color32::YELLOW),
            SimState::WatchpointHit(a) => {
                (format!("Watchpoint hit: {a:#010x}"), egui::Color32::GOLD)
            }
            SimState::WaitingInput => ("Waiting for input".into(), egui::Color32::LIGHT_BLUE),
            SimState::WaitingChar => ("Waiting for keypress".into(), egui::Color32::LIGHT_BLUE),
            SimState::Halted(0) => ("Halted (exit 0)".into(), egui::Color32::GREEN),
            SimState::Halted(n) => (format!("Halted (exit {n})"), egui::Color32::YELLOW),
            SimState::Error(m, _) => (format!("Error: {m}"), egui::Color32::RED),
        }
    }

    // ── Panel methods ─────────────────────────────────────────────────────────

    fn show_editor(&mut self, ui: &mut egui::Ui) {
        let error_pos: Option<(u32, u32)> = match &self.sim_state {
            SimState::Error(_, pos) => *pos,
            _ => None,
        };

        // Ctrl+F opens find bar
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            self.show_find = true;
        }
        // Escape: close autocomplete first, then find bar
        if self.ac.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_find = false;
        }

        if self.show_find {
            ui.horizontal(|ui| {
                ui.label("Find:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.find_query)
                        .desired_width(180.0)
                        .hint_text("search…"),
                );
                ui.label("Replace:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.replace_query)
                        .desired_width(180.0)
                        .hint_text("replacement…"),
                );
                ui.checkbox(&mut self.find_case_sensitive, "Aa");
                let match_count = if self.find_query.is_empty() {
                    0
                } else {
                    find_matches_in(&self.source, &self.find_query, self.find_case_sensitive).len()
                };
                if !self.find_query.is_empty() {
                    ui.label(format!("{match_count} match(es)"));
                }
                ui.separator();
                if ui.button("Replace All").clicked() && !self.find_query.is_empty() {
                    let q = self.find_query.clone();
                    let r = self.replace_query.clone();
                    let cs = self.find_case_sensitive;
                    let (new_src, _) = replace_all_in(&self.source, &q, &r, cs);
                    self.source = new_src;
                }
                if ui.button("✕").clicked() {
                    self.show_find = false;
                }
            });
            ui.separator();
        }

        // ── Autocomplete keyboard handling (must happen before TextEdit renders) ─
        let mut ac_nav: i32 = 0;
        let mut ac_accept = false;
        let mut ac_dismiss = false;
        if self.ac.is_some() {
            ui.input_mut(|i| {
                i.events.retain(|e| match e {
                    egui::Event::Key {
                        key: egui::Key::ArrowDown,
                        pressed: true,
                        ..
                    } => {
                        ac_nav += 1;
                        false
                    }
                    egui::Event::Key {
                        key: egui::Key::ArrowUp,
                        pressed: true,
                        ..
                    } => {
                        ac_nav -= 1;
                        false
                    }
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        pressed: true,
                        ..
                    } => {
                        ac_dismiss = true;
                        false
                    }
                    egui::Event::Key {
                        key: egui::Key::Tab | egui::Key::Enter,
                        pressed: true,
                        ..
                    } => {
                        ac_accept = true;
                        false
                    }
                    _ => true,
                });
            });
        }
        // Apply navigation
        if ac_nav != 0 {
            if let Some(ref mut ac) = self.ac {
                let n = ac.candidates.len() as i32;
                ac.selected = ((ac.selected as i32 + ac_nav).rem_euclid(n)) as usize;
            }
        }
        // Apply accepted completion to source before TextEdit renders so it sees the result
        let mut cursor_to_set: Option<(usize, usize)> = None;
        if ac_accept {
            if let Some(ref ac) = self.ac {
                let entry = ac.candidates[ac.selected].clone();
                let word_start = ac.word_start;
                let word_end = ac.word_end.min(self.source.len());
                self.source.replace_range(word_start..word_end, &entry.insert);
                if let Some((ph_start_b, ph_end_b)) = entry.first_ph {
                    let abs_start = (word_start + ph_start_b).min(self.source.len());
                    let abs_end = (word_start + ph_end_b).min(self.source.len());
                    cursor_to_set = Some((
                        self.source[..abs_start].chars().count(),
                        self.source[..abs_end].chars().count(),
                    ));
                }
            }
            self.ac = None;
        } else if ac_dismiss {
            self.ac = None;
        }
        let pending_cursor = self.pending_cursor.take();

        // ── Editor ────────────────────────────────────────────────────────────
        let ctx = ui.ctx().clone();
        let source = &mut self.source;
        let line_count = source.split('\n').count();
        let gutter_width = line_count.to_string().len();
        let gutter_text: String = (1..=line_count)
            .map(|n| format!("{n:>gutter_width$}\n"))
            .collect();

        let mut new_ac: Option<AcState> = None;
        let mut dismiss_ac = false;

        egui::ScrollArea::both()
            .id_salt("editor")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&gutter_text)
                                .monospace()
                                .size(13.0)
                                .color(egui::Color32::from_rgb(80, 90, 100)),
                        )
                        .selectable(false),
                    );

                    ui.separator();

                    let mut layouter = |_ui: &egui::Ui, text: &str, wrap_width: f32| {
                        let mut job = super::highlighter::highlight(text, error_pos);
                        job.wrap.max_width = wrap_width;
                        _ui.fonts(|f| f.layout_job(job))
                    };

                    let mut te_out = egui::TextEdit::multiline(source)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .lock_focus(true)
                        .layouter(&mut layouter)
                        .show(ui);

                    // Apply cursor selection from this-frame Tab-accept or previous click
                    if let Some((sel_start, sel_end)) = cursor_to_set.or(pending_cursor) {
                        te_out.state.cursor.set_char_range(Some(
                            egui::text::CCursorRange::two(
                                egui::text::CCursor::new(sel_start),
                                egui::text::CCursor::new(sel_end),
                            ),
                        ));
                        te_out.state.store(ui.ctx(), te_out.response.id);
                        ui.ctx().request_repaint();
                    }

                    let force_open = te_out.response.has_focus()
                        && ui.input(|i| {
                            i.modifiers.ctrl && i.key_pressed(egui::Key::Space)
                        });

                    if te_out.response.changed() || force_open {
                        if let Some(cr) = te_out.cursor_range {
                            let byte_idx = cr.primary.ccursor.index;
                            let prefix = word_before_cursor(source, byte_idx).to_owned();
                            let candidates =
                                compute_completions(&prefix, source, byte_idx);
                            if !candidates.is_empty() {
                                let cr_rect = te_out.galley.pos_from_cursor(&cr.primary);
                                let anchor = te_out.galley_pos
                                    + egui::vec2(cr_rect.min.x, cr_rect.max.y);
                                new_ac = Some(AcState {
                                    candidates,
                                    selected: 0,
                                    anchor,
                                    word_start: byte_idx.saturating_sub(prefix.len()),
                                    word_end: byte_idx,
                                });
                            } else {
                                dismiss_ac = true;
                            }
                        }
                    } else if !te_out.response.has_focus() {
                        dismiss_ac = true;
                    }
                });
            });

        if let Some(ac) = new_ac {
            self.ac = Some(ac);
        } else if dismiss_ac {
            self.ac = None;
        }

        // ── Autocomplete popup overlay ────────────────────────────────────────
        let click_result: Option<(usize, usize, AcEntry)> =
            if let Some(ref ac) = self.ac {
                let word_start = ac.word_start;
                let word_end = ac.word_end;
                let candidates = ac.candidates.clone();
                let selected = ac.selected;
                let anchor = ac.anchor;

                let mut clicked_idx: Option<usize> = None;
                egui::Area::new(egui::Id::new("ac_popup"))
                    .fixed_pos(anchor)
                    .order(egui::Order::Foreground)
                    .show(&ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_min_width(300.0);
                            egui::ScrollArea::vertical()
                                .max_height(240.0)
                                .show(ui, |ui| {
                                    ui.set_min_width(300.0);
                                    for (i, entry) in candidates.iter().enumerate() {
                                        let resp = ui.selectable_label(
                                            i == selected,
                                            RichText::new(entry.display.as_str()).monospace(),
                                        );
                                        if resp.clicked() {
                                            clicked_idx = Some(i);
                                        }
                                        if i == selected {
                                            resp.scroll_to_me(None);
                                        }
                                    }
                                });
                            ui.separator();
                            ui.add(
                                egui::Label::new(
                                    RichText::new("Enter/Tab accept  up/down navigate  Esc dismiss")
                                        .small()
                                        .weak(),
                                )
                                .wrap_mode(egui::TextWrapMode::Extend),
                            );
                        });
                    });

                clicked_idx.map(|idx| (word_start, word_end, candidates[idx].clone()))
            } else {
                None
            };

        if let Some((start, end, entry)) = click_result {
            let safe_end = end.min(self.source.len());
            self.source.replace_range(start..safe_end, &entry.insert);
            if let Some((ph_start_b, ph_end_b)) = entry.first_ph {
                let abs_start = (start + ph_start_b).min(self.source.len());
                let abs_end = (start + ph_end_b).min(self.source.len());
                self.pending_cursor = Some((
                    self.source[..abs_start].chars().count(),
                    self.source[..abs_end].chars().count(),
                ));
            }
            self.ac = None;
        }
    }

    fn show_console(&mut self, ui: &mut egui::Ui) {
        let waiting_line = matches!(self.sim_state, SimState::WaitingInput);
        let waiting_char = matches!(self.sim_state, SimState::WaitingChar);

        // Capture a single keypress for read_char (syscall 12)
        if waiting_char {
            let mut got: Option<char> = None;
            ui.input(|i| {
                for event in &i.events {
                    match event {
                        egui::Event::Text(s) => {
                            if let Some(c) = s.chars().next() {
                                got = Some(c);
                                break;
                            }
                        }
                        egui::Event::Key {
                            key: egui::Key::Enter,
                            pressed: true,
                            ..
                        } => {
                            got = Some('\n');
                            break;
                        }
                        _ => {}
                    }
                }
            });
            if let Some(c) = got {
                self.submit_char(c);
            }
        }

        let avail = if waiting_line {
            ui.available_height() - 40.0
        } else {
            ui.available_height()
        };

        // Append a block cursor when waiting for a single character
        let display: std::borrow::Cow<str> = if waiting_char {
            format!("{}█", self.console_out).into()
        } else {
            (&self.console_out).into()
        };

        egui::ScrollArea::vertical()
            .id_salt("console")
            .auto_shrink([false; 2])
            .max_height(avail.max(40.0))
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(RichText::new(display.as_ref()).monospace().size(12.0))
                        .selectable(true),
                );
            });

        if waiting_line {
            ui.separator();
            let resp = ui.horizontal(|ui| {
                ui.label("stdin:");
                let te = ui.add(
                    egui::TextEdit::singleline(&mut self.input_buf)
                        .hint_text("Enter input and press Enter")
                        .desired_width(f32::INFINITY - 60.0),
                );
                let send = ui.button("Send");
                let enter_pressed =
                    te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                send.clicked() || enter_pressed
            });
            if resp.inner {
                self.submit_input();
            }
        }
    }

    fn show_data_segment(&mut self, ui: &mut egui::Ui) {
        let Some(cpu) = &self.cpu else {
            ui.centered_and_justified(|ui| {
                ui.label("Assemble first to view the data segment.");
            });
            return;
        };
        let addr_to_labels = self
            .asm_out
            .as_ref()
            .map(|a| &a.addr_to_labels)
            .cloned()
            .unwrap_or_default();

        const BYTES_PER_ROW: u32 = 16;
        let mut display_rows: Vec<(u32, [u32; 4])> = Vec::new();
        let mut addr = DATA_BASE;
        while addr < HEAP_BASE {
            let words: [u32; 4] = std::array::from_fn(|j| cpu.mem.load_word(addr + j as u32 * 4));
            let has_label = addr_to_labels.contains_key(&addr)
                || addr_to_labels.contains_key(&(addr + 4))
                || addr_to_labels.contains_key(&(addr + 8))
                || addr_to_labels.contains_key(&(addr + 12));
            if has_label || words.iter().any(|&w| w != 0) {
                display_rows.push((addr, words));
            }
            addr = addr.wrapping_add(BYTES_PER_ROW);
        }

        if display_rows.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("Data segment is empty.");
            });
            return;
        }

        TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(100.0).resizable(true))
            .column(Column::initial(110.0).resizable(true))
            .column(Column::initial(88.0).resizable(true))
            .column(Column::initial(88.0).resizable(true))
            .column(Column::initial(88.0).resizable(true))
            .column(Column::initial(88.0).resizable(true))
            .column(Column::remainder())
            .header(20.0, |mut h| {
                h.col(|ui| {
                    ui.strong("Label");
                });
                h.col(|ui| {
                    ui.strong("Address");
                });
                h.col(|ui| {
                    ui.strong("+0");
                });
                h.col(|ui| {
                    ui.strong("+4");
                });
                h.col(|ui| {
                    ui.strong("+8");
                });
                h.col(|ui| {
                    ui.strong("+C");
                });
                h.col(|ui| {
                    ui.strong("ASCII");
                });
            })
            .body(|body| {
                body.rows(18.0, display_rows.len(), |mut row| {
                    let (row_addr, words) = display_rows[row.index()];
                    let row_labels: String = (0..4u32)
                        .filter_map(|j| addr_to_labels.get(&(row_addr + j * 4)))
                        .flat_map(|v| v.iter())
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");

                    row.col(|ui| {
                        if !row_labels.is_empty() {
                            ui.label(
                                RichText::new(row_labels.as_str())
                                    .monospace()
                                    .color(egui::Color32::from_rgb(255, 200, 80)),
                            );
                        }
                    });
                    row.col(|ui| {
                        ui.label(
                            RichText::new(format!("{row_addr:#010x}"))
                                .monospace()
                                .color(egui::Color32::GRAY),
                        );
                    });
                    for w in &words {
                        row.col(|ui| {
                            ui.label(RichText::new(format!("{w:#010x}")).monospace());
                        });
                    }
                    row.col(|ui| {
                        let ascii: String = (0..16u32)
                            .map(|j| cpu.mem.load_byte(row_addr + j))
                            .map(|b| {
                                if (32..127).contains(&b) {
                                    b as char
                                } else {
                                    '.'
                                }
                            })
                            .collect();
                        ui.label(RichText::new(ascii).monospace().weak());
                    });
                });
            });
    }

    fn show_stack_viewer(&mut self, ui: &mut egui::Ui) {
        let Some(cpu) = &self.cpu else {
            ui.centered_and_justified(|ui| {
                ui.label("Assemble first to view the stack.");
            });
            return;
        };

        let sp = cpu.regs.read(2);

        const WORD_ROWS: u32 = 80;
        let view_top = STACK_TOP.saturating_sub((WORD_ROWS - 16) * 4) & !0x3;

        TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(28.0).resizable(false))
            .column(Column::initial(110.0).resizable(true))
            .column(Column::initial(110.0).resizable(true))
            .column(Column::initial(110.0).resizable(true))
            .column(Column::remainder())
            .header(20.0, |mut h| {
                h.col(|_| {});
                h.col(|ui| {
                    ui.strong("Address");
                });
                h.col(|ui| {
                    ui.strong("Hex");
                });
                h.col(|ui| {
                    ui.strong("Signed");
                });
                h.col(|ui| {
                    ui.strong("Unsigned");
                });
            })
            .body(|body| {
                body.rows(18.0, WORD_ROWS as usize, |mut row| {
                    let addr = STACK_TOP.wrapping_sub(row.index() as u32 * 4) & !0x3;
                    if addr < view_top {
                        row.col(|_| {});
                        row.col(|_| {});
                        row.col(|_| {});
                        row.col(|_| {});
                        row.col(|_| {});
                        return;
                    }
                    let val = cpu.mem.load_word(addr);
                    let is_sp = addr == sp;
                    let used = addr >= sp;
                    let addr_color = if is_sp {
                        egui::Color32::YELLOW
                    } else if used {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::DARK_GRAY
                    };

                    row.col(|ui| {
                        if is_sp {
                            ui.label(RichText::new("sp→").small().color(egui::Color32::YELLOW));
                        }
                    });
                    row.col(|ui| {
                        ui.label(
                            RichText::new(format!("{addr:#010x}"))
                                .monospace()
                                .color(addr_color),
                        );
                    });
                    row.col(|ui| {
                        let t = RichText::new(format!("{val:#010x}")).monospace();
                        ui.label(if is_sp {
                            t.color(egui::Color32::YELLOW)
                        } else {
                            t
                        });
                    });
                    row.col(|ui| {
                        ui.label(
                            RichText::new(format!("{}", val as i32))
                                .monospace()
                                .color(addr_color),
                        );
                    });
                    row.col(|ui| {
                        ui.label(
                            RichText::new(format!("{val}"))
                                .monospace()
                                .color(addr_color),
                        );
                    });
                });
            });
    }

    fn show_text_segment(&mut self, ui: &mut egui::Ui) {
        let current_pc = self.cpu.as_ref().map(|c| c.pc);
        let source_lines: Vec<&str> = self.source.lines().collect();

        if self.asm_out.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("Assemble first to view the text segment.");
            });
            return;
        }

        let rows: Vec<(u32, u32, u32, Option<String>)> = {
            let asm = self.asm_out.as_ref().unwrap();
            asm.text_rows
                .iter()
                .map(|tr| {
                    let label = asm.addr_to_labels.get(&tr.addr).map(|v| v.join(", "));
                    (tr.addr, tr.word, tr.src_line, label)
                })
                .collect()
        };

        TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(28.0).resizable(false))
            .column(Column::initial(90.0).resizable(true))
            .column(Column::initial(110.0).resizable(true))
            .column(Column::initial(100.0).resizable(true))
            .column(Column::remainder())
            .header(20.0, |mut h| {
                h.col(|ui| {
                    ui.weak("⬤");
                });
                h.col(|ui| {
                    ui.strong("Label");
                });
                h.col(|ui| {
                    ui.strong("Address");
                });
                h.col(|ui| {
                    ui.strong("Machine Code");
                });
                h.col(|ui| {
                    ui.strong("Source");
                });
            })
            .body(|body| {
                body.rows(18.0, rows.len(), |mut row| {
                    let i = row.index();
                    let (addr, word, src_line, ref label) = rows[i];
                    let hot = current_pc == Some(addr);
                    let bp = self.breakpoints.contains_key(&addr);
                    let src = source_lines
                        .get(src_line.saturating_sub(1) as usize)
                        .copied()
                        .unwrap_or("")
                        .trim();

                    row.col(|ui| {
                        let dot = RichText::new("⬤").small().color(if bp {
                            egui::Color32::RED
                        } else {
                            egui::Color32::TRANSPARENT
                        });
                        if ui
                            .add(egui::Label::new(dot).sense(egui::Sense::click()))
                            .clicked()
                        {
                            if bp {
                                self.breakpoints.remove(&addr);
                            } else {
                                self.breakpoints.insert(addr, None);
                            }
                        }
                        if hot {
                            ui.label(RichText::new("→").color(egui::Color32::YELLOW));
                        }
                    });

                    row.col(|ui| {
                        if let Some(lbl) = label {
                            ui.label(
                                RichText::new(lbl.as_str())
                                    .monospace()
                                    .color(egui::Color32::from_rgb(255, 200, 80)),
                            );
                        }
                    });

                    row.col(|ui| {
                        let t = RichText::new(format!("{addr:#010x}")).monospace();
                        ui.label(if hot {
                            t.color(egui::Color32::YELLOW)
                        } else {
                            t
                        });
                    });

                    row.col(|ui| {
                        ui.label(RichText::new(format!("{word:#010x}")).monospace());
                    });

                    row.col(|ui| {
                        let t = RichText::new(src).monospace();
                        let resp = ui.label(if hot {
                            t.color(egui::Color32::YELLOW)
                        } else {
                            t
                        });
                        if hot {
                            resp.scroll_to_me(None);
                        }
                    });
                });
            });
    }

    fn show_call_stack(&self, ui: &mut egui::Ui) {
        let Some(cpu) = &self.cpu else {
            ui.centered_and_justified(|ui| {
                ui.label("Assemble first to see the call stack.");
            });
            return;
        };

        let addr_to_labels = self
            .asm_out
            .as_ref()
            .map(|a| a.addr_to_labels.clone())
            .unwrap_or_default();

        // Find the nearest label at or before `addr`.
        let fn_name_at = |addr: u32| -> String {
            addr_to_labels
                .iter()
                .filter(|(&a, _)| a <= addr)
                .max_by_key(|(&a, _)| a)
                .and_then(|(_, labels)| labels.first().cloned())
                .unwrap_or_else(|| format!("{addr:#010x}"))
        };

        egui::ScrollArea::vertical()
            .id_salt("call_stack_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!(
                        "→ {}  ({:#010x})",
                        fn_name_at(cpu.pc),
                        cpu.pc
                    ))
                    .monospace()
                    .color(egui::Color32::YELLOW),
                );
                for &(call_site, _) in self.call_frames.iter().rev() {
                    ui.label(
                        RichText::new(format!(
                            "  ← {}  ({:#010x})",
                            fn_name_at(call_site),
                            call_site
                        ))
                        .monospace()
                        .weak(),
                    );
                }
                if self.call_frames.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("No calls traced yet. Step or run to build the stack.")
                            .weak()
                            .small(),
                    );
                }
            });
    }

    fn show_breakpoints(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new(
                "Click ⬤ in the Text Segment to toggle breakpoints. \
                 Add an optional condition below.",
            )
            .weak()
            .small(),
        );
        ui.separator();

        if self.breakpoints.is_empty() {
            ui.label(RichText::new("No breakpoints set.").weak());
        } else {
            let mut to_remove: Option<u32> = None;
            let addrs: Vec<u32> = {
                let mut v: Vec<u32> = self.breakpoints.keys().copied().collect();
                v.sort();
                v
            };
            egui::ScrollArea::vertical()
                .id_salt("bp_scroll")
                .max_height(120.0)
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for addr in addrs {
                        let cond = self.breakpoints.get(&addr).cloned().flatten();
                        ui.horizontal(|ui| {
                            let color = ui.visuals().text_color();
                            ui.label(
                                RichText::new(format!("{addr:#010x}"))
                                    .monospace()
                                    .color(color),
                            );
                            if let Some(expr) = cond {
                                ui.label(
                                    RichText::new(format!("if {expr}"))
                                        .monospace()
                                        .weak(),
                                );
                            } else {
                                ui.label(RichText::new("(always)").weak());
                            }
                            if ui.small_button("×").clicked() {
                                to_remove = Some(addr);
                            }
                        });
                    }
                });
            if let Some(addr) = to_remove {
                self.breakpoints.remove(&addr);
            }
        }

        ui.separator();
        ui.label(RichText::new("Set / edit condition:").small());
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.bp_cond_addr_input)
                    .desired_width(120.0)
                    .hint_text("PC  e.g. 0x00400010"),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.bp_cond_expr_input)
                    .desired_width(200.0)
                    .hint_text("condition  e.g. a0 == 5  (blank = always)"),
            );
            if ui.button("Set").clicked() && !self.bp_cond_addr_input.is_empty() {
                if let Some(addr) = parse_addr(&self.bp_cond_addr_input) {
                    let expr = self.bp_cond_expr_input.trim().to_owned();
                    let cond = if expr.is_empty() { None } else { Some(expr) };
                    self.breakpoints.insert(addr, cond);
                    self.bp_cond_addr_input.clear();
                    self.bp_cond_expr_input.clear();
                }
            }
        });
    }

    fn show_watchpoints(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Address:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.watchpoint_input)
                    .hint_text("0x10010000 or decimal")
                    .desired_width(160.0),
            );
            let add = (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                || ui.button("Add").clicked();
            if add && !self.watchpoint_input.is_empty() {
                if let Some(addr) = parse_addr(&self.watchpoint_input) {
                    self.watchpoint_addrs.insert(addr);
                    if let Some(ref mut cpu) = self.cpu {
                        cpu.mem.add_watchpoint(addr);
                    }
                    self.watchpoint_input.clear();
                }
            }
            if ui.button("Clear All").clicked() {
                self.watchpoint_addrs.clear();
                if let Some(ref mut cpu) = self.cpu {
                    cpu.mem.clear_watchpoints();
                }
            }
        });
        ui.separator();

        if self.watchpoint_addrs.is_empty() {
            ui.label(RichText::new("No watchpoints set.").weak());
        } else {
            let mut to_remove: Option<u32> = None;
            let addrs: Vec<u32> = {
                let mut v: Vec<u32> = self.watchpoint_addrs.iter().copied().collect();
                v.sort();
                v
            };
            egui::ScrollArea::vertical()
                .id_salt("wp_scroll")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for addr in addrs {
                        ui.horizontal(|ui| {
                            let hit =
                                matches!(self.sim_state, SimState::WatchpointHit(a) if a == addr);
                            let color = if hit {
                                egui::Color32::GOLD
                            } else {
                                ui.visuals().text_color()
                            };
                            ui.label(
                                RichText::new(format!("{addr:#010x}"))
                                    .monospace()
                                    .color(color),
                            );
                            if ui.small_button("×").clicked() {
                                to_remove = Some(addr);
                            }
                        });
                    }
                });
            if let Some(addr) = to_remove {
                self.watchpoint_addrs.remove(&addr);
                if let Some(ref mut cpu) = self.cpu {
                    cpu.mem.remove_watchpoint(addr);
                }
            }
        }
    }
}

// ─── App ─────────────────────────────────────────────────────────────────────

#[derive(PartialEq, Eq, Clone, Copy)]
enum HelpTab {
    Pseudo,
    Rv32i,
    Rv32m,
    Rv32f,
    Rv32d,
    Zicsr,
    Directives,
    Syscalls,
}

pub struct OarsApp {
    tabs: Vec<Tab>,
    active: usize,

    steps_per_frame: u32,
    bottom_tab: BottomTab,
    mem_view_base: u32,
    register_tab: RegisterTab,
    watches: Vec<Watch>,
    watch_input: String,
    show_help: bool,
    help_tab: HelpTab,
    show_about: bool,
    dark_mode: bool,
}

const DEFAULT_SOURCE: &str = "\
        .text
        .globl main
main:
        # Write your RISC-V assembly here, then click Assemble & Run.
        li      a7, 10          # exit
        ecall
";

const CHANGED_COLOR: egui::Color32 = egui::Color32::from_rgb(100, 220, 100);

impl OarsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let mut style = (*cc.egui_ctx.style()).clone();
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
        cc.egui_ctx.set_style(style);

        Self {
            tabs: vec![Tab::new()],
            active: 0,
            steps_per_frame: 50_000,
            bottom_tab: BottomTab::Console,
            mem_view_base: DATA_BASE,
            register_tab: RegisterTab::Integer,
            watches: Vec::new(),
            watch_input: String::new(),
            show_help: false,
            help_tab: HelpTab::Pseudo,
            show_about: false,
            dark_mode: true,
        }
    }

    // ── Panels ────────────────────────────────────────────────────────────────

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let assembled = self.tabs[self.active].cpu.is_some();
            let running = matches!(
                self.tabs[self.active].sim_state,
                SimState::Running | SimState::StepOver(_) | SimState::StepOut(_)
            );
            let waiting = matches!(
                self.tabs[self.active].sim_state,
                SimState::WaitingInput | SimState::WaitingChar
            );
            let steppable = assembled && !running && !waiting;
            let can_back = assembled && !running && !self.tabs[self.active].backstepper.is_empty();

            if ui.button("Assemble").clicked() {
                self.tabs[self.active].do_assemble();
            }
            let can_run = self.tabs[self.active].cpu.is_some()
                && !matches!(
                    self.tabs[self.active].sim_state,
                    SimState::Running
                        | SimState::StepOver(_)
                        | SimState::StepOut(_)
                        | SimState::WaitingInput
                        | SimState::WaitingChar
                );
            if ui.add_enabled(can_run, egui::Button::new("Run")).clicked() {
                self.tabs[self.active].do_run();
            }

            ui.separator();

            if ui
                .add_enabled(steppable, egui::Button::new("Step"))
                .clicked()
            {
                self.tabs[self.active].do_step();
            }
            if ui
                .add_enabled(steppable, egui::Button::new("Step Over"))
                .on_hover_text("Step over call instructions")
                .clicked()
            {
                self.tabs[self.active].do_step_over();
            }
            if ui
                .add_enabled(steppable, egui::Button::new("Step Out"))
                .on_hover_text("Run until return from current function")
                .clicked()
            {
                self.tabs[self.active].do_step_out();
            }
            if ui
                .add_enabled(can_back, egui::Button::new("Backstep"))
                .clicked()
            {
                self.tabs[self.active].do_backstep();
            }
            if ui
                .add_enabled(running || waiting, egui::Button::new("Pause"))
                .clicked()
            {
                self.tabs[self.active].do_pause();
            }
            if ui
                .add_enabled(assembled, egui::Button::new("Reset"))
                .clicked()
            {
                self.tabs[self.active].do_reset();
            }

            ui.separator();

            let (msg, color) = self.tabs[self.active].status_text();
            ui.label(RichText::new(msg).color(color));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(p) = &self.tabs[self.active].file_path {
                    ui.label(RichText::new(p.display().to_string()).weak().small());
                    ui.separator();
                }
                let label = if self.steps_per_frame >= 1_000_000 {
                    format!("{:.0}M/frame", self.steps_per_frame as f32 / 1_000_000.0)
                } else if self.steps_per_frame >= 1_000 {
                    format!("{:.0}K/frame", self.steps_per_frame as f32 / 1_000.0)
                } else {
                    format!("{}/frame", self.steps_per_frame)
                };
                ui.label(RichText::new(label).small());
                let mut log_val = (self.steps_per_frame as f32).log10();
                if ui
                    .add(
                        egui::Slider::new(&mut log_val, 0.0_f32..=6.0_f32)
                            .show_value(false)
                            .text("Speed"),
                    )
                    .changed()
                {
                    self.steps_per_frame = (10_f32.powf(log_val) as u32).max(1);
                }
            });
        });
    }

    fn show_registers(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.register_tab, RegisterTab::Integer, "Integer");
            ui.selectable_value(&mut self.register_tab, RegisterTab::Float, "Float");
            ui.selectable_value(&mut self.register_tab, RegisterTab::Csr, "CSR");
            ui.selectable_value(&mut self.register_tab, RegisterTab::Watches, "Watches");
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("reg_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| match self.register_tab {
                RegisterTab::Integer => {
                    let vals: Vec<u32> = (0..32)
                        .map(|i| {
                            self.tabs[self.active]
                                .cpu
                                .as_ref()
                                .map_or(0, |c| c.regs.read(i))
                        })
                        .collect();
                    let prev = self.tabs[self.active].prev_int_regs;
                    egui::Grid::new("int_regs")
                        .num_columns(4)
                        .spacing([6.0, 1.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.weak("Num");
                            ui.weak("Name");
                            ui.weak("Hex");
                            ui.weak("Dec");
                            ui.end_row();
                            for (i, name) in REG_NAMES.iter().enumerate() {
                                let val = vals[i];
                                let changed = val != prev[i];
                                let color = if changed && i != 0 {
                                    CHANGED_COLOR
                                } else {
                                    egui::Color32::GRAY
                                };
                                ui.label(
                                    RichText::new(format!("x{i:02}")).monospace().color(color),
                                );
                                ui.label(RichText::new(*name).monospace().color(color));
                                ui.label(
                                    RichText::new(format!("{val:#010x}"))
                                        .monospace()
                                        .color(color),
                                );
                                ui.label(
                                    RichText::new(format!("{}", val as i32))
                                        .monospace()
                                        .color(color),
                                );
                                ui.end_row();
                            }
                        });
                }
                RegisterTab::Float => {
                    let fp_snap: [u64; 32] = self.tabs[self.active]
                        .cpu
                        .as_ref()
                        .map(|c| c.fp.snapshot())
                        .unwrap_or([0u64; 32]);
                    let prev = self.tabs[self.active].prev_fp_regs;
                    egui::Grid::new("fp_regs")
                        .num_columns(4)
                        .spacing([6.0, 1.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.weak("Num");
                            ui.weak("Name");
                            ui.weak("Hex");
                            ui.weak("Float");
                            ui.end_row();
                            for (i, fp_name) in FP_REG_NAMES.iter().enumerate() {
                                let raw = fp_snap[i];
                                let changed = raw != prev[i];
                                let color = if changed {
                                    CHANGED_COLOR
                                } else {
                                    egui::Color32::GRAY
                                };
                                let as_f64 = f64::from_bits(raw);
                                ui.label(
                                    RichText::new(format!("f{i:02}")).monospace().color(color),
                                );
                                ui.label(RichText::new(*fp_name).monospace().color(color));
                                ui.label(
                                    RichText::new(format!("{raw:#018x}"))
                                        .monospace()
                                        .color(color),
                                );
                                ui.label(
                                    RichText::new(format!("{as_f64:.6}"))
                                        .monospace()
                                        .color(color),
                                );
                                ui.end_row();
                            }
                        });
                }
                RegisterTab::Csr => {
                    let pc = self.tabs[self.active].cpu.as_ref().map_or(0u32, |c| c.pc);
                    let csr_vals: Vec<u32> = {
                        let addrs: &[u32] = &[
                            u32::MAX,
                            csr_addr::FFLAGS,
                            csr_addr::FRM,
                            csr_addr::FCSR,
                            csr_addr::CYCLE,
                            csr_addr::INSTRET,
                            csr_addr::MISA,
                        ];
                        addrs
                            .iter()
                            .map(|&a| {
                                if a == u32::MAX {
                                    pc
                                } else {
                                    self.tabs[self.active]
                                        .cpu
                                        .as_ref()
                                        .map_or(0, |c| c.csr.read(a))
                                }
                            })
                            .collect()
                    };
                    let csr_rows: &[(u32, &str, &str)] = &[
                        (u32::MAX, "PC", "Program Counter"),
                        (csr_addr::FFLAGS, "fflags", "FP Exception Flags"),
                        (csr_addr::FRM, "frm", "FP Rounding Mode"),
                        (csr_addr::FCSR, "fcsr", "FP Control/Status"),
                        (csr_addr::CYCLE, "cycle", "Cycle Counter (lo)"),
                        (csr_addr::INSTRET, "instret", "Instructions Retired (lo)"),
                        (csr_addr::MISA, "misa", "ISA Extensions"),
                    ];
                    egui::Grid::new("csr_regs")
                        .num_columns(3)
                        .spacing([6.0, 1.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.weak("Name");
                            ui.weak("Value");
                            ui.weak("Description");
                            ui.end_row();
                            for (idx, (_, name, desc)) in csr_rows.iter().enumerate() {
                                let val = csr_vals[idx];
                                ui.label(
                                    RichText::new(*name).monospace().color(egui::Color32::GRAY),
                                );
                                ui.label(RichText::new(format!("{val:#010x}")).monospace());
                                ui.label(RichText::new(*desc).small().weak());
                                ui.end_row();
                            }
                        });
                }
                RegisterTab::Watches => {
                    self.show_watches(ui);
                }
            });
    }

    fn show_watches(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.watch_input)
                    .desired_width(130.0)
                    .hint_text("a0, x5, fa0, 0x10010000"),
            );
            let add = ui.button("Add").clicked()
                || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if add {
                let input = self.watch_input.trim().to_owned();
                if !input.is_empty() {
                    if let Some(target) = parse_watch_target(&input) {
                        self.watches.push(Watch {
                            label: input,
                            target,
                        });
                        self.watch_input.clear();
                    }
                }
            }
        });
        ui.separator();

        if self.watches.is_empty() {
            ui.weak("No watches. Enter a register (a0, x5, fa0) or address (0x10010000) above.");
            return;
        }

        // Read CPU state up-front to avoid holding a borrow across the mutable watches.remove call
        let watch_vals: Vec<(String, String)> = self
            .watches
            .iter()
            .map(|watch| {
                if let Some(cpu) = &self.tabs[self.active].cpu {
                    match &watch.target {
                        WatchTarget::IntReg(idx) => {
                            let v = cpu.regs.read(*idx);
                            (format!("{v:#010x}"), format!("{}", v as i32))
                        }
                        WatchTarget::FpReg(idx) => {
                            let raw = cpu.fp.snapshot()[*idx];
                            let f = f64::from_bits(raw);
                            (format!("{raw:#018x}"), format!("{f:.6}"))
                        }
                        WatchTarget::Mem(addr) => {
                            let v = cpu.mem.load_word(*addr);
                            (format!("{v:#010x}"), format!("{}", v as i32))
                        }
                    }
                } else {
                    ("—".to_owned(), "—".to_owned())
                }
            })
            .collect();

        let mut to_remove: Option<usize> = None;
        egui::Grid::new("watches")
            .num_columns(4)
            .spacing([6.0, 1.0])
            .striped(true)
            .show(ui, |ui| {
                ui.weak("Name");
                ui.weak("Hex");
                ui.weak("Dec / Float");
                ui.weak("");
                ui.end_row();

                for (i, watch) in self.watches.iter().enumerate() {
                    let (hex_s, dec_s) = &watch_vals[i];
                    ui.label(
                        RichText::new(&watch.label)
                            .monospace()
                            .color(egui::Color32::GRAY),
                    );
                    ui.label(RichText::new(hex_s.as_str()).monospace());
                    ui.label(RichText::new(dec_s.as_str()).monospace());
                    if ui.small_button("×").clicked() {
                        to_remove = Some(i);
                    }
                    ui.end_row();
                }
            });

        if let Some(i) = to_remove {
            self.watches.remove(i);
        }
    }

    fn show_memory_viewer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Jump to:");
            if ui.small_button(".text").clicked() {
                self.mem_view_base = TEXT_BASE;
            }
            if ui.small_button(".data").clicked() {
                self.mem_view_base = DATA_BASE;
            }
            if ui.small_button("stack").clicked() {
                self.mem_view_base = STACK_TOP & !0xF;
            }
            ui.separator();
            ui.label(
                RichText::new(format!("base: {:#010x}", self.mem_view_base))
                    .monospace()
                    .small()
                    .weak(),
            );
        });
        ui.separator();

        const ROWS: usize = 512;
        const BYTES_PER_ROW: u32 = 16;

        if let Some(cpu) = &self.tabs[self.active].cpu {
            let base = self.mem_view_base;
            let current_pc = cpu.pc;

            TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::initial(110.0).resizable(true))
                .column(Column::initial(88.0).resizable(true))
                .column(Column::initial(88.0).resizable(true))
                .column(Column::initial(88.0).resizable(true))
                .column(Column::initial(88.0).resizable(true))
                .column(Column::remainder())
                .header(20.0, |mut h| {
                    h.col(|ui| {
                        ui.strong("Address");
                    });
                    h.col(|ui| {
                        ui.strong("+0");
                    });
                    h.col(|ui| {
                        ui.strong("+4");
                    });
                    h.col(|ui| {
                        ui.strong("+8");
                    });
                    h.col(|ui| {
                        ui.strong("+C");
                    });
                    h.col(|ui| {
                        ui.strong("ASCII");
                    });
                })
                .body(|body| {
                    body.rows(18.0, ROWS, |mut row| {
                        let i = row.index();
                        let row_addr = base.wrapping_add(i as u32 * BYTES_PER_ROW);
                        let words: [u32; 4] =
                            std::array::from_fn(|j| cpu.mem.load_word(row_addr + j as u32 * 4));
                        let hot = current_pc >= row_addr
                            && current_pc < row_addr.wrapping_add(BYTES_PER_ROW);
                        let addr_color = if hot {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::GRAY
                        };

                        row.col(|ui| {
                            ui.label(
                                RichText::new(format!("{row_addr:#010x}"))
                                    .monospace()
                                    .color(addr_color),
                            );
                        });
                        for w in &words {
                            row.col(|ui| {
                                ui.label(RichText::new(format!("{w:#010x}")).monospace());
                            });
                        }
                        row.col(|ui| {
                            let ascii: String = (0..16u32)
                                .map(|j| cpu.mem.load_byte(row_addr + j))
                                .map(|b| {
                                    if (32..127).contains(&b) {
                                        b as char
                                    } else {
                                        '.'
                                    }
                                })
                                .collect();
                            ui.label(RichText::new(ascii).monospace().weak());
                        });
                    });
                });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Assemble first to view memory.");
            });
        }
    }
}

// ─── Free helpers ─────────────────────────────────────────────────────────────

/// Evaluate a simple register-comparison condition string against the current CPU.
/// Syntax: `REG OP VALUE` where OP is `==`, `!=`, `<`, `>`, `<=`, `>=`.
/// Returns `true` (break) for empty or malformed conditions.
fn eval_condition(expr: &str, cpu: &crate::simulator::engine::CpuState) -> bool {
    for op in &["==", "!=", "<=", ">=", "<", ">"] {
        if let Some(pos) = expr.find(op) {
            let lhs = expr[..pos].trim();
            let rhs = expr[pos + op.len()..].trim();
            if let (Some(l), Some(r)) = (read_reg_val(lhs, cpu), parse_u32_val(rhs)) {
                return match *op {
                    "==" => l == r,
                    "!=" => l != r,
                    "<" => (l as i32) < (r as i32),
                    ">" => (l as i32) > (r as i32),
                    "<=" => (l as i32) <= (r as i32),
                    ">=" => (l as i32) >= (r as i32),
                    _ => true,
                };
            }
        }
    }
    true
}

fn read_reg_val(s: &str, cpu: &crate::simulator::engine::CpuState) -> Option<u32> {
    use crate::hardware::registers::REG_NAMES;
    let s = s.trim().to_lowercase();
    for (i, name) in REG_NAMES.iter().enumerate() {
        if s == *name {
            return Some(cpu.regs.read(i));
        }
    }
    if let Some(rest) = s.strip_prefix('x') {
        if let Ok(n) = rest.parse::<usize>() {
            if n < 32 {
                return Some(cpu.regs.read(n));
            }
        }
    }
    parse_u32_val(&s)
}

fn parse_u32_val(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        let clean: String = hex.chars().filter(|&c| c != '_').collect();
        u32::from_str_radix(&clean, 16).ok()
    } else {
        s.parse::<i32>().ok().map(|v| v as u32)
    }
}

// ─── Autocomplete helpers ─────────────────────────────────────────────────────

// (mnemonic prefix, full syntax template shown in dropdown and inserted)
const INSTRUCTION_TEMPLATES: &[(&str, &str)] = &[
    // Pseudo-instructions
    ("li",     "li rd, imm"),
    ("la",     "la rd, label"),
    ("mv",     "mv rd, rs"),
    ("not",    "not rd, rs"),
    ("neg",    "neg rd, rs"),
    ("nop",    "nop"),
    ("j",      "j label"),
    ("jr",     "jr rs"),
    ("ret",    "ret"),
    ("call",   "call label"),
    ("beqz",   "beqz rs, label"),
    ("bnez",   "bnez rs, label"),
    ("blez",   "blez rs, label"),
    ("bgez",   "bgez rs, label"),
    ("bltz",   "bltz rs, label"),
    ("bgtz",   "bgtz rs, label"),
    ("bgt",    "bgt rs1, rs2, label"),
    ("ble",    "ble rs1, rs2, label"),
    ("bgtu",   "bgtu rs1, rs2, label"),
    ("bleu",   "bleu rs1, rs2, label"),
    ("seqz",   "seqz rd, rs"),
    ("snez",   "snez rd, rs"),
    ("sltz",   "sltz rd, rs"),
    ("sgtz",   "sgtz rd, rs"),
    ("fmv.s",  "fmv.s fd, fs"),
    ("fmv.d",  "fmv.d fd, fs"),
    ("fabs.s", "fabs.s fd, fs"),
    ("fabs.d", "fabs.d fd, fs"),
    ("fneg.s", "fneg.s fd, fs"),
    ("fneg.d", "fneg.d fd, fs"),
    ("csrr",   "csrr rd, csr"),
    ("csrw",   "csrw csr, rs"),
    ("csrs",   "csrs csr, rs"),
    ("csrc",   "csrc csr, rs"),
    ("csrwi",  "csrwi csr, imm"),
    ("csrsi",  "csrsi csr, imm"),
    ("csrci",  "csrci csr, imm"),
    // RV32I
    ("add",    "add rd, rs1, rs2"),
    ("sub",    "sub rd, rs1, rs2"),
    ("sll",    "sll rd, rs1, rs2"),
    ("slt",    "slt rd, rs1, rs2"),
    ("sltu",   "sltu rd, rs1, rs2"),
    ("xor",    "xor rd, rs1, rs2"),
    ("srl",    "srl rd, rs1, rs2"),
    ("sra",    "sra rd, rs1, rs2"),
    ("or",     "or rd, rs1, rs2"),
    ("and",    "and rd, rs1, rs2"),
    ("addi",   "addi rd, rs1, imm"),
    ("slti",   "slti rd, rs1, imm"),
    ("sltiu",  "sltiu rd, rs1, imm"),
    ("xori",   "xori rd, rs1, imm"),
    ("ori",    "ori rd, rs1, imm"),
    ("andi",   "andi rd, rs1, imm"),
    ("slli",   "slli rd, rs1, shamt"),
    ("srli",   "srli rd, rs1, shamt"),
    ("srai",   "srai rd, rs1, shamt"),
    ("lb",     "lb rd, off(rs1)"),
    ("lh",     "lh rd, off(rs1)"),
    ("lw",     "lw rd, off(rs1)"),
    ("lbu",    "lbu rd, off(rs1)"),
    ("lhu",    "lhu rd, off(rs1)"),
    ("sb",     "sb rs2, off(rs1)"),
    ("sh",     "sh rs2, off(rs1)"),
    ("sw",     "sw rs2, off(rs1)"),
    ("beq",    "beq rs1, rs2, label"),
    ("bne",    "bne rs1, rs2, label"),
    ("blt",    "blt rs1, rs2, label"),
    ("bge",    "bge rs1, rs2, label"),
    ("bltu",   "bltu rs1, rs2, label"),
    ("bgeu",   "bgeu rs1, rs2, label"),
    ("jal",    "jal rd, label"),
    ("jalr",   "jalr rd, imm(rs1)"),
    ("lui",    "lui rd, imm"),
    ("auipc",  "auipc rd, imm"),
    ("ecall",  "ecall"),
    ("ebreak", "ebreak"),
    // RV32M
    ("mul",    "mul rd, rs1, rs2"),
    ("mulh",   "mulh rd, rs1, rs2"),
    ("mulhsu", "mulhsu rd, rs1, rs2"),
    ("mulhu",  "mulhu rd, rs1, rs2"),
    ("div",    "div rd, rs1, rs2"),
    ("divu",   "divu rd, rs1, rs2"),
    ("rem",    "rem rd, rs1, rs2"),
    ("remu",   "remu rd, rs1, rs2"),
    // RV32F
    ("flw",      "flw fd, off(rs1)"),
    ("fsw",      "fsw fs, off(rs1)"),
    ("fadd.s",   "fadd.s fd, fs1, fs2"),
    ("fsub.s",   "fsub.s fd, fs1, fs2"),
    ("fmul.s",   "fmul.s fd, fs1, fs2"),
    ("fdiv.s",   "fdiv.s fd, fs1, fs2"),
    ("fsqrt.s",  "fsqrt.s fd, fs1"),
    ("fmadd.s",  "fmadd.s fd, fs1, fs2, fs3"),
    ("fmsub.s",  "fmsub.s fd, fs1, fs2, fs3"),
    ("fnmsub.s", "fnmsub.s fd, fs1, fs2, fs3"),
    ("fnmadd.s", "fnmadd.s fd, fs1, fs2, fs3"),
    ("fmin.s",   "fmin.s fd, fs1, fs2"),
    ("fmax.s",   "fmax.s fd, fs1, fs2"),
    ("fle.s",    "fle.s rd, fs1, fs2"),
    ("flt.s",    "flt.s rd, fs1, fs2"),
    ("feq.s",    "feq.s rd, fs1, fs2"),
    ("fcvt.w.s", "fcvt.w.s rd, fs1"),
    ("fcvt.wu.s","fcvt.wu.s rd, fs1"),
    ("fcvt.s.w", "fcvt.s.w fd, rs1"),
    ("fcvt.s.wu","fcvt.s.wu fd, rs1"),
    ("fmv.x.w",  "fmv.x.w rd, fs1"),
    ("fmv.w.x",  "fmv.w.x fd, rs1"),
    ("fclass.s", "fclass.s rd, fs1"),
    ("fsgnj.s",  "fsgnj.s fd, fs1, fs2"),
    ("fsgnjn.s", "fsgnjn.s fd, fs1, fs2"),
    ("fsgnjx.s", "fsgnjx.s fd, fs1, fs2"),
    // RV32D
    ("fld",      "fld fd, off(rs1)"),
    ("fsd",      "fsd fs, off(rs1)"),
    ("fadd.d",   "fadd.d fd, fs1, fs2"),
    ("fsub.d",   "fsub.d fd, fs1, fs2"),
    ("fmul.d",   "fmul.d fd, fs1, fs2"),
    ("fdiv.d",   "fdiv.d fd, fs1, fs2"),
    ("fsqrt.d",  "fsqrt.d fd, fs1"),
    ("fmadd.d",  "fmadd.d fd, fs1, fs2, fs3"),
    ("fmsub.d",  "fmsub.d fd, fs1, fs2, fs3"),
    ("fnmsub.d", "fnmsub.d fd, fs1, fs2, fs3"),
    ("fnmadd.d", "fnmadd.d fd, fs1, fs2, fs3"),
    ("fmin.d",   "fmin.d fd, fs1, fs2"),
    ("fmax.d",   "fmax.d fd, fs1, fs2"),
    ("fle.d",    "fle.d rd, fs1, fs2"),
    ("flt.d",    "flt.d rd, fs1, fs2"),
    ("feq.d",    "feq.d rd, fs1, fs2"),
    ("fcvt.w.d", "fcvt.w.d rd, fs1"),
    ("fcvt.wu.d","fcvt.wu.d rd, fs1"),
    ("fcvt.d.w", "fcvt.d.w fd, rs1"),
    ("fcvt.d.wu","fcvt.d.wu fd, rs1"),
    ("fclass.d", "fclass.d rd, fs1"),
    ("fcvt.s.d", "fcvt.s.d fd, fs1"),
    ("fcvt.d.s", "fcvt.d.s fd, fs1"),
    ("fsgnj.d",  "fsgnj.d fd, fs1, fs2"),
    ("fsgnjn.d", "fsgnjn.d fd, fs1, fs2"),
    ("fsgnjx.d", "fsgnjx.d fd, fs1, fs2"),
    // Zicsr
    ("csrrw",  "csrrw rd, csr, rs1"),
    ("csrrs",  "csrrs rd, csr, rs1"),
    ("csrrc",  "csrrc rd, csr, rs1"),
    ("csrrwi", "csrrwi rd, csr, imm"),
    ("csrrsi", "csrrsi rd, csr, imm"),
    ("csrrci", "csrrci rd, csr, imm"),
];

const DIRECTIVES: &[&str] = &[
    ".data", ".text", ".word", ".byte", ".half", ".short", ".float", ".double",
    ".string", ".asciiz", ".asciz", ".ascii", ".space", ".align", ".globl", ".global",
    ".equ", ".set",
];

/// Return the byte offset of the first placeholder within `template`, e.g. for
/// "li rd, imm" returns Some((3, 5)) pointing at "rd".
fn first_placeholder_range(template: &str) -> Option<(usize, usize)> {
    let sp = template.find(' ')?;
    let start = sp + 1;
    let end = template[start..].find(',').map(|i| start + i).unwrap_or(template.len());
    if start < end { Some((start, end)) } else { None }
}

/// True when the cursor is past the mnemonic on the current line (operand position).
fn is_operand_position(text: &str, cursor_byte: usize) -> bool {
    let before = &text[..cursor_byte.min(text.len())];
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = &before[line_start..];
    line.trim_start().contains(|c: char| c == ' ' || c == ',')
}

/// Return the word prefix immediately before `cursor_byte` in `text`.
fn word_before_cursor(text: &str, cursor_byte: usize) -> &str {
    let before = &text[..cursor_byte.min(text.len())];
    let start = before
        .rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
        .map(|i| i + 1)
        .unwrap_or(0);
    &before[start..]
}

/// Build a filtered completion list for `prefix` at `cursor_byte`.
fn compute_completions(prefix: &str, source: &str, cursor_byte: usize) -> Vec<AcEntry> {
    let pl = prefix.to_lowercase();

    // Directive completions
    if prefix.starts_with('.') {
        if prefix.is_empty() {
            return Vec::new();
        }
        let mut entries: Vec<AcEntry> = DIRECTIVES
            .iter()
            .filter(|&&d| d.to_lowercase().starts_with(&pl))
            .map(|&d| AcEntry { display: d.to_owned(), insert: d.to_owned(), first_ph: None })
            .collect();
        entries.sort_by(|a, b| a.display.cmp(&b.display));
        return entries;
    }

    // Register / label completions (operand position: mnemonic already typed)
    if is_operand_position(source, cursor_byte) {
        // Empty prefix: show the most-used ABI integer registers as a starting point
        if prefix.is_empty() {
            const COMMON: &[&str] = &[
                "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7",
                "t0", "t1", "t2", "t3", "t4", "t5", "t6",
                "ra", "sp", "zero",
            ];
            return COMMON.iter()
                .map(|&n| AcEntry { display: n.to_owned(), insert: n.to_owned(), first_ph: None })
                .collect();
        }
        let mut seen: HashSet<String> = HashSet::new();
        let mut entries: Vec<AcEntry> = Vec::new();
        for name in crate::hardware::registers::REG_NAMES {
            if name.to_lowercase().starts_with(&pl) && seen.insert(name.to_string()) {
                entries.push(AcEntry { display: name.to_string(), insert: name.to_string(), first_ph: None });
            }
        }
        for name in crate::hardware::fp_registers::FP_REG_NAMES {
            if name.to_lowercase().starts_with(&pl) && seen.insert(name.to_string()) {
                entries.push(AcEntry { display: name.to_string(), insert: name.to_string(), first_ph: None });
            }
        }
        for line in source.lines() {
            if let Some(label) = line.trim().strip_suffix(':') {
                let label = label.trim();
                if !label.is_empty() && !label.contains(' ') && label.to_lowercase().starts_with(&pl) {
                    if seen.insert(label.to_owned()) {
                        entries.push(AcEntry { display: label.to_owned(), insert: label.to_owned(), first_ph: None });
                    }
                }
            }
        }
        entries.sort_by(|a, b| a.display.cmp(&b.display));
        return entries;
    }

    // Mnemonic completions with full syntax templates
    let mut entries: Vec<AcEntry> = INSTRUCTION_TEMPLATES
        .iter()
        .filter(|(m, _)| m.to_lowercase().starts_with(&pl))
        .map(|(_, tmpl)| AcEntry {
            display: tmpl.to_string(),
            insert: tmpl.to_string(),
            first_ph: first_placeholder_range(tmpl),
        })
        .collect();
    entries.sort_by(|a, b| a.display.cmp(&b.display));
    entries
}

/// Extract (1-based line, 1-based col) from error messages of the form
/// `"<input>:LINE:COL: TYPE error: MESSAGE"`.
fn parse_error_position(msg: &str) -> Option<(u32, u32)> {
    let rest = msg.trim().strip_prefix("<input>:")?;
    let mut parts = rest.splitn(3, ':');
    let line = parts.next()?.parse::<u32>().ok()?;
    let col = parts.next()?.trim().parse::<u32>().ok()?;
    Some((line, col))
}

/// Return byte offsets of all occurrences of `query` in `text`.
pub(crate) fn find_matches_in(text: &str, query: &str, case_sensitive: bool) -> Vec<usize> {
    if query.is_empty() {
        return vec![];
    }
    let mut matches = Vec::new();
    let (haystack, needle) = if case_sensitive {
        (text.to_owned(), query.to_owned())
    } else {
        (text.to_lowercase(), query.to_lowercase())
    };
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(&needle) {
        matches.push(start + pos);
        start += pos + needle.len().max(1);
    }
    matches
}

/// Replace all occurrences of `query` in `text` with `replacement`.
/// Returns `(new_text, replacement_count)`.
pub(crate) fn replace_all_in(
    text: &str,
    query: &str,
    replacement: &str,
    case_sensitive: bool,
) -> (String, usize) {
    if query.is_empty() {
        return (text.to_owned(), 0);
    }
    if case_sensitive {
        let count = text.matches(query).count();
        (text.replace(query, replacement), count)
    } else {
        let lower_text = text.to_lowercase();
        let lower_query = query.to_lowercase();
        let mut result = String::with_capacity(text.len());
        let mut last = 0;
        let mut count = 0;
        let mut start = 0;
        while let Some(pos) = lower_text[start..].find(&lower_query) {
            let abs = start + pos;
            result.push_str(&text[last..abs]);
            result.push_str(replacement);
            last = abs + query.len();
            start = last;
            count += 1;
        }
        result.push_str(&text[last..]);
        (result, count)
    }
}

// ─── Help content ─────────────────────────────────────────────────────────────

fn instr_table(ui: &mut egui::Ui, id: &str, entries: &[(&str, &str, &str)]) {
    egui::Grid::new(id)
        .num_columns(3)
        .spacing([12.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Instruction");
            ui.strong("Description");
            ui.strong("Example");
            ui.end_row();
            for (instr, desc, example) in entries {
                ui.label(
                    RichText::new(*instr)
                        .monospace()
                        .color(egui::Color32::LIGHT_BLUE),
                );
                ui.label(*desc);
                ui.label(RichText::new(*example).monospace().weak());
                ui.end_row();
            }
        });
}

fn show_help_content(ui: &mut egui::Ui, active: &mut HelpTab) {
    // Tab bar
    ui.horizontal(|ui| {
        ui.selectable_value(active, HelpTab::Pseudo, "Pseudo");
        ui.selectable_value(active, HelpTab::Rv32i, "RV32I");
        ui.selectable_value(active, HelpTab::Rv32m, "RV32M");
        ui.selectable_value(active, HelpTab::Rv32f, "RV32F");
        ui.selectable_value(active, HelpTab::Rv32d, "RV32D");
        ui.selectable_value(active, HelpTab::Zicsr, "Zicsr");
        ui.selectable_value(active, HelpTab::Directives, "Directives");
        ui.selectable_value(active, HelpTab::Syscalls, "Syscalls");
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("help_scroll")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
    match active {
        HelpTab::Pseudo => {
            ui.label(RichText::new("Pseudo-Instructions — most commonly used").weak());
            ui.add_space(4.0);
            instr_table(
                ui,
                "pseudo",
                &[
                    ("li  rd, imm", "Load immediate value into rd", "li   t0, 42"),
                    (
                        "la  rd, label",
                        "Load address of label into rd",
                        "la   a0, msg",
                    ),
                    ("mv  rd, rs", "rd = rs (copy register)", "mv   a0, t0"),
                    ("not rd, rs", "rd = ~rs (bitwise NOT)", "not  t0, t1"),
                    (
                        "neg rd, rs",
                        "rd = -rs (two's complement negate)",
                        "neg  t0, t1",
                    ),
                    ("nop", "No operation (addi x0, x0, 0)", "nop"),
                    ("j   label", "Unconditional jump to label", "j    loop"),
                    ("jr  rs", "Jump to address in rs", "jr   ra"),
                    ("ret", "Return from function  (jr ra)", "ret"),
                    (
                        "call label",
                        "Call function, save return addr in ra",
                        "call my_func",
                    ),
                    ("beqz rs, label", "Branch if rs == 0", "beqz t0, done"),
                    ("bnez rs, label", "Branch if rs != 0", "bnez t0, loop"),
                    ("blez rs, label", "Branch if rs <= 0", "blez t0, neg"),
                    ("bgez rs, label", "Branch if rs >= 0", "bgez t0, pos"),
                    ("bltz rs, label", "Branch if rs < 0", "bltz t0, neg"),
                    ("bgtz rs, label", "Branch if rs > 0", "bgtz t0, pos"),
                    (
                        "bgt  rs, rt, label",
                        "Branch if rs > rt  (signed)",
                        "bgt  t0, t1, big",
                    ),
                    (
                        "ble  rs, rt, label",
                        "Branch if rs <= rt  (signed)",
                        "ble  t0, t1, small",
                    ),
                    ("seqz rd, rs", "rd = 1 if rs == 0,  else 0", "seqz t0, a0"),
                    ("snez rd, rs", "rd = 1 if rs != 0,  else 0", "snez t0, a0"),
                    ("sltz rd, rs", "rd = 1 if rs < 0,   else 0", "sltz t0, a0"),
                    ("sgtz rd, rs", "rd = 1 if rs > 0,   else 0", "sgtz t0, a0"),
                ],
            );
        }

        HelpTab::Rv32i => {
            ui.label(RichText::new("RV32I — Base Integer Instructions").weak());
            ui.add_space(4.0);
            ui.label(RichText::new("Arithmetic (R-type)").strong());
            instr_table(
                ui,
                "rv32i_r",
                &[
                    ("add  rd, rs1, rs2", "rd = rs1 + rs2", "add  t0, t1, t2"),
                    ("sub  rd, rs1, rs2", "rd = rs1 - rs2", "sub  t0, t1, t2"),
                    ("and  rd, rs1, rs2", "rd = rs1 & rs2", "and  t0, t1, t2"),
                    ("or   rd, rs1, rs2", "rd = rs1 | rs2", "or   t0, t1, t2"),
                    ("xor  rd, rs1, rs2", "rd = rs1 ^ rs2", "xor  t0, t1, t2"),
                    (
                        "sll  rd, rs1, rs2",
                        "rd = rs1 << rs2[4:0]  (logical left)",
                        "sll  t0, t1, t2",
                    ),
                    (
                        "srl  rd, rs1, rs2",
                        "rd = rs1 >> rs2[4:0]  (logical right)",
                        "srl  t0, t1, t2",
                    ),
                    (
                        "sra  rd, rs1, rs2",
                        "rd = rs1 >> rs2[4:0]  (arithmetic right)",
                        "sra  t0, t1, t2",
                    ),
                    (
                        "slt  rd, rs1, rs2",
                        "rd = 1 if rs1 < rs2 (signed)",
                        "slt  t0, t1, t2",
                    ),
                    (
                        "sltu rd, rs1, rs2",
                        "rd = 1 if rs1 < rs2 (unsigned)",
                        "sltu t0, t1, t2",
                    ),
                ],
            );
            ui.add_space(4.0);
            ui.label(RichText::new("Arithmetic Immediate (I-type)").strong());
            instr_table(
                ui,
                "rv32i_i",
                &[
                    (
                        "addi  rd, rs1, imm",
                        "rd = rs1 + sign_ext(imm12)",
                        "addi t0, t1, 10",
                    ),
                    (
                        "andi  rd, rs1, imm",
                        "rd = rs1 & sign_ext(imm12)",
                        "andi t0, t1, 0xFF",
                    ),
                    (
                        "ori   rd, rs1, imm",
                        "rd = rs1 | sign_ext(imm12)",
                        "ori  t0, t1, 1",
                    ),
                    (
                        "xori  rd, rs1, imm",
                        "rd = rs1 ^ sign_ext(imm12)",
                        "xori t0, t1, -1",
                    ),
                    (
                        "slli  rd, rs1, shamt",
                        "rd = rs1 << shamt",
                        "slli t0, t1, 2",
                    ),
                    (
                        "srli  rd, rs1, shamt",
                        "rd = rs1 >> shamt (logical)",
                        "srli t0, t1, 2",
                    ),
                    (
                        "srai  rd, rs1, shamt",
                        "rd = rs1 >> shamt (arithmetic)",
                        "srai t0, t1, 2",
                    ),
                    (
                        "slti  rd, rs1, imm",
                        "rd = 1 if rs1 < imm (signed)",
                        "slti t0, t1, 5",
                    ),
                    (
                        "sltiu rd, rs1, imm",
                        "rd = 1 if rs1 < imm (unsigned)",
                        "sltiu t0,t1, 5",
                    ),
                ],
            );
            ui.add_space(4.0);
            ui.label(RichText::new("Loads").strong());
            instr_table(
                ui,
                "rv32i_load",
                &[
                    ("lw  rd, offset(rs1)", "Load 32-bit word", "lw  t0, 0(a0)"),
                    (
                        "lh  rd, offset(rs1)",
                        "Load 16-bit halfword, sign-extend",
                        "lh  t0, 2(a0)",
                    ),
                    (
                        "lhu rd, offset(rs1)",
                        "Load 16-bit halfword, zero-extend",
                        "lhu t0, 2(a0)",
                    ),
                    (
                        "lb  rd, offset(rs1)",
                        "Load 8-bit byte, sign-extend",
                        "lb  t0, 1(a0)",
                    ),
                    (
                        "lbu rd, offset(rs1)",
                        "Load 8-bit byte, zero-extend",
                        "lbu t0, 1(a0)",
                    ),
                ],
            );
            ui.add_space(4.0);
            ui.label(RichText::new("Stores").strong());
            instr_table(
                ui,
                "rv32i_store",
                &[
                    ("sw rs2, offset(rs1)", "Store 32-bit word", "sw  t0, 0(a0)"),
                    ("sh rs2, offset(rs1)", "Store low 16 bits", "sh  t0, 2(a0)"),
                    ("sb rs2, offset(rs1)", "Store low 8 bits", "sb  t0, 1(a0)"),
                ],
            );
            ui.add_space(4.0);
            ui.label(RichText::new("Branches").strong());
            instr_table(
                ui,
                "rv32i_branch",
                &[
                    (
                        "beq  rs1, rs2, label",
                        "Branch if rs1 == rs2",
                        "beq  t0, t1, done",
                    ),
                    (
                        "bne  rs1, rs2, label",
                        "Branch if rs1 != rs2",
                        "bne  t0, t1, loop",
                    ),
                    (
                        "blt  rs1, rs2, label",
                        "Branch if rs1 < rs2  (signed)",
                        "blt  t0, t1, neg",
                    ),
                    (
                        "bltu rs1, rs2, label",
                        "Branch if rs1 < rs2  (unsigned)",
                        "bltu t0, t1, wrap",
                    ),
                    (
                        "bge  rs1, rs2, label",
                        "Branch if rs1 >= rs2 (signed)",
                        "bge  t0, t1, pos",
                    ),
                    (
                        "bgeu rs1, rs2, label",
                        "Branch if rs1 >= rs2 (unsigned)",
                        "bgeu t0, t1, ok",
                    ),
                ],
            );
            ui.add_space(4.0);
            ui.label(RichText::new("Jumps & Upper").strong());
            instr_table(
                ui,
                "rv32i_jump",
                &[
                    (
                        "jal  rd, label",
                        "Jump and link — rd = PC+4, PC = label",
                        "jal  ra, my_func",
                    ),
                    (
                        "jalr rd, rs1, offset",
                        "Jump and link register",
                        "jalr zero, ra, 0",
                    ),
                    (
                        "lui  rd, imm",
                        "rd = imm << 12  (upper 20 bits)",
                        "lui  t0, 0x10010",
                    ),
                    (
                        "auipc rd, offset",
                        "rd = PC + (offset << 12)",
                        "auipc t0, 0",
                    ),
                ],
            );
        }

        HelpTab::Rv32m => {
            ui.label(RichText::new("RV32M — Multiply / Divide").weak());
            ui.add_space(4.0);
            instr_table(
                ui,
                "rv32m",
                &[
                    (
                        "mul    rd, rs1, rs2",
                        "rd = (rs1 × rs2)[31:0]  (low 32 bits)",
                        "mul  t0, t1, t2",
                    ),
                    (
                        "mulh   rd, rs1, rs2",
                        "rd = (rs1 × rs2)[63:32] signed × signed",
                        "mulh t0, t1, t2",
                    ),
                    (
                        "mulhsu rd, rs1, rs2",
                        "rd = (rs1 × rs2)[63:32] signed × unsigned",
                        "mulhsu t0,t1,t2",
                    ),
                    (
                        "mulhu  rd, rs1, rs2",
                        "rd = (rs1 × rs2)[63:32] unsigned × unsigned",
                        "mulhu t0,t1,t2",
                    ),
                    (
                        "div    rd, rs1, rs2",
                        "rd = rs1 ÷ rs2  (signed; -1 if div-by-zero)",
                        "div  t0, t1, t2",
                    ),
                    (
                        "divu   rd, rs1, rs2",
                        "rd = rs1 ÷ rs2  (unsigned; MAX if div-by-zero)",
                        "divu t0, t1, t2",
                    ),
                    (
                        "rem    rd, rs1, rs2",
                        "rd = rs1 mod rs2 (signed remainder)",
                        "rem  t0, t1, t2",
                    ),
                    (
                        "remu   rd, rs1, rs2",
                        "rd = rs1 mod rs2 (unsigned remainder)",
                        "remu t0, t1, t2",
                    ),
                ],
            );
        }

        HelpTab::Rv32f => {
            ui.label(RichText::new("RV32F — Single-Precision Floating Point").weak());
            ui.add_space(4.0);
            ui.label(RichText::new("Use fa0–fa7 for arguments, ft0–ft11 for temporaries, fs0–fs11 for saved values.").weak().small());
            ui.add_space(2.0);
            instr_table(ui, "rv32f", &[
                ("flw  fd, offset(rs)",   "Load 32-bit float from memory",             "flw  ft0, 0(a0)"),
                ("fsw  fs, offset(rs)",   "Store 32-bit float to memory",              "fsw  ft0, 0(a0)"),
                ("fadd.s fd, fs1, fs2",   "fd = fs1 + fs2",                            "fadd.s ft0,ft1,ft2"),
                ("fsub.s fd, fs1, fs2",   "fd = fs1 - fs2",                            "fsub.s ft0,ft1,ft2"),
                ("fmul.s fd, fs1, fs2",   "fd = fs1 × fs2",                            "fmul.s ft0,ft1,ft2"),
                ("fdiv.s fd, fs1, fs2",   "fd = fs1 ÷ fs2",                            "fdiv.s ft0,ft1,ft2"),
                ("fsqrt.s fd, fs1",       "fd = √fs1",                                 "fsqrt.s ft0, ft1"),
                ("fmadd.s fd,fs1,fs2,fs3","fd = fs1×fs2 + fs3",                        "fmadd.s ft0,ft1,ft2,ft3"),
                ("fmsub.s fd,fs1,fs2,fs3","fd = fs1×fs2 - fs3",                        "fmsub.s ft0,ft1,ft2,ft3"),
                ("fmin.s  fd, fs1, fs2",  "fd = min(fs1, fs2)",                        "fmin.s ft0,ft1,ft2"),
                ("fmax.s  fd, fs1, fs2",  "fd = max(fs1, fs2)",                        "fmax.s ft0,ft1,ft2"),
                ("feq.s   rd, fs1, fs2",  "rd = 1 if fs1 == fs2, else 0",             "feq.s t0,ft0,ft1"),
                ("flt.s   rd, fs1, fs2",  "rd = 1 if fs1 < fs2,  else 0",             "flt.s t0,ft0,ft1"),
                ("fle.s   rd, fs1, fs2",  "rd = 1 if fs1 <= fs2, else 0",             "fle.s t0,ft0,ft1"),
                ("fcvt.w.s  rd, fs",      "Convert float → signed int (truncate)",     "fcvt.w.s t0, ft0"),
                ("fcvt.wu.s rd, fs",      "Convert float → unsigned int (truncate)",   "fcvt.wu.s t0,ft0"),
                ("fcvt.s.w  fd, rs",      "Convert signed int → float",                "fcvt.s.w ft0, t0"),
                ("fcvt.s.wu fd, rs",      "Convert unsigned int → float",              "fcvt.s.wu ft0,t0"),
                ("fmv.w.x   fd, rs",      "Move int bits to float register (no conv)", "fmv.w.x ft0, t0"),
                ("fmv.x.w   rd, fs",      "Move float bits to int register (no conv)", "fmv.x.w t0, ft0"),
                ("fclass.s  rd, fs",      "rd = bitmask classifying fs (NaN, ±Inf…)",  "fclass.s t0, ft0"),
            ]);
        }

        HelpTab::Rv32d => {
            ui.label(RichText::new("RV32D — Double-Precision Floating Point").weak());
            ui.add_space(4.0);
            instr_table(
                ui,
                "rv32d",
                &[
                    (
                        "fld  fd, offset(rs)",
                        "Load 64-bit double from memory",
                        "fld  ft0, 0(a0)",
                    ),
                    (
                        "fsd  fs, offset(rs)",
                        "Store 64-bit double to memory",
                        "fsd  ft0, 0(a0)",
                    ),
                    (
                        "fadd.d fd, fs1, fs2",
                        "fd = fs1 + fs2  (double)",
                        "fadd.d ft0,ft1,ft2",
                    ),
                    (
                        "fsub.d fd, fs1, fs2",
                        "fd = fs1 - fs2  (double)",
                        "fsub.d ft0,ft1,ft2",
                    ),
                    (
                        "fmul.d fd, fs1, fs2",
                        "fd = fs1 × fs2  (double)",
                        "fmul.d ft0,ft1,ft2",
                    ),
                    (
                        "fdiv.d fd, fs1, fs2",
                        "fd = fs1 ÷ fs2  (double)",
                        "fdiv.d ft0,ft1,ft2",
                    ),
                    ("fsqrt.d fd, fs1", "fd = √fs1  (double)", "fsqrt.d ft0, ft1"),
                    (
                        "feq.d  rd, fs1, fs2",
                        "rd = 1 if fs1 == fs2  (double)",
                        "feq.d t0,ft0,ft1",
                    ),
                    (
                        "flt.d  rd, fs1, fs2",
                        "rd = 1 if fs1 < fs2   (double)",
                        "flt.d t0,ft0,ft1",
                    ),
                    (
                        "fle.d  rd, fs1, fs2",
                        "rd = 1 if fs1 <= fs2  (double)",
                        "fle.d t0,ft0,ft1",
                    ),
                    (
                        "fcvt.w.d  rd, fs",
                        "Convert double → signed int",
                        "fcvt.w.d t0, ft0",
                    ),
                    (
                        "fcvt.d.w  fd, rs",
                        "Convert signed int → double",
                        "fcvt.d.w ft0, t0",
                    ),
                    (
                        "fcvt.s.d  fd, fs",
                        "Convert double → single",
                        "fcvt.s.d ft0, ft1",
                    ),
                    (
                        "fcvt.d.s  fd, fs",
                        "Convert single → double",
                        "fcvt.d.s ft0, ft1",
                    ),
                    (
                        "fclass.d  rd, fs",
                        "rd = bitmask classifying fs (double)",
                        "fclass.d t0, ft0",
                    ),
                ],
            );
        }

        HelpTab::Zicsr => {
            ui.label(RichText::new("Zicsr — Control & Status Register Instructions").weak());
            ui.add_space(4.0);
            instr_table(
                ui,
                "csr",
                &[
                    (
                        "csrrw  rd, csr, rs1",
                        "rd = CSR; CSR = rs1",
                        "csrrw t0, fcsr, t1",
                    ),
                    (
                        "csrrs  rd, csr, rs1",
                        "rd = CSR; CSR |= rs1  (set bits)",
                        "csrrs t0, fflags, t1",
                    ),
                    (
                        "csrrc  rd, csr, rs1",
                        "rd = CSR; CSR &= ~rs1 (clear bits)",
                        "csrrc t0, fflags, t1",
                    ),
                    (
                        "csrrwi rd, csr, uimm",
                        "rd = CSR; CSR = zero_ext(uimm5)",
                        "csrrwi t0, frm, 0",
                    ),
                    (
                        "csrrsi rd, csr, uimm",
                        "rd = CSR; CSR |= uimm5",
                        "csrrsi t0, fflags, 1",
                    ),
                    (
                        "csrrci rd, csr, uimm",
                        "rd = CSR; CSR &= ~uimm5",
                        "csrrci t0, fflags, 1",
                    ),
                ],
            );
            ui.add_space(4.0);
            ui.label(RichText::new("Common CSR addresses:").strong());
            instr_table(
                ui,
                "csr_addrs",
                &[
                    (
                        "0x001  fflags",
                        "FP accrued exception flags (NX/UF/OF/DZ/NV)",
                        "",
                    ),
                    ("0x002  frm", "FP rounding mode (000=RNE, 001=RTZ, …)", ""),
                    ("0x003  fcsr", "FP control/status = frm<<5 | fflags", ""),
                    ("0xC00  cycle", "Cycle counter (low 32 bits)", ""),
                    (
                        "0xC02  instret",
                        "Instructions-retired counter (low 32 bits)",
                        "",
                    ),
                ],
            );
        }

        HelpTab::Directives => {
            ui.label(RichText::new("Assembler Directives").weak());
            ui.add_space(4.0);
            instr_table(
                ui,
                "directives",
                &[
                    (".text", "Switch to code segment", ".text"),
                    (".data", "Switch to data segment", ".data"),
                    (
                        ".globl label",
                        "Make label visible to linker",
                        ".globl main",
                    ),
                    (".word v1, v2, …", "Emit 32-bit word(s)", ".word 42, 0xFF"),
                    (".half v1, v2, …", "Emit 16-bit halfword(s)", ".half 0, 1"),
                    (".byte v1, v2, …", "Emit 8-bit byte(s)", ".byte 'A', 10"),
                    (
                        ".ascii \"str\"",
                        "Emit string bytes (no null terminator)",
                        ".ascii \"hi\"",
                    ),
                    (
                        ".asciiz \"str\"",
                        "Emit null-terminated string",
                        ".asciiz \"Hello\\n\"",
                    ),
                    (".string \"str\"", "Alias for .asciiz", ".string \"World\""),
                    (".align n", "Align to 2^n bytes", ".align 2"),
                    (".space n", "Reserve n zero bytes", ".space 64"),
                ],
            );
        }

        HelpTab::Syscalls => {
            ui.label(RichText::new("ECALL — System Calls  (a7 = syscall number)").weak());
            ui.add_space(4.0);
            instr_table(
                ui,
                "syscalls",
                &[
                    (
                        "1  print_int",
                        "Print a0 as signed decimal integer",
                        "li a7,1; li a0,42; ecall",
                    ),
                    (
                        "2  print_float",
                        "Print fa0 as single-precision float",
                        "li a7,2; ecall",
                    ),
                    (
                        "3  print_double",
                        "Print fa0 as double-precision float",
                        "li a7,3; ecall",
                    ),
                    (
                        "4  print_string",
                        "Print null-terminated string at a0",
                        "li a7,4; la a0,msg; ecall",
                    ),
                    (
                        "5  read_int",
                        "Read integer from stdin → a0",
                        "li a7,5; ecall",
                    ),
                    (
                        "6  read_float",
                        "Read float from stdin → fa0",
                        "li a7,6; ecall",
                    ),
                    (
                        "7  read_double",
                        "Read double from stdin → fa0",
                        "li a7,7; ecall",
                    ),
                    (
                        "8  read_string",
                        "Read string; a0=buf, a1=maxlen",
                        "li a7,8; ecall",
                    ),
                    (
                        "9  sbrk",
                        "Allocate a0 heap bytes → a0=ptr",
                        "li a7,9; li a0,64; ecall",
                    ),
                    (
                        "10 exit",
                        "Terminate program  (exit code 0)",
                        "li a7,10; ecall",
                    ),
                    (
                        "11 print_char",
                        "Print a0[7:0] as ASCII character",
                        "li a7,11; li a0,'A'; ecall",
                    ),
                    ("12 read_char", "Read one character → a0", "li a7,12; ecall"),
                    ("34 print_hex", "Print a0 as hexadecimal", "li a7,34; ecall"),
                    ("35 print_bin", "Print a0 as binary", "li a7,35; ecall"),
                    (
                        "36 print_uint",
                        "Print a0 as unsigned decimal",
                        "li a7,36; ecall",
                    ),
                    (
                        "93 exit2",
                        "Terminate with exit code in a0",
                        "li a7,93; li a0,1; ecall",
                    ),
                ],
            );
        }
    } // match
    }); // ScrollArea
}

// ─── eframe::App ─────────────────────────────────────────────────────────────

impl eframe::App for OarsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme every frame
        ctx.set_visuals(if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        // Auto-run: drive Running / StepOver / StepOut per frame.
        if matches!(
            self.tabs[self.active].sim_state,
            SimState::Running | SimState::StepOver(_) | SimState::StepOut(_)
        ) {
            self.tabs[self.active].save_prev_regs();
            let n = self.steps_per_frame;
            self.tabs[self.active].pump_steps(n);
            ctx.request_repaint();
        }

        // Floating help window
        let mut help_open = self.show_help;
        if help_open {
            let help_tab = &mut self.help_tab;
            egui::Window::new("Instruction Reference")
                .open(&mut help_open)
                .default_size([820.0, 560.0])
                .resizable(true)
                .show(ctx, |ui| {
                    show_help_content(ui, help_tab);
                });
        }
        self.show_help = help_open;

        // Floating About window
        let mut about_open = self.show_about;
        if about_open {
            egui::Window::new("About OARS")
                .open(&mut about_open)
                .resizable(false)
                .collapsible(false)
                .default_width(380.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label(RichText::new("OARS").size(28.0).strong());
                        ui.label(
                            RichText::new("Oxide Assembler and Runtime Simulator")
                                .size(13.0)
                                .weak(),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(concat!("Version ", env!("CARGO_PKG_VERSION")))
                                .monospace(),
                        );
                        ui.add_space(12.0);
                        ui.label("A single-binary RISC-V simulator for students.");
                        ui.label("No Java, no installer — just run the executable.");
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(6.0);
                        ui.label(RichText::new("© 2025 Nathan Hutchins").weak());
                        ui.label(RichText::new("MIT License").weak());
                        ui.add_space(4.0);
                        ui.label(RichText::new("Inspired by RARS and MARS").weak().small());
                        ui.add_space(8.0);
                    });
                });
        }
        self.show_about = about_open;

        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open…").clicked() {
                        self.tabs[self.active].do_open();
                        ui.close_menu();
                    }
                    if ui.button("Save").clicked() {
                        self.tabs[self.active].do_save();
                        ui.close_menu();
                    }
                    ui.separator();
                    let assembled = self.tabs[self.active].asm_out.is_some();
                    if ui
                        .add_enabled(assembled, egui::Button::new("Export Flat Binary…"))
                        .clicked()
                    {
                        self.tabs[self.active].do_export_binary();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(assembled, egui::Button::new("Export ELF…"))
                        .clicked()
                    {
                        self.tabs[self.active].do_export_elf();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("New Tab").clicked() {
                        self.tabs.push(Tab::new());
                        self.active = self.tabs.len() - 1;
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    let label = if self.dark_mode {
                        "Light Mode"
                    } else {
                        "Dark Mode"
                    };
                    if ui.button(label).clicked() {
                        self.dark_mode = !self.dark_mode;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Instruction Reference").clicked() {
                        self.show_help = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("About OARS").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        // Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.show_toolbar(ui);
        });

        // Bottom panel: Console | Memory | Data | Stack tabs
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .default_height(200.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Console, "Console");
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Memory, "Memory");
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Data, "Data Segment");
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Stack, "Stack");
                    ui.selectable_value(
                        &mut self.bottom_tab,
                        BottomTab::Watchpoints,
                        "Watchpoints",
                    );
                    ui.selectable_value(
                        &mut self.bottom_tab,
                        BottomTab::CallStack,
                        "Call Stack",
                    );
                    ui.selectable_value(
                        &mut self.bottom_tab,
                        BottomTab::Breakpoints,
                        "Breakpoints",
                    );
                });
                ui.separator();
                match self.bottom_tab {
                    BottomTab::Console => self.tabs[self.active].show_console(ui),
                    BottomTab::Memory => self.show_memory_viewer(ui),
                    BottomTab::Data => self.tabs[self.active].show_data_segment(ui),
                    BottomTab::Stack => self.tabs[self.active].show_stack_viewer(ui),
                    BottomTab::Watchpoints => self.tabs[self.active].show_watchpoints(ui),
                    BottomTab::CallStack => self.tabs[self.active].show_call_stack(ui),
                    BottomTab::Breakpoints => self.tabs[self.active].show_breakpoints(ui),
                }
            });

        // Right panel: register tabs
        egui::SidePanel::right("registers")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                self.show_registers(ui);
            });

        // Central panel: file tab bar + Editor / Text Segment
        egui::CentralPanel::default().show(ctx, |ui| {
            // File tab bar
            ui.horizontal(|ui| {
                let mut new_active = self.active;
                let mut to_close: Option<usize> = None;
                let can_close = self.tabs.len() > 1;

                for i in 0..self.tabs.len() {
                    let title = self.tabs[i].title();
                    if ui.selectable_label(i == self.active, &title).clicked() {
                        new_active = i;
                    }
                    if can_close && ui.small_button("×").on_hover_text("Close tab").clicked() {
                        to_close = Some(i);
                    }
                }
                if ui.button("+").on_hover_text("New tab").clicked() {
                    self.tabs.push(Tab::new());
                    new_active = self.tabs.len() - 1;
                }

                self.active = new_active;
                if let Some(i) = to_close {
                    self.tabs.remove(i);
                    if self.active >= self.tabs.len() {
                        self.active = self.tabs.len() - 1;
                    }
                }
            });
            ui.separator();

            // Main content sub-tabs (Editor / Text Segment)
            ui.horizontal(|ui| {
                let tab = &mut self.tabs[self.active];
                ui.selectable_value(&mut tab.main_tab, MainTab::Editor, "Editor");
                ui.selectable_value(&mut tab.main_tab, MainTab::TextSegment, "Text Segment");
            });
            ui.separator();

            let main = self.tabs[self.active].main_tab;
            match main {
                MainTab::Editor => self.tabs[self.active].show_editor(ui),
                MainTab::TextSegment => self.tabs[self.active].show_text_segment(ui),
            }
        });
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_error_position ──────────────────────────────────────────────────

    #[test]
    fn parse_pos_lex_error() {
        let msg = "<input>:3:5: lex error: invalid character '@'";
        assert_eq!(parse_error_position(msg), Some((3, 5)));
    }

    #[test]
    fn parse_pos_parse_error() {
        let msg = "<input>:10:1: parse error: unexpected token 'bad_token'";
        assert_eq!(parse_error_position(msg), Some((10, 1)));
    }

    #[test]
    fn parse_pos_assemble_error() {
        let msg = "<input>:1:8: assembler error: undefined label 'foo'";
        assert_eq!(parse_error_position(msg), Some((1, 8)));
    }

    #[test]
    fn parse_pos_runtime_no_location() {
        let msg = "runtime error at PC 0x00400000: illegal instruction";
        assert_eq!(parse_error_position(msg), None);
    }

    #[test]
    fn parse_pos_empty() {
        assert_eq!(parse_error_position(""), None);
    }

    #[test]
    fn parse_pos_line_1_col_1() {
        assert_eq!(
            parse_error_position("<input>:1:1: parse error: x"),
            Some((1, 1))
        );
    }

    // ── find_matches_in ───────────────────────────────────────────────────────

    #[test]
    fn find_case_sensitive_basic() {
        let m = find_matches_in("add a0, a0, a1\nadd a2, a3, a4", "add", true);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0], 0);
        assert_eq!(m[1], 15);
    }

    #[test]
    fn find_case_insensitive() {
        let m = find_matches_in("ADD a0, a0\nadd a1, a2", "add", false);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn find_empty_query_returns_empty() {
        assert!(find_matches_in("hello world", "", true).is_empty());
    }

    #[test]
    fn find_no_match() {
        assert!(find_matches_in("add a0, a1", "sub", true).is_empty());
    }

    #[test]
    fn find_overlapping_not_double_counted() {
        // "aaa" has one non-overlapping "aa" at offset 0
        let m = find_matches_in("aaa", "aa", true);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0], 0);
    }

    // ── replace_all_in ────────────────────────────────────────────────────────

    #[test]
    fn replace_case_sensitive() {
        let (result, count) = replace_all_in("add a0\nadd a1", "add", "sub", true);
        assert_eq!(result, "sub a0\nsub a1");
        assert_eq!(count, 2);
    }

    #[test]
    fn replace_case_insensitive() {
        let (result, count) = replace_all_in("ADD a0\nadd a1", "add", "sub", false);
        assert_eq!(result, "sub a0\nsub a1");
        assert_eq!(count, 2);
    }

    #[test]
    fn replace_empty_query_no_op() {
        let (result, count) = replace_all_in("hello", "", "X", true);
        assert_eq!(result, "hello");
        assert_eq!(count, 0);
    }

    #[test]
    fn replace_no_match() {
        let (result, count) = replace_all_in("add a0", "sub", "mul", true);
        assert_eq!(result, "add a0");
        assert_eq!(count, 0);
    }

    #[test]
    fn replace_with_empty_replacement() {
        let (result, count) = replace_all_in("# comment\n# comment", "# comment", "", true);
        assert_eq!(result, "\n");
        assert_eq!(count, 2);
    }

    // ── Tab helpers ───────────────────────────────────────────────────────────

    #[test]
    fn tab_title_untitled() {
        let t = Tab::new();
        assert_eq!(t.title(), "untitled.s");
    }

    #[test]
    fn tab_title_from_path() {
        let mut t = Tab::new();
        t.file_path = Some(PathBuf::from("/home/user/hello.s"));
        assert_eq!(t.title(), "hello.s");
    }

    #[test]
    fn tab_new_starts_idle() {
        let t = Tab::new();
        assert!(matches!(t.sim_state, SimState::Idle));
    }
}
