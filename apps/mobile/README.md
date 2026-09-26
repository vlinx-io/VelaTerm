# VelaTerm 手机 App

本工程使用 Capacitor 8，包含 iOS 和 Android 原生工程。连接首页随 App 打包；连接成功后，独立的系统 WebView 加载远端 VelaTerm 的完整 Web 项目界面。项目、文件、会话与终端继续由远端服务提供。

当前提供 SSH、URL 和 Remote 三种入口。Remote 通过 `velaterm.com` 账号访问同账号客户端开放的项目和 AI 会话；系统后台推送尚未接入。旧根目录 `mobile/` 已删除。

## 使用方式

- **URL**：填写 VelaTerm 服务的 HTTPS 地址，可保存服务密码或进入网页后登录。当前 HTTP 只允许回环地址；局域网地址也需 HTTPS。自签名证书需在原生对话框中核对并确认指纹，证书变化会再次确认。页面加载时同一指纹的多个并发校验只弹出一个对话框，共用同一决定；不同主机或已变化的指纹依次排队，同一时间最多显示一个对话框。无法弹出对话框时按取消处理，连接失败而不是一直等待；确认后才写入信任记录，取消不写入。加载失败页会附带系统错误说明及错误域和错误码。
- **SSH**：填写主机、SSH 端口、用户名和密码或私钥。首次连接和主机指纹变化均需明确确认。手机在本机建立 SSH 端口转发，再通过 WebView 访问远端回环 HTTP 服务。
- **Remote**：点击右上角账号图标登录，再在“我的设备”中查看各设备的共享内容。设备在线且服务端确认共享已就绪时，才可在独立窗口打开工作空间、项目或会话；离线、未共享或共享未就绪时不提供打开操作。设备状态每 5 秒刷新。通过 E2EE 访问 AI 会话，不开放直接终端和文件管理接口。
- **服务选择**：可以指定已有 VelaTerm 回环 HTTP 服务端口及密码，也可以自动读取远端服务记录。勾选“允许下载并启动”后，才允许安装及启动服务。
- **手机视图**：保留现有完整 App，在窄屏通过项目、会话、文件页切换。后端为 Claude、Codex、OpenCode 选择会话视图，其余使用终端。已有运行中的终端代理不会自动停止；用户可以确认切换或保留终端。

SSH 两端均支持密码和 OpenSSH Ed25519 私钥。iOS 当前 SSH 依赖的私钥解密支持 AES-128/256-CTR，bcrypt 轮数须小于 32；测试覆盖未加密私钥及 AES-256-CTR、16 轮 bcrypt 的私钥。自定义高轮数或其他加密格式会明确报错。iOS 不启用依赖中的旧 RSA/SHA-1 认证；RSA 用户需使用其他认证方式，RSA SHA-2 支持仍待补齐。Android 使用 SSHJ 的 OpenSSH 私钥解析器，RSA 路径尚未单独验证。

默认视图选择依赖服务端的 `prepare_mobile_session` RPC。旧服务会显示升级提示，用户可选择终端继续访问。构建手机壳不会更新远端 Web 界面或服务端程序。资源生成器以 `src-tauri/tauri.conf.json` 为服务端版本来源，写入前核对根 `package.json` 与 Cargo 版本一致，再同步自动下载脚本和 runtime；本批对应现有版本 `0.2.2`，手机包仍为 `0.1.0`，未升版或发布。已发布的同版本服务端产物不能视为包含本批尚未发布的修复，远端仍需更新到相应实现。

## 构建

需要 Node.js 22、pnpm；iOS 需要 Xcode 和 iOS 17 以上，Android 需要 JDK 21、Android SDK 36，最低 Android 7（API 24）。本机使用 Xcode 26.2、iOS 26.3 模拟器验证。

```sh
cd apps/mobile
pnpm install --frozen-lockfile
pnpm sync
pnpm ios
# 或
pnpm android
```

命令行构建 Android 调试包：

```sh
cd android
./gradlew :app:assembleChinaDebug
```

产物为 `android/app/build/outputs/apk/china/debug/app-china-debug.apk`。Kotlin 增量编译在 `android/gradle.properties` 显式开启；日常构建不执行 `clean`。依赖、编译参数或源码变化会重新编译相关任务，缓存损坏或发布复现检查才需要清理。

iOS 模拟器构建：

```sh
xcodebuild -project ios/App/App.xcodeproj -scheme App \
  -configuration Debug -destination 'generic/platform=iOS Simulator' \
  -derivedDataPath .build/ios CODE_SIGNING_ALLOWED=NO build
```

真机安装需要在 Xcode 中设置开发团队及签名。2026-09-09 曾完成开发签名构建并安装到 iPhone 17 Pro，当时自动启动因锁屏被拒绝。2026-09-20 复核时，该设备已使用 iOS 27.0（24A437），现有 Xcode 26.2 无法挂载所需的开发者磁盘映像（DDI），因此本批没有安装或运行真机新包。当前没有生成可分发 IPA，也没有发布应用或创建推送资源。

### Android 渠道与签名

Android 分为两个渠道（product flavor）：`china` 集成个推及华为、小米、OPPO、vivo、魅族、荣耀厂商推送通道，是默认渠道；`play` 不包含上述推送 SDK，任务通知显示为“此构建尚未配置推送通道”。个推国内版 SDK 不符合 Google Play 的上架要求，因此两个渠道分开构建。推送 SDK 的调用集中在 `PushProvider`，两个渠道分别位于 `plugins/remote/android/src/china/` 与 `src/play/`。推送服务部署前，自行分发的 APK 同样使用 `play` 渠道。

release 构建使用发布密钥签名。密钥与密码不进仓库，默认从 `~/.config/velaterm/android-release.properties` 读取（权限 0600，字段为 `storeFile`、`storePassword`、`keyAlias`、`keyPassword`），也可以用 Gradle 属性或环境变量 `VELA_ANDROID_SIGNING` 指定其他路径；文件不存在时 release 构建不签名。已安装的 APK 只能被同一密钥签名的新版本覆盖升级，密钥遗失后用户须卸载重装，因此须另行备份 `~/.config/velaterm/` 下的密钥库与配置文件。日后上架 Google Play 时，在 Play 应用签名中选择使用现有密钥并上传这把密钥，已安装的用户即可从 Play 继续升级。

```sh
cd apps/mobile
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
pnpm apk:play      # 自行分发的 APK：android/app/build/outputs/apk/play/release/app-play-release.apk
pnpm bundle:play   # Google Play 的 AAB：android/app/build/outputs/bundle/playRelease/app-play-release.aab
```

GraalVM JDK 的 `jlink` 无法处理 Android SDK 36 的 `core-for-system-modules.jar`，需使用 Android Studio 自带的 JDK。每次发布新版本前须递增 `android/app/build.gradle` 中的 `versionCode`。APK 上传到 `dl.velaterm.com/android/<版本>/`（R2 与腾讯云 COS 同名镜像，国内使用 `dl.velaterm.cn`）。

`pnpm dev` 仅预览连接首页，固定监听 `127.0.0.1:41571`，浏览器中不会模拟原生 SSH 功能。远端项目 Web 界面仍从仓库根目录构建。

## 两个平台如何保持一致

`src/remote.ts` 定义统一插件接口：`list/save/remove/connect/disconnect/status/scanURL/account` 和 `state` 事件。Swift 与 Kotlin 分别实现系统安全存储、SSH、端口转发、项目 WebView 和账号操作。共享远端准备代码位于 `plugins/remote/shared/`；`pnpm sync` 生成压缩脚本和 iOS 资源副本，不能直接修改生成副本。

原生插件的用户可见文本（Swift、Kotlin 以及 Android 注入的 `download.js`）不写在源码里，而是来自共享词典 `src/i18n/locales/`：键位于 `mobile.native.*` 组，参数用 `{name}` 占位符表示，由 `MobileText.get(key, values)`（Swift 与 Kotlin 同名）在原生侧按设备语言解析并替换；返回给起始页的 `call.reject(...)` 错误也经此本地化。`scripts/sync-resources.mjs` 自动收集英文词典中所有 `mobile.*` 键，写入两份 `native-text.json`（`plugins/remote/shared/` 与 iOS `Bootstrap/`），新增原生文本时只需在各语言词典中添加键并运行 `node scripts/sync-resources.mjs`（`pnpm sync` 已包含此步骤）。`download.js` 的文本由 Android 插件在注入脚本前以 `window.__VELATERM_DOWNLOAD_TEXT__` 提供。 生成器还从两项 `mobile.native.*UsageDescription` 文案生成 11 种语言的 `InfoPlist.strings`，并将英文回退写入 `Info.plist`；iOS 相机和局域网权限说明与主词典保持一致。所有输入校验通过后才写入资源，内容不变时保留文件及其修改时间。

凭据保存在 iOS Keychain 或由 Android Keystore 密钥加密的存储中，不返回连接列表、不进入 URL、不写入前端 localStorage。远端网页没有 Capacitor SSH 插件桥接权限。网页只能通过现有服务的登录与权限访问项目。

远端准备需要 Python 3；下载安装还需要支持 Ed25519 的 OpenSSL。程序下载限定 `dl.velaterm.com`，校验 SHA-256 和桌面端同一 minisign 公钥。文件写入远端 `~/.velaterm/`，已有桌面数据库可复用。安装授权由连接表单中默认关闭的复选框表达。

远端服务端口首次从 `10000–49151` 选择，写入 `mobile-service.json`；手机隧道端口首次选择后保存在原生连接记录中，只有冲突时更换。断开手机连接不停止远端服务。前台恢复尝试重连，但不承诺操作系统允许 App 在后台永久维持 SSH。

## 验证

```sh
pnpm test
python3 -m unittest discover -s tests -p 'test_*.py'
sh scripts/test-ios-native.sh
```

`scripts/test-ios-native.sh` 用本机 `swiftc` 编译不依赖 UIKit 的 `TrustPromptCoordinator.swift` 及其测试，无需设备、模拟器或 xcodebuild。测试覆盖相同指纹合并、不同请求串行、取消与代次失效、重复回调、显示失败和超时。UIKit 弹窗及加载恢复页另由 iOS 模拟器原生测试验证；主机测试不能代替原生界面或真机验证。

`src/native-text.test.ts`（随 `pnpm test` 运行）对照真实产物检查原生文本：插件源码中的用户可见文案必须通过词典读取，调用键与占位符须在全部语言中完整匹配，两份 `native-text.json` 及 11 种语言的 `InfoPlist.strings` 须与主词典一致。`src/resource-sync.test.ts` 验证版本不一致、VERSION 缺失或重复时在写入前失败，以及资源内容和修改时间的幂等性。Android 的信任提示由主线程协调、worker 负责存储；本批已完成 JVM 测试、模拟器原生测试、编译和无源码变化的缓存复用验证。

原生集成测试使用本机隔离的 SSH/HTTP/WebSocket 测试服务，不执行用户远端命令：

```sh
python3 -m venv .build/ssh-fixture-venv
.build/ssh-fixture-venv/bin/pip install -r tests/requirements.txt
.build/ssh-fixture-venv/bin/python tests/ssh_fixture.py
```

保持测试服务运行。其端口保存在 `.build/fixtures/ports.json`，测试密钥、指纹及 iOS 测试资源也在此目录生成，均不提交。测试将临时替换模拟器 App 的连接记录，并在结束时恢复；只使用专用测试模拟器。

- iOS：将上述构建命令的 destination 换为已启动模拟器的 ID，并使用 `CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- CODE_SIGN_STYLE=Manual test`。Keychain 测试需要 ad-hoc 签名，不能用无签名测试宿主。
- Android：构建 `:app:assembleDebug :app:assembleDebugAndroidTest`。专用模拟器使用 ports.json 中的 `console/device/grpc` 端口，ADB 服务使用 `adb` 端口，然后运行 `python3 tests/run_android.py`。模拟器通过 `10.0.2.2` 访问本机 fixture。

2026-09-20 本批验证：两端模拟器构建与原生测试通过，Android 无源码变化的再次构建全部复用缓存。iOS 和 Android 模拟器均实际连接本机局域网地址上的隔离 VelaTerm TLS 服务，覆盖信任、拒绝后不重复提示、显式重试、旧代次失效及预置旧指纹后的确认；iOS 原生 URLSession 仍拒绝同一自签名证书。指纹变化测试使用预置旧记录，不能当作现场更换证书的证据。11 种语言的首页及连接表单在 vlx-browser 专用 Profile 中检查了布局、深层 URL、前进和后退，两端原生恢复页也逐语言检查；浏览器原生桥使用隔离替身，真实 TLS 连接由模拟器另行验证。

尚未完成物理手机的 LAN 与 Tailscale 验收：现有 iPhone 的系统版本与 Xcode 工具链不兼容，没有可用 Android 真机，也没有已配置的 Tailscale 测试端点。本批未执行公网账号或第三方登录、相机识别、系统权限和保存面板、后台推送，以及 Linux/macOS/Windows 真实远端自动安装验收。文件导出已接入 iOS 分享面板和 Android 保存面板，Android 单次上限为 64 MB；既有浏览器 Blob 导出验证不能替代系统面板验收。当前为部分完成，不能视为整体验收或发布完成。

## 图标维护

iOS 图标使用 `assets/icon-ios.svg` 满版母版，禁止复制桌面留白图标。修改后运行 `pnpm icons`；`pnpm build` 和 `pnpm sync` 会自动检查产物。完整要求见[项目图标规范](../../docs/design/图标规范_20260629_1043.md)。

## 扫码访问 URL

首页点击“扫码连接”，或在 URL 连接表单点击“扫码填写”。首次使用需允许相机权限。识别成功后显示服务地址，并自动补充空白的连接名称；确认地址后点击“保存并连接”。也可以仅保存连接，稍后打开。取消或识别失败不会覆盖原表单。

iOS 使用 AVFoundation，Android 使用内置 ZXing 解码器；画面仅用于本机识别，不保存照片、不上传画面。二维码需直接包含服务 URL，复用手动输入的原生校验：公网使用 HTTPS，不允许地址内嵌账号密码；HTTP 仅限设备回环地址。扫码结果不会自动访问，也不会写入导航 URL。

此前扫码功能的验证记录包括路由测试、两端构建、Android 无改动再次构建，以及 vlx-browser 专用 Profile 中的 390px 布局和 13 项交互检查。浏览器检查使用隔离页面的原生桥接替身，覆盖填入、取消、无效内容、权限错误、保存连接和历史导航；不能代替相机硬件测试。当时的版本曾安装到已授权的 iPhone；本批未安装或运行真机新包，iOS、Android 的实际相机识别及系统权限弹窗仍待真机验收。

## 手机视图接入约定

URL 和 SSH 连接直接打开目标地址，不追加视图参数，也不注入强制页面选择标记。网页使用项目原有的手机检测和 MobileApp，不得另建桌面手机外框或借手机连接切换会话引擎。手机布局保持独立，不参与桌面布局镜像同步。扫码与 App 版本显示仅属于手机连接壳。

## 版本与构建时间

连接列表及连接表单底部显示 App 版本号和北京时间的构建时间（精确到秒）。时间在 Vite 构建时固定，不是启动时间；原生版本由 `package.json` 提供，`pnpm sync` 同步到 iOS 与 Android。每次打包前运行 `pnpm sync`，再执行原生构建和安装。这里显示的是手机壳的版本，远端项目页面是否更新仍取决于远端服务。

## 复制连接与任务提醒（2026-09-13）

连接卡片提供“复制并编辑”，地址为 `#/connections/<id>/copy`。打开时只是草稿，取消不新增记录；保存时由原生层按 `copyFromId` 继承凭据，并分配新 ID、重新分配本地隧道端口。原连接保持不变。配置及凭据完全相同时复用现有记录，也不覆盖原名称；失效的来源不得退化为普通新增。

URL/SSH 项目网页可通过来源受限的 `__VELATERM_NOTIFICATIONS__` 调用原生通知。iOS 使用 UserNotifications，Android 使用通知渠道及运行时权限；前台会话也允许提醒，点击通知后重新连接并用 `session` 查询参数定位会话。App 安装与提供此调用的远端网页必须分别更新。账号 Remote 项目页也使用原生 WebView 接入这条桥接；账号登录继续使用系统浏览器。

前台本地通知依赖项目网页运行。APNs、Android 个推及厂商通道、远端订阅与 velaterm.com 中转代码已接入；实际后台和锁屏通知仍需平台凭据、部署及真机验收，不能将本地通知或构建成功当作后台推送验收。

手机首页、连接表单和账号页共用现有品牌图标母版；手机网页列表沿用网站的深浅色 Logo。不使用字母 V 作为品牌占位图，不重复绘制另一套 Logo。

## 账号登录与 Remote

账号页地址为 `#/account`，设备列表为 `#/remote`。登录使用系统浏览器窗口中的网站账号页，支持网站提供的邮箱密码和第三方登录。确认在此设备登录后，App 轮询领取设备凭据并显示账号；登录本身不会开启共享。退出会撤销该手机的设备凭据及由它建立的远程浏览器会话。

2026-09-13：网站、桌面和手机界面统一设备与共享内容的呈现方式，登录界面不再要求用户理解“绑定设备”。相关 29 项现有测试、网站与桌面前端构建、iOS 开发签名构建及 Android 调试构建通过。网站真实本地账号操作与手机宽度布局已验证；手机和桌面原生桥接在本轮浏览器验证中使用替身。用户确认后，新版账号页已更新到官网，公网资源、健康检查和浏览器复查通过。新版 App 尚未安装到真机，发布验收仍需完成既有真机检查。详见[界面整理报告](../../plans/processed/账号设备与共享界面整理_20260913.md)。

2026-09-13 后续：补齐在线与实际共享状态同步。官网已上线并实测离线入口拦截和在线工作空间访问；手机端已接入相同的服务端状态，11 项现有测试、前端与两端原生构建通过。浏览器中的原生桥接使用替身，本轮未更新真机安装。详见[状态同步报告](../../plans/processed/设备在线与共享状态同步_20260913.md)。

账号凭据只保存在原生安全存储中。打开远程窗口时，原生层申请 60 秒、单次有效的浏览器票据，用 fragment 交给网站兑换 HttpOnly 会话。账号登录使用 iOS SFSafariViewController 或 Android Custom Tabs；具体 Remote 项目使用原生 WebView，以接入通知和随时返回。远程目录和会话内容沿用网站 E2EE 通道。

2026-09-10：两端最终构建和 6 项手机测试通过；账号服务的设备隔离、票据兑换及撤销在本地真实服务中验证。390px 浏览器预览检查了账号页和客户端列表，无横向溢出，账号面板与列表间距为 24px；预览使用合成的原生接口响应。原生界面工具因运行环境配置错误不可用，手机真机授权、返回 App 和独立窗口仍待验收。新增 Remote 接口及 V11 数据迁移尚未部署到公网，本次产物未安装到用户设备。

2026-09-12：补齐登录过程的网络重试、跨页面轮询和待确认请求恢复。待确认请求与账号凭据一样保存在原生安全存储中；App 重启后进入账号或 Remote 页，会继续检查登录结果。登录请求是否过期由账号服务判定。Remote 列表显示同账号客户端及其开放范围数量，也保留尚未开启远程访问的客户端。

本次 11 项手机测试、iOS 模拟器构建、iOS 真机开发签名构建及 Android 调试构建通过；Android 再次构建的 91 项任务全部复用缓存。使用 vlx-browser 专用 Profile 验证了登录重试界面，以及真实本地中继中的共享范围、E2EE、手机会话页、深层链接、前进后退和访问撤销。登录界面的原生接口使用替身，不能代替真机授权测试。公网新隧道入口仍返回 404，本次未部署服务、未安装新版 App，整体验收尚未通过。详见[验收报告](../../plans/processed/远程访问与手机登录验收_20260912.md)。

同日后续：用户要求继续修复并验证后，已更新公网账号中继与 nginx，修复主机首次建立 HTTPS 隧道时缺少 rustls provider 初始化的问题。正式域名上的真实 E2EE 会话已收到 Claude 回复；范围隔离、终端拒绝、访客重连及撤销界面通过。iOS 和 Android 模拟器分别通过 1 项真实原生账号集成测试，涵盖安全存储恢复、授权和退出。新版已安装到 iPhone 17 Pro，但启动仍因锁屏被拒绝；未连接 Android 真机。现有主机客户端也需更新到对应协议版本。详见[上线与联调报告](../../plans/processed/远程访问与手机真机联调_20260912.md)。

原生账号集成测试默认不访问公网。显式运行前，应准备可删除的专用账号并在 vlx-browser 测试 Profile 登录；测试启动后会将待确认设备的 code 和 URL 写入 App 私有目录的 `remote-account-request.json`，由该 Profile 完成确认。iOS 需预先在 App Documents 目录创建 `remote-account-fixture.json` 标记，并只运行 `AppTests/RemoteAccountIntegrationTests`；测试结束删除标记。Android instrumentation 需显式传入 `-e remoteAccount true -e class com.velaterm.mobile.RemoteAccountIntegrationTest`。测试恢复原有安全存储，验证结束后还应删除专用账号和遗留测试文件。这些测试不代替物理手机中的第三方登录与后台恢复验收。

### 连接期间的加载与返回

点击 URL 或 SSH 连接后，手机壳立即显示居中的加载状态和返回入口。远端 WebView 首次加载、重新连接时继续显示原生加载页；只有检测到已渲染的项目界面或可操作的登录页面后才移除，不以 HTML 加载完成作为界面可用的依据。加载超过 30 秒时显示恢复提示，网络错误、空白文档和脚本未启动时仍可返回。

页面就绪检测由 App 注入 `plugins/remote/shared/page-readiness.js`，仅接受当前服务主框架的回报，因此此项修复只需更新手机 App。返回立即关闭页面，旧连接资源在后台释放，取消后的连接结果不能重新打开页面。
# 任务推送

通知设置位于“账号 → 任务通知”。后台推送需要远端、velaterm.com 中转及带推送配置的原生包同时就绪；未配置的开发包会明确显示状态。详细数据边界、签名、Android 厂商配置与验收清单见 [手机任务推送](../../docs/design/mobile-push_20260913.md)。

通知显示会话名称和本轮回复的简短摘要，点击后重新连接并打开对应会话。摘要经过既定推送通道，系统通知可见范围由手机的通知预览设置决定。没有可用正文时显示状态提示；Android 厂商通道会按较短的长度限制截取。
