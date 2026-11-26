#!/usr/bin/env python3
"""量下载引擎让 APK 大多少（按 ABI 分包以后，每个 ABI 的包里只有自己那份 lib/<ABI>/）。

APK 就是 zip：引擎在 APK 里占的 = lib/<ABI>/ 下引擎的文件（压缩后）+ assets/captae-engine/（engine.zip 原样存）。
装到手机上以后另外占：原生库解压出来的大小（extractNativeLibs=true）+ 首次启动解压的数据。

用法：python3 -I scripts/mobile/android-engine/size-report.py <APK> [<ABI>...]
"""
import sys
import zipfile

# 不是引擎的（Rust 核心、JNA），不算
NOT_ENGINE = ('libcaptae_mobile.so', 'libjnidispatch.so')


def mb(n):
    return f'{n / 1e6:.2f} MB'


def main():
    apk = sys.argv[1]
    abis = sys.argv[2:] or ['arm64-v8a', 'x86_64']
    z = zipfile.ZipFile(apk)
    infos = z.infolist()
    assets = [i for i in infos if i.filename.startswith('assets/captae-engine/')]
    a_c = sum(i.compress_size for i in assets)
    a_u = sum(i.file_size for i in assets)
    print(f'assets/captae-engine：APK 里 {mb(a_c)}（{len(assets)} 个文件）')
    for abi in abis:
        libs = [i for i in infos if i.filename.startswith(f'lib/{abi}/') and i.filename.rsplit('/', 1)[1] not in NOT_ENGINE]
        c = sum(i.compress_size for i in libs)
        u = sum(i.file_size for i in libs)
        print(f'{abi}：原生库 {len(libs)} 个，APK 里 {mb(c)}，装好后解压出来 {mb(u)}')
        print(f'  → 这个 ABI 的包增加 {mb(c + a_c)}；装好后再占 {mb(u)}（原生库）+ 首次启动解压的数据')
        big = sorted(libs, key=lambda i: -i.compress_size)[:8]
        for i in big:
            print(f'    {i.filename.rsplit("/", 1)[1]:32s} {mb(i.compress_size):>10s}（解压 {mb(i.file_size)}）')
    print(f'engine.zip 解压前 {mb(a_u)}')


if __name__ == '__main__':
    main()
