"""Load an import-free PE32 image into a gap shared by all captured address spaces."""
import hashlib
import struct
import pefile


def free_gap(regions, size, avoid=()):
    spans = [(r[0], r[0]+r[1]) for rs in regions for r in rs if r[2] != 0x10000]
    spans += list(avoid)
    for base in range(0x50000000, 0x70000000, 0x10000):
        if not any(lo < base + size and base < hi for lo, hi in spans):
            return base
    raise ValueError('no shared free gap for the DLL')


class Port:
    def __init__(self, path, regions, avoid=()):
        raw = path.read_bytes()
        self.sha = hashlib.sha256(raw).hexdigest()
        pe = pefile.PE(data=raw)
        if pe.FILE_HEADER.Machine != 0x14c:
            raise ValueError('port must be PE32 x86')
        if getattr(pe, 'DIRECTORY_ENTRY_IMPORT', ()) or getattr(pe, 'DIRECTORY_ENTRY_DELAY_IMPORT', ()):
            raise ValueError('port has imports')
        self.base = free_gap(regions, pe.OPTIONAL_HEADER.SizeOfImage, avoid)
        delta = self.base - pe.OPTIONAL_HEADER.ImageBase
        img = bytearray(pe.get_memory_mapped_image())
        img.extend(bytes(pe.OPTIONAL_HEADER.SizeOfImage-len(img)))
        relocs = getattr(pe, 'DIRECTORY_ENTRY_BASERELOC', ())
        if delta and pe.FILE_HEADER.Characteristics & 1:
            raise ValueError('DLL has stripped relocations and cannot be relocated')
        for block in relocs:
            for r in block.entries:
                if r.type == 3:
                    v = struct.unpack_from('<I', img, r.rva)[0]
                    struct.pack_into('<I', img, r.rva, (v + delta) & 0xffffffff)
                elif r.type != 0:
                    raise ValueError(f'unsupported PE relocation {r.type}')
        self.image = bytes(img)
        self.exports = {}
        for sym in pe.DIRECTORY_ENTRY_EXPORT.symbols:
            name = (sym.name or b'').decode()
            if name.startswith('fc4p_') and len(name) == 13:
                self.exports[int(name[5:], 16)] = self.base + sym.address
