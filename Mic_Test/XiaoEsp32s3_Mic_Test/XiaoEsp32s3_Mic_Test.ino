#include <Arduino.h>
#include "driver/i2s_pdm.h"
#include "driver/gpio.h"

static constexpr gpio_num_t MIC_CLK = GPIO_NUM_42;
static constexpr gpio_num_t MIC_DIN = GPIO_NUM_41;
static const int SAMPLE_RATE = 16000;

i2s_chan_handle_t rx_chan;

void setup() {
  Serial.begin(921600);   // match Python

  i2s_chan_config_t chan_cfg = {
    .id = I2S_NUM_0, .role = I2S_ROLE_MASTER,
    .dma_desc_num = 8, .dma_frame_num = 256,
    .auto_clear = true, .intr_priority = 0,
  };
  ESP_ERROR_CHECK(i2s_new_channel(&chan_cfg, NULL, &rx_chan));

  i2s_pdm_rx_config_t pdm_cfg = {
    .clk_cfg  = I2S_PDM_RX_CLK_DEFAULT_CONFIG(SAMPLE_RATE),
    .slot_cfg = I2S_PDM_RX_SLOT_DEFAULT_CONFIG(I2S_DATA_BIT_WIDTH_16BIT, I2S_SLOT_MODE_MONO),
    .gpio_cfg = { .clk = MIC_CLK, .din = MIC_DIN }
  };
  // try RIGHT first; if silence, change to LEFT and reflash
  pdm_cfg.slot_cfg.slot_mask = I2S_PDM_SLOT_RIGHT;

  ESP_ERROR_CHECK(i2s_channel_init_pdm_rx_mode(rx_chan, &pdm_cfg));
  ESP_ERROR_CHECK(i2s_channel_enable(rx_chan));
}

void loop() {
  static int16_t buf[1024];
  size_t nbytes = 0;
  if (i2s_channel_read(rx_chan, buf, sizeof(buf), &nbytes, portMAX_DELAY) == ESP_OK && nbytes) {
    Serial.write(reinterpret_cast<uint8_t*>(buf), nbytes); // raw s16le @16k
     // --- Uncomment below to visualize in Serial Plotter ---
    // for (size_t i = 0; i < nbytes / 2; i += 32) {
    //   Serial.println(buf[i]);
    // }
  }
}