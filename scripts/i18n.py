#!/usr/bin/env python3
"""多语言词表的检查、导出和导入（规矩见 docs/I18N.md）。

三处界面文字，写法都是「代码里写中文原文，按原文查译文」：
  ui    桌面端界面和插件工具栏面板  apps/desktop/src/**/*.ts(x) 里的 t("…") tx("…") N_("…")
                                     词表 apps/desktop/src/i18n/locales/<语言>/<分册>.json
  rust  核心库、命令行、桌面端外壳  **/*.rs 里的 tr!("…") tr_key!("…")
                                     词表 crates/captae-core/locales/<语言>/<分册>.json
  ext   插件网页里的部件、后台      extension/**/*.js 里的 tr('…') N_('…')
                                     词表 extension/lib/i18n/<语言>.json
中文（简体）是原文，没有词表。其他语言：en zh-TW ja ko es fr de。

用法：
  python3 scripts/i18n.py check [--strict] [--area ui|rust|ext] [--lang L] [-v]
      源码里用到的原文，每种语言的词表里有没有；占位符 {名字}、标签 <b>…</b> 对不对得上；复数写法；没用到的旧条目。
      不带 --strict：只有英文必须齐（其他语言缺的只报数量，运行时退回英文）；带 --strict：所有语言都要齐，发版前用。
  python3 scripts/i18n.py export L [--area A] [--part P1,P2] [--all] [--out work/i18n/todo/L.jsonl]
      导出 L 缺的条目（--all：全部，用来复查），每行一个 JSON：
      {"id", "area", "part", "zh"（原文）, "ctx"（「§」后面的说明）, "en"（英文）, "ph"（占位符）, "where"（第一次用到的位置）}
      同时写一份 <out>.map.json（编号 → 哪个词表的哪条原文），导入时用。
  python3 scripts/i18n.py tidy [--area A] [--dry-run]
      整理词表：英文里同一句放在几个分册的，挪到共用的分册（界面 common、Rust core）；其他语言的条目放到和英文同一个分册；
      源码里不用了、英文词表里也没有的旧条目删掉。别的会话、子代理加过条目以后跑一次。
  python3 scripts/i18n.py quotes [--lang L] [-v]
      句子里用「」引用的界面名字（按钮、页面、设置项），译文要和那个名字自己的译文一样。列出对不上的。
  python3 scripts/i18n.py import L FILE [--map MAP]
      FILE 每行 {"id": 12, "t": "译文"}，带数量的写 {"id": 12, "t": {"one": "…", "other": "…"}}。
      MAP 默认是 FILE 旁边、去掉 .done 的同名 .map.json，或者 work/i18n/todo/L.jsonl.map.json。
      占位符对不上的不写进去，列出来。写回时按英文词表的顺序排。
"""
import argparse
import json
import os
import re
import sys
from collections import OrderedDict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LANGS = ["en", "zh-TW", "ja", "ko", "es", "fr", "de"]
PLURAL_KEYS = {"zero", "one", "two", "few", "many", "other"}

AREAS = OrderedDict(
    ui=dict(
        dirs=["apps/desktop/src"],
        exts=(".ts", ".tsx"),
        skip=("apps/desktop/src/components/ui/", "apps/desktop/src/dev/", "apps/desktop/src/i18n/locales/", "/mock.ts"),
        calls=r"\b(?:t|tx|N_)\(\s*",
        lang="ts",
        catalog="apps/desktop/src/i18n/locales/{lang}/{part}.json",
    ),
    rust=dict(
        dirs=["crates/captae-core/src", "crates/captae-cli/src", "crates/captae-remote/src", "crates/captae-mobile/src", "apps/desktop/src-tauri/src"],
        exts=(".rs",),
        skip=(),
        calls=r"\b(?:tr|tr_key)!\(\s*",
        lang="rust",
        catalog="crates/captae-core/locales/{lang}/{part}.json",
    ),
    ext=dict(
        dirs=["extension"],
        exts=(".js",),
        skip=("extension/popup/",),
        calls=r"\b(?:tr|tAny|N_)\(\s*",
        lang="js",
        catalog="extension/lib/i18n/{lang}.json",
    ),
)

# ------------------------------------------------------------------ 读源码


def strip_comments(src, lang):
    """注释换成空格（位置、换行不变），字符串原样留着"""
    out = list(src)
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            for k in range(i, j):
                if src[k] != "\n":
                    out[k] = " "
            i = j
        elif c == '"' or c == "`" or (c == "'" and lang != "rust"):
            i = skip_string(src, i, c)
        elif c == "'" and lang == "rust":
            # Rust：'a'、'\n'、'\u{1F600}' 是字符；'static 是生命周期
            m = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\.|[^\\'])'", src[i:])
            i += len(m.group(0)) if m else 1
        elif lang == "rust" and c == "r" and re.match(r'r#*"', src[i:]) and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            m = re.match(r'r(#*)"', src[i:])
            end = '"' + m.group(1)
            j = src.find(end, i + len(m.group(0)))
            i = n if j < 0 else j + len(end)
        else:
            i += 1
    return "".join(out)


def skip_string(src, i, q):
    j = i + 1
    while j < len(src):
        if src[j] == "\\":
            j += 2
            continue
        if src[j] == q:
            return j + 1
        j += 1
    return j


def unescape(s, lang):
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        nx = s[i + 1] if i + 1 < len(s) else ""
        if nx == "\n" and lang == "rust":
            # Rust 的续行：反斜杠、换行和下一行开头的空白都去掉
            i += 2
            while i < len(s) and s[i] in " \t\n\r":
                i += 1
            continue
        if nx == "u" and i + 2 < len(s) and s[i + 2] == "{":
            j = s.index("}", i)
            out.append(chr(int(s[i + 3 : j], 16)))
            i = j + 1
            continue
        if nx == "u":
            out.append(chr(int(s[i + 2 : i + 6], 16)))
            i += 6
            continue
        if nx == "x":
            out.append(chr(int(s[i + 2 : i + 4], 16)))
            i += 4
            continue
        out.append({"n": "\n", "t": "\t", "r": "\r", "0": "\0"}.get(nx, nx))
        i += 2
    return "".join(out)


def scan(area):
    """源码里用到的原文 → [(文件, 行号)]，按第一次出现的顺序"""
    a = AREAS[area]
    used = OrderedDict()
    lit = r'"((?:[^"\\]|\\.)*)"' if a["lang"] != "js" else r"""(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)')"""
    pat = re.compile(a["calls"] + lit, re.S)
    for d in a["dirs"]:
        for base, dirs, files in os.walk(os.path.join(ROOT, d)):
            dirs.sort()
            for f in sorted(files):
                if not f.endswith(a["exts"]):
                    continue
                path = os.path.join(base, f)
                rel = os.path.relpath(path, ROOT)
                if any(s in rel for s in a["skip"]):
                    continue
                src = strip_comments(open(path, encoding="utf-8").read(), a["lang"])
                for m in pat.finditer(src):
                    raw = next(g for g in m.groups() if g is not None)
                    key = unescape(raw, a["lang"])
                    line = src.count("\n", 0, m.start()) + 1
                    used.setdefault(key, []).append(f"{rel}:{line}")
    return used


# ------------------------------------------------------------------ 词表


def parts_of(area):
    a = AREAS[area]
    if "{part}" not in a["catalog"]:
        return ["ext"]
    d = os.path.join(ROOT, os.path.dirname(a["catalog"].format(lang="en", part="x")))
    return sorted(f[:-5] for f in os.listdir(d) if f.endswith(".json"))


def catalog_path(area, lang, part):
    return os.path.join(ROOT, AREAS[area]["catalog"].format(lang=lang, part=part))


def load_part(area, lang, part):
    p = catalog_path(area, lang, part)
    if not os.path.exists(p):
        return OrderedDict()
    return json.load(open(p, encoding="utf-8"), object_pairs_hook=OrderedDict)


def save_part(area, lang, part, d):
    p = catalog_path(area, lang, part)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w", encoding="utf-8") as f:
        json.dump(d, f, ensure_ascii=False, indent=2)
        f.write("\n")


def source(key):
    return key.split("§", 1)[0]


def context(key):
    return key.split("§", 1)[1] if "§" in key else ""


PH = re.compile(r"\{(\w+)\}")
TAG = re.compile(r"<(\w+)>")


def marks(s, area):
    """占位符和标签（标签只有界面的 tx 用）"""
    ph = sorted(set(PH.findall(s)))
    tags = sorted(set(TAG.findall(s))) if area == "ui" else []
    return ph, tags


def forms(v):
    return list(v.values()) if isinstance(v, dict) else [v]


def problems_of(area, key, v):
    out = []
    if isinstance(v, dict):
        if "{n}" not in key:
            out.append("按数量分了几种说法，但原文里没有 {n}")
        if "other" not in v or not set(v) <= PLURAL_KEYS:
            out.append(f"复数写法要有 other，只能用 {sorted(PLURAL_KEYS)}")
        if not all(isinstance(x, str) for x in v.values()):
            out.append("复数的每种说法都要是字符串")
    elif not isinstance(v, str):
        out.append("译文要是字符串，或者按数量分的 {one, other}")
        return out
    want = marks(source(key), area)
    for f in forms(v):
        if isinstance(f, str) and marks(f, area) != want:
            got = marks(f, area)
            out.append(f"占位符 / 标签对不上：原文 {want[0] + want[1]}，译文 {got[0] + got[1]}")
            break
        if isinstance(f, str) and not f.strip():
            out.append("译文是空的")
            break
    return out


# ------------------------------------------------------------------ 插件面板用到的文件

SRC = os.path.join(ROOT, "apps", "desktop", "src")
POPUP_ENTRY = os.path.join(SRC, "extension-popup", "main.tsx")
POPUP_PARTS = ("common", "popup")
IMPORT = re.compile(r"""(?:from\s+|import\s*\(\s*)["']((?:\.|@)/[^"']+)["']""")


def resolve(base_dir, spec):
    p = os.path.join(SRC, spec[2:]) if spec.startswith("@/") else os.path.normpath(os.path.join(base_dir, spec))
    for c in (p, p + ".ts", p + ".tsx", os.path.join(p, "index.ts"), os.path.join(p, "index.tsx")):
        if os.path.isfile(c) and c.endswith((".ts", ".tsx")):
            return c
    return None


def popup_files():
    """插件工具栏面板从入口一路 import 到的源码（不含 affectUI 组件、i18n 自己、演示数据）。面板只加载 common、popup 两个分册，
    这些文件里用到的原文英文要在这两个分册里"""
    seen, todo = set(), [POPUP_ENTRY]
    while todo:
        f = todo.pop()
        if f in seen:
            continue
        seen.add(f)
        for spec in IMPORT.findall(open(f, encoding="utf-8").read()):
            r = resolve(os.path.dirname(f), spec)
            rel = os.path.relpath(r, ROOT) if r else ""
            if r and not any(s in rel for s in ("/components/ui/", "/i18n/", "/dev/", "mock")):
                todo.append(r)
    return {os.path.relpath(f, ROOT) for f in seen}


def popup_problems(used):
    files = popup_files()
    have = set()
    for p in POPUP_PARTS:
        have |= set(load_part("ui", "en", p))
    out = []
    for k, places in used.items():
        hit = [w for w in places if w.split(":")[0] in files]
        if hit and k not in have:
            out.append(f"「{k}」（{hit[0]}）插件面板也用到，英文要放在 common 或 popup 分册（面板只加载这两个）")
    return out


# ------------------------------------------------------------------ check


def check(args):
    bad = False
    areas = [args.area] if args.area else list(AREAS)
    langs = [args.lang] if args.lang else LANGS
    for area in areas:
        used = scan(area)
        parts = parts_of(area)
        print(f"\n== {area}：源码里 {len(used)} 句原文，分册 {', '.join(parts)}")
        en_parts = {p: load_part(area, "en", p) for p in parts}
        for lang in langs:
            merged, where, dup = {}, {}, []
            invalid = []
            for p in parts:
                try:
                    d = load_part(area, lang, p)
                except json.JSONDecodeError as e:
                    invalid.append(f"{p}.json 不是合法的 JSON：{e}")
                    continue
                for k, v in d.items():
                    if k in merged and merged[k] != v:
                        dup.append(f"「{k}」在 {where[k]} 和 {p} 里译得不一样")
                    merged[k], where[k] = v, p
            missing = [k for k in used if k not in merged]
            unused = [k for k in merged if k not in used]
            wrong = [(k, msg) for k, v in merged.items() if k in used for msg in problems_of(area, k, v)]
            # 英文以外的语言：条目要放在和英文同一个分册里
            misplaced = []
            if lang != "en":
                en_where = {k: p for p, d in en_parts.items() for k in d}
                misplaced = [f"「{k}」在 {where[k]}，英文在 {en_where[k]}" for k in merged if k in en_where and where[k] != en_where[k]]
            must = lang == "en" or args.strict
            if area == "ui" and lang == "en":
                invalid += popup_problems(used)
            fail = invalid or wrong or (must and (missing or dup))
            bad = bad or bool(fail)
            state = "不对" if fail else ("缺" if missing else "齐")
            print(f"  {lang:6s} {state}：{len(merged)} 条，缺 {len(missing)}，没用到 {len(unused)}，有问题 {len(wrong) + len(invalid)}，译文不一致 {len(dup)}，分册不对 {len(misplaced)}")
            show = args.verbose or must
            for x in invalid:
                print("    ✗", x)
            for k, msg in wrong[:50]:
                print(f"    ✗ 「{k}」：{msg}")
            if show:
                for k in missing[: (10**9 if args.verbose else 40)]:
                    print(f"    缺「{k}」（{used[k][0]}）")
                if len(missing) > 40 and not args.verbose:
                    print(f"    ……还缺 {len(missing) - 40} 条（-v 看全部）")
                for x in dup[:20]:
                    print("    ✗", x)
            if args.verbose:
                for k in unused:
                    print(f"    没用到「{k}」（{where[k]}）")
                for x in misplaced:
                    print("    分册不对", x)
    print("\n" + ("有要修的问题" if bad else "通过"))
    return 1 if bad else 0


# ------------------------------------------------------------------ export / import


def export(args):
    lang = args.lang_pos
    out = args.out or os.path.join(ROOT, "work", "i18n", "todo", f"{lang}.jsonl")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    rows, mapping, i = [], {}, 0
    for area in [args.area] if args.area else list(AREAS):
        used = scan(area)
        for p in parts_of(area):
            if args.part and p not in args.part.split(","):
                continue
            en = load_part(area, "en", p)
            have = load_part(area, lang, p) if lang != "en" else OrderedDict()
            for k, v in en.items():
                if k not in used:
                    continue
                if k in have and not args.all:
                    continue
                i += 1
                ph, tags = marks(source(k), area)
                row = OrderedDict(id=i, area=area, part=p, zh=source(k))
                if context(k):
                    row["ctx"] = context(k)
                row["en"] = v
                if ph or tags:
                    row["ph"] = ph + [f"<{t}>" for t in tags]
                row["where"] = used[k][0]
                if k in have:
                    row["now"] = have[k]
                rows.append(row)
                mapping[i] = [area, p, k]
    with open(out, "w", encoding="utf-8") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    json.dump(mapping, open(out + ".map.json", "w", encoding="utf-8"), ensure_ascii=False)
    print(f"{lang}：导出 {len(rows)} 条 → {os.path.relpath(out, ROOT)}（编号表 {os.path.relpath(out, ROOT)}.map.json）")
    return 0


def do_import(args):
    lang = args.lang_pos
    mp = args.map
    if not mp:
        guess = [re.sub(r"(\.done)?(-\d+)?\.jsonl$", ".jsonl.map.json", args.file), os.path.join(ROOT, "work", "i18n", "todo", f"{lang}.jsonl.map.json")]
        mp = next((g for g in guess if os.path.exists(g)), guess[-1])
    mapping = json.load(open(mp, encoding="utf-8"))
    touched, ok, rejected = {}, 0, []
    for n, line in enumerate(open(args.file, encoding="utf-8"), 1):
        line = line.strip()
        if not line:
            continue
        try:
            r = json.loads(line)
        except json.JSONDecodeError as e:
            rejected.append(f"第 {n} 行不是 JSON：{e}")
            continue
        m = mapping.get(str(r.get("id")))
        if not m:
            rejected.append(f"第 {n} 行：编号 {r.get('id')} 不在编号表里")
            continue
        area, part, key = m
        v = r.get("t")
        msgs = problems_of(area, key, v)
        if msgs:
            rejected.append(f"#{r['id']}「{key}」：{'；'.join(msgs)}")
            continue
        touched.setdefault((area, part), {})[key] = v
        ok += 1
    for (area, part), new in touched.items():
        en = load_part(area, "en", part)
        cur = load_part(area, lang, part)
        cur.update(new)
        ordered = OrderedDict((k, cur[k]) for k in en if k in cur)
        for k in cur:
            if k not in ordered:
                ordered[k] = cur[k]
        save_part(area, lang, part, ordered)
    print(f"{lang}：写入 {ok} 条（{len(touched)} 个分册），没写入 {len(rejected)} 条")
    for x in rejected:
        print("  ✗", x)
    return 1 if rejected else 0


# ------------------------------------------------------------------ tidy


SHARED = {"ui": "common", "rust": "core"}


def tidy(args):
    write = not args.dry_run
    for area in [args.area] if args.area else list(AREAS):
        used = scan(area)
        parts = parts_of(area)
        en = {p: load_part(area, "en", p) for p in parts}
        # 英文：源码里已经不用的删掉（原文改了，旧的那句就没用了）
        for p in parts:
            for k in [k for k in en[p] if k not in used]:
                del en[p][k]
                print(f"  英文删掉用不到的「{k}」（{p}）")
        if write:
            for p in parts:
                save_part(area, "en", p, en[p])
        # 英文：同一句在几个分册里的，挪到共用的分册（界面的插件面板只加载 common 和 popup，几个页面都用的要在 common）
        if area in SHARED:
            shared = SHARED[area]
            where = {}
            for p in parts:
                for k in en[p]:
                    where.setdefault(k, []).append(p)
            for k, ps in where.items():
                if len(ps) < 2:
                    continue
                vals = {json.dumps(en[p][k], ensure_ascii=False) for p in ps}
                if len(vals) > 1:
                    print(f"  ✗ 英文「{k}」在 {', '.join(ps)} 里译得不一样，没有动，先改成一样")
                    continue
                v = en[ps[0]][k]
                for p in ps:
                    del en[p][k]
                en[shared][k] = v
                print(f"  英文「{k}」在 {', '.join(ps)} 里都有，挪到 {shared}")
            if write:
                for p in parts:
                    save_part(area, "en", p, en[p])
        en_where = {k: p for p in parts for k in en[p]}
        for lang in LANGS[1:]:
            cur = {p: load_part(area, lang, p) for p in parts}
            new = {p: OrderedDict() for p in parts}
            moved = dropped = 0
            for p in parts:
                for k, v in cur[p].items():
                    if k not in en_where and k not in used:
                        dropped += 1
                        if args.verbose:
                            print(f"  {lang} 删掉用不到的「{k}」")
                        continue
                    tgt = en_where.get(k, p)
                    if tgt != p:
                        moved += 1
                    if k in new[tgt] and new[tgt][k] != v:
                        print(f"  ✗ {lang}「{k}」在几个分册里译得不一样，留了 {tgt} 里的：{json.dumps(new[tgt][k], ensure_ascii=False)}")
                        continue
                    new[tgt][k] = v
            changed = False
            for p in parts:
                ordered = OrderedDict((k, new[p][k]) for k in en[p] if k in new[p])
                for k in new[p]:
                    if k not in ordered:
                        ordered[k] = new[p][k]
                if list(ordered.items()) != list(cur[p].items()):
                    changed = True
                    if write:
                        save_part(area, lang, p, ordered)
            if moved or dropped or changed:
                print(f"  {area} {lang}：挪了 {moved} 条，删了 {dropped} 条" + ("" if write else "（没写，--dry-run）"))
    return 0


# ------------------------------------------------------------------ quotes

QUOTE = re.compile(r"「([^」]+)」")


def quotes(args):
    bad = 0
    # 有的引用故意保留英文（别的应用的菜单名「Settings › MCP」）：写成英文的也算对
    en_src = {}
    for area in AREAS:
        for p in parts_of(area):
            for k, v in load_part(area, "en", p).items():
                for f in forms(v):
                    en_src.setdefault(source(k), set()).add(f)
    for lang in [args.lang] if args.lang else LANGS:
        entries = []  # (区域, 键, 译文)
        by_src = {}  # 原文 → 这种语言里它的各种译文
        for area in AREAS:
            for p in parts_of(area):
                for k, v in load_part(area, lang, p).items():
                    entries.append((area, k, v))
                    for f in forms(v):
                        by_src.setdefault(source(k), set()).add(f)
        misses = []
        for area, k, v in entries:
            for name in QUOTE.findall(source(k)):
                for seg in (x.strip() for x in name.split("›")):
                    cands = by_src.get(seg)
                    if not cands or "{" in seg:
                        continue
                    cands = cands | en_src.get(seg, set())
                    for f in forms(v):
                        low = f.lower()
                        if not any(c.lower() in low for c in cands):
                            misses.append((area, k, seg, f, sorted(cands)))
        bad += len(misses)
        print(f"{lang}：{len(misses)} 处引用的名字和那个名字自己的译文对不上")
        for area, k, seg, f, cands in misses[: (10**9 if args.verbose else 30)]:
            print(f"  [{area}]「{source(k)[:50]}」引用「{seg}」→ 应该含 {' / '.join(cands)}\n      现在：{f}")
        if len(misses) > 30 and not args.verbose:
            print(f"  ……还有 {len(misses) - 30} 处（-v 看全部）")
    return 1 if bad else 0


def main():
    ap = argparse.ArgumentParser(description="多语言词表的检查、导出和导入")
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check")
    c.add_argument("--strict", action="store_true")
    c.add_argument("--area", choices=list(AREAS))
    c.add_argument("--lang", choices=LANGS)
    c.add_argument("-v", "--verbose", action="store_true")
    e = sub.add_parser("export")
    e.add_argument("lang_pos", metavar="LANG", choices=LANGS)
    e.add_argument("--area", choices=list(AREAS))
    e.add_argument("--all", action="store_true")
    e.add_argument("--part", help="只导出这几个分册，逗号分开（插件网页部件的分册叫 ext）")
    e.add_argument("--out")
    t = sub.add_parser("tidy")
    t.add_argument("--area", choices=list(AREAS))
    t.add_argument("--dry-run", action="store_true")
    t.add_argument("-v", "--verbose", action="store_true")
    q = sub.add_parser("quotes")
    q.add_argument("--lang", choices=LANGS)
    q.add_argument("-v", "--verbose", action="store_true")
    i = sub.add_parser("import")
    i.add_argument("lang_pos", metavar="LANG", choices=LANGS)
    i.add_argument("file")
    i.add_argument("--map")
    args = ap.parse_args()
    sys.exit({"check": check, "export": export, "import": do_import, "tidy": tidy, "quotes": quotes}[args.cmd](args))


if __name__ == "__main__":
    main()
