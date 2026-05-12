use std::collections::VecDeque;
use std::path::PathBuf;

use egui::RichText;
use egui_extras::{Column, TableBuilder};

use crate::assembler::{
    codegen::{self, AssemblyOutput},
    parser,
};
use crate::hardware::{fp_registers::FP_REG_NAMES, memory::TEXT_BASE, registers::REG_NAMES};
use crate::simulator::{
    backstepper::Backstepper,
    engine::{self, CpuState, StepOutcome},
};

// ─── State ───────────────────────────────────────────────────────────────────

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
enum BottomTab {
    Console,
    TextSegment,
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

    bottom_tab: BottomTab,
}

const DEFAULT_SOURCE: &str = "\
        .text
        .globl main
main:
        # Write your RISC-V assembly here, then click Assemble & Run.
        li      a7, 10          # exit
        ecall
";

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
            bottom_tab: BottomTab::Console,
        }
    }

    // ── Actions ──────────────────────────────────────────────────────────────

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

    fn do_assemble_and_run(&mut self) {
        if self.do_assemble() {
            self.sim_state = SimState::Running;
        }
    }

    fn do_step(&mut self) {
        if let Some(ref mut cpu) = self.cpu {
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
        // Re-assemble into a fresh CPU but keep the same source/asm_out
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
        // Cap console output at 64 KiB
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
        // Try to resume — re-enter Running so pump_steps picks it up next frame
        self.sim_state = SimState::Running;
    }

    // ── Status text ──────────────────────────────────────────────────────────

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

    // ── Panels ───────────────────────────────────────────────────────────────

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Open").clicked() {
                self.do_open();
            }
            if ui.button("Save").clicked() {
                self.do_save();
            }

            ui.separator();

            let assembled = self.cpu.is_some();
            let running = matches!(self.sim_state, SimState::Running);
            let waiting = matches!(self.sim_state, SimState::WaitingInput);
            let steppable = assembled && !running && !waiting;
            let can_back = assembled && !running && self.backstepper.len() > 0;

            if ui.button("Assemble & Run").clicked() {
                self.do_assemble_and_run();
            }

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
        ui.heading("Registers");
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("reg_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.label(RichText::new("Integer").strong());
                egui::Grid::new("int_regs")
                    .num_columns(3)
                    .spacing([6.0, 1.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for (i, name) in REG_NAMES.iter().enumerate() {
                            let (val, changed) = if let Some(cpu) = &self.cpu {
                                (cpu.regs.read(i), false)
                            } else {
                                (0, false)
                            };
                            let _ = changed;
                            ui.label(RichText::new(format!("x{i:02}")).monospace().weak());
                            ui.label(RichText::new(*name).monospace());
                            ui.label(RichText::new(format!("{val:#010x}")).monospace());
                            ui.end_row();
                        }
                    });

                ui.add_space(8.0);
                ui.collapsing("FP Registers", |ui| {
                    egui::Grid::new("fp_regs")
                        .num_columns(3)
                        .spacing([6.0, 1.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for (i, fp_name) in FP_REG_NAMES.iter().enumerate() {
                                let val = if let Some(cpu) = &self.cpu {
                                    cpu.fp.read_f64(i)
                                } else {
                                    0.0
                                };
                                ui.label(RichText::new(format!("f{i:02}")).monospace().weak());
                                ui.label(RichText::new(*fp_name).monospace());
                                ui.label(RichText::new(format!("{val:.6}")).monospace());
                                ui.end_row();
                            }
                        });
                });
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
                .column(Column::initial(95.0))
                .column(Column::initial(95.0))
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
                                ui.label("->");
                            }
                        });
                        row.col(|ui| {
                            let t = RichText::new(format!("{:#010x}", tr.addr)).monospace();
                            ui.label(if hot { t.strong() } else { t });
                        });
                        row.col(|ui| {
                            ui.label(RichText::new(format!("{:#010x}", tr.word)).monospace());
                        });
                        row.col(|ui| {
                            let resp = ui.label(RichText::new(src).monospace());
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

// ─── eframe::App ─────────────────────────────────────────────────────────────

impl eframe::App for OarsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-run: execute a burst of instructions each frame.
        if matches!(self.sim_state, SimState::Running) {
            self.pump_steps(self.steps_per_frame);
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.show_toolbar(ui);
        });

        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .default_height(200.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.bottom_tab, BottomTab::Console, "Console");
                    ui.selectable_value(
                        &mut self.bottom_tab,
                        BottomTab::TextSegment,
                        "Text Segment",
                    );
                });
                ui.separator();
                match self.bottom_tab {
                    BottomTab::Console => self.show_console(ui),
                    BottomTab::TextSegment => self.show_text_segment(ui),
                }
            });

        egui::SidePanel::right("registers")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                self.show_registers(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_editor(ui);
        });
    }
}
