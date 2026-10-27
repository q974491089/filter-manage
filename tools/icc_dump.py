# -*- coding: utf-8 -*-
"""ICC profile structure dump — 为什么 6(咋办).icc 在 colorcpl 里不生效？

按 ICC.1 (ISO 15076-1) 解剖 128 字节头 + 标签表，
对照已知能生效的 RSWOP.icm，找出 Windows mscms 会拒绝的点。
"""
import struct
import sys
import os

REQUIRED_V2_DISPLAY = [b"desc", b"wtpt", b"rXYZ", b"gXYZ", b"bXYZ", b"rTRC", b"gTRC", b"bTRC"]
KNOWN_TAGS = {
    b"A2B0": "device->PCS 8-bit LUT", b"A2B1": "device->PCS LUT (rel.col)",
    b"B2A0": "PCS->device LUT", b"wtpt": "media white point", b"bkpt": "black point",
    b"rXYZ": "red matrix column", b"gXYZ": "green matrix column", b"bXYZ": "blue matrix column",
    b"rTRC": "red tone curve", b"gTRC": "green tone curve", b"bTRC": "blue tone curve",
    b"kTRC": "gray tone curve", b"desc": "profile description", b"cprt": "copyright",
    b"chad": "chromatic adaptation", b"vcgt": "video card gamma table (vcgt)",
    b"DMND": "device manufacturer name", b"DMDD": "device model name",
    b"mmod": "monitor model (v4)", b"meta": "metadata (v4)",
}

def sig4(b):
    return b.decode("ascii", "replace").strip()

def dump(path):
    print("=" * 70)
    print("文件:", path, " 实际大小:", os.path.getsize(path))
    with open(path, "rb") as f:
        data = f.read()
    if len(data) < 132:
        print("!! 文件太小，连头+标签计数都放不下")
        return
    # 逐字段解 128 字节头（ICC.1）
    size = struct.unpack(">I", data[0:4])[0]
    cmm = data[4:8]
    version = data[8:12]
    devclass = data[12:16]
    colorspace = data[16:20]
    pcs = data[20:24]
    date12 = data[24:36]
    magic = data[36:40]
    platform = data[40:44]
    flags = struct.unpack(">I", data[44:48])[0]
    manuf = data[48:52]
    model = data[52:56]
    attrs = struct.unpack(">Q", data[56:64])[0]
    intent = struct.unpack(">I", data[64:68])[0]
    ill = struct.unpack(">iii", data[68:80])
    creator = data[80:84]

    vmaj, vmin = version[0], version[1] >> 4
    print("  头部 size 字段 =", size, "  ← 与实际大小", os.path.getsize(path), "一致" if size == os.path.getsize(path) else "  ** 不一致！Windows 安装校验会挂 **")
    print("  CMM=%s  版本=%d.%d  设备类=%s  色彩空间=%s  PCS=%s" % (
        sig4(cmm), vmaj, vmin, sig4(devclass), sig4(colorspace), sig4(pcs)))
    print("  acsp 魔数 =", sig4(magic), "  ← 必须是 acsp" if magic == b"acsp" else "  ** 不是 acsp！头部损坏 **")
    print("  平台=%s flags=0x%x 厂商=%s 型号=%s 渲染意图=%d" % (
        sig4(platform), flags, sig4(manuf), sig4(model), intent))
    d50 = (0x0000F6D6, 0x00010000, 0x0000D32D)
    print("  PCS 光源 = (0x%08X, 0x%08X, 0x%08X) %s" % (
        ill[0], ill[1], ill[2], "← D50 正确" if ill == d50 else "  ** 不是 D50 **"))

    # 标签表
    count = struct.unpack(">I", data[128:132])[0]
    if count > 1024:
        print("  ** 标签数量 %d 离谱，标签表损坏 **" % count)
        return
    print("  标签数 =", count)
    tags = {}
    off = 132
    problems = []
    for i in range(count):
        tsig, toff, tsize = struct.unpack(">4sII", data[off:off + 12])
        off += 12
        known = KNOWN_TAGS.get(tsig, "?")
        print("    %s  offset=%-6d size=%-6d  (%s)" % (sig4(tsig), toff, tsize, known))
        key = tsig
        if key in tags:
            problems.append("标签 %s 重复" % sig4(tsig))
        tags[key] = (toff, tsize)
        if toff < 128 or toff + tsize > len(data):
            problems.append("标签 %s 越界 (off=%d size=%d file=%d)" % (sig4(tsig), toff, tsize, len(data)))
        if tsize == 0:
            problems.append("标签 %s 长度为 0" % sig4(tsig))
        # 标签数据要求 4 字节对齐（ICC.1 规定 offset 应为 4 的倍数）
        if toff % 4 != 0:
            problems.append("标签 %s offset=%d 未按 4 字节对齐（部分校验器会拒）" % (sig4(tsig), toff))

    missing = [t.decode() for t in REQUIRED_V2_DISPLAY if t not in tags and b"A2B0" not in tags]
    if vmaj < 4 and missing and b"A2B0" not in tags:
        problems.append("v2 显示 profile 缺必需标签: %s" % ",".join(missing))
    if b"vcgt" in tags:
        toff, tsize = tags[b"vcgt"]
        body = data[toff:toff + tsize]
        vtype = body[4:8] if len(body) >= 8 else b"????"
        print("  vcgt: offset=%d size=%d 类型=%s" % (toff, tsize, sig4(vtype)))
        # vcgt 表类型：头 12 字节 = 保留(4)+类型(4)+保留(4)，之后每通道 2 字节头
        if len(body) >= 12 + 6 and struct.unpack(">I", body[4:8])[0] == 0:
            entries = struct.unpack(">H", body[14:16])[0] if len(body) >= 16 else 0
            # 通道头各 2 字节 entryCount + 2 字节 entrySize（交错：R 头后紧跟 R 数据？不——
            # vcgt table 布局是 R头/G头/B头 然后数据，各通道头 4 字节，紧挨排布）
            r_off = 12 + 3 * 4
            if len(body) >= r_off + entries * 2 and entries > 0:
                r = struct.unpack(">%dH" % entries, body[r_off:r_off + entries * 2])
                pts = [0, 64, 128, 192, min(255, entries - 1)]
                print("    vcgt R 采样 =", " ".join("%d:%d" % (p, r[p] >> 8) for p in pts))
    else:
        print("  vcgt: ** 没有 **")

    print("  " + ("** 问题清单 **" if problems else "结构检查未发现问题"))
    for p in problems:
        print("    - " + p)
    print()

for p in sys.argv[1:]:
    dump(p)
