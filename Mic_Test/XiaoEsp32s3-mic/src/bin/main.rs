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
use esp_hal::Blocking;
use esp_println::println;
use nb;

const SAMPLE_RATE: u32 = 16_000;
const SAMPLE_SIZE: usize = 2; // 16-bit samples = 2 bytes
const SAMPLES_PER_BUFFER: usize = 512; // 512 samples per buffer
const BUFFER_SIZE: usize = SAMPLES_PER_BUFFER * SAMPLE_SIZE; // 1024 bytes
const GAIN_SHIFT: u8 = 0;
const GAIN_BOOST: i16 = 40;
const WARMUP_FRAMES: u32 = (SAMPLE_RATE / SAMPLES_PER_BUFFER as u32) * 2 + 32; // ~2s + margin
const DIAG_INTERVAL: u32 = 500; // how often to print diagnostics
const DROPPED_WARN_THRESHOLD: u32 = 1;
const MAX_USB_WRITE_SPINS: u32 = 10_000;

#[esp_hal::main]
fn main() -> ! {
    let peripherals = init(esp_hal::Config::default());

    // ----- I²S setup (required for both test tone and microphone) -----
    // println!("Starting PDM microphone streaming...");

    // Enable I2S0 clock + reset
    let sys = unsafe { &*pac::SYSTEM::ptr() };
    sys.perip_clk_en0().modify(|_, w| w.i2s0_clk_en().set_bit());
    sys.perip_rst_en0().modify(|_, w| w.i2s0_rst().set_bit());
    sys.perip_rst_en0().modify(|_, w| w.i2s0_rst().clear_bit());

    let regs = unsafe { &*esp32s3::I2S0::ptr() };

    // USB
    let mut usb: UsbSerialJtag<'static, Blocking> = UsbSerialJtag::new(peripherals.USB_DEVICE);

    // I²S config - Use proper PDM configuration
    let dma_ch = peripherals.DMA_CH0;
    let i2s_cfg = I2sConfig::new_tdm_philips()
        .with_sample_rate(Rate::from_hz(SAMPLE_RATE))
        .with_channels(Channels::MONO);

    let i2s = I2s::new(peripherals.I2S0, dma_ch, i2s_cfg).ok().unwrap();

    // DMA buffers - Use larger buffers like IDF version
    let (mut rx_buf, rx_desc, tx_buf, tx_desc) =
        dma_circular_buffers!(BUFFER_SIZE, BUFFER_SIZE);
    tx_buf.fill(0);

    // TX BCLK on GPIO42 - PDM clock
    let mut i2s_tx = i2s.i2s_tx.with_bclk(peripherals.GPIO42).build(tx_desc);

    // PDM clock configuration: 16 kHz * 256 = 4.096 MHz (matching IDF)
    regs.tx_clkm_conf().modify(|_, w| unsafe {
        w.tx_clk_active().set_bit()
            .tx_clk_sel().bits(2)        // 160 MHz source
            .tx_clkm_div_num().bits(39)  // 160e6 / 39 ≈ 4.102 MHz (Fs×256 for 16 kHz)
    });
    let _tx_xfer = i2s_tx.write_dma_circular(&tx_buf).ok().unwrap();

    // RX DIN on GPIO41  
    let mut i2s_rx = i2s.i2s_rx.with_din(peripherals.GPIO41).build(rx_desc);
    regs.rx_clkm_conf().modify(|_, w| unsafe {
        w.rx_clk_active().set_bit()
            .rx_clk_sel().bits(2)        // 160 MHz
            .rx_clkm_div_num().bits(39)  // match TX: ≈4.102 MHz
    });

    // Configure PDM RX mode with proper settings
    regs.rx_conf().modify(|_, w| {
        w.rx_tdm_en().clear_bit()
            .rx_pdm_en().set_bit()
            .rx_mono().set_bit()
            .rx_mono_fst_vld().clear_bit() // use RIGHT slot
            .rx_pdm2pcm_en().set_bit()
            .rx_pdm_sinc_dsr_16_en().clear_bit()
    });

    // Enable only the RIGHT PDM slot and declare two total RX slots
    regs.rx_tdm_ctrl().modify(|_, w| unsafe {
        w.rx_tdm_pdm_chan0_en().clear_bit()
            .rx_tdm_pdm_chan1_en().set_bit()
            .rx_tdm_tot_chan_num().bits(1)
    });

    // Configure 16-bit samples
    regs.rx_conf1().modify(|_, w| unsafe {
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
    let mut errors = 0u32;
    let mut dropped_frames = 0u32;
    let mut last_reported_dropped = 0u32;

    let mut buffer = [0u8; BUFFER_SIZE];
    let mut warmup_frames = WARMUP_FRAMES;
    let mut dc_acc: i32 = 0;
    let mut diag_counter = 0u32;

    loop {
        let avail = match rx_xfer.available() {
            Ok(v) => v,
            Err(_) => {
                errors = errors.wrapping_add(1);
                continue;
            }
        };

        if avail == 0 {
            continue;
        }

        let chunk = core::cmp::min(avail, BUFFER_SIZE);
        let read_bytes = match rx_xfer.pop(&mut buffer[..chunk]) {
            Ok(n) => n,
            Err(_) => {
                errors = errors.wrapping_add(1);
                continue;
            }
        };

        if read_bytes == 0 {
            continue;
        }

        let mut idx = 0;
        let mut total_abs: i64 = 0;
        let mut even_abs: i64 = 0;
        let mut odd_abs: i64 = 0;
        let mut even_count: i64 = 0;
        let mut odd_count: i64 = 0;
        let mut zero_crossings: u32 = 0;
        let mut prev_sign: i8 = 0;
        let mut sample_idx: i64 = 0;

        while idx + 1 < read_bytes {
            let mut sample = i16::from_le_bytes([buffer[idx], buffer[idx + 1]]);
            if GAIN_SHIFT > 0 {
                sample >>= GAIN_SHIFT;
            }

            dc_acc += ((sample as i32) - dc_acc) / 256;
            sample -= dc_acc as i16;

            sample = sample.saturating_mul(GAIN_BOOST);

            let abs = sample.wrapping_abs() as i64;
            total_abs += abs;
            if sample_idx % 2 == 0 {
                even_abs += abs;
                even_count += 1;
            } else {
                odd_abs += abs;
                odd_count += 1;
            }

            let sign = if sample > 0 { 1 } else if sample < 0 { -1 } else { 0 };
            if sign != 0 && prev_sign != 0 && sign != prev_sign {
                zero_crossings = zero_crossings.saturating_add(1);
            }
            if sign != 0 {
                prev_sign = sign;
            }

            let bytes = sample.to_le_bytes();
            buffer[idx] = bytes[0];
            buffer[idx + 1] = bytes[1];
            idx += 2;
            sample_idx += 1;
        }

        diag_counter = diag_counter.wrapping_add(1);
        if diag_counter >= DIAG_INTERVAL {
            let sample_count = if sample_idx > 0 { sample_idx } else { 1 };
            let total_avg = total_abs / sample_count;
            let even_avg = if even_count > 0 { even_abs / even_count } else { 0 };
            let odd_avg = if odd_count > 0 { odd_abs / odd_count } else { 0 };
            let zero_ratio = zero_crossings as f32 / sample_count as f32;

            let classification = if total_avg < 50 {
                "silence"
            } else if even_avg < 50 && odd_avg > 200 {
                "channel-mismatch (chipmunk?)"
            } else if zero_ratio > 0.6 {
                "high zero-cross (noise?)"
            } else {
                "speech-like"
            };
            // println!(
            //     "diag avg:{} even:{} odd:{} zero_ratio:{:.2} => {}",
            //     total_avg, even_avg, odd_avg, zero_ratio, classification
            // );
            diag_counter = 0;
        }

        if warmup_frames > 0 {
            warmup_frames -= 1;
        } else {
            let mut sent_all = true;
            let mut out_idx = 0;
            while out_idx < read_bytes {
                let mut spins = 0;
                loop {
                    match usb.write_byte_nb(buffer[out_idx]) {
                        Ok(()) => {
                            out_idx += 1;
                            break;
                        }
                        Err(nb::Error::WouldBlock) => {
                            spins += 1;
                            core::hint::spin_loop();
                            if spins >= MAX_USB_WRITE_SPINS {
                                sent_all = false;
                                break;
                            }
                        }
                        Err(_) => {
                            sent_all = false;
                            errors = errors.wrapping_add(1);
                            break;
                        }
                    }
                }
                if !sent_all {
                    break;
                }
            }

            if !sent_all {
                dropped_frames = dropped_frames.wrapping_add(1);
            }
        }

        frames = frames.wrapping_add(1);
        if frames % 100 == 0 {
            led.toggle();
        }

        if frames % DIAG_INTERVAL == 0 {
            // println!(
            //     "frames:{} errors:{} dropped:{}",
            //     frames, errors, dropped_frames
            // );
            if dropped_frames > last_reported_dropped
                && dropped_frames - last_reported_dropped >= DROPPED_WARN_THRESHOLD
            {
                // println!(
                //     "warning: dropped {} frame(s) since last check",
                //     dropped_frames - last_reported_dropped
                // );
            }
            last_reported_dropped = dropped_frames;
        }
    }
}