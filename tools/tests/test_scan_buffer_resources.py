"""Synthetic corpus tests; no game assets or pixel-decoder claims."""
from pathlib import Path
import io
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import scan_buffer_resources as scanner


def fixture(records=(), minor=0):
    width = 20 if minor == 0 else 24
    header = bytearray(96)
    header[:4] = b"DBPF"
    struct.pack_into("<II", header, 4, 1, 0)
    struct.pack_into("<IIII", header, 0x20, 7, len(records), 96, len(records) * width)
    struct.pack_into("<I", header, 0x3C, minor)
    index, payload = bytearray(), bytearray()
    for t, g, instance, body in records:
        fields = [t, g, instance & 0xFFFFFFFF]
        if minor:
            fields.append(instance >> 32)
        fields += [96 + len(records) * width + len(payload), len(body)]
        index += struct.pack("<" + "I" * len(fields), *fields)
        payload += body
    return bytes(header + index + payload)


class ScanTests(unittest.TestCase):
    def test_valid_zero_and_png_control(self):
        result = scanner.package(fixture([(scanner.PNG, 0, 0, b"control")]))
        self.assertEqual(result["target_count"], 0)
        self.assertEqual(result["png_type_count"], 1)

    def test_all_markers(self):
        bodies = [b"BM" + bytes(52), struct.pack("<III", scanner.TARGET, 1, 1),
                  struct.pack("<IIIII", 1, 4, 1, 1, 8), struct.pack("<IIIII", 2, 0, 1, 1, 8)]
        result = scanner.package(fixture([(scanner.TARGET, 0, i, b) for i, b in enumerate(bodies)]))
        self.assertEqual([x["form"] for x in result["targets"]], ["bmp", "v0", "v1", "v2"])

    def test_index71_preserves_instance(self):
        body = struct.pack("<III", scanner.TARGET, 1, 1)
        result = scanner.package(fixture([(scanner.TARGET, 2, 0x123456789ABCDEF0, body)], 1))
        self.assertEqual(result["targets"][0]["instance"], "123456789ABCDEF0")

    def test_compressed_target(self):
        body = struct.pack("<III", scanner.TARGET, 1, 1)
        stream = b"\x10\xfb\x00\x00\x0c\xe2" + body + b"\xfc"
        packed = struct.pack("<I", len(stream) + 4) + stream
        directory = struct.pack("<IIII", scanner.TARGET, 3, 4, len(body))
        result = scanner.package(fixture([(scanner.TARGET, 3, 4, packed), (scanner.DIR, 0, 0, directory)]))
        self.assertTrue(result["targets"][0]["outer_compressed"])

    def test_bad_index_and_extent(self):
        for offset, value in [(0x2C, 19), (96 + 12, 0xFFFFFFFF)]:
            data = bytearray(fixture([(scanner.PNG, 0, 0, b"x")]))
            struct.pack_into("<I", data, offset, value)
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                scanner.package(data)

    def test_bad_target_and_dir(self):
        for records in [[(scanner.TARGET, 0, 0, b"unknown!")],
                        [(scanner.TARGET, 0, 0, struct.pack("<II", 2, 0))],
                        [(scanner.DIR, 0, 0, b"x")]]:
            with self.subTest(records=records), self.assertRaises(ValueError):
                scanner.package(fixture(records))

    def test_unsupported_outer_framing_fails(self):
        directory = struct.pack("<IIII", scanner.TARGET, 0, 0, 12)
        data = fixture([(scanner.TARGET, 0, 0, bytes(4) + b"\x11\xfb" + bytes(15)),
                        (scanner.DIR, 0, 0, directory)])
        with self.assertRaisesRegex(ValueError, "outer QFS framing"):
            scanner.package(data)

    def test_unsupported_package_version(self):
        data = bytearray(fixture())
        struct.pack_into("<I", data, 4, 2)
        with self.assertRaisesRegex(ValueError, "unsupported"):
            scanner.package(data)

    def test_symlink_is_an_explicit_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "package").write_bytes(fixture())
            try:
                (root / "link").symlink_to(root / "package")
            except OSError:
                self.skipTest("symlink creation unavailable")
            result = scanner.scan([root])
            self.assertFalse(result["complete"])
            self.assertEqual(result["package_count"], 1)

    def test_missing_and_no_packages_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertFalse(scanner.scan([Path(tmp) / "missing"])["complete"])
            self.assertFalse(scanner.scan([tmp])["complete"])

    def test_magic_discovery_and_determinism(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "odd.extension").write_bytes(fixture())
            (root / "readme.txt").write_text("not a package")
            first = scanner.scan([root])
            self.assertTrue(first["complete"])
            self.assertEqual(first["non_dbpf_files"], 1)
            self.assertEqual(first, scanner.scan([root]))

    def test_embedded_marker_across_chunk_boundary(self):
        # Initial magic read consumes four bytes before chunked search starts.
        data = b"x" * (scanner.CHUNK_SIZE + 1) + scanner.EMBEDDED + bytes(96)
        stream = io.BytesIO(data)
        self.assertTrue(scanner.embedded_candidate(stream, stream.read(4)))
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "valid").write_bytes(fixture())
            (Path(tmp) / "embedded").write_bytes(data)
            result = scanner.scan([tmp])
            self.assertFalse(result["complete"])
            self.assertEqual(result["non_dbpf_files"], 1)
            self.assertIn("embedded-package", result["errors"][0]["error"])

    def test_plain_dbpf_text_is_not_embedded_candidate(self):
        self.assertFalse(scanner.embedded_candidate(io.BytesIO(b"text DBPF text")))
        self.assertFalse(scanner.embedded_candidate(io.BytesIO(scanner.EMBEDDED[:-4])))

    def test_bad_package_and_target_propagate_as_incomplete(self):
        samples = [b"DBPF", fixture([(scanner.TARGET, 0, 0, b"unknown!")]),
                   fixture([(scanner.TARGET, 0, 0, struct.pack("<II", 1, 0))])]
        for sample in samples:
            with self.subTest(sample=sample[:16]), tempfile.TemporaryDirectory() as tmp:
                (Path(tmp) / "valid").write_bytes(fixture())
                (Path(tmp) / "invalid").write_bytes(sample)
                result = scanner.scan([tmp])
                self.assertFalse(result["complete"])
                self.assertEqual(result["package_count"], 1)
                self.assertEqual(len(result["errors"]), 1)

    def test_compressed_dir_size_mismatch(self):
        body = struct.pack("<III", scanner.TARGET, 1, 1)
        stream = b"\x10\xfb\x00\x00\x0c\xe2" + body + b"\xfc"
        packed = struct.pack("<I", len(stream) + 4) + stream
        directory = struct.pack("<IIII", scanner.TARGET, 0, 0, 13)
        with self.assertRaisesRegex(ValueError, "output-size mismatch"):
            scanner.package(fixture([(scanner.TARGET, 0, 0, packed), (scanner.DIR, 0, 0, directory)]))

    def test_impossible_initial_backreference_is_reported(self):
        # No output exists when this copy instruction asks for distance one.
        stream = b"\x10\xfb\x00\x00\x03\x00\x00\xfc"
        packed = struct.pack("<I", len(stream) + 4) + stream
        directory = struct.pack("<IIII", scanner.TARGET, 0, 0, 3)
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "valid").write_bytes(fixture())
            (Path(tmp) / "bad").write_bytes(fixture([
                (scanner.TARGET, 0, 0, packed), (scanner.DIR, 0, 0, directory)]))
            result = scanner.scan([tmp])
            self.assertFalse(result["complete"])
            self.assertEqual(len(result["errors"]), 1)


if __name__ == "__main__":
    unittest.main()
