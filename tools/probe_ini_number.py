"""Analysis-only probe of the supported executable's embedded decimal scanner.

Requires pefile and Unicorn. No game bytes are distributed by this tool. This
executes the original scanner, supplies C initial-locale thread context, and
does not run ReadEntry. ReadNumber's boolean is derived from its verified
assignment-count-equals-one rule. No host CRT conversion is used.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

import pefile
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ESP, UC_X86_REG_EIP


SUPPORTED_SHA256 = "3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b"
SCANNER = 0x009EF536
GET_THREAD_CONTEXT = 0x009F7604
STOP_PC = 0x04000000
STEP_BUDGET = 1_000_000
INPUT = 0x02000000
FORMAT = 0x00ACF148
OUTPUT = 0x02003000
THREAD_CONTEXT = 0x020F0000
STACK = 0x030FF000
SENTINEL = 0x12345678


def signed(bits):
    return bits if bits < 0x80000000 else bits - 0x100000000


def vectors():
    # Expectations express observable modulo arithmetic, not a CRT algorithm.
    numbers = ["0", "12", "-1", "2147483647", "2147483648", "2147483649",
               "4294967295", "4294967296", "4294967297", "-2147483648",
               "-2147483649", "-4294967295", "-4294967296", "-4294967297",
               "9223372036854775808", "18446744073709551615",
               "18446744073709551616", "18446744073709551617",
               "9" * 1024, "0" * 1024]
    result = [(value, 1, int(value) & 0xFFFFFFFF) for value in numbers]
    result.extend([(text, 1, bits) for text, bits in [
        ("+4294967297junk", 1), ("4294967297e3", 1),
        (" \t-4294967297", 0xFFFFFFFF), ("012", 12), ("0x10", 0)]])
    result.extend([(text, count, SENTINEL) for text, count in [
        ("", -1), (" \t", -1), ("+", -1), ("-", -1),
        ("abc", 0), ("+abc", 0), ("--1", 0)]])
    return result


def probe(image, base, text, expected_count, expected_bits):
    machine = Uc(UC_ARCH_X86, UC_MODE_32)
    machine.mem_map(base, (len(image) + 4095) & ~4095)
    machine.mem_write(base, image)
    machine.mem_map(0, 4096)  # Windows x86 SEH fixture.
    machine.mem_map(INPUT, 0x100000)
    machine.mem_map(0x03000000, 0x100000)
    machine.mem_map(STOP_PC, 4096)

    def read32(address):
        return struct.unpack("<I", machine.mem_read(address, 4))[0]

    def write32(address, value):
        machine.mem_write(address, struct.pack("<I", value))

    locale = read32(0x00B1921C)
    if read32(locale + 0x28) != 1:
        raise RuntimeError("Unsupported initial locale character mode")
    write32(THREAD_CONTEXT + 0x64, locale)
    write32(THREAD_CONTEXT + 0x68, read32(0x00B19220))
    hook_calls = 0

    def supply_thread_context(uc, address, size, user_data):
        nonlocal hook_calls
        hook_calls += 1
        stack = uc.reg_read(UC_X86_REG_ESP)
        uc.reg_write(UC_X86_REG_EAX, THREAD_CONTEXT)
        uc.reg_write(UC_X86_REG_EIP, read32(stack))
        uc.reg_write(UC_X86_REG_ESP, stack + 4)

    # Only thread-context acquisition is intercepted; digit classification,
    # input handling, accumulation, and destination assignment execute natively.
    machine.hook_add(UC_HOOK_CODE, supply_thread_context,
                     begin=GET_THREAD_CONTEXT, end=GET_THREAD_CONTEXT)
    machine.mem_write(INPUT, text.encode("ascii") + b"\0")
    if machine.mem_read(FORMAT, 4) != b"%ld\0":
        raise RuntimeError("ReadNumber's native format literal does not match")
    write32(OUTPUT, SENTINEL)
    machine.mem_write(STACK, struct.pack("<IIII", STOP_PC, INPUT, FORMAT, OUTPUT))
    machine.reg_write(UC_X86_REG_ESP, STACK)
    machine.emu_start(SCANNER, STOP_PC, count=STEP_BUDGET)
    stop = machine.reg_read(UC_X86_REG_EIP)
    if stop != STOP_PC:
        raise RuntimeError(f"Step budget exhausted at {stop:#x}")
    count = signed(machine.reg_read(UC_X86_REG_EAX))
    bits = read32(OUTPUT)
    return {"input": text, "scanner_assignment_count": count,
            "read_number_boolean": count == 1, "output_bits": f"0x{bits:08X}",
            "output_signed": signed(bits), "stop_pc": f"0x{stop:08X}",
            "thread_context_hook_calls": hook_calls,
            "expected": {"scanner_assignment_count": expected_count,
                         "read_number_boolean": expected_count == 1,
                         "output_bits": f"0x{expected_bits:08X}",
                         "output_signed": signed(expected_bits)},
            "passed": count == expected_count and bits == expected_bits}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exe", type=Path)
    parser.add_argument("--output", type=Path, help="Optional JSON report path")
    args = parser.parse_args()
    digest = hashlib.sha256(args.exe.read_bytes()).hexdigest()
    if digest != SUPPORTED_SHA256:
        parser.error(f"Unsupported executable SHA-256: {digest}")
    if args.output and (args.output.resolve() == args.exe.resolve() or
                        (args.output.exists() and args.output.samefile(args.exe))):
        parser.error("Output must not overwrite the executable")
    pe = pefile.PE(str(args.exe), fast_load=True)
    image = pe.get_memory_mapped_image()
    results = [probe(image, pe.OPTIONAL_HEADER.ImageBase,
                     *case) for case in vectors()]
    report = {"executable_sha256": digest, "scope": "Embedded sscanf with %ld; "
              "scanner-only emulation; ReadEntry not run; ReadNumber boolean "
              "derived from verified wrapper assignment-count == 1",
              "fixture": "Initial C locale; only thread-context getter hooked",
              "step_budget_per_case": STEP_BUDGET, "cases": results,
              "passed": all(case["passed"] for case in results)}
    rendered = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
