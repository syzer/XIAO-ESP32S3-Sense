#![no_std]
#![no_main]

esp_bootloader_esp_idf::esp_app_desc!();

use esp_backtrace as _;
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use esp_hal::{
    dma_circular_buffers,
    gpio::{Level, Output, OutputConfig},
    i2s::master::{Channels, Config as I2sConfig, DataFormat, I2s},
    init,
    time::Rate,
};
use esp_println::println;
use esp32s3 as pac;
// use embedded_hal::delay::DelayNs;

#[esp_hal::main]
fn main() -> ! {
    // Initialize base HAL peripherals
    let peripherals = init(esp_hal::Config::default());

    // --- Enable I2S0 peripheral clock and reset peripheral ---
    let sys = unsafe { &*pac::SYSTEM::ptr() };
    sys.perip_clk_en0().modify(|_, w| w.i2s0_clk_en().set_bit());
    // Pulse reset: assert, then deassert
    sys.perip_rst_en0().modify(|_, w| w.i2s0_rst().set_bit());
    sys.perip_rst_en0().modify(|_, w| w.i2s0_rst().clear_bit());

    let i2s_regs = unsafe { &*esp32s3::I2S0::ptr() };

    // I2S0 is reset above via SYSTEM.perip_rst_en0.

    // --- USB-Serial-JTAG for raw PCM streaming ---
    let mut usb = UsbSerialJtag::new(peripherals.USB_DEVICE);

    // --- Step 3: Create I2S instance (DMA CH0) ---
    const SAMPLE_RATE: u32 = 16_000;
    const BUFFER_SIZE: usize = 512;
    let dma_channel = peripherals.DMA_CH0;
    let i2s_config = I2sConfig::new_tdm_philips()
        .with_sample_rate(Rate::from_hz(SAMPLE_RATE))
        .with_data_format(DataFormat::Data16Channel16)
        .with_channels(Channels::MONO);

    let i2s = match I2s::new(peripherals.I2S0, dma_channel, i2s_config) {
        Ok(i2s) => i2s,
        Err(_e) => loop {
            core::hint::spin_loop()
        },
    };

    // --- Step 4: Attach GPIO pins (clock + data) and build RX ---
    let (mut rx_buffer, rx_descriptors, _, _) = dma_circular_buffers!(BUFFER_SIZE * 2, 0);

    let mut i2s_rx = i2s
        .i2s_rx
        .with_din(peripherals.GPIO41)
        .with_bclk(peripherals.GPIO42) // let I2S drive BCLK (PDM CLK)
        .build(rx_descriptors);

    // --- Program RX clock for PDM front-end (~1.024 MHz when Fs=16 kHz, ÷64) ---
    // ~1.024 MHz BCLK from CLK160: 160_000_000 / 156 ≈ 1.0256 MHz
    i2s_regs.rx_clkm_conf().modify(|_, w| unsafe {
        w.rx_clk_active()
            .set_bit()
            .rx_clk_sel()
            .bits(2) // 2 = CLK160 source
            .rx_clkm_div_num()
            .bits(156) // integer divider
    });

    // --- Configure RX for PDM → PCM ---
    // Clear TDM, enable PDM, enable SINC PDM->PCM, set decimation ÷64, mono RIGHT
    i2s_regs.rx_conf().modify(|_, w| {
        w.rx_tdm_en()
            .clear_bit()
            .rx_pdm_en()
            .set_bit()
            .rx_mono()
            .set_bit()
            .rx_pdm2pcm_en()
            .set_bit()
            .rx_pdm_sinc_dsr_16_en()
            .clear_bit()
    });

    // Pick which half (LEFT/RIGHT) feeds mono. RIGHT usually matches XIAO S3 Sense.
    // If you get silence, change `.set_bit()` to `.clear_bit()` below.
    i2s_regs.rx_conf().modify(|_, w| {
        w.rx_mono_fst_vld().set_bit() // 1: use right; 0: use left (depending on wiring)
    });

    // Latch config and start RX
    i2s_regs.rx_conf().modify(|_, w| w.rx_update().set_bit());

    // Start DMA AFTER clocks & mode are set
    let mut transfer = match i2s_rx.read_dma_circular(&mut rx_buffer) {
        Ok(t) => t,
        Err(_e) => loop {
            core::hint::spin_loop()
        },
    };
    i2s_regs.rx_conf().modify(|_, w| w.rx_start().set_bit());

    // LED on GPIO21 for heartbeat
    let mut led = Output::new(peripherals.GPIO21, Level::Low, OutputConfig::default());
    let mut frame_count: u32 = 0;

    // --- Step 5: Stream S16LE (mono @ 16kHz) over USB-Serial-JTAG ---
    loop {
        if let Ok(avail) = transfer.available() {
            if avail > 0 {
                let read_size = core::cmp::min(avail, BUFFER_SIZE * 2);
                let mut rcv = [0u8; BUFFER_SIZE * 2];
                if transfer.pop(&mut rcv[..read_size]).is_ok() {
                    // DEBUG: print first sample and simple stats until non-zero data observed.
                    // Once verified, revert to raw USB streaming above.
                    if read_size >= 2 {
                        let mut min_v: i16 = i16::MAX;
                        let mut max_v: i16 = i16::MIN;
                        let mut nonzero: usize = 0;
                        let samples = read_size / 2;
                        for i in (0..read_size).step_by(2) {
                            let s = i16::from_le_bytes([rcv[i], rcv[i + 1]]);
                            if s != 0 {
                                nonzero += 1;
                            }
                            if s < min_v {
                                min_v = s;
                            }
                            if s > max_v {
                                max_v = s;
                            }
                        }
                        let first = i16::from_le_bytes([rcv[0], rcv[1]]);
                        println!(
                            "DBG Mic: first={} min={} max={} nz={}/{}",
                            first, min_v, max_v, nonzero, samples
                        );
                    }
                    frame_count = frame_count.wrapping_add(1);
                    if frame_count % 100 == 0 {
                        led.toggle();
                    }
                }
            }
        }
    }
}
