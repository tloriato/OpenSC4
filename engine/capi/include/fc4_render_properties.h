/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef FC4_RENDER_PROPERTIES_H
#define FC4_RENDER_PROPERTIES_H
#include <stdint.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct fc4_renderprops fc4_renderprops;
/* cdecl int32 input/result; both callbacks borrowed until destroy. */
typedef int32_t (*fc4_renderprops_primitive)(int32_t);
fc4_renderprops* fc4_renderprops_create(fc4_renderprops_primitive upper, fc4_renderprops_primitive lower);
void fc4_renderprops_destroy(fc4_renderprops* owner);
/* kind 0=bool, 1=int, 2=float; invalid count/find/name returns -1. */
int32_t fc4_renderprops_count(const fc4_renderprops* owner, uint32_t kind);
int32_t fc4_renderprops_find(const fc4_renderprops* owner, uint32_t kind, const uint8_t* bytes, size_t len);
/* Raw byte copy without NUL; capacity zero queries length; -2 insufficient capacity. */
int32_t fc4_renderprops_name(const fc4_renderprops* owner, uint32_t kind, int32_t index, uint8_t* output, size_t capacity);
/* bool 0/1, int32 bits, IEEE binary32 bits. Return 1 success, 0 rejected. */
int32_t fc4_renderprops_get(const fc4_renderprops* owner, uint32_t kind, int32_t index, uint32_t* output);
int32_t fc4_renderprops_set(const fc4_renderprops* owner, uint32_t kind, int32_t index, uint32_t bits);
#ifdef __cplusplus
}
#endif
#endif
