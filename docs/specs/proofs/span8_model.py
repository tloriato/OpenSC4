"""Model of the cSpanRender8 RenderBlt bodies (docs/specs/span-render-8.md section 7): one destination byte per pixel."""
KEYM = [0xFFF, 0x7FFF, 0x7FFF, 0xFFFF, 0xFFFFFF, 0xFFFFFF, 0xFF]   # caller's key mask per source (565 spec 6.1)
KCM = KEYM[:6] + [0]   # bits of the texel each keyed body compares: the 8-bit keyed row compares none
def alpha(f, s):             # source texel -> 8-bit alpha (section 7.2)
    if f == 0: return (s >> 12) * 17
    if f == 1: return 255 if s & 0x8000 else 0
    if f == 4: return s >> 24
    if f == 6: return s
    return 255
def model(f, key, kind, s, D, col, keyv):
    if key and (s & KCM[f]) == keyv: return D
    k = kind & 3
    if k & 1: return 255   # ConstantAlpha and ModulateAlpha store 0xFF
    a = alpha(f, s)
    if k == 0: return a
    return (a * (col >> 24) + 128) >> 8
