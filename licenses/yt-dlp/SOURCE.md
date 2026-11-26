# yt-dlp 2026.10.26

- 源码许可证：Unlicense，全文见 `LICENSE.Unlicense`
- 安装包使用官方发布的 PyInstaller 目录版（含 Python 解释器等组件），官方说明这类打包文件整体按 GPL-3.0-or-later 分发，全文见 `COPYING.GPLv3`。
  打包进去的 Python、OpenSSL、curl-impersonate 等组件各自的许可证在目录版自带的 `_internal/THIRD_PARTY_LICENSES.txt` 里（原样随附）。
  下载时用 yt-dlp 官方公钥验证 `SHA2-256SUMS.sig`、核对 SHA256。

| 平台 | 官方文件 | 装好以后在 |
|---|---|---|
| macOS（arm64） | `yt-dlp_macos.zip` | `Captae.app/Contents/Resources/yt-dlp/yt-dlp_macos` |
| Windows（x64） | `yt-dlp_win.zip`（SHA256 `fd51b212cf8380c7a52ee08c0d6be7f3c59b2f2c832eaf7b45acfae6eab57419`） | 安装目录的 `yt-dlp\yt-dlp.exe` |
| Linux（x86_64） | `yt-dlp_linux.zip`（SHA256 `f34053415136e55073dd64ba5326d161ecc23ce0e98c081d8752867b37809450`） | `/usr/lib/Captae/yt-dlp/yt-dlp_linux` |

- 上游源码和发布页：https://github.com/yt-dlp/yt-dlp/releases/tag/2026.10.26
- 装好以后 Captae 会自动更新 yt-dlp（同样从官方发布页下载、验签），新版本放在数据目录里，不改安装包里的这一份。
