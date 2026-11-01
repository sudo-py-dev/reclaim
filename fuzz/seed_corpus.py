"""Seed fuzzing with real formats and a valid deleted NTFS resident record."""
from pathlib import Path
import shutil
import struct

root = Path(__file__).resolve().parent
corpus = root / "corpus" / "scan"
corpus.mkdir(parents=True, exist_ok=True)
containers = root / "containers" / "corpus" / "containers"
containers.mkdir(parents=True, exist_ok=True)
for fixture in (root.parent / "tests" / "fixtures").iterdir():
    if fixture.is_file() and fixture.suffix != ".md" and fixture.stat().st_size <= 64 * 1024:
        shutil.copyfile(fixture, corpus / fixture.name)
        if fixture.suffix in {".sqlite", ".ogg", ".opus", ".ogv"}:
            shutil.copyfile(fixture, containers / fixture.name)

record = bytearray(1024)
record[:4] = b"FILE"
for offset, value in [(4, 48), (6, 3), (16, 7), (20, 64), (48, 0xBEEF)]:
    struct.pack_into("<H", record, offset, value)
struct.pack_into("<I", record, 28, len(record))
at = 64

def attribute(kind, value):
    global at
    length = (24 + len(value) + 7) // 8 * 8
    struct.pack_into("<II", record, at, kind, length)
    struct.pack_into("<I", record, at + 16, len(value))
    struct.pack_into("<H", record, at + 20, 24)
    record[at + 24:at + 24 + len(value)] = value
    at += length

attribute(0x10, bytes(48))
name = "report\u202e.txt".encode("utf-16-le")
filename = bytearray(66) + name
filename[64:66] = bytes([len(name) // 2, 1])
attribute(0x30, filename)
attribute(0x80, b"resident data")
struct.pack_into("<I", record, at, 0xFFFFFFFF)
struct.pack_into("<I", record, 24, at + 4)
for index, tail in enumerate([510, 1022]):
    record[50 + index * 2:52 + index * 2] = record[tail:tail + 2]
    struct.pack_into("<H", record, tail, 0xBEEF)
(corpus / "ntfs-resident").write_bytes(record)
(corpus / "document.pdf").write_bytes(b"%PDF-1.7\nexample\n%%EOF")
