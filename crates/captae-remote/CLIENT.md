# captae-remote 接入说明（给 crates/captae-mobile 和两个 App）

手机在同一个 Wi-Fi 下遥控电脑上的 Captae。电脑端（`server` 特性）由桌面端开；手机端用这里的 Rust 客户端（`client` 特性），由 `crates/captae-mobile` 包成 UniFFI 接口给 Swift / Kotlin。
设计在 `work/mobile/PLAN.md` 5.3，实测记录在 `work/mobile/M2.md`。

## 1. 依赖怎么写

```toml
# crates/captae-mobile/Cargo.toml
captae-remote = { path = "../captae-remote", default-features = false, features = ["client"] }
# Android 想用 Rust 做 Bonjour 发现时再加 "discover"；iOS 不加（见第 4 节）
```

`client` 不依赖 captae-core，只有 reqwest（rustls + ring）、tokio-rustls、tokio-tungstenite、spake2。证书校验不看系统根证书，所以手机上不需要 `rustls-platform-verifier`。

## 2. 要包出去的接口

| Rust | 做什么 | 给 App 的建议写法 |
|---|---|---|
| `proto::PairLink::parse(s)` | 扫到的字符串 → 配对链接；不是 Captae 的二维码回 None | `parsePairLink(String) -> PairLink?` |
| `client::pair_qr(&link, &PhoneHello)` | 扫码配对 | `async pairQr(link, phone) throws -> Credentials` |
| `client::pair_code(host, port, "123456", &PhoneHello)` | 发现 + 6 位数配对 | `async pairCode(host, port, digits, phone) throws -> Credentials` |
| `Client::connect(creds)` / `Client::new(creds)` | 连一台配对过的电脑（`new` 不联网） | 一台电脑一个对象 |
| `client.reconnect()` / `add_hosts(..)` / `credentials()` | 换网络、Bonjour 发现新地址后重新找；拿排好序的地址存回去 | |
| `client.me()` `jobs()` `add(&AddJob)` `action(id, "pause")` `set_variant(id, i)` `choose_entries(id, ids)` `probe(url)` | 任务 | |
| `client.library(&LibQuery)` `asset(id)` `thumb(id)` `file_range(id, a, b)` `download_to(id, path, progress)` | 素材库、存到手机（断点续传；续传按位置写，上一次被中途丢掉时还没落盘的写入不会错位） | `download_to` 包成带 `CancelToken` 的长任务（UniFFI 的 async 不能取消，和 M1 的做法一样） |
| `client.subscribe()` / `subscribe_every(最长间隔)` → `Subscription::recv()` | 实时事件，断了自动重连（`subscribe` 最多 30 秒找一次；captae-mobile 用 `subscribe_every(15 秒)`，PLAN 5.3） | 包成回调接口（`RemoteListener.onEvent(event)`），丢掉订阅就停 |
| `client::error_code(&e)` / `remote_error(&e)` | 错误代码、下载失败详情 `fail`、配对码还能试几次 `left` | 包成带 `code` 字段的错误枚举 |

`PhoneHello { name, platform }`：`name` 写在电脑上「已配对的手机」和下载列表「来自 iPhone」里（iOS 16 起 `UIDevice.name` 只给「iPhone」，就用它；Android 用设备名），`platform` 是 `ios` / `android`。

## 3. 存什么、存哪

`Credentials { name, fp, hosts, port, token, device }`：
- `token`（设备令牌，256 位）存 iOS 钥匙串 / Android Keystore，别的字段可以放 App Group / 普通存储。
- `fp`（证书指纹）是这台电脑的身份：地址会变，指纹不变。Bonjour 发现到的 TXT 里有 `fp`，按它认出是哪一台，再 `add_hosts`。
- 每次 `reconnect` 成功以后把 `credentials()` 存回去（连上的地址排到第一个）。

## 4. 发现电脑

Bonjour 服务类型 `_captae._tcp`，实例名是电脑名，TXT：`v=1`（协议版本）、`fp=<证书指纹>`、`name=<电脑名>`。
- **iOS**：用 `NWBrowser` 找 `_captae._tcp`（Info.plist 写 `NSBonjourServices = ["_captae._tcp"]` 和 `NSLocalNetworkUsageDescription`）。
  Rust 的 `discover` 特性要自己收发组播，iOS 上需要 Apple 特批的 `com.apple.developer.networking.multicast`，所以不用。
- **Android**：系统 NSD（`NsdManager`），或者打开 `discover` 特性用 `discover::discover(Duration)`。
- TXT 里的 `fp` 没有经过验证，只用来认出是哪一台；连接时一律按配对时存下的指纹钉死。

## 5. 错误代码

服务端（`proto::code`）：`UNAUTHORIZED`（没配对、被撤销 → 删掉这台电脑的令牌，提示重新配对）、`NOT_ALLOWED`（白名单外）、`BAD_REQUEST`、`NOT_FOUND`、
`PAIR_EXPIRED`（配对码过期）、`PAIR_WRONG`（6 位数不对，带 `left`）、`PAIR_VOID`（错 5 次作废）、`PAIR_CLOSED`（电脑上没在显示配对码 / 码用过了）、`RANGE`、`FAILED`（带 `fail` 时和桌面端同一套 15 个错误码）。
客户端（`client::code`）：`UNREACHABLE`（不在同一个 Wi-Fi、电脑没开）、`CERT_MISMATCH`（不是配对过的那台，或者电脑重新生成了证书）、`PAIR_MISMATCH`（6 位数输错，或者有人冒充电脑）、`BAD_LINK`。
错误里的 `message` 只给开发者看；给人看的文字由 App 按代码写（照 `work/mobile/design/BRIEF.md` 的说法）。

## 6. 实时事件

`RemoteEvent`：`Connected`、`Jobs(快照)`、`Job(任务)`、`JobRemoved(id)`、`Finished { id, title, asset_id }`、`Library`、`Disconnected { reason, retry_in }`、`Revoked`。
- 每次连上（包括重连）先收到一份 `Jobs` 快照：用它**整个替换**电脑上的任务列表，断线期间漏掉的变化就补上了。
- 任务的字段和桌面端 `Job` 一样（`apps/desktop/src/lib/types.ts`），去掉了电脑上的文件路径和本机封面路径；`from` 是从哪台手机加的。
- 电脑每 20 秒发一次 ping，60 秒什么都没收到就当断了；重连等 1、2、4… 最多 30 秒。`Revoked` 以后不再重连。

## 7. 接口一览（HTTPS，`/v1`，除了配对都要 `Authorization: Bearer <令牌>`）

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/v1/info` | 不用令牌：`{app, v, version, name}` |
| POST | `/v1/pair` | 扫码：`{code, name, platform}` → `{token, device, name}` |
| POST | `/v1/pair/start`、`/v1/pair/finish` | 6 位数（SPAKE2，`client::pair_code` 里做好了） |
| GET | `/v1/me` | `{device, name, computer, version}` |
| GET / POST | `/v1/jobs` | 任务列表 / 加任务 `{url, quality?, format?, audio_only?, preset?}`，多了别的字段整个拒掉 |
| POST | `/v1/jobs/{id}/action` | `{action}`：toggle / pause / resume / now / up / down / cancel / remove / retry / segretry / pickgo / stop |
| POST | `/v1/jobs/{id}/variant` | `{index}`：M3U8 选清晰度 |
| POST | `/v1/jobs/{id}/entries` | `{ids}`：播放列表选条目 |
| POST | `/v1/probe` | `{url}`：读取信息（电脑上读，不带登录状态） |
| GET | `/v1/library` | `?q&kind&site&format&days&tag&limit&offset` → `{items, total}` |
| GET | `/v1/library/{id}`、`/thumb`、`/file` | 素材、封面、文件（`Range`、`If-Range`、`ETag`） |
| GET | `/v1/events` | WebSocket |

其余一律 403 `NOT_ALLOWED`：改设置、读 cookies / 登录状态、AI 助手、删素材、插件那套接口都没有。

已经包好的 UniFFI 接口（`RemoteHub`）和 App 要接的东西见 `crates/captae-mobile/REMOTE.md`。
