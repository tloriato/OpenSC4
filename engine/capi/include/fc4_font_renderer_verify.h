/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef FC4_FONT_RENDERER_VERIFY_H
#define FC4_FONT_RENDERER_VERIFY_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct fc4_fr_rect { int32_t left, top, right, bottom; } fc4_fr_rect;
/* Safe backing lengths are transport metadata, never native record fields. */
typedef struct fc4_fr_record {
    fc4_fr_rect destination;
    const uint8_t* source;
    uint32_t source_len, source_pitch;
    int32_t source_width, source_height;
    uint32_t character;
} fc4_fr_record;
typedef struct fc4_fr_target {
    int32_t width, height;
    uint32_t depth, format, format_depth, pitch;
    uint8_t* pixels;
    uint32_t pixel_len;
    uint8_t lock_al, unlock_al;
} fc4_fr_target;
typedef struct fc4_fr_trace { uint32_t slot, a, b; } fc4_fr_trace;
/* operation 0 draw, 1 lock, 2 unlock. Returns 0=completed, 1=refused,
 * 2=trace overflow. result is draw bool or lock/unlock AL. No internal events. */
uint32_t fc4_font_renderer_verify(uint32_t operation, const fc4_fr_target* target,
    const fc4_fr_record* records, uint32_t count, uint32_t mode,
    const uint8_t* colours, uint32_t colour_len, const fc4_fr_rect* clip,
    fc4_fr_trace* trace, uint32_t trace_capacity, uint32_t* trace_count, uint8_t* result);
#ifdef __cplusplus
}
#endif
#endif
