// Windows：给 captae.exe 写上版本资源（文件属性里的「文件版本」「产品版本」、产品名、说明、版权），和桌面端（tauri-build 写的）一样。
// 没有的话文件属性里版本是空的（0.0.0.0），安装包里只有它对不上（Devin 的 Windows U2）。
// 用依赖树里本来就有的 tauri-winres（tauri-build 也用它）：MSVC 用 Windows SDK 的 rc.exe，交叉检查（x86_64-pc-windows-gnu）用 mingw 的 windres。
// 只看要编译出的程序是不是 Windows 的（build.rs 在打包的机器上运行，cfg!(windows) 说的是那台机器）
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut r = tauri_winres::WindowsResource::new();
    r.set("ProductName", "Captae")
        .set("FileDescription", "Captae command line, browser extension host and MCP server")
        .set("InternalName", "captae")
        .set("OriginalFilename", "captae.exe")
        .set("CompanyName", "Captae contributors")
        .set("LegalCopyright", "Captae contributors · AGPL-3.0-or-later");
    if let Err(e) = r.compile() {
        panic!("captae.exe 的版本资源没编进去：{e}");
    }
}
