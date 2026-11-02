#!/usr/bin/env python3
"""Generate project-owned parser fixtures; never executes their contents."""
import gzip
import io
from pathlib import Path
import struct as s
import tarfile
import zlib

OUT = Path(__file__).resolve().parent.parent / 'tests/fixtures/formats'
OUT.mkdir(parents=True, exist_ok=True)

def put(name, data):
    (OUT / name).write_bytes(data)

def u16(b, p, n): s.pack_into('<H', b, p, n)
def u32(b, p, n): s.pack_into('<I', b, p, n)
def u64(b, p, n): s.pack_into('<Q', b, p, n)

# PE32 and PE32+ with a single inert section, plus an optional certificate record.
for wide, ext, flags, cert in [(False, 'exe', 2, False), (True, 'dll', 0x2002, False), (True, 'exe', 2, True)]:
    b = bytearray(1040 if cert else 1024)
    b[:2] = b'MZ'; u32(b, 60, 128); b[128:132] = b'PE\0\0'
    u16(b, 132, 0x8664 if wide else 0x14c); u16(b, 134, 1)
    optional = 240 if wide else 224
    u16(b, 148, optional); u16(b, 150, flags)
    o = 152; u16(b, o, 0x20b if wide else 0x10b)
    u32(b, o+32, 4096); u32(b, o+36, 512); u32(b, o+56, 8192); u32(b, o+60, 512)
    u16(b, o+68, 3); u32(b, o+(108 if wide else 92), 16)
    p = o+optional; b[p:p+8] = b'.text\0\0\0'
    u32(b, p+8, 1); u32(b, p+12, 4096); u32(b, p+16, 512); u32(b, p+20, 512); u32(b, p+36, 0x60000020)
    b[512] = 0xc3
    if cert:
        u32(b, o+112+32, 1024); u32(b, o+112+36, 16)
        u32(b, 1024, 16); u16(b, 1028, 0x200); u16(b, 1030, 2)
    put(('certificate' if cert else ('pe64' if wide else 'pe32'))+'.'+ext, b)

# ELF32/ELF64, each byte order; a loadable file image with no section table.
for wide in [False, True]:
    for endian in ['<', '>']:
        eh, ph = (64, 56) if wide else (52, 32)
        b=bytearray(128); b[:7] = b'\x7fELF'+bytes([2 if wide else 1, 1 if endian=='<' else 2, 1])
        def write(p, fmt, n): s.pack_into(endian+fmt, b, p, n)
        write(16,'H',2); write(18,'H',62 if wide else 3); write(20,'I',1)
        write(32 if wide else 28, 'Q' if wide else 'I', eh)
        h=52 if wide else 40; write(h,'H',eh); write(h+2,'H',ph); write(h+4,'H',1)
        write(eh,'I',1); write(eh+(32 if wide else 16),'Q' if wide else 'I',len(b)); write(eh+(40 if wide else 20),'Q' if wide else 'I',len(b))
        put(f'elf{64 if wide else 32}-{ "le" if endian=="<" else "be"}.elf', b)

# CFB V3/V4 with one FAT and root directory sector. CLSIDs identify Installer types.
for major, shift in [(3,9), (4,12)]:
    for ext, clsid in [('msi',0xc1084), ('msp',0xc1086), ('mst',0xc1082), ('ole',0)]:
        sector=1<<shift; b=bytearray(sector*3)
        b[:8]=bytes.fromhex('d0cf11e0a1b11ae1'); u16(b,24,0x3e); u16(b,26,major);u16(b,28,0xfffe);u16(b,30,shift);u16(b,32,6)
        u32(b,40,1 if major==4 else 0);u32(b,44,1);u32(b,48,1);u32(b,56,4096);u32(b,60,0xfffffffe);u32(b,68,0xfffffffe)
        b[76:512]=b'\xff'*436;u32(b,76,0)
        b[sector:sector*2]=b'\xff'*sector;u32(b,sector,0xfffffffd);u32(b,sector+4,0xfffffffe)
        root=sector*2;name='Root Entry\0'.encode('utf-16le');b[root:root+len(name)]=name
        u16(b,root+64,len(name));b[root+66]=5;b[root+67]=1
        for p in [68,72,76]:u32(b,root+p,0xffffffff)
        u32(b,root+116,0xfffffffe)
        if clsid:
            u32(b,root+80,clsid);b[root+88]=0xc0;b[root+95]=0x46
        put(f'cfb{major}.{ext}',b)

# 7z empty logical header with checked start/next-header CRCs.
header=b'\x01\0'; start=s.pack('<QQI',0,len(header),zlib.crc32(header))
put('empty.7z',b"7z\xbc\xaf\x27\x1c\0\x04"+s.pack('<I',zlib.crc32(start))+start+header)

def tar_bytes(name, content):
    buf=io.BytesIO()
    with tarfile.open(fileobj=buf,mode='w',format=tarfile.USTAR_FORMAT) as tar:
        info=tarfile.TarInfo(name);info.size=len(content);info.mtime=0;tar.addfile(info,io.BytesIO(content))
    data=buf.getvalue()
    # Keep exactly two end blocks; optional tape blocking padding isn't file data.
    end=512+((len(content)+511)//512)*512+1024
    return data[:end]

payload=b'Reclaim format recovery fixture\n'
put('sample.tar',tar_bytes('payload.txt',payload))
put('sample.gz',gzip.compress(payload,mtime=0))

def ar_member(name, data):
    header=f'{name+"/":<16}{0:<12}{0:<6}{0:<6}{"100644":<8}{len(data):<10}`\n'.encode()
    assert len(header)==60
    return header+data+(b'\n' if len(data)%2 else b'')
control=b'Package: reclaim-fixture\nVersion: 1.0\nArchitecture: all\nMaintainer: Reclaim contributors\nDescription: Project-owned recovery fixture\n'
deb=b'!<arch>\n'+ar_member('debian-binary',b'2.0\n')+ar_member('control.tar.gz',gzip.compress(tar_bytes('control',control),mtime=0))+ar_member('data.tar.gz',gzip.compress(tar_bytes('usr/share/reclaim-fixture.txt',payload),mtime=0))
put('sample.deb',deb)

# RPM v3 lead + signature size tag + minimal main header/payload framing.
main=b'\x8e\xad\xe8\x01\0\0\0\0'+s.pack('>II',1,4)+s.pack('>IIII',1000,4,0,1)+s.pack('>I',0)
packed=gzip.compress(b'070701'+payload,mtime=0)
sig=b'\x8e\xad\xe8\x01\0\0\0\0'+s.pack('>II',1,4)+s.pack('>IIII',1000,4,0,1)+s.pack('>I',len(main)+len(packed))
lead=bytearray(96);lead[:6]=b'\xed\xab\xee\xdb\x03\0';s.pack_into('>H',lead,78,5)
put('sample.rpm',lead+sig+b'\0'*((-len(sig))%8)+main+packed)

# One-folder CAB, uncompressed CFDATA with an optional zero checksum.
name=b'payload.txt\0';data_at=36+8+16+len(name);total=data_at+8+len(payload)
b=bytearray(total);b[:4]=b'MSCF';u32(b,8,total);u32(b,16,44);b[24:26]=b'\x03\x01';u16(b,26,1);u16(b,28,1)
u32(b,36,data_at);u16(b,40,1);u32(b,44,len(payload));b[60:60+len(name)]=name
u16(b,data_at+4,len(payload));u16(b,data_at+6,len(payload));b[data_at+8:]=payload
put('sample.cab',b)

def rar_block(body, data=b''):
    assert len(body)<128
    header=bytes([len(body)])+body
    return s.pack('<I',zlib.crc32(header))+header+data
# Main, file (stored method), end. Data checksum is not required by the format.
rar=b'Rar!\x1a\x07\x01\0'+rar_block(b'\x01\0\0')+rar_block(bytes([2,2,len(payload),0,len(payload),0,0,1,1])+b'a',payload)+rar_block(b'\x05\0\0')
put('sample.rar',rar)

# One RGBA QOI pixel and a DEX header/map with valid Adler32.
put('sample.qoi',b'qoif'+s.pack('>II',1,1)+b'\x04\0\xff\x10\x20\x30\xff'+b'\0'*7+b'\x01')
b=bytearray(128);b[:8]=b'dex\n039\0';u32(b,32,len(b));u32(b,36,112);u32(b,40,0x12345678);u32(b,52,112);u32(b,104,16);u32(b,108,112)
u32(b,112,1);u16(b,116,0x1000);u32(b,120,1);u32(b,124,112)
import hashlib
b[12:32]=hashlib.sha1(b[32:]).digest();u32(b,8,zlib.adler32(b[12:]));put('sample.dex',b)

for ext,name in [('xar','payload.txt'),('pkg','PackageInfo')]:
    xml=f'<?xml version="1.0"?><xar><toc><file id="1"><name>{name}</name><type>file</type><data><length>{len(payload)}</length><offset>0</offset><size>{len(payload)}</size><encoding style="application/octet-stream"/></data></file></toc></xar>'.encode()
    toc=zlib.compress(xml)
    put('sample.'+ext,b'xar!'+s.pack('>HHQQI',28,1,len(toc),len(xml),0)+toc+payload)
