🎚️ 2. Reset I2S0 before touching registers

🕓 3. Configure clocks

The PDM input clock must be ≈ 1 MHz (16 kHz × 64).
Example:
i2s.clkm_conf().modify(|_, w| {
    w.clka_en().set_bit();
    w.clkm_div_a().bits(0);
    w.clkm_div_b().bits(0);
    w.clkm_div_num().bits(1)
});

🎤 4. Enable PDM → PCM mode
i2s.rx_conf().modify(|_, w| {
    w.rx_tdm_en().clear_bit();
    w.rx_pdm_en().set_bit();     // Enable PDM front-end
    w.rx_mono().set_bit();       // Mono mode
    w
});
i2s.rx_conf1().modify(|_, w| w.rx_pdm2pcm_en().set_bit());

🔢 5. Set decimation ratio (SINC downsample)
In the PAC it’s normally under rx_pdm_conf():
i2s.rx_pdm_conf().modify(|_, w| unsafe {
    w.rx_sinc_osr2().bits(64);   // 64× decimation
    w
});

🎧 6. Configure slot and channel width
i2s.rx_tdm_ctrl().modify(|_, w| unsafe {
    w.rx_total_chan_num().bits(1);
    w.rx_chan_bits().bits(16);
    w
});

▶️ 7. Start RX
i2s.conf().modify(|_, w| w.rx_start().set_bit());


🔍 8. Verify
	Scope GPIO42: ~1 MHz square wave = OK.
	•	Scope GPIO41: noisy PDM waveform = mic output present.

If both signals look good → the hardware side is alive.
Then, to capture PCM, read the RX FIFO or attach DMA later.



Re‑enable the USB writes so you can actually monitor the stream once it works. Right now they’re still commented out inside the mic branch.
Inspect the raw bytes rather than 16‑bit little‑endian pairs. Dump buffer[..32] as hex so we can see if the words are really changing or we’re misinterpreting bit order.
Channel selection – we currently call regs.rx_conf().modify(|_, w| w.rx_mono_fst_vld().clear_bit()); which picks LEFT. Many modules wire PDM data on the other slot; try flipping it by setting the bit and rerun the 30 s test.
SINC decimation bits – we commented out .rx_pdm_sinc_dsr_64_en().set_bit(). Without the x64 filter you can end up with low‑level DC. Try enabling the 64× decimator and disable the other ones (16/32).
Sample formatting – right now we assume the hardware gives us 16‑bit signed PCM. If the result is actually 24‑bit or 12‑bit left‑aligned in 16 bits, we need to realign it (e.g., subtract midpoint 0x0800 and left-shift). Once you print raw bytes we’ll know which case it is.

1. Use the non‑blocking API and drop/backoff when the host can’t keep up.
}
   for &byte in &buffer[..read_bytes] {       if let Err(nb::Error::WouldBlock) = usb.write_byte_nb(byte) {           break; // or sleep/backoff       }   }
2. Chunk to the host’s packet size. Write smaller slices—e.g. repeatedly write 64 bytes from the buffer instead of a whole 1 KB frame. That keeps SoX fed more evenly: 
   for chunk in buffer[..read_bytes].chunks(64) {       let _ = usb.write(chunk);   }

3. Keep the espflash --monitor (or your record_with_timestamps.sh) running so something is always reading. You can pipe its stdout into SoX on the host if you want the speaker output without starting a separate command later.