# Project TODO

This file lists actionable steps to replace the synthetic microphone scaffold with a working PDM I2S receiver for the XIAO ESP32S3 Sense.

Guiding principle: validate each change with a short run:

```fish
timeout 20s cargo run
```

## High priority

- [ ] Implement I2S0 PDM RX initialization (low-level registers or PAC)
  - Configure pin matrix: CLK = GPIO42, DIN = GPIO41
  - Set PDM/I2S clock dividers to produce 16 kHz sampling (target 16_000 Hz)
  - Configure data format: 16-bit, mono, RIGHT slot mask (match Arduino sketch)
  - Configure PDM decimation/filter registers (PDM_RX configuration)
  - Configure DMA / descriptors (e.g. 8 descriptors, frame size 256 or ring of 1024 samples)
  - Start channel and enable interrupts or DMA completion

- [ ] Replace synthetic generator with real DMA or FIFO reads into `frame: [i16; 1024]`
  - Provide a `read_frame()` that returns number of bytes filled
  - Keep decimated debug printing until verified

- [ ] Basic capture test
  - Run `timeout 20s cargo run`
  - Expect serial output lines like `frame <n> <samples...>` within the window
  - If silence: flip slot mask to LEFT and retry

## Medium priority

- [ ] Add a binary/raw-mode serial option that writes S16LE PCM directly (for use with Python/ffmpeg)
- [ ] Support WAV framing or a small header when dumping to serial
- [ ] Improve buffering: double-buffer or lock-free ring with DMA callbacks

## Cleanups & docs

- [ ] Move inline `mic` module into `src/mic.rs` (shared lib) once init is stable
- [ ] Add `README.md` with run instructions and expected output format (pinout, sample rate, slot)
- [x] Keep `agents.md` for the agent verification procedure (20s checks)

## Follow-ups

- Investigate using `esp-idf` I2S PDM driver via `esp-idf-sys` or `esp-idf-hal` if low-level work becomes brittle.
- Add unit/integration tests for sample handling code.


---

If you want, I can start on the first high-priority item now and wire the concrete register sequence for PDM RX on the ESP32-S3.
