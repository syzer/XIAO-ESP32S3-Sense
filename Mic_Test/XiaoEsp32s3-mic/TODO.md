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
