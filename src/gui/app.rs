use std::collections::VecDeque;
use std::path::PathBuf;

use egui::RichText;
use egui_extras::{Column, TableBuilder};

use crate::assembler::{
    codegen::{self, AssemblyOutput},
    parser,
};
use crate::hardware::{
    csr::addr as csr_addr, fp_registers::FP_REG_NAMES, memory::TEXT_BASE,
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
    Halted(i32),
    Error(String),
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum RegisterTab {
    Integer,
    Float,
    Csr,
}

// ─── App ─────────────────────────────────────────────────────────────────────

pub struct OarsApp {
    source: String,
    file_path: Option<PathBuf>,

    cpu: Option<CpuState>,
    asm_out: Option<AssemblyOutput>,
    sim_state: SimState,
    backstepper: Backstepper,

    console_out: String,
    input_buf: String,
    input_queue: VecDeque<String>,

    steps_per_frame: u32,

    register_tab: RegisterTab,

    // Register snapshots for change highlighting
    prev_int_regs: [u32; 32],
    prev_fp_regs: [u64; 32],

    show_help: bool,
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
            source: DEFAULT_SOURCE.to_owned(),
            file_path: None,
            cpu: None,
            asm_out: None,
            sim_state: SimState::Idle,
            backstepper: Backstepper::new(),
            console_out: String::new(),
            input_buf: String::new(),
            input_queue: VecDeque::new(),
            steps_per_frame: 50_000,
            register_tab: RegisterTab::Integer,
            prev_int_regs: [0u32; 32],
            prev_fp_regs: [0u64; 32],
            show_help: false,
        }
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
                    self.sim_state = SimState::Error(e.to_string());
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
                self.sim_state = SimState::Error(e.to_string());
            } else {
                self.file_path = Some(path);
            }
        }
    }

    fn do_assemble(&mut self) -> bool {
        self.do_reset_state();
        let stmts = match parser::parse(&self.source) {
            Ok(s) => s,
            Err(e) => {
                self.sim_state = SimState::Error(e.to_string());
                return false;
            }
        };
        let mut cpu = CpuState::new(TEXT_BASE);
        match codegen::assemble(&stmts, &mut cpu.mem) {
            Err(e) => {
                self.sim_state = SimState::Error(e.to_string());
                false
            }
            Ok(out) => {
                cpu.pc = out.entry;
                self.cpu = Some(cpu);
                self.asm_out = Some(out);
                self.sim_state = SimState::Ready;
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
            self.prev_int_regs = cpu.regs.snapshot();
            self.prev_fp_regs = cpu.fp.snapshot();
            self.backstepper.push(cpu.pc, &cpu.regs, &cpu.fp);
            let outcome = engine::step_one(cpu, &mut self.console_out, &mut self.input_queue);
            self.apply_outcome(outcome);
            if matches!(self.sim_state, SimState::Running) {
                self.sim_state = SimState::Paused;
            }
        }
    }

    fn do_backstep(&mut self) {
        if let Some(ref mut cpu) = self.cpu {
            self.prev_int_regs = cpu.regs.snapshot();
            self.prev_fp_regs = cpu.fp.snapshot();
            if self
                .backstepper
                .pop(&mut cpu.pc, &mut cpu.regs, &mut cpu.fp)
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
        self.clear_prev_regs();
    }

    fn pump_steps(&mut self, n: u32) {
        for _ in 0..n {
            if !matches!(self.sim_state, SimState::Running) {
                return;
            }
            if let Some(ref mut cpu) = self.cpu {
                let outcome = engine::step_one(cpu, &mut self.console_out, &mut self.input_queue);
                self.apply_outcome(outcome);
            } else {
                self.sim_state = SimState::Idle;
                return;
            }
        }
    }

    fn apply_outcome(&mut self, outcome: StepOutcome) {
        match outcome {
            StepOutcome::Continue => {}
            StepOutcome::NeedInput => {
                self.sim_state = SimState::WaitingInput;
            }
            StepOutcome::Halted(c) => {
                self.sim_state = SimState::Halted(c);
            }
            StepOutcome::Faulted(m) => {
                self.sim_state = SimState::Error(m);
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
        self.console_out.push_str(&format!("> {line}"));
        self.input_buf.clear();
        self.sim_state = SimState::Running;
    }

    // ── Status ────────────────────────────────────────────────────────────────

    fn status_text(&self) -> (String, egui::Color32) {
        match &self.sim_state {
            SimState::Idle => ("Not assembled".into(), egui::Color32::GRAY),
            SimState::Ready => ("Ready".into(), egui::Color32::GREEN),
            SimState::Running => ("Running...".into(), egui::Color32::YELLOW),
            SimState::Paused => ("Paused".into(), egui::Color32::WHITE),
            SimState::WaitingInput => ("Waiting for input".into(), egui::Color32::LIGHT_BLUE),
            SimState::Halted(0) => ("Halted (exit 0)".into(), egui::Color32::GREEN),
            SimState::Halted(n) => (format!("Halted (exit {n})"), egui::Color32::YELLOW),
            SimState::Error(m) => (format!("Error: {m}"), egui::Color32::RED),
        }
    }

    // ── Panels ────────────────────────────────────────────────────────────────

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let assembled = self.cpu.is_some();
            let running = matches!(self.sim_state, SimState::Running);
            let waiting = matches!(self.sim_state, SimState::WaitingInput);
            let steppable = assembled && !running && !waiting;
            let can_back = assembled && !running && self.backstepper.len() > 0;

            if ui.button("Assemble").clicked() {
                self.do_assemble();
            }
            let can_run = self.cpu.is_some()
                && !matches!(self.sim_state, SimState::Running | SimState::WaitingInput);
            if ui
                .add_enabled(can_run, egui::Button::new("Run"))
                .clicked()
            {
                self.do_run();
            }

            ui.separator();

            if ui
                .add_enabled(steppable, egui::Button::new("Step"))
                .clicked()
            {
                self.do_step();
            }
            if ui
                .add_enabled(can_back, egui::Button::new("Backstep"))
                .clicked()
            {
                self.do_backstep();
            }
            if ui
                .add_enabled(running || waiting, egui::Button::new("Pause"))
                .clicked()
            {
                self.do_pause();
            }
            if ui
                .add_enabled(assembled, egui::Button::new("Reset"))
                .clicked()
            {
                self.do_reset();
            }

            ui.separator();

            let (msg, color) = self.status_text();
            ui.label(RichText::new(msg).color(color));

            if let Some(p) = &self.file_path {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(p.display().to_string()).weak().small());
                });
            }
        });
    }

    fn show_editor(&mut self, ui: &mut egui::Ui) {
        let title = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled.s".to_owned());
        ui.heading(title);
        ui.separator();

        egui::ScrollArea::both()
            .id_salt("editor")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.source)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .code_editor(),
                );
            });
    }

    fn show_registers(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.register_tab, RegisterTab::Integer, "Integer");
            ui.selectable_value(&mut self.register_tab, RegisterTab::Float, "Float");
            ui.selectable_value(&mut self.register_tab, RegisterTab::Csr, "CSR");
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("reg_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| match self.register_tab {
                RegisterTab::Integer => {
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
                                let val = self.cpu.as_ref().map_or(0, |c| c.regs.read(i));
                                let changed = val != self.prev_int_regs[i];
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
                                    RichText::new(format!("{val:#010x}")).monospace().color(color),
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
                                let raw = self.cpu.as_ref().map_or(0u64, |c| c.fp.snapshot()[i]);
                                let changed = raw != self.prev_fp_regs[i];
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
                    let pc = self.cpu.as_ref().map_or(0u32, |c| c.pc);
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
                            for (addr, name, desc) in csr_rows {
                                let val = if *addr == u32::MAX {
                                    pc
                                } else {
                                    self.cpu.as_ref().map_or(0, |c| c.csr.read(*addr))
                                };
                                ui.label(RichText::new(*name).monospace().color(egui::Color32::GRAY));
                                ui.label(RichText::new(format!("{val:#010x}")).monospace());
                                ui.label(RichText::new(*desc).small().weak());
                                ui.end_row();
                            }
                        });
                }
            });
    }

    fn show_console(&mut self, ui: &mut egui::Ui) {
        let waiting = matches!(self.sim_state, SimState::WaitingInput);

        let avail = if waiting {
            ui.available_height() - 40.0
        } else {
            ui.available_height()
        };

        egui::ScrollArea::vertical()
            .id_salt("console")
            .auto_shrink([false; 2])
            .max_height(avail.max(40.0))
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(RichText::new(&self.console_out).monospace().size(12.0))
                        .selectable(true),
                );
            });

        if waiting {
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

    fn show_text_segment(&mut self, ui: &mut egui::Ui) {
        let current_pc = self.cpu.as_ref().map(|c| c.pc);
        let source_lines: Vec<&str> = self.source.lines().collect();

        if let Some(asm) = &self.asm_out {
            let rows = &asm.text_rows;
            TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::initial(20.0).resizable(false))
                .column(Column::initial(110.0).resizable(true))
                .column(Column::initial(100.0).resizable(true))
                .column(Column::remainder())
                .header(20.0, |mut h| {
                    h.col(|_| {});
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
                        let tr = &rows[i];
                        let hot = current_pc == Some(tr.addr);
                        let src = source_lines
                            .get(tr.src_line.saturating_sub(1) as usize)
                            .copied()
                            .unwrap_or("")
                            .trim();

                        row.col(|ui| {
                            if hot {
                                ui.label(RichText::new("→").color(egui::Color32::YELLOW));
                            }
                        });
                        row.col(|ui| {
                            let t = RichText::new(format!("{:#010x}", tr.addr)).monospace();
                            ui.label(if hot { t.color(egui::Color32::YELLOW) } else { t });
                        });
                        row.col(|ui| {
                            ui.label(RichText::new(format!("{:#010x}", tr.word)).monospace());
                        });
                        row.col(|ui| {
                            let t = RichText::new(src).monospace();
                            let resp =
                                ui.label(if hot { t.color(egui::Color32::YELLOW) } else { t });
                            if hot {
                                resp.scroll_to_me(None);
                            }
                        });
                    });
                });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Assemble first to view the text segment.");
            });
        }
    }
}

// ─── Help content (free functions) ───────────────────────────────────────────

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
                ui.label(RichText::new(*instr).monospace().color(egui::Color32::LIGHT_BLUE));
                ui.label(*desc);
                ui.label(RichText::new(*example).monospace().weak());
                ui.end_row();
            }
        });
}

fn show_help_content(ui: &mut egui::Ui) {
    ui.label(RichText::new("OARS Instruction Reference — RV32IMFD + Zicsr, RARS-compatible").weak());
    ui.add_space(4.0);

    egui::CollapsingHeader::new("📌  Pseudo-Instructions  (most commonly used)")
        .default_open(true)
        .show(ui, |ui| {
            instr_table(ui, "pseudo", &[
                ("li  rd, imm",          "Load immediate value into rd",              "li   t0, 42"),
                ("la  rd, label",        "Load address of label into rd",             "la   a0, msg"),
                ("mv  rd, rs",           "rd = rs (copy register)",                   "mv   a0, t0"),
                ("not rd, rs",           "rd = ~rs (bitwise NOT)",                    "not  t0, t1"),
                ("neg rd, rs",           "rd = -rs (two's complement negate)",        "neg  t0, t1"),
                ("nop",                  "No operation (addi x0, x0, 0)",             "nop"),
                ("j   label",            "Unconditional jump to label",               "j    loop"),
                ("jr  rs",               "Jump to address in rs",                     "jr   ra"),
                ("ret",                  "Return from function  (jr ra)",             "ret"),
                ("call label",           "Call function, save return addr in ra",     "call my_func"),
                ("beqz rs, label",       "Branch if rs == 0",                         "beqz t0, done"),
                ("bnez rs, label",       "Branch if rs != 0",                         "bnez t0, loop"),
                ("blez rs, label",       "Branch if rs <= 0",                         "blez t0, neg"),
                ("bgez rs, label",       "Branch if rs >= 0",                         "bgez t0, pos"),
                ("bltz rs, label",       "Branch if rs < 0",                          "bltz t0, neg"),
                ("bgtz rs, label",       "Branch if rs > 0",                          "bgtz t0, pos"),
                ("bgt  rs, rt, label",   "Branch if rs > rt  (signed)",               "bgt  t0, t1, big"),
                ("ble  rs, rt, label",   "Branch if rs <= rt  (signed)",              "ble  t0, t1, small"),
                ("seqz rd, rs",          "rd = 1 if rs == 0,  else 0",               "seqz t0, a0"),
                ("snez rd, rs",          "rd = 1 if rs != 0,  else 0",               "snez t0, a0"),
                ("sltz rd, rs",          "rd = 1 if rs < 0,   else 0",               "sltz t0, a0"),
                ("sgtz rd, rs",          "rd = 1 if rs > 0,   else 0",               "sgtz t0, a0"),
            ]);
        });

    egui::CollapsingHeader::new("🔢  RV32I — Base Integer Instructions")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Arithmetic (R-type)").strong());
            instr_table(ui, "rv32i_r", &[
                ("add  rd, rs1, rs2",  "rd = rs1 + rs2",                           "add  t0, t1, t2"),
                ("sub  rd, rs1, rs2",  "rd = rs1 - rs2",                           "sub  t0, t1, t2"),
                ("and  rd, rs1, rs2",  "rd = rs1 & rs2",                           "and  t0, t1, t2"),
                ("or   rd, rs1, rs2",  "rd = rs1 | rs2",                           "or   t0, t1, t2"),
                ("xor  rd, rs1, rs2",  "rd = rs1 ^ rs2",                           "xor  t0, t1, t2"),
                ("sll  rd, rs1, rs2",  "rd = rs1 << rs2[4:0]  (logical left)",     "sll  t0, t1, t2"),
                ("srl  rd, rs1, rs2",  "rd = rs1 >> rs2[4:0]  (logical right)",    "srl  t0, t1, t2"),
                ("sra  rd, rs1, rs2",  "rd = rs1 >> rs2[4:0]  (arithmetic right)", "sra  t0, t1, t2"),
                ("slt  rd, rs1, rs2",  "rd = 1 if rs1 < rs2 (signed)",             "slt  t0, t1, t2"),
                ("sltu rd, rs1, rs2",  "rd = 1 if rs1 < rs2 (unsigned)",           "sltu t0, t1, t2"),
            ]);
            ui.add_space(4.0);
            ui.label(RichText::new("Arithmetic Immediate (I-type)").strong());
            instr_table(ui, "rv32i_i", &[
                ("addi  rd, rs1, imm", "rd = rs1 + sign_ext(imm12)",               "addi t0, t1, 10"),
                ("andi  rd, rs1, imm", "rd = rs1 & sign_ext(imm12)",               "andi t0, t1, 0xFF"),
                ("ori   rd, rs1, imm", "rd = rs1 | sign_ext(imm12)",               "ori  t0, t1, 1"),
                ("xori  rd, rs1, imm", "rd = rs1 ^ sign_ext(imm12)",               "xori t0, t1, -1"),
                ("slli  rd, rs1, shamt","rd = rs1 << shamt",                       "slli t0, t1, 2"),
                ("srli  rd, rs1, shamt","rd = rs1 >> shamt (logical)",             "srli t0, t1, 2"),
                ("srai  rd, rs1, shamt","rd = rs1 >> shamt (arithmetic)",          "srai t0, t1, 2"),
                ("slti  rd, rs1, imm", "rd = 1 if rs1 < imm (signed)",             "slti t0, t1, 5"),
                ("sltiu rd, rs1, imm", "rd = 1 if rs1 < imm (unsigned)",           "sltiu t0,t1, 5"),
            ]);
            ui.add_space(4.0);
            ui.label(RichText::new("Loads").strong());
            instr_table(ui, "rv32i_load", &[
                ("lw  rd, offset(rs1)", "Load 32-bit word",                         "lw  t0, 0(a0)"),
                ("lh  rd, offset(rs1)", "Load 16-bit halfword, sign-extend",        "lh  t0, 2(a0)"),
                ("lhu rd, offset(rs1)", "Load 16-bit halfword, zero-extend",        "lhu t0, 2(a0)"),
                ("lb  rd, offset(rs1)", "Load 8-bit byte, sign-extend",             "lb  t0, 1(a0)"),
                ("lbu rd, offset(rs1)", "Load 8-bit byte, zero-extend",             "lbu t0, 1(a0)"),
            ]);
            ui.add_space(4.0);
            ui.label(RichText::new("Stores").strong());
            instr_table(ui, "rv32i_store", &[
                ("sw rs2, offset(rs1)", "Store 32-bit word",                        "sw  t0, 0(a0)"),
                ("sh rs2, offset(rs1)", "Store low 16 bits",                        "sh  t0, 2(a0)"),
                ("sb rs2, offset(rs1)", "Store low 8 bits",                         "sb  t0, 1(a0)"),
            ]);
            ui.add_space(4.0);
            ui.label(RichText::new("Branches").strong());
            instr_table(ui, "rv32i_branch", &[
                ("beq  rs1, rs2, label","Branch if rs1 == rs2",                     "beq  t0, t1, done"),
                ("bne  rs1, rs2, label","Branch if rs1 != rs2",                     "bne  t0, t1, loop"),
                ("blt  rs1, rs2, label","Branch if rs1 < rs2  (signed)",            "blt  t0, t1, neg"),
                ("bltu rs1, rs2, label","Branch if rs1 < rs2  (unsigned)",          "bltu t0, t1, wrap"),
                ("bge  rs1, rs2, label","Branch if rs1 >= rs2 (signed)",            "bge  t0, t1, pos"),
                ("bgeu rs1, rs2, label","Branch if rs1 >= rs2 (unsigned)",          "bgeu t0, t1, ok"),
            ]);
            ui.add_space(4.0);
            ui.label(RichText::new("Jumps & Upper").strong());
            instr_table(ui, "rv32i_jump", &[
                ("jal  rd, label",      "Jump and link — rd = PC+4, PC = label",    "jal  ra, my_func"),
                ("jalr rd, rs1, offset","Jump and link register",                   "jalr zero, ra, 0"),
                ("lui  rd, imm",        "rd = imm << 12  (upper 20 bits)",          "lui  t0, 0x10010"),
                ("auipc rd, offset",    "rd = PC + (offset << 12)",                 "auipc t0, 0"),
            ]);
        });

    egui::CollapsingHeader::new("✖  RV32M — Multiply / Divide")
        .default_open(false)
        .show(ui, |ui| {
            instr_table(ui, "rv32m", &[
                ("mul    rd, rs1, rs2", "rd = (rs1 × rs2)[31:0]  (low 32 bits)",          "mul  t0, t1, t2"),
                ("mulh   rd, rs1, rs2", "rd = (rs1 × rs2)[63:32] signed × signed",        "mulh t0, t1, t2"),
                ("mulhsu rd, rs1, rs2", "rd = (rs1 × rs2)[63:32] signed × unsigned",      "mulhsu t0,t1,t2"),
                ("mulhu  rd, rs1, rs2", "rd = (rs1 × rs2)[63:32] unsigned × unsigned",    "mulhu t0,t1,t2"),
                ("div    rd, rs1, rs2", "rd = rs1 ÷ rs2  (signed; -1 if div-by-zero)",    "div  t0, t1, t2"),
                ("divu   rd, rs1, rs2", "rd = rs1 ÷ rs2  (unsigned; MAX if div-by-zero)", "divu t0, t1, t2"),
                ("rem    rd, rs1, rs2", "rd = rs1 mod rs2 (signed remainder)",             "rem  t0, t1, t2"),
                ("remu   rd, rs1, rs2", "rd = rs1 mod rs2 (unsigned remainder)",           "remu t0, t1, t2"),
            ]);
        });

    egui::CollapsingHeader::new("🔵  RV32F — Single-Precision Floating Point")
        .default_open(false)
        .show(ui, |ui| {
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
        });

    egui::CollapsingHeader::new("🟣  RV32D — Double-Precision Floating Point")
        .default_open(false)
        .show(ui, |ui| {
            instr_table(ui, "rv32d", &[
                ("fld  fd, offset(rs)",   "Load 64-bit double from memory",            "fld  ft0, 0(a0)"),
                ("fsd  fs, offset(rs)",   "Store 64-bit double to memory",             "fsd  ft0, 0(a0)"),
                ("fadd.d fd, fs1, fs2",   "fd = fs1 + fs2  (double)",                  "fadd.d ft0,ft1,ft2"),
                ("fsub.d fd, fs1, fs2",   "fd = fs1 - fs2  (double)",                  "fsub.d ft0,ft1,ft2"),
                ("fmul.d fd, fs1, fs2",   "fd = fs1 × fs2  (double)",                  "fmul.d ft0,ft1,ft2"),
                ("fdiv.d fd, fs1, fs2",   "fd = fs1 ÷ fs2  (double)",                  "fdiv.d ft0,ft1,ft2"),
                ("fsqrt.d fd, fs1",       "fd = √fs1  (double)",                       "fsqrt.d ft0, ft1"),
                ("feq.d  rd, fs1, fs2",   "rd = 1 if fs1 == fs2  (double)",            "feq.d t0,ft0,ft1"),
                ("flt.d  rd, fs1, fs2",   "rd = 1 if fs1 < fs2   (double)",            "flt.d t0,ft0,ft1"),
                ("fle.d  rd, fs1, fs2",   "rd = 1 if fs1 <= fs2  (double)",            "fle.d t0,ft0,ft1"),
                ("fcvt.w.d  rd, fs",      "Convert double → signed int",               "fcvt.w.d t0, ft0"),
                ("fcvt.d.w  fd, rs",      "Convert signed int → double",               "fcvt.d.w ft0, t0"),
                ("fcvt.s.d  fd, fs",      "Convert double → single",                   "fcvt.s.d ft0, ft1"),
                ("fcvt.d.s  fd, fs",      "Convert single → double",                   "fcvt.d.s ft0, ft1"),
                ("fclass.d  rd, fs",      "rd = bitmask classifying fs (double)",      "fclass.d t0, ft0"),
            ]);
        });

    egui::CollapsingHeader::new("⚙  Zicsr — Control & Status Register Instructions")
        .default_open(false)
        .show(ui, |ui| {
            instr_table(ui, "csr", &[
                ("csrrw  rd, csr, rs1",  "rd = CSR; CSR = rs1",                     "csrrw t0, fcsr, t1"),
                ("csrrs  rd, csr, rs1",  "rd = CSR; CSR |= rs1  (set bits)",        "csrrs t0, fflags, t1"),
                ("csrrc  rd, csr, rs1",  "rd = CSR; CSR &= ~rs1 (clear bits)",      "csrrc t0, fflags, t1"),
                ("csrrwi rd, csr, uimm", "rd = CSR; CSR = zero_ext(uimm5)",         "csrrwi t0, frm, 0"),
                ("csrrsi rd, csr, uimm", "rd = CSR; CSR |= uimm5",                  "csrrsi t0, fflags, 1"),
                ("csrrci rd, csr, uimm", "rd = CSR; CSR &= ~uimm5",                 "csrrci t0, fflags, 1"),
            ]);
            ui.add_space(4.0);
            ui.label(RichText::new("Common CSR addresses:").strong());
            instr_table(ui, "csr_addrs", &[
                ("0x001  fflags",  "FP accrued exception flags (NX/UF/OF/DZ/NV)", ""),
                ("0x002  frm",     "FP rounding mode (000=RNE, 001=RTZ, …)",       ""),
                ("0x003  fcsr",    "FP control/status = frm<<5 | fflags",           ""),
                ("0xC00  cycle",   "Cycle counter (low 32 bits)",                   ""),
                ("0xC02  instret", "Instructions-retired counter (low 32 bits)",    ""),
            ]);
        });

    egui::CollapsingHeader::new("📝  Assembler Directives")
        .default_open(false)
        .show(ui, |ui| {
            instr_table(ui, "directives", &[
                (".text",            "Switch to code segment",                  ".text"),
                (".data",            "Switch to data segment",                  ".data"),
                (".globl label",     "Make label visible to linker",            ".globl main"),
                (".word v1, v2, …",  "Emit 32-bit word(s)",                     ".word 42, 0xFF"),
                (".half v1, v2, …",  "Emit 16-bit halfword(s)",                 ".half 0, 1"),
                (".byte v1, v2, …",  "Emit 8-bit byte(s)",                      ".byte 'A', 10"),
                (".ascii \"str\"",   "Emit string bytes (no null terminator)",   ".ascii \"hi\""),
                (".asciiz \"str\"",  "Emit null-terminated string",              ".asciiz \"hi\\n\""),
                (".space n",         "Reserve n zero bytes",                    ".space 64"),
                (".align n",         "Align to 2^n byte boundary",              ".align 2"),
                (".float f",         "Emit 32-bit IEEE float",                  ".float 3.14"),
                (".double d",        "Emit 64-bit IEEE double",                 ".double 2.718"),
            ]);
        });

    egui::CollapsingHeader::new("📞  Syscall Reference  (a7 = service number, ecall)")
        .default_open(false)
        .show(ui, |ui| {
            instr_table(ui, "syscalls", &[
                ("1  print_int",     "Print a0 as signed decimal",               "li a7,1; mv a0,t0; ecall"),
                ("2  print_float",   "Print fa0 as float",                       "li a7,2; ecall"),
                ("3  print_double",  "Print fa0 as double",                      "li a7,3; ecall"),
                ("4  print_string",  "Print null-terminated string at a0",       "li a7,4; la a0,msg; ecall"),
                ("5  read_int",      "Read integer from stdin → a0",             "li a7,5; ecall"),
                ("6  read_float",    "Read float from stdin → fa0",              "li a7,6; ecall"),
                ("7  read_double",   "Read double from stdin → fa0",             "li a7,7; ecall"),
                ("8  read_string",   "Read string; a0=buf, a1=max bytes",        "li a7,8; la a0,buf; li a1,32; ecall"),
                ("10 exit",          "Terminate program (exit code 0)",          "li a7,10; ecall"),
                ("11 print_char",    "Print a0[7:0] as ASCII character",         "li a7,11; li a0,'A'; ecall"),
                ("12 read_char",     "Read one character → a0",                  "li a7,12; ecall"),
                ("34 print_hex",     "Print a0 as hexadecimal",                  "li a7,34; ecall"),
                ("35 print_bin",     "Print a0 as binary",                       "li a7,35; ecall"),
                ("36 print_uint",    "Print a0 as unsigned decimal",             "li a7,36; ecall"),
                ("93 exit2",         "Terminate with exit code in a0",           "li a7,93; li a0,1; ecall"),
            ]);
        });
}

// ─── eframe::App ─────────────────────────────────────────────────────────────

impl eframe::App for OarsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-run: save snapshot before burst so highlights show per-frame changes.
        if matches!(self.sim_state, SimState::Running) {
            self.save_prev_regs();
            self.pump_steps(self.steps_per_frame);
            ctx.request_repaint();
        }

        // Floating help window
        let mut help_open = self.show_help;
        if help_open {
            egui::Window::new("Instruction Reference")
                .open(&mut help_open)
                .default_size([760.0, 540.0])
                .resizable(true)
                .scroll([false, true])
                .show(ctx, |ui| {
                    show_help_content(ui);
                });
        }
        self.show_help = help_open;

        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open…").clicked() {
                        self.do_open();
                        ui.close_menu();
                    }
                    if ui.button("Save").clicked() {
                        self.do_save();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Instruction Reference").clicked() {
                        self.show_help = true;
                        ui.close_menu();
                    }
                });
            });
        });

        // Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.show_toolbar(ui);
        });

        // Bottom panel: Console only
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .default_height(200.0)
            .show(ctx, |ui| {
                ui.strong("Console");
                ui.separator();
                self.show_console(ui);
            });

        // Right panel: register tabs
        egui::SidePanel::right("registers")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                self.show_registers(ui);
            });

        // Centre: Text Segment on top (large), Editor below (resizable)
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::TopBottomPanel::bottom("editor_panel")
                .resizable(true)
                .default_height(220.0)
                .min_height(80.0)
                .show_inside(ui, |ui| {
                    self.show_editor(ui);
                });

            ui.strong("Text Segment");
            ui.separator();
            self.show_text_segment(ui);
        });
    }
}
