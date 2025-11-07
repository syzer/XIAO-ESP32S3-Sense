#![no_std]
#![no_main]

esp_bootloader_esp_idf::esp_app_desc!();

use esp_backtrace as _;
use esp_hal::{
    init,
    gpio::{Output, Level, OutputConfig},
};
use esp32s3 as pac;
use embedded_hal::delay::DelayNs;

#[esp_hal::main]
fn main() -> ! {
    // Initialize base HAL peripherals
    let peripherals = init(esp_hal::Config::default());

    // --- Enable I2S0 peripheral clock and clear reset ---
    let sys = unsafe { &*pac::SYSTEM::ptr() };
    sys.perip_clk_en0().modify(|_, w| w.i2s0_clk_en().set_bit());
    sys.perip_rst_en0().modify(|_, w| w.i2s0_rst().clear_bit());

    // --- Get direct register block for I2S0 ---
    let i2s = unsafe { &*pac::I2S0::ptr() };

    // TODO: Fix I2S register access - these methods don't exist in the PAC
    // Need to check actual register structure
    // --- Reset internal I2S state ---
    // i2s.conf().modify(|_, w| w.rx_reset().set_bit().tx_reset().set_bit());
    // i2s.conf().modify(|_, w| w.rx_reset().clear_bit().tx_reset().clear_bit());

    // --- Configure RX clocking ---
    // 16 kHz × 64 = ~1.024 MHz PDM clock target
    // i2s.clkm_conf().modify(|_, w| {
    //     w.clka_en().set_bit();
    //     w.clkm_div_a().bits(0);
    //     w.clkm_div_b().bits(0);
    //     w.clkm_div_num().bits(1)
    // });

    // --- Configure RX for PDM → PCM ---
    // Based on reference: clear TDM, set PDM
    i2s.rx_conf().modify(|_, w| {
        w.rx_tdm_en().clear_bit()
         .rx_pdm_en().set_bit()   // enable PDM front-end
         .rx_mono().set_bit()     // mono mode
    });

    // TODO: Check correct field name for PDM2PCM enable
    // i2s.rx_conf1().modify(|_, w| {
    //     w.rx_pdm2pcm_en().set_bit() // enable sinc filter
    // });

    // TODO: Fix these register accesses
    // --- Set PDM decimation (downsample ratio = 64) ---
    // NOTE: Field name might differ by PAC version — check docs if this fails
    // if let Some(reg) = i2s.rx_pdm_conf.as_ref() {
    //     reg.modify(|_, w| unsafe {
    //         w.rx_sinc_osr2().bits(64); // try 64x downsample
    //         w
    //     });
    // }

    // --- Select RIGHT channel (like Arduino example) ---
    // i2s.rx_tdm_ctrl().modify(|_, w| unsafe {
    //     w.rx_total_chan_num().bits(1);
    //     w.rx_chan_bits().bits(16);
    //     w
    // });

    // --- Enable RX ---
    // i2s.conf().modify(|_, w| w.rx_start().set_bit());

    // --- Simple LED blink loop to show life ---
    let mut led = Output::new(peripherals.GPIO21, Level::Low, OutputConfig::default());

    loop {
        led.toggle();
        esp_hal::delay::Delay::new().delay_ms(500u32);
    }
}