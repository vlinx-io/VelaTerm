# 分屏异常诊断

- 创建时间: 2026-09-05 20:27

原生菜单中的分屏命令（包括 macOS 的 `⌘D`、`⌘⇧D`）只发送给当前焦点窗口。没有焦点窗口时不执行命令。本地窗口、SSH 窗口和配对 URL 窗口不会因为另一个窗口的菜单操作而同时分屏。

界面镜像功能仍按其配置同步布局。开启镜像的客户端可能正常接收到其他客户端创建的分屏，这类事件的来源记录为 `mirror`。

## 日志位置

更新并重新构建应用后，分屏诊断日志会写入应用数据目录下的 `logs/split.log`。

- macOS 发行版：`~/Library/Application Support/io.vlinx.vlxterm.release/logs/split.log`。
- macOS 默认开发版：`~/Library/Application Support/io.vlinx.vlxterm/logs/split.log`；使用独立应用标识的实例以其实际数据目录为准。
- SSH、配对 URL 原生窗口：写入承载该窗口的本地桌面应用日志，`clientId` 为本地窗口标签。
- 普通浏览器、Electron：写入所连接后端的数据目录，`clientId` 为后端分配的连接标识。
- 无界面服务：写入其数据目录下的 `logs/split.log`，数据目录可由既有的 `--data-dir` 参数指定。

每个日志文件最多约 4 MiB，轮转时只保留一个 `split.previous.log`。Unix 系统中新建日志文件的权限为 `0600`。

启动应用或服务前可设置环境变量：

| 变量 | 默认行为 | 配置说明 |
| --- | --- | --- |
| `VLX_SPLIT_LOG_DIR` | 应用数据目录下的 `logs/` | 覆盖分屏日志目录 |
| `VLX_SPLIT_LOG_LEVEL` | `info` | `info`、`debug`、`trace` 记录分屏事件；`warn`、`error`、`off` 不记录这些信息级事件 |

## 记录内容

控制台和文件使用相同格式：`yyyy-MM-dd HH:mm:ss [INFO ] [system] event=split clientId=… data=…`。行首为服务端时间；结构化数据中的 `clientAtMs` 是客户端触发时的 Unix 毫秒时间，可用于跨窗口对照。

记录包含来源、会话 ID、父会话 ID、页签 ID 及分屏方向。来源分别为：

| `source` | 含义 |
| --- | --- |
| `shortcut` | 前端快捷键 |
| `menu` | 原生菜单或应用菜单，包括原生菜单快捷键 |
| `pane-button` | 终端标题栏的分屏按钮 |
| `mirror` | 通过界面镜像收到的分屏 |
| `unknown` | 未标注来源的内部调用 |

日志不包含终端输入输出、会话名称、工作目录、服务器地址或认证信息。镜像事件只记录收到的会话 ID，不伪造本地父会话或方向。

当前窗口最近 200 条记录仍可通过开发者工具中的 `window.__vlxSplitLog` 查看。`persistence` 为 `pending`、`saved`、`disabled` 或 `failed`，分别表示正在写入、后端已确认写入、后端配置为不记录信息级事件或写入失败。窗口重载会清空内存记录，已写入的文件不受影响。后端不可用或目录不可写时不会阻止分屏，控制台会提示失败，不会自动重试；窗口关闭前尚未完成的请求也可能未落盘。

## 回归验证

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib --features native-menu-tests native_menu::tests
cargo test --manifest-path src-tauri/Cargo.toml --lib --no-default-features split_trace::tests
pnpm exec vitest run src/store/splitTrace.test.ts src/hooks/shortcutRegistry.test.ts src/hooks/useKeyboardShortcuts.test.tsx src/layout/TitleBar/AppMenuBar.test.tsx src/layout/CenterPane/keepAlive.test.tsx
pnpm exec tsc --noEmit
```

原生事件测试使用 Tauri 模拟运行时建立三个窗口，验证命令只送达指定窗口，以及无焦点或焦点标签失效时不向其他窗口发送。此测试不启动真实桌面窗口。
