//! 证书校验的检查程序：用 MobileCore（系统证书校验）读几个站点，看该过的过、该拒的拒。
//! iOS 模拟器上：编成 aarch64-apple-ios-sim，用 `xcrun simctl spawn <设备> <程序> <数据目录>` 跑。Android 上用引擎的仪器测试 PlatformTlsTest（要 JVM 初始化）。
//! 直链都用 .mp4 结尾：读取信息走 Rust 自己的 HTTP 客户端
use captae_mobile::*;

fn main() {
    let dir = std::env::args().nth(1).expect("用法：tls_check <数据目录>");
    let core = MobileCore::new(InitOptions { data_dir: format!("{dir}/data"), downloads_dir: format!("{dir}/dl"), platform: Platform::IosStore, language: "zh-CN".into(), native_lib_dir: None }).unwrap();
    println!("证书校验 {}", core.info().tls);
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let cases = [
        ("Blender 直链", "https://download.blender.org/durian/trailer/sintel_trailer-480p.mp4", "pass"),
        ("Let's Encrypt 有效", "https://valid-isrgrootx1.letsencrypt.org/captae-tls-check.mp4", "tls-ok"),
        // iOS 只记录：Apple 的系统校验平时只用 OCSP 查吊销，Let's Encrypt 停了 OCSP 以后查不出来（0.6.2 和 0.7.1 的 Apple 部分逻辑一样，不是这次升级带来的）。
        // Android 上要被拒，见 PlatformTlsTest
        ("Let's Encrypt 已吊销", "https://revoked-isrgrootx1.letsencrypt.org/captae-tls-check.mp4", "info"),
        ("自签证书", "https://self-signed.badssl.com/captae-tls-check.mp4", "reject"),
    ];
    let mut bad = 0;
    for (name, url, want) in cases {
        let r = rt.block_on(core.probe(url.into(), None));
        let (ok, why) = match &r {
            Ok(p) => (true, format!("{:?} 字节", p.size)),
            Err(e) => (false, format!("{} | {}", e.detail().code, e.detail().raw)),
        };
        let cert = why.to_lowercase().contains("certificate");
        let good = match want {
            "info" => true,
            "pass" => ok,
            "tls-ok" => ok || !cert,
            _ => !ok && cert,
        };
        if !good {
            bad += 1;
        }
        println!("{} {name}：成功 {ok}；{why}", if want == "info" { "INFO" } else if good { "OK  " } else { "FAIL" });
    }
    println!("{}", if bad == 0 { "TLS CHECK OK" } else { "TLS CHECK FAILED" });
    std::process::exit(if bad == 0 { 0 } else { 1 });
}
