# 遥控电脑：App 要接什么（iOS / Android）

captae-mobile 把 captae-remote 的客户端包成 `RemoteHub`（`src/remote/`），给 Swift / Kotlin 用：配对、连接、电脑上的任务、电脑的素材库、存到手机、「等电脑上线」队列、「全部设备」的合并视图。
协议和安全上的做法见 `crates/captae-remote/CLIENT.md`、`work/mobile/M2.md`；这次的实现和实测记录在 `work/mobile/M2b.md`。

Swift 里方法名是 camelCase（`pairQr(text:)`、`setActive(active:)`），Kotlin 一样；错误和本机 API 一样是 `MobileError.Failed(detail:)` / `MobileException.Failed`。

## 1. 建起来

```swift
let core = try MobileCore(o: InitOptions(...))                      // 本机那套，见 work/mobile/M1.md 第 3 节
let hub = try RemoteHub(core: core,
                        options: RemoteOptions(phoneName: UIDevice.current.name,   // 电脑上「已配对的手机」「来自 iPhone」用它
                                               cacheDir: <Caches 目录>),            // 电脑素材的封面缓存在 <缓存目录>/captae-remote-thumbs/
                        secure: KeychainStore())                    // 见第 2 节
hub.setListener(listener: MyRemoteListener())                       // 见第 5 节
hub.setActive(active: true)                                         // App 到前台；到后台时 false
```

- App 启动时建一个 `RemoteHub`、一直用着（和 `MobileCore` 一样）。建的时候不联网，只读出配对过的电脑（`<数据目录>/remote/computers.json`）。
- `setActive(true)`：连上所有配对过的电脑（每台一个 WebSocket，按配对时的证书指纹钉死）；断了自动重连，最多 15 秒找一次（PLAN 5.3）。
  `setActive(false)`：全部断开（iOS 在后台不找电脑：没有服务器就没有推送）。iOS 在 `scenePhase` 变化时调，Android 在 `ProcessLifecycleOwner` 的 onStart / onStop 调。
- 回到前台、或者发现服务（第 3 节）认出一台离线的电脑时，马上再找一次，不等重连的间隔。

## 2. 安全存储回调（令牌只放在这里）

```swift
final class KeychainStore: SecureStore {           // UniFFI 生成的协议
    func read(key: String) -> String?              // 没有返回 nil
    func write(key: String, value: String) -> Bool // 写不进去返回 false：配对算失败，不留下没有令牌的电脑
    func delete(key: String)
}
```

- **iOS**：钥匙串 `kSecClassGenericPassword`，`kSecAttrService` 写 App 的标识，`kSecAttrAccount` = key，`kSecAttrAccessible` 用 `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`
  （不跟着备份到别的手机：令牌是电脑发给这台手机的）。分享扩展也要发任务的话，用同一个 Keychain Access Group。
- **Android**：Android Keystore 里一把 AES-GCM 密钥（`KeyGenParameterSpec`，`setUserAuthenticationRequired(false)`），值加密后存 SharedPreferences；
  或者 `androidx.security:security-crypto` 的 `EncryptedSharedPreferences`。
- 键形如 `captae.remote.token.<证书指纹>`，值是 256 位的设备令牌（43 个字符）。**Rust 不把令牌写进任何文件**，`computers.json` 里只有名字、地址、端口、时间。
- 回调可能在任何线程上调（App 的线程、Rust 的后台线程），要线程安全；钥匙串和 Keystore 本身就是。回调里不要再调 `RemoteHub`。
- 钥匙串被清掉（换手机恢复备份、用户重置）：那台电脑的状态是 `NeedsPairing`，界面提示重新配对。

## 3. 发现电脑（App 做，Rust 只收结果）

服务类型 `_captae._tcp`，实例名是电脑名，TXT：`v=1`、`fp=<证书指纹>`、`name=<电脑名>`。

- **iOS**：`NWBrowser(for: .bonjourWithTXTRecord(type: "_captae._tcp", domain: nil), using: .tcp)`。browseResults 里的 endpoint 是服务，不是地址：
  对每个结果开一个 `NWConnection`，`.ready` 以后从 `currentPath?.remoteEndpoint` 拿到 `.hostPort(host, port)`，取出 IP（IPv6 不带 `%网卡`、不带方括号）就关掉连接。
  不要用 Rust 的 `discover` 特性：它自己收发组播，iOS 上要 Apple 特批的 `com.apple.developer.networking.multicast`。
- **Android**：`NsdManager.discoverServices("_captae._tcp", NsdManager.PROTOCOL_DNS_SD, …)`，找到的用 `registerServiceInfoCallback`（API 34 起）或 `resolveService` 解析；
  `serviceInfo.hostAddresses` 是地址，`attributes` 是 TXT（值是 `ByteArray`，按 UTF-8 转字符串）。
- 解析好以后交进来：
  ```swift
  let id = hub.discovered(service: DiscoveredService(name: 实例名, hosts: ["192.168.1.5"], port: 47863, txt: ["v": "1", "fp": …, "name": …]))
  // 返回设备 id：是配对过的那台（按 TXT 的 fp 认），新地址已经记下、离线的马上再连；nil：没配对过，进了 nearby()
  hub.lost(name: 实例名)   // NWBrowser 的 removed / NSD 的 onServiceLost
  ```
- TXT 里的 `fp` 没有经过验证，只用来认出是哪一台；连接时一律按配对时存下的指纹钉死。认错了也只会连不上（`cert_mismatch`），不会把令牌发给别人。
- 发现只在前台、「管理电脑」或「添加电脑」开着时跑就够了；电脑换了 IP 也会经发现认回来。

## 4. 配对、设备列表

| 调用 | 做什么 |
|---|---|
| `parsePairLink(text)` | 扫到的内容是不是 Captae 的配对二维码；是的话返回电脑名、地址、指纹、`known`（配对过），先给用户看「要连 Sol 的 MacBook Pro 吗」。不联网 |
| `async pairQr(text)` | 扫码配对（按二维码里的指纹钉死证书，一次性码换令牌）→ `MobileDevice` |
| `async pairCode(host, port, digits)` | 6 位数配对（SPAKE2）：`host`、`port` 用 `nearby()` 里那台的，`digits` 是电脑上显示的配对码 |
| `nearby()` | 发现到的、还没配对过的电脑（「添加电脑」里列出来，点了输配对码） |
| `devices()` | 设备菜单：第一个是这台手机（id `"phone"`，`local = true`），后面是配对过的电脑（id 是证书指纹） |
| `device(id)` / `rename(id, name)` | 一台设备；在手机上改名（空 = 用电脑自己报的名字，不改电脑那边） |
| `remove(id)` | 删掉一台电脑：断开、删令牌、去掉等它上线的任务。电脑那边「已配对的手机」里还有这台手机，界面提示到电脑的「设置 › 手机」里撤销 |

`MobileDevice`：`name`、`computerName`、`status`（`Local` / `Idle` / `Connecting` / `Online` / `Offline` / `Revoked` / `NeedsPairing`）、`lastSeen`、`lastUsed`（「发到…」默认选最近用过的那台）、
`pairedAt`、`running`（正在下几个）、`waiting`（等它上线的几个）、`hosts`、`port`。标题下面那行小字（「在线 · 正在下 3 个」「离线 · 上次在线 今天 14:20」）从这里拼。

同一台电脑再配对一次（扫码或 6 位数）：还是一台，令牌换新的；被撤销过的恢复正常。

## 5. 事件（`RemoteListener`）

和本机的 `CoreListener` 同一个样子，多了设备 id。在 Rust 的后台线程上调，界面自己切主线程；回调里可以再调 `RemoteHub`（调回调时不持有锁）。

| 回调 | 什么时候 |
|---|---|
| `onJob(DeviceJob)` | 电脑上某个任务变了（进度最多每 250 毫秒一次）。`DeviceJob { device, job: MobileJob, from }`，`from` 是从哪台手机加的 |
| `onRemoved(device, id)` / `onFinished(device, id, title, assetId)` / `onLibrary(device)` | 电脑上任务没了 / 下完进了电脑的素材库 / 电脑素材库变了 |
| `onResync(device)` | 刚连上、重连上（收到整份任务快照）、被撤销：重新调 `remoteJobs` / `mergedJobs`，断线时漏掉的变化就补上了 |
| `onDevice(MobileDevice)` | 一台电脑的状态、名字、正在下的数、等上线的数变了 |
| `onDevices()` | 配对了新电脑、删了一台、附近发现的电脑变了：重新调 `devices()` / `nearby()` |
| `onPending()` / `onPendingSent(task, job)` | 「等电脑上线」队列变了 / 队列里的一条自动发出去了（可以弹一条「已发到 Sol 的 MacBook Pro」） |
| `onSave(SaveProgress)` | 存到手机的进度、暂停、完成、失败（进度最多每 250 毫秒一次） |
| `onSavesRestored([SaveProgress])` | App 重开后找回了上次没存完的（第一次 `setActive(true)` 时发一次）：把进度条显示回来；那台电脑连上就自动接着下，之后照常 `onSave` |

`MobileJob` 和本机的同一个类型；电脑上的任务 `path` 恒为空（电脑上的路径不给手机），`thumb` 是网上的封面地址（电脑本机的封面文件不给）。
任务失败的说法是电脑那边按电脑的语言生成的；错误码、按钮（`failure.code`、`failure.action`）和桌面端一样，网站名按手机的语言翻译。

## 6. 电脑上的任务

| 调用 | 做什么 |
|---|---|
| `remoteJobs(device)` | 最近一次收到的任务（不联网）；电脑离线时是断开前最后看到的 |
| `async refreshRemoteJobs(device)` | 重新向电脑要一次（下拉刷新） |
| `remoteJobLog(device, id)` | 下载记录（任务详情里的「下载记录」） |
| `async send(device, SendOptions)` | 发到电脑：在线就发（`SendResult.sent(job)`）；不在线、或者发的时候连不上，进「等电脑上线」（`.queued(task)`）。`SendOptions`：`url`、`quality`、`format`、`audioOnly`、`preset`（电脑的白名单只收这几项）、`title`（只在队列里显示） |
| `async remoteAction(device, id, MobileAction)` | 暂停、继续、取消、重试、停止录制……（和桌面端的 `JobAction` 一样） |
| `async remoteSetVariant(device, id, index)` / `async remoteChooseEntries(device, id, ids)` | M3U8 选清晰度 / 播放列表选条目 |
| `async remoteProbe(device, url, cancel)` | 在电脑上读取信息（不带电脑的登录状态），`CancelToken` 取消时返回 `cancelled` |

## 7. 「等电脑上线」

- 存在 `<数据目录>/remote/pending.json`（App 被结束了也在）。电脑连上（`Connected`）以后按加的先后自动发出去，发一条回调一次 `onPendingSent`。
- 电脑收到了但不接受（链接太长、白名单外）：那一条记下 `error`、不再自动发，`retryPending(id)` 再试、`removePending(id)` 删掉。
- `pending()` 每条带 `canRunHere`：能不能改成在这台手机上下。`moveToPhone(ids)`（空 = 全部）调本机的 `add`：
  iOS 上架版只改得动直链和 M3U8（`unsupported_here` / `probe_first` 的留在队列里，不算出错），Android、iOS 完整版都能改。返回 `MoveResult { moved, kept }`。
- 被撤销、令牌没了的电脑，`send` 直接报错（`unauthorized` / `needs_pairing`），不进队列。

## 8. 电脑的素材库、存到手机

| 调用 | 做什么 |
|---|---|
| `async remoteLibrary(device, LibraryQuery)` / `async remoteAsset(device, id)` | 查电脑的素材库（和本机的 `LibraryQuery` 一样：类型、搜索、分页）。`path` 是空的，`thumb` 是缓存过的封面 |
| `async remoteThumbnail(device, id, refresh)` | 拿封面，缓存到 `<缓存目录>/captae-remote-thumbs/<指纹>/<id>.jpg`，返回路径；电脑上没有封面时 nil |
| `saveToPhone(device, assetId, path)` | 把文件下到 App 给的 `path`（App 自己的目录里），马上返回 `SaveProgress`，之后看 `onSave`。下完自动导入这台手机的素材库（带电脑上的标题、来源、时长、封面），`SaveProgress.localAssetId` 是手机素材库里的 id；存相册 / 「文件」还是 App 的事，`path` 上的文件要留着（素材记的就是它） |
| `pauseSave(id)` / `resumeSave(id)` / `cancelSave(id)` | 暂停（下了的留着）/ 接着下（暂停了的、失败了的）/ 取消并删掉下了一半的文件 |
| `saves()` / `forgetSave(id)` | 全部记录（在下的、重开后找回的、7 天内下完的）/ 从记录里去掉 |

手机素材库的导入也可以单独用：`MobileCore.importFile(path, ImportInfo)`（标题、类型、来源链接 / 网页 / 网站名原文、上传者、格式、宽高、时长、标签、封面图片路径都可以不给；
类型按扩展名猜，MP4 / M4A 的时长、宽高从文件头读；封面复制进素材库自己的目录；文件不复制不移动；同一个文件导入过就返回原来那条），素材库变了回调 `CoreListener.onLibrary`。

断点续传：`path` 旁边有一个 `<path>.etag` 记着电脑上文件的版本，电脑上的文件换过了就从头下；下完删掉它。

记录存在 `<数据目录>/remote/saves.json`（和离线队列放一起，只在状态变化时写，进度按文件实际大小算）：
- App 被结束时还在下的：重开后是 `Running`，第一次 `setActive(true)` 时发 `onSavesRestored`，那台电脑连上就接着下（也可以 `resumeSave(id)` 手动接）。暂停了的、失败了的照旧，不自动下。
- 下完的带 `doneAt`，留 7 天（界面写「已存到手机」），过期的重开时清掉；删掉电脑时它的记录一起清掉。
- `path` 要放在重开后还在的目录里（App 的 Application Support / filesDir，不要放 tmp），否则重开后只能从头下。

## 9. 「全部设备」的合并视图

- `mergedJobs(device, search)` → `[JobGroup { device, jobs: [DeviceJob], pending: [PendingTask] }]`：每台电脑一组（电脑上的任务 + 等它上线的），最后是这台手机。
  `device` 给了只要那一台（设备菜单选了某台时），`search` 按标题、链接、来源过滤（不分大小写）。空的组也在，界面自己决定藏不藏。
- `async mergedLibrary(query, device)` → `[LibrarySection { device, items: [DeviceAsset], total, error }]`：几台电脑同时查，离线 / 被撤销的那段带 `error`、`items` 是空的。
- `localDeviceId()` = `"phone"`。

## 10. 错误代码

`MobileFailure.code`（小写；`action` 是界面上的下一步按钮）：

| code | 什么时候 | action |
|---|---|---|
| `unreachable` | 连不上（不在同一个 Wi-Fi、电脑没开 Captae、没打开「让手机连接这台电脑」、iOS 本地网络权限被拒） | `retry` |
| `cert_mismatch` | 连上的不是配对过的那台，或者电脑重新生成了证书 | `pair` |
| `unauthorized` | 在电脑上被撤销了：令牌已经删掉，状态 `Revoked` | `pair` |
| `needs_pairing` | 安全存储里没有令牌，或者配对时写不进去 | `pair` |
| `pair_mismatch` / `pair_wrong` / `pair_void` / `pair_expired` / `pair_closed` / `bad_link` | 配对：6 位数不对 / 二维码的码不对 / 错太多次作废 / 过期 / 电脑上没在显示配对码 / 不是 Captae 的二维码 | `none` |
| `not_allowed` / `bad_request` / `not_found` / `range` | 白名单外 / 电脑不接受（链接不完整、太长）/ 电脑上没有了 / 续传时文件变了 | `none` |
| `no_device` / `no_task` | id 不对（电脑删掉了、存到手机的记录没了） | `none` |
| `remote_failed` | 别的通信错误（`raw` 里有原文） | `retry` |
| 15 个桌面端错误码（`need_login`……） | 电脑上加任务、读取信息失败，带着和桌面端同一套详情 | 和桌面端一样 |

`message` 是按手机当前语言生成的一句话，可以直接显示（`tr!`，词表 `crates/captae-core/locales/<语言>/app.json`）；判断一律按 `code`。

## 11. 权限和配置

**iOS（Info.plist）**
```xml
<key>NSLocalNetworkUsageDescription</key>
<string>Captae finds and controls your computer on the same Wi-Fi.</string>   <!-- 说法由做 iOS 的同事按 BRIEF 定，InfoPlist.strings 里翻译 -->
<key>NSBonjourServices</key>
<array><string>_captae._tcp</string></array>
```
- 第一次 NWBrowser 开始找、或者第一次连局域网地址时系统问一次「本地网络」。用户拒了：发现不到，Rust 连电脑报 `unreachable`；界面提示去「设置 › 隐私与安全性 › 本地网络」打开。
- Rust 自己做 TLS（钉证书指纹），不经过 URLSession，不需要 ATS 例外。
- 扫码用 VisionKit（`NSCameraUsageDescription`），扫到的字符串交给 `parsePairLink` / `pairQr`。
- 分享扩展要发到电脑的话，和 App 用同一个 App Group 放数据目录、同一个 Keychain Access Group 放令牌；同一时间只让一边开 `RemoteHub`（扩展里发完就丢掉）。

**Android（AndroidManifest.xml）**
```xml
<uses-permission android:name="android.permission.INTERNET" />
<uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
<uses-permission android:name="android.permission.CHANGE_WIFI_MULTICAST_STATE" />  <!-- NSD 找服务时拿 MulticastLock，有的机型不拿收不到 -->
```
- NsdManager 本身不要运行时权限。targetSdk 36（PLAN 5.5）不受 Android 16 起的「本地网络访问」限制；以后把 targetSdk 升到 37 前要再核对一次这条（`ACCESS_LOCAL_NETWORK`，在「附近的设备」权限组里，要在运行时申请）。
- 不需要 `usesCleartextTraffic`（都是 HTTPS）。
- 和本机 API 一样，`CaptaeTls.init(applicationContext)` 照旧调（只影响本机下载；遥控电脑按指纹钉证书，不用系统证书）。

## 12. 存在哪里

| 什么 | 哪里 |
|---|---|
| 设备令牌 | 只在 App 的安全存储（第 2 节） |
| 配对过的电脑（名字、改的名字、指纹、地址、端口、电脑给的编号、时间、是否被撤销） | `<数据目录>/remote/computers.json` |
| 等电脑上线的任务 | `<数据目录>/remote/pending.json` |
| 电脑素材的封面 | `<缓存目录>/captae-remote-thumbs/<指纹>/<素材 id>.jpg`（系统清掉了下次再拿） |
| 存到手机的记录 | `<数据目录>/remote/saves.json`（下完的留 7 天） |
| 存到手机的文件 | App 给的路径（和旁边的 `.etag`，下完删掉） |
