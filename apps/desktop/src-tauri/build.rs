//! 编译前先生成 Info.plist 的各语言说明，再交给 tauri-build。

use std::path::Path;

/// Info.plist 里要翻译的说明：键和原文（原文要和 src/remote_cmds.rs 的 LOCAL_NETWORK_USAGE 一样，测试会核对）
const KEYS: &[(&str, &str)] = &[("NSLocalNetworkUsageDescription", "让同一个 Wi-Fi 下配对过的手机找到这台电脑")];

/// 词表的语言 → macOS 的 .lproj 名。简体中文就是原文；繁体中文缺译文时退回原文，其他语言退回英文（和 i18n.rs 一样）
const LANGS: &[(&str, &str)] = &[("en", "en"), ("zh-CN", "zh-Hans"), ("zh-TW", "zh-Hant"), ("ja", "ja"), ("ko", "ko"), ("es", "es"), ("fr", "fr"), ("de", "de")];

fn catalog(lang: &str) -> serde_json::Value {
    let p = format!("../../../crates/captae-core/locales/{lang}/app.json");
    println!("cargo:rerun-if-changed={p}");
    std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// 生成 gen/infoplist/<语言>.lproj/InfoPlist.strings，tauri.conf.json 的 bundle.resources 把它们放进 Contents/Resources/
fn infoplist_strings() {
    let en = catalog("en");
    for (lang, lproj) in LANGS {
        let cat = if *lang == "zh-CN" { serde_json::Value::Null } else { catalog(lang) };
        let mut out = String::from("/* 编译时由 apps/desktop/src-tauri/build.rs 按词表生成，不要手改 */\n");
        for (key, src) in KEYS {
            let text = cat[*src]
                .as_str()
                .or_else(|| if matches!(*lang, "zh-CN" | "zh-TW") { Some(*src) } else { en[*src].as_str() })
                .unwrap_or(src);
            let esc = text.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!("\"{key}\" = \"{esc}\";\n"));
        }
        let dir = Path::new("gen/infoplist").join(format!("{lproj}.lproj"));
        std::fs::create_dir_all(&dir).expect("建 gen/infoplist");
        let f = dir.join("InfoPlist.strings");
        // 内容没变不重写，免得每次都触发重新打包资源
        if std::fs::read_to_string(&f).ok().as_deref() != Some(out.as_str()) {
            std::fs::write(&f, out).expect("写 InfoPlist.strings");
        }
    }
}

fn main() {
    infoplist_strings();
    tauri_build::build()
}
