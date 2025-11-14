/*
 * WAV Recorder for Seeed XIAO ESP32S3 Sense (Espressif core 3.3.2)
 * Uses low-level I2S PDM driver instead of Seeed's I2S wrapper.
 */

#include <Arduino.h>
#include "driver/i2s_pdm.h"
#include "driver/gpio.h"

#include "FS.h"
#include "SD.h"
#include "SPI.h"

// ----- CONFIG -----
#define RECORD_TIME   20        // seconds (max ~240 unless you increase PSRAM)
#define WAV_FILE_NAME "arduino_rec"

#define SAMPLE_RATE   16000U
#define SAMPLE_BITS   16
#define WAV_HEADER_SIZE 44
#define VOLUME_GAIN   2         // left-shift bits (2 = x4)

// XIAO ESP32S3 Sense internal PDM mic pins
static constexpr gpio_num_t MIC_CLK = GPIO_NUM_42;
static constexpr gpio_num_t MIC_DIN = GPIO_NUM_41;

i2s_chan_handle_t rx_chan;

// ----- WAV HEADER -----
void generate_wav_header(uint8_t *wav_header, uint32_t wav_size, uint32_t sample_rate)
{
  // See: http://soundfile.sapp.org/doc/WaveFormat/
  uint32_t file_size = wav_size + WAV_HEADER_SIZE - 8;
  uint32_t byte_rate = sample_rate * SAMPLE_BITS / 8;
  const uint8_t set_wav_header[] = {
    'R','I','F','F',
    (uint8_t)(file_size      ),
    (uint8_t)(file_size >>  8),
    (uint8_t)(file_size >> 16),
    (uint8_t)(file_size >> 24),
    'W','A','V','E',
    'f','m','t',' ',
    0x10,0x00,0x00,0x00,      // Subchunk1Size = 16
    0x01,0x00,                // PCM
    0x01,0x00,                // mono
    (uint8_t)(sample_rate      ),
    (uint8_t)(sample_rate >>  8),
    (uint8_t)(sample_rate >> 16),
    (uint8_t)(sample_rate >> 24),
    (uint8_t)(byte_rate      ),
    (uint8_t)(byte_rate >>  8),
    (uint8_t)(byte_rate >> 16),
    (uint8_t)(byte_rate >> 24),
    0x02,0x00,                // BlockAlign = 2
    0x10,0x00,                // BitsPerSample = 16
    'd','a','t','a',
    (uint8_t)(wav_size      ),
    (uint8_t)(wav_size >>  8),
    (uint8_t)(wav_size >> 16),
    (uint8_t)(wav_size >> 24),
  };
  memcpy(wav_header, set_wav_header, sizeof(set_wav_header));
}

// ----- RECORDING -----
void record_wav()
{
  uint32_t bytes_per_second = SAMPLE_RATE * SAMPLE_BITS / 8;
  uint32_t record_size      = bytes_per_second * RECORD_TIME;

  Serial.printf("Ready to start recording %u bytes (~%u s)...\n",
                record_size, RECORD_TIME);

  File file = SD.open("/" WAV_FILE_NAME ".wav", FILE_WRITE);
  if (!file) {
    Serial.println("Failed to open WAV file on SD!");
    while (1) {}
  }

  // Write WAV header placeholder
  uint8_t wav_header[WAV_HEADER_SIZE];
  generate_wav_header(wav_header, record_size, SAMPLE_RATE);
  file.write(wav_header, WAV_HEADER_SIZE);

  // PSRAM buffer
  uint8_t *rec_buffer = (uint8_t *)ps_malloc(record_size);
  if (!rec_buffer) {
    Serial.printf("ps_malloc(%u) failed!\n", record_size);
    while (1) {}
  }
  Serial.printf("PSRAM used: %d bytes\n", ESP.getPsramSize() - ESP.getFreePsram());

  size_t sample_size = 0;
  esp_err_t err = i2s_channel_read(rx_chan, rec_buffer, record_size, &sample_size, portMAX_DELAY);
  if (err != ESP_OK || sample_size == 0) {
    Serial.printf("Record failed: err=%d, size=%u\n", err, (unsigned)sample_size);
  } else {
    Serial.printf("Recorded %u bytes\n", (unsigned)sample_size);
  }

  // Simple volume boost (no saturation handling, same as original)
  for (uint32_t i = 0; i + 1 < sample_size; i += 2) {
    int16_t *s = (int16_t *)(rec_buffer + i);
    *s <<= VOLUME_GAIN;
  }

  Serial.printf("Writing to file...\n");
  size_t written = file.write(rec_buffer, sample_size);
  if (written != sample_size) {
    Serial.printf("Write mismatch: wrote %u / %u\n",
                  (unsigned)written, (unsigned)sample_size);
  }

  free(rec_buffer);
  file.close();
  Serial.printf("Recording done.\n");
}

// ----- SETUP / LOOP -----
void setup() {
  Serial.begin(115200);
  while (!Serial) {}

  Serial.println("Init I2S PDM...");

  // I2S channel config
  i2s_chan_config_t chan_cfg = {
    .id           = I2S_NUM_0,
    .role         = I2S_ROLE_MASTER,
    .dma_desc_num = 8,
    .dma_frame_num= 256,
    .auto_clear   = true,
    .intr_priority= 0,
  };
  ESP_ERROR_CHECK(i2s_new_channel(&chan_cfg, NULL, &rx_chan));

  // PDM RX config
  i2s_pdm_rx_config_t pdm_cfg = {
    .clk_cfg  = I2S_PDM_RX_CLK_DEFAULT_CONFIG(SAMPLE_RATE),
    .slot_cfg = I2S_PDM_RX_SLOT_DEFAULT_CONFIG(
                  I2S_DATA_BIT_WIDTH_16BIT,
                  I2S_SLOT_MODE_MONO),
    .gpio_cfg = { .clk = MIC_CLK, .din = MIC_DIN }
  };

  // XIAO ESP32S3 internal mic lives on LEFT
  pdm_cfg.slot_cfg.slot_mask = I2S_PDM_SLOT_LEFT;

  ESP_ERROR_CHECK(i2s_channel_init_pdm_rx_mode(rx_chan, &pdm_cfg));
  ESP_ERROR_CHECK(i2s_channel_enable(rx_chan));

  Serial.println("Init SD card...");
  if (!SD.begin(21)) {
    Serial.println("Failed to mount SD card!");
    while (1) {}
  }

  record_wav();
}

void loop() {
  delay(1000);
  Serial.print(".");
}