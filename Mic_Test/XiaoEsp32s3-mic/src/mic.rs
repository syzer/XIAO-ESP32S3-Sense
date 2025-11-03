#![allow(dead_code)]
//! Minimal scaffolding for a PDM microphone capture on ESP32-S3 using I2S0.
//!
//! This is OPTION B (no_std + esp-hal) groundwork: we set up a struct and
//! placeholder read path. Actual PDM register configuration is still TODO.
//! Next step will populate `init()` with proper register writes for:
//! - Clock dividers for 16 kHz sample rate
//! - PDM RX enable & decimation filters
//! - Mono 16-bit slot (RIGHT or LEFT selectable)
//! - DMA descriptors pointing to a ring of sample buffers
//!
//! Safety notes:
//! - Direct register access will require `unsafe` and PAC types once added.
//! - We isolate unsafe to small blocks so later auditing is simpler.
//!
//! For now `read()` synthesizes a simple ramp so the main loop can exercise
//! throughput and timing while we incrementally add real hardware capture.

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
    // Simple synthetic buffer until DMA + I2S configured
    synth_phase: Cell<i16>,
}

impl MicPdm {
    pub fn new(slot: SlotSelect) -> Self {
        // Placeholder: later we'll take `peripherals.I2S0`, enable clocks, etc.
        // Design contract:
        //   init() must leave I2S0 in running RX state producing 16-bit samples
        //   into a DMA ring we can drain. For first milestone we poll FIFO.
        Self { slot, synth_phase: Cell::new(0) }
    }

    /// Configure hardware registers for PDM RX.
    ///
    /// CURRENTLY A STUB. Returns Ok immediately.
    pub fn init(&self) -> Result<(), &'static str> {
        // TODO: Acquire peripheral clocks & reset: e.g.
        // let system = unsafe { &*esp_hal::peripherals::SYSTEM::ptr() }; (if exposed)
        // Configure I2S0 clock, PDM RX mode, slot mask based on self.slot.
        Ok(())
    }

    /// Read one frame of 16-bit samples. For now produce a synthetic ramp so
    /// downstream code can treat it like real PCM.
    pub fn read_frame(&self, out: &mut [i16]) -> usize {
        let mut phase = self.synth_phase.get();
        for s in out.iter_mut() {
            // Simple triangle-ish waveform for visibility: counts up then wraps.
            phase = phase.wrapping_add(173); // arbitrary step
            *s = phase;
        }
        self.synth_phase.set(phase);
        out.len() * core::mem::size_of::<i16>()
    }

    pub fn slot(&self) -> SlotSelect { self.slot }
}
