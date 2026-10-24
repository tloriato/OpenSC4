"""Read-only DBPF corpus inventory for persistence PR2.

Usage: python tools/scan_buffer_resources.py ROOT [ROOT ...] --output OUTSIDE_ROOTS.json
All regular files are inspected, regardless of extension. Embedded packages are
reported as unsupported candidates. Index 7.1 support is an inventory extension,
not a claim that the native game supports it. SHA-256 identifies exact bytes;
container/index header versions do not establish a game build or edition. Target
headers are classified, not decoded/validated as pixels, including inner v2 QFS.
"""
import argparse
import hashlib
import json
from pathlib import Path
import stat
import struct

from dbpf import qfs_decompress

TARGET = 0x62B9DA24
DIR = 0xE86B1EEF
PNG = 0x856DDBAC
EMBEDDED = bytes.fromhex("80 9D 88 EC 8F 24 03 6C C9 A6 31 56 5B CF 77 22") + b"DBPF"
CHUNK_SIZE = 65536


def embedded_candidate(stream, initial=b""):
    """Find the native embedded-package marker without loading a nonpackage file."""
    tail = initial
    while True:
        chunk = stream.read(CHUNK_SIZE)
        combined = tail + chunk
        if EMBEDDED in combined:
            return True
        if not chunk:
            return False
        tail = combined[-(len(EMBEDDED) - 1):]


def package(data):
    """Validate container/index extents, inventory keys, classify target headers."""
    def words(offset, count):
        if offset < 0 or offset + count * 4 > len(data):
            raise ValueError("truncated header/index")
        return struct.unpack_from("<" + "I" * count, data, offset)

    def extent(offset, size):
        if offset > len(data) or size > len(data) - offset:
            raise ValueError("record/index outside file")

    if len(data) < 96 or data[:4] != b"DBPF":
        raise ValueError("truncated or invalid DBPF header")
    major, minor = words(4, 2)
    imajor, count, index, size = words(0x20, 4)
    iminor = words(0x3C, 1)[0]
    if (major, minor) not in ((1, 0), (1, 1)) or imajor != 7 or iminor not in (0, 1):
        raise ValueError("unsupported container/index version")
    width = 20 if iminor == 0 else 24
    if size != count * width:
        raise ValueError("index size/count mismatch")
    extent(index, size)
    entries = []
    for n in range(count):
        fields = words(index + n * width, width // 4)
        t, g, low = fields[:3]
        instance = low if width == 20 else (fields[3] << 32) | low
        off, length = fields[-2:]
        extent(off, length)
        entries.append(((t, g, instance), off, length))
    compressed = {}
    for (t, _, _), off, length in entries:
        if t != DIR:
            continue
        stride = 16 if width == 20 else 20
        if length % stride:
            raise ValueError("DIR size is not a whole number of records")
        for pos in range(off, off + length, stride):
            fields = words(pos, stride // 4)
            instance = fields[2] if width == 20 else (fields[3] << 32) | fields[2]
            key = fields[0], fields[1], instance
            if key in compressed and compressed[key] != fields[-1]:
                raise ValueError("conflicting DIR records")
            compressed[key] = fields[-1]
    targets = []
    for key, off, length in entries:
        if key[0] != TARGET:
            continue
        payload = data[off:off + length]
        outer = key in compressed
        if outer:
            if len(payload) < 9 or payload[4:6] != b"\x10\xfb":
                raise ValueError("unsupported or truncated outer QFS framing")
            payload = qfs_decompress(payload)
            if len(payload) != compressed[key]:
                raise ValueError("outer QFS/DIR output-size mismatch")
        if len(payload) < 8:
            raise ValueError("truncated target header")
        marker = struct.unpack_from("<I", payload)[0]
        if payload[:2] == b"BM":
            form, minimum = "bmp", 54
        elif marker == TARGET:
            form, minimum = "v0", 12
        elif marker in (1, 2):
            form, minimum = "v" + str(marker), 20
        else:
            raise ValueError("unknown target marker")
        if len(payload) < minimum:
            raise ValueError("truncated " + form + " target header")
        targets.append({"group": f"{key[1]:08X}", "instance": f"{key[2]:016X}",
                        "form": form, "outer_compressed": outer})
    return {"container_version": f"{major}.{minor}", "index_version": f"{imajor}.{iminor}",
            "entry_count": count, "target_count": len(targets), "targets": targets,
            "png_type_count": sum(key[0] == PNG for key, _, _ in entries)}


def scan(roots):
    roots = list(roots)
    result = {"scope": "DBPF index and target-header classification only; no pixel or inner-v2 validation",
              "roots": [str(Path(root).absolute()) for root in roots],
              "complete": True, "packages": [], "non_dbpf_files": 0, "errors": []}

    def error(label, message):
        result["errors"].append({"path": label, "error": str(message)})

    def visit(path, label):
        try:
            info = path.lstat()
            if stat.S_ISLNK(info.st_mode) or getattr(info, "st_file_attributes", 0) & 0x400:
                error(label, "link/reparse point skipped")
            elif stat.S_ISDIR(info.st_mode):
                for child in sorted(path.iterdir(), key=lambda p: p.name):
                    visit(child, label + "/" + child.name)
            elif stat.S_ISREG(info.st_mode):
                with path.open("rb") as stream:
                    magic = stream.read(4)
                    if magic != b"DBPF":
                        result["non_dbpf_files"] += 1
                        if embedded_candidate(stream, magic):
                            error(label, "unsupported embedded-package candidate")
                        return
                    data = magic + stream.read()
                record = {"path": label, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()}
                record.update(package(data))
                result["packages"].append(record)
            else:
                error(label, "nonregular file skipped")
        except (OSError, ValueError, IndexError, struct.error) as exc:
            error(label, exc)

    for n, root in enumerate(roots):
        visit(Path(root), f"root{n}")
    if not result["packages"]:
        error("corpus", "no successfully parsed DBPF packages")
    result["target_count"] = sum(p["target_count"] for p in result["packages"])
    result["package_count"] = len(result["packages"])
    result["complete"] = not result["errors"]
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("roots", nargs="+")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    # An output inside an input directory could change the corpus on the next run.
    if args.output:
        destination = args.output.resolve()
        for root in args.roots:
            source = Path(root).resolve()
            if destination == source or source in destination.parents:
                parser.error("output must be outside all scanned roots")
    result = scan(args.roots)
    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")
    return 0 if result["complete"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
