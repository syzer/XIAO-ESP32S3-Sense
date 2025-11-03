#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use esp_println::{println, print};

// Inline microphone scaffold module (avoids separate bin confusion)
mod mic {
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

}
use mic::{MicPdm, FRAME_SAMPLES, SlotSelect};

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.0.0
    println!("🎤 Starting XIAO ESP32S3 Microphone Test");
    println!("📋 Initializing system configuration...");
    
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    println!("⚡ CPU Clock set to maximum frequency");
    
    let peripherals = esp_hal::init(config);
    println!("🔌 Hardware peripherals initialized");

    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 73744);
    println!("💾 Heap allocator configured (73744 bytes)");

    println!("⏰ Initializing timer group...");
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);
    println!("✅ Timer initialized and started");

    println!("📡 Initializing Wi-Fi/BLE controller...");
    let radio_init = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    println!("📶 Radio initialization complete");
    
    let (mut _wifi_controller, _interfaces) =
        esp_radio::wifi::new(&radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");
    println!("🌐 Wi-Fi controller initialized");

    println!("🚀 System initialization complete!");
    println!("� Setting up PDM microphone scaffold (Option B)...");
    let mic = MicPdm::new(SlotSelect::Right); // try Right first, same as Arduino
    mic.init().expect("Mic init stub failed");
    println!("✅ Mic scaffold ready (synthetic data until registers added)");

    // Working buffer for one frame
    let mut frame = [0i16; FRAME_SAMPLES];
    let mut frames = 0u32;
    let start = embassy_time::Instant::now();
    println!("🎬 Capturing synthetic frames. (Replace with real PDM soon)");
    loop {
        let bytes = mic.read_frame(&mut frame);
        frames += 1;

        // Print every Nth sample to keep output light (decimate)
        if frames % 20 == 0 { // roughly every 20 frames
            print!("frame {} ", frames);
            for i in (0..FRAME_SAMPLES).step_by(128) { // sparse samples
                print!("{} ", frame[i]);
            }
            println!("");
        }

        // Sleep a bit to simulate pacing as if waiting on DMA read
        Timer::after(Duration::from_millis(10)).await;

        // Exit after ~20s to satisfy quick test expectation (can remove later)
        if start.elapsed() > Duration::from_secs(20) {
            println!("⏱️ 20s capture window complete. Total frames: {}", frames);
            loop { /* halt */ }
        }
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0/examples/src/bin
}
