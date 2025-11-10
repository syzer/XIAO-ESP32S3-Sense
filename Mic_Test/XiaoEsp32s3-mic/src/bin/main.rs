#![no_std]
#![no_main]

esp_bootloader_esp_idf::esp_app_desc!();

use esp_backtrace as _;
use esp_hal::{
    dma_circular_buffers,
    gpio::{Level, Output, OutputConfig},
    i2s::master::{Channels, Config as I2sConfig, I2s},
    init,
    time::Rate,
    usb_serial_jtag::UsbSerialJtag,
};
use esp32s3 as pac;
use esp_println::{println, print};
use esp_hal::Blocking;
use libm::sqrtf;

const SAMPLE_RATE: u32 = 16_000;
const SAMPLE_SIZE: usize = 2; // 16-bit samples = 2 bytes
const SAMPLES_PER_BUFFER: usize = 256; // 256 samples per buffer
const BUFFER_SIZE: usize = SAMPLES_PER_BUFFER * SAMPLE_SIZE; // 512 bytes
const TEST_TONE_FREQ: u32 = 1000;

// flip this to true to validate end-to-end without waiting for I²S/DMA
const TEST_TONE: bool = false;
const DEBUG_PRINT: bool = false; // set true to print samples instead of streaming audio

#[esp_hal::main]
fn main() -> ! {
    let peripherals = init(esp_hal::Config::default());

    // ----- I²S setup (required for both test tone and microphone) -----
    println!("Starting PDM microphone streaming...");

    // Enable I2S0 clock + reset
    let sys = unsafe { &*pac::SYSTEM::ptr() };
    sys.perip_clk_en0().modify(|_, w| w.i2s0_clk_en().set_bit());
    sys.perip_rst_en0().modify(|_, w| w.i2s0_rst().set_bit());
    sys.perip_rst_en0().modify(|_, w| w.i2s0_rst().clear_bit());

    let regs = unsafe { &*esp32s3::I2S0::ptr() };

    // USB
    let mut usb: UsbSerialJtag<'static, Blocking> = UsbSerialJtag::new(peripherals.USB_DEVICE);

    // I²S config - FIXED: Use proper data format for mono
    let dma_ch = peripherals.DMA_CH0;
    let i2s_cfg = I2sConfig::new_tdm_philips()
        .with_sample_rate(Rate::from_hz(SAMPLE_RATE))
        // .with_data_format(DataFormat::Mono)  // Try S16 format for 16-bit
        .with_channels(Channels::MONO);         // Correct: Mono channel

    let i2s = I2s::new(peripherals.I2S0, dma_ch, i2s_cfg).ok().unwrap();

    // DMA buffers - FIXED: Consistent buffer sizes
    let (mut rx_buf, rx_desc, tx_buf, tx_desc) =
        dma_circular_buffers!(BUFFER_SIZE, BUFFER_SIZE);  // FIXED: Use BUFFER_SIZE consistently
    tx_buf.fill(0);

    // TX BCLK on GPIO42
    let mut i2s_tx = i2s.i2s_tx.with_bclk(peripherals.GPIO42).build(tx_desc);

    // PDM typically needs higher bit clock. For 16kHz with 64x oversampling:
    // Bit clock = 16kHz * 64 * 1 bit/sample = 1.024 MHz
    regs.tx_clkm_conf().modify(|_, w| unsafe {
        w.tx_clk_active().set_bit()
            .tx_clk_sel().bits(2)        // 160 MHz source
            // .tx_clkm_div_num().bits(156) // 160e6 / 156 ≈ 1.0256 MHz
            .tx_clkm_div_num().bits(64)

    });
    let _tx_xfer = i2s_tx.write_dma_circular(&tx_buf).ok().unwrap();

    // RX DIN on GPIO41  
    let mut i2s_rx = i2s.i2s_rx.with_din(peripherals.GPIO41).build(rx_desc);
    regs.rx_clkm_conf().modify(|_, w| unsafe {
        w.rx_clk_active().set_bit()
            .rx_clk_sel().bits(2)        // 160 MHz
            // .rx_clkm_div_num().bits(156)
            .rx_clkm_div_num().bits(64)
    });

    // FIXED: Correct PDM to PCM configuration
    regs.rx_conf().modify(|_, w| {
        w.rx_tdm_en().clear_bit()
            .rx_pdm_en().set_bit()
            .rx_mono().set_bit()
            .rx_pdm2pcm_en().set_bit()
            .rx_pdm_sinc_dsr_16_en().clear_bit()
    });
    // regs.rx_conf().modify(|_, w| w.rx_mono_fst_vld().clear_bit()); // LEFT channel
    regs.rx_conf().modify(|_, w| w.rx_mono_fst_vld().clear_bit()); // LEFT channel (XIAO S3 Sense mic)
    // Pack RX FIFO as 16-bit samples
    // regs.fifo_conf().modify(|_, w| unsafe { w.rx_fifo_mod().bits(1) });


    // 16-bit mono words in FIFO/TDM
    regs.rx_conf1().modify(|_, w| unsafe {
        // 16-bit mono words in FIFO/TDM
        w.rx_bits_mod().bits(16 - 1)
         .rx_tdm_chan_bits().bits(16 - 1)
         .rx_msb_shift().set_bit()
    });
    regs.rx_conf().modify(|_, w| w.rx_update().set_bit());

    let mut rx_xfer = i2s_rx.read_dma_circular(&mut rx_buf).ok().unwrap();
    regs.rx_conf().modify(|_, w| w.rx_start().set_bit());

    // LED heartbeat (GPIO21)
    let mut led = Output::new(peripherals.GPIO21, Level::Low, OutputConfig::default());
    let mut frames = 0u32;
    let mut _errors = 0u32;

    // Prime host with zeros to reduce latency
    let prime = [0u8; BUFFER_SIZE];
    for _ in 0..32 {
        for ch in prime.chunks(64) {
            usb.write(ch).ok();
        }
    }

    // Shared state for either mode
    let tone_step: u32 = (TEST_TONE_FREQ * 65536) / SAMPLE_RATE;
    let mut tone_phase: u32 = 32768;
    let tone_enabled = TEST_TONE;
    let mut skip_frames = if tone_enabled { 0 } else { 32 };
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut sample_words = [0i16; SAMPLES_PER_BUFFER];
    let mut _usb_bytes_total: usize = 0;
    let mut _usb_bytes_window: usize = 0;
    let mut _usb_samples_window: usize = 0;
    let mut dc_acc: i32 = 0;

    loop {
        let mut sum: i64 = 0;
        let mut num_samples: usize = 0;
        let mut take: usize = 0;
        let mut have_frame = false;

        if tone_enabled {
            num_samples = SAMPLES_PER_BUFFER;
            take = BUFFER_SIZE;
            for i in 0..num_samples {
                tone_phase = tone_phase.wrapping_add(tone_step);
                let sample = (((tone_phase >> 8) as u8) as i16 - 128) * 256;
                buffer[i * 2..i * 2 + 2].copy_from_slice(&sample.to_le_bytes());
                sample_words[i] = sample;
                sum += sample as i64;
            }
            have_frame = true;
        } else if let Ok(avail) = rx_xfer.available() {
            if avail == 0 {
                continue;
            }
            take = core::cmp::min(avail, BUFFER_SIZE);
            if take == 0 {
                continue;
            }
            match rx_xfer.pop(&mut buffer[..take]) {
                Ok(_) => {
                    num_samples = take / 2;
                    for i in 0..num_samples {
                        let raw = u16::from_le_bytes([buffer[i * 2], buffer[i * 2 + 1]]) & 0x0fff;
                        let mut sample = ((raw as i32) - 2048) as i16;
                        sample <<= 4;
                        dc_acc += ((sample as i32) - dc_acc) / 256;
                        sample -= dc_acc as i16;
                        buffer[i * 2..i * 2 + 2].copy_from_slice(&sample.to_le_bytes());
                        sample_words[i] = sample;
                        sum += sample as i64;
                    }
                    have_frame = true;
                }
                Err(_e) => {
                    _errors += 1;
                    led.toggle();
                }
            }
        }

        if !have_frame {
            continue;
        }

        let avg = if num_samples > 0 {
            sum / num_samples as i64
        } else {
            0
        };

        let mut sum_sq_diff: i64 = 0;
        for i in 0..num_samples {
            let diff = sample_words[i] as i64 - avg;
            sum_sq_diff += diff * diff;
        }
        let variance = if num_samples > 0 {
            sum_sq_diff / num_samples as i64
        } else {
            0
        };
        let std_dev = sqrtf(variance as f32) as i32;
        let first_sample = if num_samples > 0 { sample_words[0] } else { 0 };

        let mut min_sample = i16::MAX;
        let mut max_sample = i16::MIN;
        for &s in &sample_words[..num_samples] {
            min_sample = min_sample.min(s);
            max_sample = max_sample.max(s);
        }

        if !tone_enabled {
            if skip_frames > 0 {
                skip_frames -= 1;
                continue;
            }
        }

        if DEBUG_PRINT {
            let n = core::cmp::min(32, num_samples);
            for i in 0..n {
                let s = i16::from_le_bytes([buffer[i * 2], buffer[i * 2 + 1]]);
                print!("{}{}", s, if i + 1 == n { "\n" } else { ", " });
            }
            println!(
                "Frame {}: first={}, avg={}, std={}, min={}, max={}, samples={}",
                frames, first_sample, avg, std_dev, min_sample, max_sample, num_samples
            );
            println!(
                "Raw bytes: {:02x?}",
                &buffer[..core::cmp::min(32, take.max(num_samples * 2))]
            );
        } else {
            for ch in buffer[..take.max(num_samples * 2)].chunks(64) {
                usb.write(ch).ok();
            }
        }

        frames = frames.wrapping_add(1);

        if frames % 100 == 0 {
            led.toggle();
        }
    }
}