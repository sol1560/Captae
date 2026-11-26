#!/usr/bin/env python3
"""汇总打进安装包的 Rust、npm 和 Android 依赖的许可证，生成 licenses/rust-crates.md、licenses/rust-crates-mobile.md、licenses/npm-packages.md
和 licenses/android-libraries.md。

不联网、不装额外工具：
  - Rust：`cargo tree --offline -e normal --target <目标>` 取 captae-desktop 和 captae-cli 在三个平台
    （macOS aarch64、Windows x86_64 MSVC、Linux x86_64 GNU）上真正编进去的包，合成一张表、注明哪些平台才有
    （windows-sys、webview2-com、webkit2gtk 这类只在一个平台上编进去；不含只在编译时、测试时用的），
    `cargo metadata --offline` 取每个包的许可证、仓库、作者，
    再到本机 cargo 缓存里的源码目录读它自带的许可证文件，摘出版权行。
  - 手机端：同样的做法，统计 captae-mobile 在 aarch64-apple-ios 和 aarch64-linux-android 上编进去的包
    （UniFFI 是 MPL-2.0，自带的许可证文件原样附上），写进 licenses/rust-crates-mobile.md，标出每个包用在哪个平台。
  - npm：apps/desktop/package-lock.json 里不是开发依赖的包（就是 dependencies 连同它们带进来的），
    版权行从 apps/desktop/node_modules 里各包的许可证文件摘。
  - Android App：apps/android 里跑 Gradle（init 脚本 scripts/android-license-poms.gradle）取 `releaseRuntimeClasspath`，也就是打进 APK 的
    Maven 依赖（Compose、CameraX、ZXing、JNA、backdrop……，含引擎模块带进来的），许可证、来源从各包的 POM 读（自己的 POM 没写就找父 POM）。
    这一步例外要联网：Gradle 缓存里多半只有 .module 元数据，POM 要从 Google Maven / Maven Central 下（只下 POM 文件，公开、免费）。
    Gradle 跑不通（没装 Android SDK、没联网）时跳过这一份，不改原来的文件。
    iOS App 不用第三方包（只有系统框架），文件里写一句说明。
许可证是「A OR B」的，有 MIT 就按 MIT、没有就按 Apache-2.0 用；按这两种用的，列出版权行 + 文末一份标准原文；
其余的（BSD、ISC、Zlib、MPL-2.0、Unicode-3.0、CDLA……）把这个包自带的许可证文件原样附在文末（相同的只附一份）。

用法：python3 scripts/gen-licenses.py      （依赖变了以后重新跑，结果提交进仓库）
"""
import json
import os
import re
import subprocess
import sys
from collections import OrderedDict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "licenses")
# 三个平台的安装包：(rustc 目标, 表里的叫法)。跑之前要 `cargo fetch --locked`，不然离线拿不到 Windows / Linux 专有 crate 的元数据
DESKTOP_TARGETS = [("aarch64-apple-darwin", "macOS"), ("x86_64-pc-windows-msvc", "Windows"), ("x86_64-unknown-linux-gnu", "Linux")]
OWN = {"captae-core", "captae-desktop", "captae-cli", "captae-mobile"}
# 手机端：iOS 真机和 Android arm64（模拟器的 aarch64-apple-ios-sim、x86_64-linux-android 编进去的包和它们一样）
MOBILE_TARGETS = [("aarch64-apple-ios", "iOS"), ("aarch64-linux-android", "Android")]
LICENSE_FILE = re.compile(r"(?i)^(licen[cs]e|copying|unlicense|notice)([-._].*)?$")
# 标准许可证文本里的占位版权行，不算版权人
PLACEHOLDER = re.compile(r"(?i)\[yyyy\]|\[name of copyright owner\]|<year>|<copyright holders?>|copyright notice|copyright \(c\) <|copyright holders and contributors|copyright owner")


def run(cmd, cwd=ROOT):
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if p.returncode != 0:
        sys.exit(f"运行失败：{' '.join(cmd)}\n{p.stderr.strip()}")
    return p.stdout


def split_top(expr, op):
    """按最外层的 AND / OR 切开（不切括号里面的）"""
    parts, depth, cur, toks = [], 0, [], re.findall(r"\(|\)|[^\s()]+", expr)
    for t in toks:
        if t == "(":
            depth += 1
        elif t == ")":
            depth -= 1
        if depth == 0 and t == op:
            parts.append(" ".join(cur))
            cur = []
        else:
            cur.append(t)
    parts.append(" ".join(cur))
    return [p.strip() for p in parts if p.strip()]


def strip_parens(e):
    e = e.strip()
    while e.startswith("(") and e.endswith(")"):
        e = e[1:-1].strip()
    return e


def choose(expr):
    """许可证表达式 → 我们按哪几种用（AND 的每一块各选一种）"""
    e = (expr or "").replace("/", " OR ")
    picks = []
    for part in split_top(e, "AND"):
        alts = [strip_parens(a) for a in split_top(strip_parens(part), "OR")]
        alts = [a for a in alts if a]
        if not alts:
            continue
        picks.append("MIT" if "MIT" in alts else "Apache-2.0" if "Apache-2.0" in alts else alts[0])
    return picks or ["（没写）"]


def license_files(d):
    try:
        names = sorted(os.listdir(d))
    except OSError:
        return []
    return [os.path.join(d, n) for n in names if LICENSE_FILE.match(n) and os.path.isfile(os.path.join(d, n))]


def read(p):
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def copyright_lines(files):
    seen = OrderedDict()
    for p in files:
        for line in read(p).splitlines():
            s = line.strip().strip("*#/").strip()
            # 只认真正的版权行：「Copyright …」「© …」「(c) 2018 …」；许可证条文里以 copyright / (c) 开头的句子不算
            real = re.match(r"(?i)^(copyright\b(?!\s+(license|notice|holder|owner|statement|and\b))|©|\(c\)\s*\d{4})", s)
            if real and not PLACEHOLDER.search(s) and len(s) < 200:
                seen[re.sub(r"\s+", " ", s)] = True
    return list(seen)


def md_cell(s):
    return s.replace("|", "\\|").replace("\n", " ")


def mit_body(text):
    i = text.find("Permission is hereby granted")
    return text[i:].strip() if i >= 0 else None


# ---------------- Rust ----------------
def tree(pkgs, target):
    """这几个包在这个目标上真正编进去的第三方包：{(名字, 版本)}"""
    want = set()
    for pkg in pkgs:
        out = run(["cargo", "tree", "--offline", "-p", pkg, "-e", "normal", "--target", target, "--prefix", "none", "-f", "{p}"])
        for line in out.splitlines():
            line = line.replace(" (*)", "").strip()
            if not line:
                continue
            name, ver = line.split(" ")[:2]
            if name not in OWN:
                want.add((name, ver.lstrip("v")))
    return want


def rust(want=None, where=None):
    """want：要列的包；where：{(名字, 版本): 用在哪个平台}（「平台」一列）"""
    meta = json.loads(run(["cargo", "metadata", "--offline", "--format-version", "1"]))
    by = {(p["name"], p["version"]): p for p in meta["packages"]}
    rows, verbatim, std_texts = [], OrderedDict(), {}
    for key in sorted(want, key=lambda k: (k[0].lower(), k[1])):
        p = by[key]
        d = os.path.dirname(p["manifest_path"])
        files = license_files(d)
        expr = p.get("license") or ("见 " + p["license_file"] if p.get("license_file") else "")
        picks = choose(p.get("license") or "")
        cr = copyright_lines(files)
        if not cr:
            authors = [re.sub(r"\s*<[^>]*>", "", a).strip() for a in p.get("authors") or []]
            cr = ["作者：" + "、".join(a for a in authors if a)] if any(authors) else []
        repo = p.get("repository") or p.get("homepage") or f"https://crates.io/crates/{key[0]}"
        rows.append((key[0], key[1], expr, " + ".join(picks), repo, cr, (where or {}).get(key, "")))
        # 标准文本：从现成的包里拿一份
        for f in files:
            t = read(f)
            if "MIT" not in std_texts and "MIT" in os.path.basename(f).upper() and mit_body(t):
                std_texts["MIT"] = mit_body(t)
            if "Apache-2.0" not in std_texts and "APACHE" in os.path.basename(f).upper() and "END OF TERMS AND CONDITIONS" in t and "Version 2.0" in t:
                std_texts["Apache-2.0"] = t[: t.find("END OF TERMS AND CONDITIONS") + len("END OF TERMS AND CONDITIONS")].strip()
        # 不是 MIT / Apache-2.0 的：自带的许可证文件原样附上（跳过它另外附的 MIT、Apache 原文）
        if set(picks) - {"MIT", "Apache-2.0"}:
            keep = [f for f in files if not re.search(r"(?i)(^|[-._])(mit|apache)([-._]|$)", os.path.basename(f))]
            if keep:
                for f in keep:
                    t = read(f).strip()
                    k = re.sub(r"\s+", " ", t)
                    verbatim.setdefault(k, {"text": t, "who": []})["who"].append(f"{key[0]} {key[1]}（{os.path.basename(f)}）")
            else:
                miss = " + ".join(x for x in picks if x not in ("MIT", "Apache-2.0"))
                k = "SPDX:" + miss
                verbatim.setdefault(k, {"text": f"这些包没有附许可证文件，许可证是 {miss}，标准文本见 https://spdx.org/licenses/{miss.split(' + ')[0]}.html ，版权归各自的作者（见上表）。", "who": []})["who"].append(f"{key[0]} {key[1]}")
    return rows, verbatim, std_texts


DESKTOP_HEAD = [
    "# 打进安装包的 Rust 依赖",
    "",
    "由 `scripts/gen-licenses.py` 生成（不要手改）：`captae-desktop` 和 `captae-cli` 在 macOS（aarch64）、Windows（x86_64 MSVC）、",
    "Linux（x86_64 GNU）上真正编进去的第三方 crate，合成一张表，「平台」一列写只在哪些平台上编进去（「全部」就是三个平台都有），",
]
MOBILE_HEAD = [
    "# 打进手机 App 的 Rust 依赖",
    "",
    "由 `scripts/gen-licenses.py` 生成（不要手改）：`captae-mobile`（iOS 的 xcframework、Android 的 libcaptae_mobile.so）",
    "在 iOS（aarch64-apple-ios）和 Android（aarch64-linux-android）上真正编进去的第三方 crate，「平台」一列写用在哪边。",
    "生成绑定的命令行（uniffi-bindgen，只在电脑上跑）不编进 App，不列。UniFFI（MPL-2.0）没有改过，源码见表里的仓库地址。",
]


def write_rust(rows, verbatim, std_texts, head=None, name="rust-crates.md"):
    from collections import Counter

    c = Counter(r[2] for r in rows)
    mobile = any(r[6] for r in rows)
    lines = (head or DESKTOP_HEAD) + [
        "不含只在编译时、测试时用的。许可证是「A OR B」的，Captae 有 MIT 就按 MIT、没有就按 Apache-2.0 用（「按」那一列）；",
        "按这两种用的，版权行列在表里、许可证原文在文末；其余许可证（BSD、ISC、Zlib、MPL-2.0、Unicode-3.0、CDLA-Permissive-2.0……）",
        "把这个 crate 自带的许可证文件原样附在文末。所有 crate 都没有改过，源码在各自的仓库和 crates.io 上对应的版本里",
        "（MPL-2.0 的几个也一样：没有改动，源码见表里的仓库地址）。",
        "",
        f"一共 {len(rows)} 个。许可证表达式的分布：",
        "",
    ]
    for lic, n in c.most_common():
        lines.append(f"- {lic or '（没写）'}：{n}")
    if mobile:
        lines += ["", "| crate | 版本 | 平台 | 许可证 | 按 | 来源 | 版权 |", "|---|---|---|---|---|---|---|"]
    else:
        lines += ["", "| crate | 版本 | 许可证 | 按 | 来源 | 版权 |", "|---|---|---|---|---|---|"]
    for crate, ver, expr, pick, repo, cr, plat in rows:
        mid = f" {plat} |" if mobile else ""
        lines.append(f"| {md_cell(crate)} | {ver} |{mid} {md_cell(expr)} | {md_cell(pick)} | {md_cell(repo)} | {md_cell('；'.join(cr) or '—')} |")
    lines += ["", "## MIT 许可原文", "", "（各 crate 的版权行见上表）", "", "```", std_texts.get("MIT", "（没找到）"), "```", ""]
    lines += ["## Apache License 2.0 原文", "", "```", std_texts.get("Apache-2.0", "（没找到）"), "```", ""]
    lines += ["## 其余许可证：各 crate 自带的许可证文件（原样）", ""]
    for v in verbatim.values():
        lines += ["### " + "、".join(v["who"]), "", "```", v["text"], "```", ""]
    with open(os.path.join(OUT, name), "w", encoding="utf-8") as f:
        f.write("\n".join(lines))


def per_target(pkgs, targets):
    """按平台取依赖，返回 (全部的包, {包: 「全部」或「macOS + Linux」这样的平台})"""
    per = {label: tree(pkgs, t) for t, label in targets}
    want = set().union(*per.values())
    where = {k: "全部" if all(k in per[l] for _, l in targets) else " + ".join(l for _, l in targets if k in per[l]) for k in want}
    return want, where


def rust_desktop():
    return rust(*per_target(["captae-desktop", "captae-cli"], DESKTOP_TARGETS))


def rust_mobile():
    per = {label: tree(["captae-mobile"], t) for t, label in MOBILE_TARGETS}
    want = set().union(*per.values())
    where = {k: " + ".join(label for _, label in MOBILE_TARGETS if k in per[label]) for k in want}
    return rust(want, where)


# ---------------- npm ----------------
def npm():
    app = os.path.join(ROOT, "apps", "desktop")
    lock = json.load(open(os.path.join(app, "package-lock.json"), encoding="utf-8"))
    rows, texts = [], OrderedDict()
    for path, info in sorted(lock["packages"].items()):
        if not path or info.get("dev") or info.get("devOptional"):
            continue
        name = path.split("node_modules/")[-1]
        d = os.path.join(app, path)
        files = license_files(d)
        cr = copyright_lines(files)
        lic = info.get("license") or ""
        picks = choose(lic)
        repo = ""
        pj = os.path.join(d, "package.json")
        if os.path.exists(pj):
            r = json.load(open(pj, encoding="utf-8")).get("repository")
            repo = (r.get("url") if isinstance(r, dict) else r) or ""
            repo = re.sub(r"^git\+|\.git$", "", repo).replace("git://", "https://")
        rows.append((name, info.get("version", ""), lic, " + ".join(picks), repo or f"https://www.npmjs.com/package/{name}", cr))
        for f in files:
            t = read(f)
            if "MIT" in picks and "MIT" not in texts and mit_body(t):
                texts["MIT"] = mit_body(t)
            if "0BSD" in picks and "0BSD" not in texts:
                texts["0BSD"] = t.strip()
    return rows, texts


def write_npm(rows, texts):
    lines = [
        "# 打进界面的 npm 依赖",
        "",
        "由 `scripts/gen-licenses.py` 生成（不要手改）：`apps/desktop/package-lock.json` 里不是开发依赖的包，",
        "也就是 `package.json` 的 `dependencies` 连同它们带进来的。桌面端界面（嵌在 Captae 程序里）和插件的工具栏面板",
        "（`extension/popup/`）都用这些。开发依赖（TypeScript、Vite、vite-plugin-singlefile 等）只在构建时用，不进安装包，不列。",
        "许可证是「A OR B」的按 MIT 用。",
        "",
        "| 包 | 版本 | 许可证 | 按 | 来源 | 版权 |",
        "|---|---|---|---|---|---|",
    ]
    for name, ver, lic, pick, repo, cr in rows:
        lines.append(f"| {md_cell(name)} | {ver} | {md_cell(lic)} | {md_cell(pick)} | {md_cell(repo)} | {md_cell('；'.join(cr) or '—')} |")
    for k, t in texts.items():
        lines += ["", f"## {k} 许可原文", "", "（版权行见上表）" if k == "MIT" else "", "```", t, "```"]
    lines.append("")
    with open(os.path.join(OUT, "npm-packages.md"), "w", encoding="utf-8") as f:
        f.write("\n".join(lines))


# ---------------- Android App（Gradle） ----------------
ANDROID = os.path.join(ROOT, "apps", "android")
GRADLE_CACHE = os.path.join(os.path.expanduser("~"), ".gradle", "caches", "modules-2", "files-2.1")
# 本地 Maven 仓库（scripts/mobile/build-rust-android.sh 放的 rustls-platform-verifier）
LOCAL_MAVEN = os.path.join(ROOT, "target", "mobile", "maven")
# POM 里没写许可证的：按上游项目声明的许可证记
# POM 里列了几个许可证时默认是「都适用」（写成 A + B，比如 CameraX core 里带了 BSD 许可的 libyuv）；二选一的在这里写明
KNOWN_LICENSE = {
    "org.rustls:rustls-platform-verifier": ("MIT OR Apache-2.0", "https://github.com/rustls/rustls-platform-verifier"),
    # POM 的说明：「You can freely decide which license you want to apply」
    "net.java.dev.jna:jna": ("LGPL-2.1-or-later OR Apache-2.0", "https://github.com/java-native-access/jna"),
}
LICENSE_NAMES = [
    (re.compile(r"(?i)apache.*2"), "Apache-2.0"),
    (re.compile(r"(?i)^mit\b|mit license"), "MIT"),
    (re.compile(r"(?i)lgpl.*2\.1|lesser general public license.*2\.1"), "LGPL-2.1-or-later"),
    (re.compile(r"(?i)eclipse public license.*2|epl.*2"), "EPL-2.0"),
    (re.compile(r"(?i)bsd.*3|new bsd|revised bsd"), "BSD-3-Clause"),
    (re.compile(r"(?i)bsd.*2|simplified bsd"), "BSD-2-Clause"),
]


def spdx(name):
    for rx, sp in LICENSE_NAMES:
        if rx.search(name or ""):
            return sp
    return name or ""


# Gradle init 脚本找到的 POM：「group:artifact:version」→ 文件路径（android() 填）
POMS = {}


def find_pom(g, a, v):
    p = POMS.get(f"{g}:{a}:{v}")
    if p and os.path.exists(p):
        return p
    # 本地 Maven 仓库（rustls-platform-verifier）
    p = os.path.join(LOCAL_MAVEN, *g.split("."), a, v, f"{a}-{v}.pom")
    return p if os.path.exists(p) else None


def pom_tag(xml, tag):
    m = re.search(rf"<{tag}>\s*(.*?)\s*</{tag}>", xml, re.S)
    return m.group(1).strip() if m else ""


def pom_info(g, a, v, depth=0):
    """(许可证名字列表, 项目地址, packaging)。自己没写许可证 / 地址的往父 POM 找"""
    p = find_pom(g, a, v)
    if not p:
        return [], "", ""
    xml = re.sub(r"<!--.*?-->", "", read(p), flags=re.S)
    block = re.search(r"<licenses>(.*?)</licenses>", xml, re.S)
    lic = re.findall(r"<license>.*?<name>\s*(.*?)\s*</name>.*?</license>", block.group(1), re.S) if block else []
    scm = re.search(r"<scm>.*?<url>\s*(.*?)\s*</url>.*?</scm>", xml, re.S)
    url = (scm.group(1) if scm else "") or pom_tag(re.sub(r"<(parent|scm|licenses|developers|organization|distributionManagement|dependencies|build|profiles)>.*?</\1>", "", xml, flags=re.S), "url")
    packaging = pom_tag(re.sub(r"<(parent|dependencies|dependencyManagement|build)>.*?</\1>", "", xml, flags=re.S), "packaging")
    if (not lic or not url) and depth < 5:
        par = re.search(r"<parent>(.*?)</parent>", xml, re.S)
        if par:
            pl, pu, _ = pom_info(pom_tag(par.group(1), "groupId"), pom_tag(par.group(1), "artifactId"), pom_tag(par.group(1), "version"), depth + 1)
            lic = lic or pl
            url = url or pu
    return lic, url, packaging


def android():
    """App 打进 APK 的 Maven 依赖：[(group:artifact, 版本, 许可证, 来源)]。拿不到时 None"""
    gradlew = os.path.join(ANDROID, "gradlew")
    if not os.path.exists(gradlew):
        return None
    init = os.path.join(ROOT, "scripts", "android-license-poms.gradle")
    p = subprocess.run([gradlew, "-q", "--no-configuration-cache", "--init-script", init, ":app:captaeLicensePoms"], cwd=ANDROID, capture_output=True, text=True)
    if p.returncode != 0:
        print(f"跳过 Android：Gradle 没跑通（没装 Android SDK？没联网？先在 apps/android 编一次 App）\n{p.stderr.strip()[-600:]}", file=sys.stderr)
        return None
    seen = OrderedDict()
    for line in p.stdout.splitlines():
        if line.startswith("CAPTAE-DEP "):
            g, a, v = line.split(" ", 1)[1].strip().split(":")
            seen[(g, a)] = v
        elif line.startswith("CAPTAE-POM "):
            k, path = line.split(" ", 1)[1].strip().split("|", 1)
            POMS[k] = path
    rows, unknown = [], []
    for (g, a), v in sorted(seen.items()):
        names, url, packaging = pom_info(g, a, v)
        if packaging == "pom":
            # BOM、只有 POM 的聚合包：不带代码
            continue
        key = f"{g}:{a}"
        if key in KNOWN_LICENSE:
            lic, url = KNOWN_LICENSE[key][0], url or KNOWN_LICENSE[key][1]
        else:
            lic = " + ".join(OrderedDict((spdx(n), 1) for n in names)) if names else ""
        if not lic:
            unknown.append(f"{key}:{v}")
            lic = "（POM 里没写，待查）"
        url = url or (f"https://maven.google.com/web/index.html#{g}:{a}:{v}" if g.startswith(("androidx.", "com.google.android")) else f"https://central.sonatype.com/artifact/{g}/{a}/{v}")
        rows.append((key, v, lic, re.sub(r"\.git$", "", url.replace("scm:git:", "").replace("git://", "https://"))))
    if unknown:
        print("Android：这些依赖的 POM 里没写许可证，表里标了「待查」：" + "、".join(unknown), file=sys.stderr)
    return rows


def write_android(rows):
    lines = [
        "# Android App 打进 APK 的第三方库",
        "",
        "由 `scripts/gen-licenses.py` 生成（不要手改）：`apps/android` 的 App 在 `releaseRuntimeClasspath` 里的 Maven 依赖（Gradle init 脚本",
        "`scripts/android-license-poms.gradle` 列出，App、玻璃组件 `:glass`、下载引擎 `:engine` 带进来的都在里面），许可证和来源从各包的 POM（和父 POM）读。",
        "只有 POM、不带代码的 BOM 不列；只在编译时、测试时用的不列。版本以 Gradle 解析出来的为准。",
        "",
        "- 引擎里打包的 Python、yt-dlp、QuickJS、FFmpeg 的许可证在 `licenses/python/`、`yt-dlp-android/`、`quickjs/`、`ffmpeg-android/`；Rust 核心（`libcaptae_mobile.so`）的在 `rust-crates-mobile.md`。",
        "- 「A OR B」是二选一：JNA 是 LGPL-2.1-or-later OR Apache-2.0，按 Apache-2.0 用。「A + B」是都适用：CameraX core 里带了 libyuv 的代码（BSD 许可）。",
        "- **iOS App 不用任何第三方 Swift 包**：只用系统框架（SwiftUI、VisionKit、Network、PhotosUI……），Rust 核心的依赖见 `rust-crates-mobile.md`。",
        "",
        "| 库 | 版本 | 许可证 | 来源 |",
        "|---|---|---|---|",
    ]
    for key, ver, lic, url in rows:
        lines.append(f"| {md_cell(key)} | {ver} | {md_cell(lic)} | {md_cell(url)} |")
    lines.append("")
    with open(os.path.join(OUT, "android-libraries.md"), "w", encoding="utf-8") as f:
        f.write("\n".join(lines))


if __name__ == "__main__":
    r = rust_desktop()
    write_rust(*r)
    m = rust_mobile()
    write_rust(*m, head=MOBILE_HEAD, name="rust-crates-mobile.md")
    n = npm()
    write_npm(*n)
    a = android()
    if a is not None:
        write_android(a)
    print(f"licenses/rust-crates.md：{len(r[0])} 个 crate，原样附上的许可证文本 {len(r[1])} 份")
    print(f"licenses/rust-crates-mobile.md：{len(m[0])} 个 crate，原样附上的许可证文本 {len(m[1])} 份")
    print(f"licenses/npm-packages.md：{len(n[0])} 个包")
    print(f"licenses/android-libraries.md：{len(a)} 个库" if a is not None else "licenses/android-libraries.md：跳过（见上面的原因）")
