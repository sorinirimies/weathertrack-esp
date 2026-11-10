// Waveshare ESP32-P4-WIFI6-Touch-LCD-7B display + touch bring-up.
// Pins and timings mirror Waveshare's BSP (waveshare/esp32_p4_wifi6_touch_lcd_7b 3.0.1).

#include "wt_display.h"

#include <string.h>

#include "freertos/FreeRTOS.h"

// Legacy I2C driver on purpose: esp-idf-hal links it, and the legacy and the
// "driver_ng" I2C drivers abort at boot if both end up in the same image.
#include "driver/i2c.h"
#include "driver/ledc.h"
#include "esp_cache.h"
#include "esp_check.h"
#include "esp_ldo_regulator.h"
#include "esp_lcd_ek79007.h"
#include "esp_lcd_mipi_dsi.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_ops.h"
#include "esp_lcd_touch.h"
#include "esp_lcd_touch_gt911.h"
#include "esp_log.h"

static const char *TAG = "wt_display";

#define PIN_I2C_SCL        8
#define PIN_I2C_SDA        7
#define PIN_LCD_RST        33
#define PIN_BACKLIGHT      32
#define DSI_LANES          2
#define DSI_LANE_MBPS      1000
#define DSI_PHY_LDO_CHAN   3
#define DSI_PHY_LDO_MV     2500

static esp_lcd_panel_handle_t s_panel;
static esp_lcd_touch_handle_t s_touch;
static uint16_t *s_fb;
static bool s_inited;

static esp_err_t backlight_init(void)
{
    const ledc_timer_config_t timer = {
        .speed_mode = LEDC_LOW_SPEED_MODE,
        .duty_resolution = LEDC_TIMER_10_BIT,
        .timer_num = LEDC_TIMER_1,
        .freq_hz = 5000,
        .clk_cfg = LEDC_AUTO_CLK,
    };
    ESP_RETURN_ON_ERROR(ledc_timer_config(&timer), TAG, "ledc timer");
    const ledc_channel_config_t ch = {
        .gpio_num = PIN_BACKLIGHT,
        .speed_mode = LEDC_LOW_SPEED_MODE,
        .channel = LEDC_CHANNEL_1,
        .intr_type = LEDC_INTR_DISABLE,
        .timer_sel = LEDC_TIMER_1,
        .duty = 0,
        .hpoint = 0,
        .flags = {.output_invert = 1},
    };
    return ledc_channel_config(&ch);
}

void wt_display_backlight(int percent)
{
    if (percent < 0) percent = 0;
    if (percent > 100) percent = 100;
    ledc_set_duty(LEDC_LOW_SPEED_MODE, LEDC_CHANNEL_1, (1023 * percent) / 100);
    ledc_update_duty(LEDC_LOW_SPEED_MODE, LEDC_CHANNEL_1);
}

static esp_err_t touch_init(void)
{
    const i2c_config_t i2c_cfg = {
        .mode = I2C_MODE_MASTER,
        .sda_io_num = PIN_I2C_SDA,
        .scl_io_num = PIN_I2C_SCL,
        .sda_pullup_en = GPIO_PULLUP_ENABLE,
        .scl_pullup_en = GPIO_PULLUP_ENABLE,
        .master.clk_speed = 400000,
    };
    ESP_RETURN_ON_ERROR(i2c_param_config(I2C_NUM_1, &i2c_cfg), TAG, "i2c cfg");
    ESP_RETURN_ON_ERROR(i2c_driver_install(I2C_NUM_1, I2C_MODE_MASTER, 0, 0, 0), TAG, "i2c install");

    esp_lcd_panel_io_i2c_config_t io_cfg = ESP_LCD_TOUCH_IO_I2C_GT911_CONFIG();
    // Log every device on the bus (helps when a board revision moves things).
    for (uint8_t addr = 0x08; addr < 0x78; addr++) {
        uint8_t dummy;
        if (i2c_master_read_from_device(I2C_NUM_1, addr, &dummy, 1, pdMS_TO_TICKS(20)) == ESP_OK) {
            ESP_LOGI(TAG, "i2c device at 0x%02x", addr);
        }
    }

    // GT911 answers at 0x5D or 0x14 depending on INT/RESET levels at power-up.
    // Probe by reading its product-ID register (0x8140).
    const uint8_t reg[2] = {0x81, 0x40};
    uint8_t id[4] = {0};
    uint8_t found = 0;
    const uint8_t candidates[2] = {ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS, ESP_LCD_TOUCH_IO_I2C_GT911_ADDRESS_BACKUP};
    for (int i = 0; i < 2 && !found; i++) {
        if (i2c_master_write_read_device(I2C_NUM_1, candidates[i], reg, 2, id, 4, pdMS_TO_TICKS(100)) == ESP_OK) {
            found = candidates[i];
            ESP_LOGI(TAG, "GT911 at 0x%02x, product id '%c%c%c%c'", found, id[0], id[1], id[2], id[3]);
        }
    }
    if (!found) {
        ESP_LOGE(TAG, "touch controller not found");
        return ESP_ERR_NOT_FOUND;
    }
    io_cfg.dev_addr = found;
    io_cfg.scl_speed_hz = 0; // legacy I2C driver: speed is set on the bus, not per device
    esp_lcd_panel_io_handle_t io = NULL;
    ESP_RETURN_ON_ERROR(esp_lcd_new_panel_io_i2c_v1(I2C_NUM_1, &io_cfg, &io), TAG, "touch io");

    const esp_lcd_touch_config_t tp_cfg = {
        .x_max = WT_LCD_W,
        .y_max = WT_LCD_H,
        .rst_gpio_num = GPIO_NUM_NC,
        .int_gpio_num = GPIO_NUM_NC,
        .levels = {.reset = 0, .interrupt = 0},
        // Touch layer is mounted rotated 180° relative to the panel (as in the BSP).
        .flags = {.swap_xy = 0, .mirror_x = 1, .mirror_y = 1},
    };
    return esp_lcd_touch_new_i2c_gt911(io, &tp_cfg, &s_touch);
}

esp_err_t wt_display_init(void)
{
    if (s_inited) return ESP_OK;

    ESP_RETURN_ON_ERROR(backlight_init(), TAG, "backlight");

    // DSI PHY supply (LDO_VO3 -> VDD_MIPI_DPHY).
    static esp_ldo_channel_handle_t phy_ldo;
    const esp_ldo_channel_config_t ldo_cfg = {
        .chan_id = DSI_PHY_LDO_CHAN,
        .voltage_mv = DSI_PHY_LDO_MV,
    };
    ESP_RETURN_ON_ERROR(esp_ldo_acquire_channel(&ldo_cfg, &phy_ldo), TAG, "dphy ldo");

    esp_lcd_dsi_bus_handle_t dsi_bus = NULL;
    const esp_lcd_dsi_bus_config_t bus_cfg = {
        .bus_id = 0,
        .num_data_lanes = DSI_LANES,
        .phy_clk_src = 0,
        .lane_bit_rate_mbps = DSI_LANE_MBPS,
    };
    ESP_RETURN_ON_ERROR(esp_lcd_new_dsi_bus(&bus_cfg, &dsi_bus), TAG, "dsi bus");

    esp_lcd_panel_io_handle_t io = NULL;
    const esp_lcd_dbi_io_config_t dbi_cfg = {
        .virtual_channel = 0,
        .lcd_cmd_bits = 8,
        .lcd_param_bits = 8,
    };
    ESP_RETURN_ON_ERROR(esp_lcd_new_panel_io_dbi(dsi_bus, &dbi_cfg, &io), TAG, "dbi io");

    esp_lcd_dpi_panel_config_t dpi_cfg = EK79007_1024_600_PANEL_60HZ_CONFIG(LCD_COLOR_PIXEL_FORMAT_RGB565);
    dpi_cfg.num_fbs = 1;
    ek79007_vendor_config_t vendor_cfg = {
        .mipi_config = {
            .dsi_bus = dsi_bus,
            .dpi_config = &dpi_cfg,
        },
    };
    const esp_lcd_panel_dev_config_t panel_cfg = {
        .bits_per_pixel = 16,
        .rgb_ele_order = LCD_RGB_ELEMENT_ORDER_RGB,
        .reset_gpio_num = PIN_LCD_RST,
        .vendor_config = &vendor_cfg,
    };
    ESP_RETURN_ON_ERROR(esp_lcd_new_panel_ek79007(io, &panel_cfg, &s_panel), TAG, "ek79007");
    ESP_RETURN_ON_ERROR(esp_lcd_panel_reset(s_panel), TAG, "panel reset");
    ESP_RETURN_ON_ERROR(esp_lcd_panel_init(s_panel), TAG, "panel init");

    void *fb = NULL;
    ESP_RETURN_ON_ERROR(esp_lcd_dpi_panel_get_frame_buffer(s_panel, 1, &fb), TAG, "get fb");
    s_fb = (uint16_t *)fb;
    memset(s_fb, 0, WT_LCD_W * WT_LCD_H * sizeof(uint16_t));
    wt_display_flush_rows(0, WT_LCD_H);

    // Touch is optional: the UI still works without it.
    if (touch_init() != ESP_OK) {
        ESP_LOGW(TAG, "continuing without touch");
        s_touch = NULL;
    }

    wt_display_backlight(100);
    s_inited = true;
    ESP_LOGI(TAG, "display ready %dx%d", WT_LCD_W, WT_LCD_H);
    return ESP_OK;
}

uint16_t *wt_display_framebuffer(void)
{
    return s_fb;
}

esp_err_t wt_display_flush_rows(uint32_t y0, uint32_t y1)
{
    if (!s_fb || y1 <= y0) return ESP_OK;
    if (y1 > WT_LCD_H) y1 = WT_LCD_H;
    uint8_t *start = (uint8_t *)s_fb + (size_t)y0 * WT_LCD_W * sizeof(uint16_t);
    size_t size = (size_t)(y1 - y0) * WT_LCD_W * sizeof(uint16_t);
    return esp_cache_msync(start, size, ESP_CACHE_MSYNC_FLAG_DIR_C2M | ESP_CACHE_MSYNC_FLAG_UNALIGNED);
}

bool wt_touch_read(uint16_t *x, uint16_t *y)
{
    if (!s_touch) return false;
    uint16_t tx[1], ty[1];
    uint8_t cnt = 0;
    if (esp_lcd_touch_read_data(s_touch) != ESP_OK) return false;
    if (!esp_lcd_touch_get_coordinates(s_touch, tx, ty, NULL, &cnt, 1) || cnt == 0) return false;
    *x = tx[0];
    *y = ty[0];
    return true;
}
