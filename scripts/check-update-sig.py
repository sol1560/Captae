#!/usr/bin/env python3
"""核对应用内更新包的签名（docs/DESIGN.md 第 15 节）。只用 Python 标准库，不用另装 minisign。

用法：python3 -I scripts/check-update-sig.py <更新包> <版本号> [tauri.conf.json]

和 Captae 里的更新插件（tauri-plugin-updater，minisign-verify）做同样的检查：
- `<更新包>.sig` 是 tauri.conf.json 里 plugins.updater.pubkey 那把公钥对应的私钥签的（钥匙编号一致，Ed25519 签名对得上）；
- 签名里「受信任的注释」也签过（全局签名对得上），里面的 version 和要发的版本号一样（Captae 开着 requireSignedVersion）、file 是这个文件名。

Ed25519 的验签照 RFC 8032 第 6 节的参考实现写（纯 Python，验一次几十毫秒）；minisign 的 ED 签名先对文件做 BLAKE2b-512（hashlib 自带）。
"""
import base64
import hashlib
import json
import os
import sys

# ---------------------------------------------------------------- Ed25519（RFC 8032 第 6 节）

P = 2**255 - 19
D = -121665 * pow(121666, P - 2, P) % P
Q = 2**252 + 27742317777372353535851937790883648493
SQRT_M1 = pow(2, (P - 1) // 4, P)


def _add(a, b):
    A = (a[1] - a[0]) * (b[1] - b[0]) % P
    B = (a[1] + a[0]) * (b[1] + b[0]) % P
    C = 2 * a[3] * b[3] * D % P
    Dd = 2 * a[2] * b[2] % P
    E, F, G, H = B - A, Dd - C, Dd + C, B + A
    return (E * F, G * H, F * G, E * H)


def _mul(s, pt):
    acc = (0, 1, 1, 0)
    while s > 0:
        if s & 1:
            acc = _add(acc, pt)
        pt = _add(pt, pt)
        s >>= 1
    return acc


def _equal(a, b):
    return (a[0] * b[2] - b[0] * a[2]) % P == 0 and (a[1] * b[2] - b[1] * a[2]) % P == 0


def _recover_x(y, sign):
    if y >= P:
        return None
    x2 = (y * y - 1) * pow(D * y * y + 1, P - 2, P)
    if x2 == 0:
        return None if sign else 0
    x = pow(x2, (P + 3) // 8, P)
    if (x * x - x2) % P != 0:
        x = x * SQRT_M1 % P
    if (x * x - x2) % P != 0:
        return None
    if (x & 1) != sign:
        x = P - x
    return x


def _decompress(s):
    if len(s) != 32:
        return None
    y = int.from_bytes(s, "little")
    sign = y >> 255
    y &= (1 << 255) - 1
    x = _recover_x(y, sign)
    return None if x is None else (x, y, 1, x * y % P)


_GY = 4 * pow(5, P - 2, P) % P
_GX = _recover_x(_GY, 0)
BASE = (_GX, _GY, 1, _GX * _GY % P)


def ed25519_verify(public, msg, sig):
    if len(public) != 32 or len(sig) != 64:
        return False
    a = _decompress(public)
    r = _decompress(sig[:32])
    if not a or not r:
        return False
    s = int.from_bytes(sig[32:], "little")
    if s >= Q:
        return False
    h = int.from_bytes(hashlib.sha512(sig[:32] + public + msg).digest(), "little") % Q
    return _equal(_mul(s, BASE), _add(r, _mul(h, a)))


# ---------------------------------------------------------------- minisign

def fail(msg):
    print(f"更新包签名不对：{msg}", file=sys.stderr)
    sys.exit(1)


def lines_of(b64text):
    """Tauri 的公钥和 .sig 都是把 minisign 的文本整个再 base64 一次"""
    try:
        return base64.b64decode(b64text.strip()).decode("utf-8").splitlines()
    except Exception:
        fail("不是 Tauri 的 base64 格式")


def main():
    if len(sys.argv) < 3:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    path, version = sys.argv[1], sys.argv[2]
    conf = sys.argv[3] if len(sys.argv) > 3 else os.path.join(os.path.dirname(__file__), "..", "apps", "desktop", "src-tauri", "tauri.conf.json")
    pub_b64 = json.load(open(conf, encoding="utf-8"))["plugins"]["updater"]["pubkey"]
    pub_lines = lines_of(pub_b64)
    pk = base64.b64decode(pub_lines[1])
    if len(pk) != 42 or pk[:2] != b"Ed":
        fail("公钥格式不对")
    key_id, public = pk[2:10], pk[10:]

    sig_lines = lines_of(open(path + ".sig", encoding="utf-8").read())
    if len(sig_lines) < 4 or not sig_lines[2].startswith("trusted comment: "):
        fail(".sig 的格式不对")
    raw = base64.b64decode(sig_lines[1])
    if len(raw) != 74:
        fail(".sig 里的签名长度不对")
    alg, sig_key, sig = raw[:2], raw[2:10], raw[10:]
    if sig_key != key_id:
        fail(f"签名用的钥匙（{sig_key[::-1].hex().upper()}）不是 tauri.conf.json 里的公钥（{key_id[::-1].hex().upper()}）")
    with open(path, "rb") as f:
        if alg == b"ED":
            h = hashlib.blake2b(digest_size=64)
            for chunk in iter(lambda: f.read(1 << 20), b""):
                h.update(chunk)
            msg = h.digest()
        elif alg == b"Ed":
            msg = f.read()
        else:
            fail("不认识的签名算法")
    if not ed25519_verify(public, msg, sig):
        fail("文件内容和签名对不上")
    trusted = sig_lines[2][len("trusted comment: "):]
    if not ed25519_verify(public, sig + trusted.encode("utf-8"), base64.b64decode(sig_lines[3])):
        fail("受信任的注释和签名对不上")
    fields = dict(f.split(":", 1) for f in trusted.split("\t") if ":" in f)
    if fields.get("version", "").lstrip("v") != version.lstrip("v"):
        fail(f"签名里的版本是 {fields.get('version') or '（没有）'}，要发的是 {version}")
    if fields.get("file") != os.path.basename(path):
        fail(f"签名里的文件名是 {fields.get('file')}，这个文件叫 {os.path.basename(path)}")
    print(f"签名核对通过：{os.path.basename(path)}（版本 {version}，钥匙 {key_id[::-1].hex().upper()}）")


if __name__ == "__main__":
    main()
