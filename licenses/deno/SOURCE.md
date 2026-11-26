# deno 2.9.8

- 许可证：MIT，全文见 `LICENSE.md`（来自 https://github.com/denoland/deno/blob/v2.9.8/LICENSE.md）
- 安装包使用官方发布的二进制，原样，没有改动。Captae 只把它作为独立程序交给 yt-dlp 运行 YouTube 的 JS 验证（`--js-runtimes deno:<路径>`），不链接它的代码

| 平台 | 官方文件 | SHA256 | 装好以后在 |
|---|---|---|---|
| macOS（arm64） | `deno-aarch64-apple-darwin.zip` | `9f0bf660393da2eaff044cdc7444cb08c73f235849b61d224d825a194ce7c98e` | `Contents/MacOS/deno`（保留 Deno 官方的代码签名） |
| Windows（x64） | `deno-x86_64-pc-windows-msvc.zip` | `cba84b26e0df5ae7f74fc66b86a95b866872a86292ebd84fa1e959927edd93bf` | 安装目录的 `deno.exe` |
| Linux（x86_64） | `deno-x86_64-unknown-linux-gnu.zip` | `e890975bc2549807bbcc0b584ffafda72cc7c93349dd99c80483dd784f6c4609` | `/usr/lib/Captae/deno` |

- deno 内含的第三方组件（V8 等）的许可证随 deno 源码发布：https://github.com/denoland/deno/tree/v2.9.8
- 上游源码和发布页：https://github.com/denoland/deno/releases/tag/v2.9.8
