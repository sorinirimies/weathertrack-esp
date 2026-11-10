// Minimal C shim for the Waveshare ESP32-P4-WIFI6-Touch-LCD-7B:
// 1024x600 MIPI-DSI panel (EK79007), PWM backlight and GT911 touch.
#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#define WT_LCD_W 1024
#define WT_LCD_H 600

// Power the DSI PHY, bring up panel + touch. Call once.
esp_err_t wt_display_init(void);

// RGB565 frame buffer (WT_LCD_W * WT_LCD_H pixels) scanned out by the DSI DMA.
uint16_t *wt_display_framebuffer(void);

// Write CPU-cached rows [y0, y1) back to PSRAM so the DMA sees them.
esp_err_t wt_display_flush_rows(uint32_t y0, uint32_t y1);

// 0..100
void wt_display_backlight(int percent);

// Returns true while a finger is down; x/y in panel pixels.
bool wt_touch_read(uint16_t *x, uint16_t *y);
