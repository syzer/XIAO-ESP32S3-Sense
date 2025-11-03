#![allow(dead_code)]
//! Minimal scaffolding for a PDM microphone capture on ESP32-S3 using I2S0.
//! Duplicate of earlier `src/mic.rs` placed here so `mod mic;` in `main.rs` works.
//! TODO: remove `src/mic.rs` or convert to a shared library layout later.

use core::cell::Cell;

pub const SAMPLE_RATE_HZ: u32 = 16_000;
pub const FRAME_SAMPLES: usize = 1024; // matches Arduino example buffer length

#[derive(Copy, Clone, Debug)]
pub enum SlotSelect {
    Left,
    Right,
}

pub struct MicPdm {
    slot: SlotSelect,
    synth_phase: Cell<i16>,
}

impl MicPdm {
    pub fn new(slot: SlotSelect) -> Self {
        Self { slot, synth_phase: Cell::new(0) }
    }

    pub fn init(&self) -> Result<(), &'static str> {
        // TODO: hardware register configuration for PDM RX
        Ok(())
    }

    pub fn read_frame(&self, out: &mut [i16]) -> usize {
        let mut phase = self.synth_phase.get();
        for s in out.iter_mut() {
            phase = phase.wrapping_add(173);
            *s = phase;
        }
        self.synth_phase.set(phase);
        out.len() * core::mem::size_of::<i16>()
    }

    pub fn slot(&self) -> SlotSelect { self.slot }
}
