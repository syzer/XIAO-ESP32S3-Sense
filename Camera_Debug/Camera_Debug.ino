/*********
  Simple ESP32-S3 XIAO camera streamer
OPI PSRAM ENABLE
*********/

#include "esp_camera.h"
#include "esp_wifi.h"
#include <WiFi.h>
#include "esp_timer.h"
#include "img_converters.h"
#include "Arduino.h"
#include "fb_gfx.h"
#include "esp_http_server.h"

#include "wifi_credentials.h"   // defines WIFI_SSID and WIFI_PASSWORD

#define CAMERA_MODEL_XIAO_ESP32S3
#include "camera_pins.h"

#define PART_BOUNDARY "123456789000000000000987654321"

static const char* _STREAM_CONTENT_TYPE =
  "multipart/x-mixed-replace;boundary=" PART_BOUNDARY;
static const char* _STREAM_BOUNDARY = "\r\n--" PART_BOUNDARY "\r\n";
static const char* _STREAM_PART =
  "Content-Type: image/jpeg\r\nContent-Length: %u\r\n\r\n";

httpd_handle_t stream_httpd = NULL;

// ------------- MJPEG stream handler -------------

static esp_err_t stream_handler(httpd_req_t *req) {
  camera_fb_t *fb = NULL;
  esp_err_t res = ESP_OK;
  size_t _jpg_buf_len = 0;
  uint8_t *_jpg_buf = NULL;
  char part_buf[64];

  res = httpd_resp_set_type(req, _STREAM_CONTENT_TYPE);
  if (res != ESP_OK) {
    return res;
  }

  while (true) {
    fb = esp_camera_fb_get();
    if (!fb) {
      Serial.println("Camera capture failed");
      res = ESP_FAIL;
    } else {
      if (fb->format != PIXFORMAT_JPEG) {
        bool jpeg_converted = frame2jpg(fb, 80, &_jpg_buf, &_jpg_buf_len);
        esp_camera_fb_return(fb);
        fb = NULL;
        if (!jpeg_converted) {
          Serial.println("JPEG compression failed");
          res = ESP_FAIL;
        }
      } else {
        _jpg_buf_len = fb->len;
        _jpg_buf = fb->buf;
      }
    }

    if (res == ESP_OK) {
      size_t hlen = snprintf(part_buf, sizeof(part_buf), _STREAM_PART, _jpg_buf_len);
      res = httpd_resp_send_chunk(req, (const char *)part_buf, hlen);
    }
    if (res == ESP_OK) {
      res = httpd_resp_send_chunk(req, (const char *)_jpg_buf, _jpg_buf_len);
    }
    if (res == ESP_OK) {
      res = httpd_resp_send_chunk(req, _STREAM_BOUNDARY, strlen(_STREAM_BOUNDARY));
    }

    if (fb) {
      esp_camera_fb_return(fb);
      fb = NULL;
      _jpg_buf = NULL;
    } else if (_jpg_buf) {
      free(_jpg_buf);
      _jpg_buf = NULL;
    }

    if (res != ESP_OK) {
      break;
    }
  }

  return res;
}

// ------------- Start HTTP server -------------

void startCameraServer() {
  httpd_config_t config = HTTPD_DEFAULT_CONFIG();
  config.server_port = 80;

  httpd_uri_t index_uri = {
    .uri       = "/",
    .method    = HTTP_GET,
    .handler   = stream_handler,
    .user_ctx  = NULL
  };

  if (httpd_start(&stream_httpd, &config) == ESP_OK) {
    httpd_register_uri_handler(stream_httpd, &index_uri);
    Serial.println("HTTP camera server started");
  } else {
    Serial.println("Failed to start HTTP server");
  }
}

// ------------- Setup -------------

void setup() {
  Serial.begin(115200);
  while (!Serial) {}

  Serial.println("\nBooting...");

  // -------- WiFi FIRST (same as your working test) --------
  Serial.print("Connecting to WiFi: ");
  Serial.println(WIFI_SSID);

  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);

  // lower TX power to reduce current spikes
  esp_wifi_set_max_tx_power(20);   // 20 * 0.25 dBm ≈ 5 dBm

  uint8_t retries = 20;
  while (WiFi.status() != WL_CONNECTED && retries--) {
    delay(500);
    Serial.print(".");
  }
  Serial.println();

  if (WiFi.status() != WL_CONNECTED) {
    Serial.println("WiFi connect FAILED");
    while (true) { delay(1000); }
  }

  Serial.println("WiFi connected");
  Serial.print("IP: http://");
  Serial.println(WiFi.localIP());

  delay(1000);

  // -------- Camera init (identical to your working camera test) --------
  Serial.println("Init camera...");

  camera_config_t config = {0};
  config.ledc_channel = LEDC_CHANNEL_0;
  config.ledc_timer   = LEDC_TIMER_0;
  config.pin_d0       = Y2_GPIO_NUM;
  config.pin_d1       = Y3_GPIO_NUM;
  config.pin_d2       = Y4_GPIO_NUM;
  config.pin_d3       = Y5_GPIO_NUM;
  config.pin_d4       = Y6_GPIO_NUM;
  config.pin_d5       = Y7_GPIO_NUM;
  config.pin_d6       = Y8_GPIO_NUM;
  config.pin_d7       = Y9_GPIO_NUM;
  config.pin_xclk     = XCLK_GPIO_NUM;
  config.pin_pclk     = PCLK_GPIO_NUM;
  config.pin_vsync    = VSYNC_GPIO_NUM;
  config.pin_href     = HREF_GPIO_NUM;
  config.pin_sscb_sda = SIOD_GPIO_NUM;
  config.pin_sscb_scl = SIOC_GPIO_NUM;
  config.pin_pwdn     = PWDN_GPIO_NUM;
  config.pin_reset    = RESET_GPIO_NUM;
  // config.xclk_freq_hz = 20000000;
  config.xclk_freq_hz = 24000000;
  config.frame_size   = FRAMESIZE_VGA;
  config.pixel_format = PIXFORMAT_JPEG;
  config.grab_mode    = CAMERA_GRAB_WHEN_EMPTY;
  config.fb_location  = CAMERA_FB_IN_PSRAM;
  config.jpeg_quality = 20;
  config.fb_count     = 2;
  config.grab_mode    = CAMERA_GRAB_LATEST;    // drop old frames

  esp_err_t err = esp_camera_init(&config);
  if (err != ESP_OK) {
    Serial.printf("Camera init failed 0x%x\n", err);
    while (true) { delay(1000); }
  }

  sensor_t *s = esp_camera_sensor_get();
  if (s) { // not required but make it brighter 
    s->set_vflip(s, 1);   // vertical flip  (top ↔ bottom)

    // Basic brightness: range -2 .. 2
    s->set_brightness(s, 2);      // 2 = brighter

    // A bit more saturation/contrast if you want
    // s->set_contrast(s, 1);        // -2..2 // can make it apear green
    // s->set_saturation(s, 1);      // -2..2

    // Make sure auto exposure / auto gain are on
    // s->set_exposure_ctrl(s, 1);   // 1 = auto
    // s->set_gain_ctrl(s, 1);       // 1 = auto

    // Allow higher analog gain if needed
    // s->set_gainceiling(s, (gainceiling_t)2); // GAINCEILING_4X  // can make it apear green
}

  Serial.println("Camera OK");

  Serial.println("Starting HTTP stream server...");
  startCameraServer();

  Serial.print("Open this in a browser: http://");
  Serial.println(WiFi.localIP());
}

// ------------- Loop -------------

void loop() {
  delay(10);
}