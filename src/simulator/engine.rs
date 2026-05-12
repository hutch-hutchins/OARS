use crate::cli::RunOpts;
use crate::hardware::{
    memory::{Memory, STACK_TOP},
    registers::RegisterFile,
};
use crate::isa::rv32i::{step, StepResult};
use crate::simulator::syscalls;
use crate::util::error::OarsError;
use anyhow::Result;
use serde::Serialize;
use std::io::{self, BufRead, Write};

pub struct CpuState {
    pub regs: RegisterFile,
    pub mem:  Memory,
    pub pc:   u32,
}

impl CpuState {
    pub fn new(entry: u32) -> Self {
        let mut regs = RegisterFile::new();
        // Set up standard ABI state: sp = stack top, gp = 0 (no linker)
        regs.write(2, STACK_TOP); // sp
        Self { regs, mem: Memory::new(), pc: entry }
    }
}

#[derive(Serialize)]
pub struct Telemetry {
    pub instructions: u64,
    pub exit_code: i32,
}

/// Run the simulation until exit or max_steps.
pub fn run(
    cpu: &mut CpuState,
    opts: &RunOpts,
    stdout: &mut dyn Write,
    stdin:  &mut dyn BufRead,
) -> Result<Telemetry> {
    let mut steps: u64 = 0;

    loop {
        if opts.max_steps > 0 && steps >= opts.max_steps {
            break;
        }

        let word = cpu.mem.load_word(cpu.pc);
        match step(word, cpu.pc, &mut cpu.regs, &mut cpu.mem)? {
            StepResult::Next(next_pc) => {
                cpu.pc = next_pc;
            }
            StepResult::Ecall => {
                let pc = cpu.pc;
                let cont = syscalls::dispatch(
                    &mut cpu.regs,
                    &mut cpu.mem,
                    pc,
                    stdout,
                    stdin,
                )?;
                if !cont {
                    let exit_code = cpu.regs.read(10) as i32;
                    return Ok(Telemetry { instructions: steps, exit_code });
                }
                cpu.pc = cpu.pc.wrapping_add(4);
            }
            StepResult::Ebreak => {
                // In CLI mode, treat EBREAK as halt
                break;
            }
        }

        steps += 1;
    }

    Ok(Telemetry { instructions: steps, exit_code: 0 })
}
