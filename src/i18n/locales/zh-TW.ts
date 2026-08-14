//! Traditional Chinese dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const zhTW: typeof en = {
  // ── Common ──
  "common.cancel": "取消", // Cancel
  "common.confirm": "確定", // OK
  "common.delete": "刪除", // Delete
  "common.save": "儲存", // Save
  "common.create": "建立", // Create
  "common.close": "關閉", // Close
  "common.copy": "複製", // Copy
  "common.cut": "剪下", // Cut
  "common.paste": "貼上", // Paste
  "common.selectAll": "全選", // Select All
  "common.copied": "已複製", // Copied
  "common.retry": "重試", // Retry
  "common.refresh": "重新整理", // Refresh
  "common.loading": "載入中…", // Loading…
  "common.prev": "上一個", // Previous
  "common.next": "下一個", // Next
  "common.on": "開", // On
  "common.off": "關", // Off
  "common.gotIt": "知道了", // Got it
  "common.rename": "重新命名", // Rename
  "common.edit": "編輯", // Edit
  "common.open": "開啟", // Open
  "common.session": "會話", // Session

  // ── Session types and status ──
  "kind.terminal": "終端機", // Terminal
  "kind.browser": "瀏覽器", // Browser
  "status.idle": "閒置", // Idle
  "status.running": "執行中", // Running
  "status.exited": "已結束", // Exited
  "status.error": "異常", // Error
  "status.working": "處理中", // Working
  "status.asking": "待確認", // Needs confirmation
  "status.waiting": "已查看", // Viewed
  "status.unavailable": "狀態無法取得",
  "indicator.unread": "未讀 · 待查看", // Unread · awaiting review

  // ── Title bar ──
  "titlebar.builtAt": (time) => `建置於 ${time}`, // Built at {time}
  "titlebar.versionMismatch": (frontend, backend) =>
    `版本不一致：前端 v${frontend} ≠ 後端 v${backend}，請重新建置或同步部署。`, // Version mismatch

  "titlebar.hotReloadedAt": (time) => `熱更新於 ${time}`, // Hot reloaded at {time}
  "titlebar.themeSystem": (resolved) => `跟隨系統（目前${resolved}）`, // Follow system (currently {resolved})
  "titlebar.themeDark": "深色", // Dark
  "titlebar.themeLight": "淺色", // Light
  "titlebar.browser": "內建瀏覽器（⌘⇧B）", // Built-in Browser (⌘⇧B)
  "titlebar.remoteAccess": "遠端存取（瀏覽器）", // Remote Access (Browser)
  "titlebar.connectRemote": "連線到遠端服務", // Connect to Remote Server
  "titlebar.share": "分享", // Share
  "share.title": "分享 VelaTerm", // Share VelaTerm
  "share.subtitle":
    "我們是 VelaTerm 背後的一個小團隊。如果你喜歡它，歡迎把 VelaTerm 分享給更多人。讓更多人知道我們，對我們真的很重要。謝謝你的支持！❤️", // We're a small team behind VelaTerm. If you enjoy it, please share VelaTerm with others…
  "share.copyLink": "複製連結", // Copy link
  "share.copied": "已複製！", // Copied!
  "share.wechatMoments": "微信朋友圈",
  "share.weibo": "微博",
  "share.xiaohongshu": "小紅書",
  "share.xiaohongshuAction": "複製分享文案和連結，然後開啟小紅書創作中心",
  "share.wechatQrTitle": "分享到微信朋友圈",
  "share.wechatQrHint": "請使用微信掃碼開啟連結，再選擇「分享到朋友圈」。",
  "share.backToPlatforms": "返回分享平台",
  "titlebar.appearance": "外觀設定", // Appearance
  "titlebar.showLeft": "顯示左欄", // Show sidebar
  "titlebar.hideLeft": "隱藏左欄", // Hide sidebar
  "titlebar.showRight": "顯示資訊面板", // Show info panel
  "titlebar.hideRight": "隱藏資訊面板", // Hide info panel

  // ── Settings ──
  "settings.title": "設定", // Settings
  "settings.catTerminal": "終端機", // Terminal
  "settings.catBehavior": "行為", // Behavior
  "settings.catAgents": "智能體", // Agents
  "settings.permDefault": "預設", // Default
  "settings.permYolo": "YOLO", // YOLO
  "settings.yoloHint": (flag: string) =>
    `啟動時附加 ${flag}，跳過全部權限確認，請謹慎使用。`,
  "settings.permViaEnvHint":
    "透過設定檔注入跳過全部權限確認（無命令列旗標）。僅影響該工作階段啟動時的行為。", // YOLO flag hint
  "settings.catGeneral": "一般", // General
  "settings.cliLabel": "Shell 指令",
  "settings.cliInstall": "安裝 ‘vela’ 指令",
  "settings.cliUninstall": "解除安裝 ‘vela’ 指令",
  "settings.cliInstalledAt": (path: string) => `已安裝至 ${path}`,
  "settings.cliConflict": (path: string) =>
    `${path} 已存在其他 ‘vela’ 指令，VelaTerm 不會覆寫它。`,
  "settings.cliHint": "像 VS Code 的 `code` 一樣，將 `vela <專案路徑>` 加入 PATH。",
  "settings.agentArgsHint":
    "各類型智能體新建工作階段時套用的預設啟動參數。新建或編輯單個工作階段時設定的參數會覆寫此處的預設。留空表示不帶參數。", // Agent default launch args hint
  "settings.agentPathLabel": "可執行檔路徑（可選）", // Executable path (optional)
  "settings.agentPathPlaceholder": "如 ~/.local/bin/claude——留空則從 PATH 尋找", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "設定後，該類型工作階段一律按這條完整路徑啟動，不再從 PATH 尋找命令。適用於「已安裝但不在 shell PATH 上」的情況。一鍵安裝成功且能偵測到安裝位置時會自動填入。", // Agent executable path hint
  "settings.appearance": "外觀", // Appearance
  "settings.accent": "強調色", // Accent
  "settings.accentAuto": "跟隨明暗", // Follow theme
  "settings.density": "密度", // Density
  "settings.densityCompact": "緊湊", // Compact
  "settings.densityRegular": "標準", // Regular
  "settings.densityComfy": "寬鬆", // Comfy
  "settings.pane": "分割窗格", // Panes
  "settings.paneFlush": "無縫", // Flush
  "settings.paneCard": "卡片", // Card
  "settings.divider": "分隔線", // Divider
  "settings.dividerSubtle": "極細", // Subtle
  "settings.dividerVisible": "可見", // Visible
  "settings.nav": "左欄", // Sidebar
  "settings.navTree": "標準", // Tree
  "settings.navCompact": "緊湊", // Compact
  "settings.tabs": "分頁", // Tabs
  "settings.dynamicStatusFilter": "狀態篩選動態增加",
  "settings.tabSingle": "單分頁", // Single
  "settings.tabMulti": "多分頁", // Multi
  "settings.maxLiveTabs": "背景保活上限", // Background limit
  "settings.defaultShell": "預設 Shell", // Default shell
  "settings.spawnConfirm": "派生前確認", // Confirm before spawn
  "settings.usageRefresh": "額度刷新", // Usage refresh
  "settings.cleanImages": "自動清理貼上的圖片",
  "settings.cleanImagesHint":
    "貼上或拖入終端的圖片會先存成暫存檔（把路徑傳給 agent）。開啟後：結束時刪除本次工作階段產生的這些暫存圖，啟動時清理超過 24 小時的殘留。文件內的圖片不受影響。",
  "settings.cleanImagesNow": "立即清理",
  "settings.cleanImagesResult": (n: number, size: string) =>
    `已清理 ${n} 個暫存圖片（釋放 ${size}）。`,
  "settings.cleanImagesEmpty": "沒有需要清理的暫存圖片。",
  "settings.imagePasteMode": "圖片貼上",
  "settings.imagePasteUpload": "貼上檔案路徑",
  "settings.imagePasteAgent": "原生圖片貼上",
  "settings.imagePasteHint":
    "選擇貼上圖片時寫入的內容（僅本機桌面端）。貼上檔案路徑：把圖片存成暫存檔，在輸入框顯示可讀路徑（Codex 顯示 image_path: …）。原生圖片貼上：觸發 Claude 或 Codex 讀取系統剪貼簿並顯示自身的圖片預留位置。",
  "settings.imagePasteRemoteHint":
    "遠端工作階段固定貼上檔案路徑，讓智能體能在其所在機器讀取圖片；原生圖片貼上僅在本機桌面端可用。",
  "spawn.title": "啟動派生會話？", // Start spawned session?
  "spawn.fromSession": "來自", // From
  "spawn.promptLabel": "提示詞", // Prompt
  "spawn.agentLabel": "智能體", // Agent
  "spawn.modelLabel": "模型", // Model
  "spawn.effortLabel": "推理強度", // Effort
  "spawn.worktreeLabel": "獨立 git worktree", // Separate git worktree
  "spawn.launch": "啟動", // Launch
  "spawn.remaining": (n: number) => `還有 ${n} 個待確認`, // ${n} more pending
  "spawn.notifyTitle": "派生會話待確認", // Spawn session awaiting confirmation
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "查看變更…",
  "changes.title": "變更",
  "changes.loading": "載入中…",
  "changes.loadingDiff": "載入 diff…",
  "changes.noChanges": "沒有變更",
  "changes.selectFile": "選擇檔案查看",
  "changes.binary": "二進位檔案，無法逐行 diff",
  "tree.merge": "合併…",
  "tree.copyWorktreePath": "複製 worktree 路徑",
  "tree.openWorktreeDir": "開啟 worktree 目錄",
  "tree.deleteWorktreeMenu": "刪除 worktree…",
  "tree.deleteWorktreeTitle": "刪除 worktree",
  "tree.deleteWorktreeBody": "選擇要刪除的 worktree，會從磁碟上刪掉它的工作目錄。",
  "tree.deleteWorktreePlaceholder": "選擇一個 worktree…",
  "tree.deleteWorktreeForce": "強制刪除（捨棄未提交的變更）",
  "tree.convertToNormalSession": "轉為普通會話",
  "tree.convertToNormalGroup": "轉為普通群組",
  "merge.title": "合併分支",
  "merge.desc": "選好來源分支與目標分支，把來源合併進目標；方向可用中間按鈕調換。",
  "merge.notRepo": "該會話目錄不是 git 倉庫。",
  "merge.loadingBranches": "正在讀取分支…",
  "merge.loadingDiff": "正在載入差異…",
  "merge.sourceLabel": "來源分支",
  "merge.targetLabel": "目標分支",
  "merge.selectBranch": "選擇分支…",
  "merge.swap": "調換方向",
  "merge.pickHint": "選好來源與目標分支後，這裡會顯示合併將帶入的變更。",
  "merge.changes": (target: string) => `將帶入「${target}」的變更`,
  "merge.noChanges": "沒有檔案變更。",
  "merge.sameBranch": "來源與目標是同一條分支。",
  "merge.branchGone": "所選分支已不存在，請重新選擇。",
  "merge.upToDate": "目標分支已包含來源分支的變更，無需合併。",
  "merge.targetNotCheckedOut": (target: string) =>
    `目標分支「${target}」沒有被任何工作樹 checkout，無法本機合併。請先在某個工作樹切到該分支。`,
  "merge.targetDirty": "目標分支所在工作樹有未提交變更，合併可能受阻。",
  "merge.sourceDirtyNote": "來源分支所在工作樹有未提交變更，會先提交再合併。",
  "merge.commitMsgLabel": "提交訊息",
  "merge.commitMsgPlaceholder": "描述這次變更（作為提交訊息）",
  "merge.apply": "合併",
  "merge.commitAndApply": "提交並合併",
  "merge.working": "正在合併…",
  "merge.doneMsg": (source: string, target: string) => `已把「${source}」合併進「${target}」。`,
  "merge.conflictMsg": (target: string) =>
    `合併出現衝突，請到「${target}」所在工作樹的終端機裡解決後提交：`,
  "merge.close": "關閉",
  "gitea.title": "Gitea 整合",
  "gitea.desc":
    "設定 Gitea 伺服器後，可用「開 PR」的方式落地 worktree。token 存進系統鑰匙圈（不可用時退回明文）。",
  "gitea.baseUrl": "伺服器位址",
  "gitea.token": "存取 token",
  "gitea.tokenSet": "已儲存（留空則保留）",
  "gitea.tokenPlaceholder": "個人存取 token",
  "gitea.test": "測試連線",
  "gitea.saved": "已儲存。",
  "settings.renderer": "終端算繪器", // Terminal renderer
  "settings.redrawOnReveal": "切回分頁時重繪", // Redraw on tab switch
  "settings.catAdvanced": "進階", // Advanced
  "settings.outputScheduler": "前台優先輸出", // Foreground-priority output
  "settings.recordSessions": "記錄會話日誌", // Record session logs
  "settings.recordSessionsHint":
    "預設關。開啟後會把會話的終端輸出存成日誌檔，供歸檔回放與搜尋。普通終端會話一律不錄；agent 會話歸檔讀自己的對話記錄。", // Record session logs hint
  "settings.fonts": "字型", // Fonts
  "settings.uiFont": "介面字型", // Interface font
  "settings.uiFontSize": "介面字級", // Interface size
  "settings.termFont": "終端機字型", // Terminal font
  "settings.termFontSize": "終端機字級", // Terminal size
  "settings.fontDefault": "預設", // Default
  "settings.fontCustom": "自訂…", // Custom
  "settings.fontAuto": "自動", // Auto
  "settings.fontSmaller": "縮小", // Smaller
  "settings.fontLarger": "放大", // Larger
  "settings.fontReset": "重設", // Reset
  "settings.sound": "通知提示音", // Notification sound
  "settings.language": "語言", // Language
  "settings.langAuto": "自動（跟隨系統）", // Auto (system)
  "settings.skillLabel": "Vela 技能",
  "settings.skillInstall": "安裝", // Install
  "settings.skillInstalled": "重新安裝", // Reinstall
  "settings.skillInvokeHint":
    "Claude：/vspawn <任務>；Codex：$vspawn <任務>。安裝後若 Codex 未列出技能，請建立新的 Codex 工作階段。",
  // Notification permission guidance
  "settings.notify": "系統通知", // System notifications
  "settings.notifyGranted": "已開啟", // Enabled
  "settings.notifyAllow": "允許通知", // Allow notifications
  "settings.notifyOffHint":
    "允許 VelaTerm 在智能體需要你輸入或任務完成時通知你。", // Allow VelaTerm to alert you when an agent needs your input or finishes a task.
  "settings.notifyDeniedHint": "通知已被系統封鎖。開啟方法：", // Notifications are blocked. To turn them on:
  "settings.notifyStepsMac":
    "開啟「系統設定 ▸ 通知 ▸ VelaTerm」，開啟「允許通知」（建議樣式選橫幅或提醒）。", // open System Settings ▸ Notifications ▸ VelaTerm and turn on Allow Notifications (Banners or Alerts recommended).
  "settings.notifyStepsWin":
    "開啟「設定 ▸ 系統 ▸ 通知」，啟用 VelaTerm，並確認「專注輔助 / 勿擾」沒有封鎖它。", // open Settings ▸ System ▸ Notifications, enable VelaTerm, and make sure Focus assist / Do not disturb isn't blocking it.
  "settings.notifyStepsLinux":
    "在桌面環境的「設定 ▸ 通知」裡允許 VelaTerm。", // open your desktop's Settings ▸ Notifications and allow VelaTerm.
  "settings.notifyStepsBrowser":
    "點擊網址列的站點權限圖示，把通知設為「允許」。", // click the site-permission icon in the address bar and set Notifications to Allow.
  "settings.notifyUnsupported": "目前環境不支援系統通知。", // Notifications aren't available in this environment.
  "settings.notifyOpenSettings": "開啟系統設定", // Open System Settings
  // Shortcut categories
  "settings.catShortcuts": "快捷鍵", // Shortcuts
  "settings.scOpenProject": "開啟專案", // Open project
  "settings.scNewTab": "新增終端機", // New terminal
  "settings.scNewBrowserTab": "新增瀏覽器分頁", // New browser tab
  "settings.scClosePane": "關閉窗格／分頁", // Close pane / tab
  "settings.scSplitRight": "向右分割", // Split right
  "settings.scSplitDown": "向下分割", // Split down
  "settings.scSearch": "在終端機中搜尋", // Find in terminal
  "settings.scGlobalSearch": "搜尋所有工作階段", // Search all sessions
  "settings.scSaveDoc": "儲存文件", // Save document
  "settings.scRecording": "請按下按鍵…", // Press keys…
  "settings.scHint": "點一下快捷鍵，再按下新的組合鍵（需含 Cmd/Ctrl）。", // hint
  "settings.scReset": "還原為預設", // Restore defaults
  "settings.scConflict": (label: string) => `已被「${label}」使用`, // conflict

  // ── Remote access panel ──
  "remote.title": "遠端存取（瀏覽器）", // Remote Access (Browser)
  "remote.desc":
    "啟用後，同一區域網路的裝置用瀏覽器開啟下方位址、輸入密碼，即可獲得與桌面一致的介面。", // Once enabled, devices on the same LAN…
  "remote.needPassword": "請先設定存取密碼", // Please set an access password first
  "remote.running": (port) => `執行中 · 連接埠 ${port}`, // Running · port {port}
  "remote.urlsHint":
    "用瀏覽器開啟下面與你裝置同一 WiFi / 網段的位址（多張網卡時挑對的那個；VPN/隧道位址排在最後，外部裝置多半連不上）：", // Open the address on the same WiFi / subnet…
  "remote.copyUrl": "點擊複製位址", // Click to copy address
  "remote.moreUrls": (n: number) => `其它 ${n} 個連結`, // N more urls
  "remote.lessUrls": "收起", // Show less
  "remote.stop": "停止服務", // Stop Server
  "remote.passwordPlaceholder": "設定存取密碼", // Set access password
  "remote.starting": "啟動中…", // Starting…
  "remote.start": "啟動服務", // Start Server
  "remote.portLabel": "連接埠", // Port
  "remote.portInvalid": "連接埠必須是 1 到 65535 之間的數字", // Port must be between 1 and 65535
  "remote.fingerprintLabel": "憑證指紋（SHA-256）", // Certificate fingerprint (SHA-256)
  "remote.fingerprintHint":
    "首次連線時瀏覽器會提示憑證不受信任，這是自簽憑證的正常現象；核對此指紋可確認連線的是本機。", // On first connect, browsers warn the certificate is untrusted…

  "remote.pairingCreate": "產生配對連結", // Create pairing link
  "remote.pairingRegenerate": "重新產生連結（踢掉全部裝置）", // Regenerate link (disconnects all)
  "remote.pairingCreating": "產生中…", // Generating…
  "remote.pairingHint":
    "用瀏覽器開啟後輸入密碼。連結含存取憑證，只分享給自己的裝置。", // Open in a browser, then enter the password…

  "remote.devicesLabel": "已配對裝置", // Paired devices
  "remote.lastSeen": "最後連線", // Last seen
  "remote.revoke": "撤銷", // Revoke
  "remote.deviceBlock": "禁止存取", // Block
  "remote.deviceBlockConfirm": "確認禁止", // Confirm block
  "remote.deviceBlockHint":
    "被禁裝置會被中斷且無法重連（需重新用配對連結），其他裝置不受影響。", // Block hint
  "remote.devicesEmpty": "尚無已配對裝置", // No paired devices yet

  // ── Remote connection panel ──
  "connect.title": "連線到遠端服務", // Connect to Remote Server
  "connect.pairingPlaceholder": "貼上配對連結", // Paste pairing link
  "connect.confirmConnect": "指紋無誤，連線", // Fingerprint matches, connect
  "connect.desc": "輸入遠端 VelaTerm 的位址和密碼，在新視窗中連線並操控。", // Enter the address and password…
  "connect.addressPlaceholder": "IP 位址，如 192.168.1.100", // IP address, e.g. 192.168.1.100
  "connect.portPlaceholder": "連接埠", // Port
  "connect.connecting": "連線中…", // Connecting…
  "connect.connect": "連線", // Connect
  "connect.stagePreparing": "準備伺服器…",
  "connect.stageTransferring": "傳輸伺服器…",
  "connect.stageStarting": "啟動伺服器…",
  "connect.sshFingerprintLabel": (kt: string) => `SSH 主機指紋（${kt}）`,
  "connect.sshHostNew": "首次連線這台主機，請核對指紋一致後再繼續。",
  "connect.sshHostChanged":
    "⚠ 這台主機的金鑰變了：可能是伺服器重裝，也可能是中間人攻擊。確認無誤再繼續。",
  "connect.urlCertChanged":
    "⚠ 這台伺服器的憑證指紋自你上次確認後變了：可能是伺服器重裝，也可能是中間人攻擊。確認無誤再繼續。",
  "connect.sshPasswordLabel": "SSH 密碼",
  "connect.sshPasswordPlaceholder": "帳戶密碼",
  "connect.savedHosts": "最近連線",
  "connect.savedHostsAll": "全部最近主機",
  "connect.showAllHosts": (n: number) => `檢視全部 (${n})`,
  "connect.forgetHost": "忘記此主機",
  "connect.savedHasPassword": "已儲存密碼",
  "connect.rememberPassword": "記住密碼",
  "connect.urlPasswordPlaceholder": "登入密碼",
  "connect.shareDesktopDb": "共用遠端桌面版的資料庫",
  "connect.shareDesktopDbHint":
    "與遠端機器的桌面版共用同一資料庫（建議兩邊同版本）。不勾則使用獨立資料庫。",

  // ── Sidebar (project tree, menus, and dialogs) ──
  "tree.newSession": "新增會話", // New Session
  "tree.newTerminalSession": "新增終端機會話", // New Terminal Session
  "tree.newBrowserPage": "新增瀏覽器頁面", // New Browser Page
  "tree.newAgentSession": (agent) => `新增 ${agent} 會話`, // New {agent} Session
  "tree.newAgentSessionGroup": "更多 Agent 會話", // More Agent Session
  "tree.newAgentSessionCustom": "自訂參數新增…", // New with launch args…
  "tree.resumeSession": "恢復會話…", // Resume Session…
  "tree.newGroup": "新增群組", // New Group
  "tree.newSubgroup": "新增子群組", // New Subgroup
  "tree.newChildSession": "新增子會話", // New Child Session
  "tree.openSelected": "開啟選取的會話", // Open Selected Sessions
  "tree.archiveSelected": "封存選取的會話", // Archive Selected Sessions
  "tree.moveSelected": "移動所選到…", // Move Selected to…
  "tree.deleteSelected": (n) => `刪除選取的 ${n} 項`, // Delete {n} Selected Items
  "tree.removeProject": "移除專案", // Remove Project
  "tree.deleteGroup": "刪除群組", // Delete Group
  "tree.deleteSession": "刪除會話", // Delete Session
  "tree.projectRoot": "專案根（無群組）", // Project root (no group)
  "tree.moveToSession": "移到會話下（成為子會話）", // Move under a session (as child)
  "tree.moveTo": "移動到…", // Move to…
  "tree.openNewTab": "在新分頁開啟", // Open in New Tab
  "tree.forkSession": "Fork 會話", // Fork Session
  "tree.exportSession": "匯出會話…", // Export Session…
  "tree.sessionInfo": "會話資訊", // Session Info
  "tree.groupInfo": "分組資訊", // Group Info
  "info.branch": "分支", // Branch
  "info.path": "路徑", // Path
  "info.recentCommits": "最近提交", // Recent Commits
  "info.noCommits": "無提交", // No commits
  "tree.killProcess": "結束處理程序", // Kill Process
  "tree.archiveSession": "封存會話", // Archive Session
  "tree.archiveGroup": "封存分組", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "臨時", // scratch
  "tree.persistSession": "轉為永久會話…", // Make Permanent Session…
  "tree.persistDoc": "儲存到磁碟…", // Save to Disk…
  "tree.closeScratch": "關閉草稿", // Close Scratch
  "tree.importProject": "匯入專案", // Import Project
  "tree.createProject": "建立專案",
  "tree.cloneProject": "從 Git 複製", // Clone from Git
  "createProject.title": "建立專案",
  "createProject.name": "專案名稱",
  "createProject.namePlaceholder": "我的專案",
  "createProject.into": "建立位置",
  "createProject.choose": "選擇…",
  "createProject.noParent": "請選擇上層資料夾",
  "createProject.invalidName": "請輸入不含 / 或 \\ 的單一資料夾名稱。",
  "createProject.creating": "正在建立…",
  "createProject.submit": "建立專案",
  "clone.title": "複製 Git 儲存庫", // Clone Git Repository
  "clone.url": "儲存庫網址", // Repository URL
  "clone.urlPlaceholder": "https://… 或 git@…",
  "clone.branch": "分支（選填）", // Branch (optional)
  "clone.branchPlaceholder": "留空則用預設分支", // Default branch if empty
  "clone.folder": "資料夾名稱", // Folder name
  "clone.folderPlaceholder": "留空則自動取儲存庫名稱", // Auto from URL
  "clone.into": "複製到", // Clone into
  "clone.choose": "選擇…", // Choose…
  "clone.noParent": "請選擇一個上層資料夾", // Choose a parent folder
  "clone.cloning": "複製中…", // Cloning…
  "clone.cancelling": "正在取消…",
  "clone.stageStarting": "正在啟動 Git…",
  "clone.stageConnecting": "正在連線至儲存庫…",
  "clone.stagePreparing": "正在準備物件…",
  "clone.stageReceiving": "正在接收物件…",
  "clone.stageResolving": "正在解析差異…",
  "clone.stageCheckout": "正在簽出檔案…",
  "clone.stageFinalizing": "正在完成複製…",
  "clone.stageImporting": "正在匯入專案…",
  "clone.elapsed": (seconds: number) => `已用時 ${seconds} 秒`,
  "clone.slowHint": "已連續 30 秒沒有進度，請檢查遠端機器的網路或 Proxy；你也可以取消後重試。",
  "clone.submit": "複製", // Clone
  "tree.globalSearch": "搜尋所有會話 (⌘⇧F)", // Search All Sessions (⌘⇧F)
  "tree.archivedSessions": "已封存會話", // Archived Sessions
  "tree.searchPlaceholder": "搜尋會話 / 群組…", // Search sessions / groups…
  "tree.clearSearch": "清空搜尋", // Clear search
  "tree.filterWorking": "工作中", // Working
  "tree.filterAsking": "等待處理", // Pending
  "tree.filterWaiting": "已查看", // Viewed
  "tree.filterStatus": "狀態篩選", // Filter by status
  "tree.refreshStatusFilter": "重新整理狀態篩選",
  "tree.refreshStatusMatch": "重新整理狀態",
  "tree.filterStatusSection": "狀態", // Status
  "tree.filterMarkSection": "標記", // Mark
  "tree.viewMainName": "主分身",
  "tree.viewUntitled": "未命名分身",
  "tree.viewDefaultName": (n) => `分身 ${n}`,
  "tree.viewPrimary": "主分身",
  "tree.viewManage": "管理分身",
  "tree.viewSetPrimary": "設為主分身",
  "tree.viewRename": "重新命名分身",
  "tree.viewName": "分身名稱",
  "tree.viewDelete": "刪除分身",
  "tree.viewDeletePrimary": "主分身不能刪除",
  "tree.viewDeleteTitle": "刪除樹分身",
  "tree.viewDeleteConfirm": (name) =>
    `確定刪除「${name}」嗎？其儲存的搜尋與篩選條件會被移除，專案和工作階段不受影響。`,
  "tree.viewSplitRight": "向右切分樹分身",
  "tree.viewSplitDown": "向下切分樹分身",
  "tree.viewAdd": "複製目前的樹分身到新分頁",
  "tree.viewCount": (n) => `${n} 個樹分身`,
  "mark.menu": "標記", // Mark
  "mark.urgent": "緊急", // Urgent
  "mark.important": "重要", // Important
  "mark.bug": "缺陷", // Bug
  "mark.done": "已完成", // Done
  "mark.wip": "進行中", // In progress
  "mark.pinned": "置頂關注", // Pinned
  "mark.idea": "想法", // Idea
  "mark.caution": "注意", // Caution
  "tree.clearAllNotifications": "清除全部通知標識（會話小點與 Dock 角標）", // Clear all notification badges…
  "tree.noProjectsPre": "還沒有專案。點擊資料夾圖示，或按 ", // No projects yet. Click the folder button, or press
  "tree.noProjectsPost": " 匯入一個目錄開始。", // to import a directory.
  "tree.openProject": "開啟專案", // Open Project
  "tree.noAttention": "沒有符合狀態篩選的會話", // No sessions match the status filter
  "tree.noMatch": "無符合結果", // No matches

  // Dialog fields
  "tree.groupName": "群組名稱", // Group name
  "tree.sessionNameAuto": "會話名稱（留空自動命名）", // Session name (leave empty to auto-name)
  "tree.editSession": "編輯會話", // Edit Session
  "tree.sessionName": "會話名稱", // Session name
  "tree.shellLabel": "Shell（留空用系統預設）", // Shell (leave empty for system default)
  "tree.shellMenu": "Shell",
  "tree.downloadFullGitbash": "下載完整 Git Bash",
  "gitbash.title": "Git Bash",
  "gitbash.downloading": "正在下載完整 Git Bash…",
  "gitbash.extracting": "正在解壓完整 Git Bash…",
  "gitbash.done": "完整 Git Bash 已就緒。",
  "gitbash.failed": "下載 Git Bash 失敗",
  "tree.shellSystemDefault": "系統預設", // System default
  "form.customOption": "自訂…", // Custom…
  "tree.cwdLabel": "工作目錄（留空用專案根）", // Working directory (leave empty for project root)
  "tree.initCmdLabel": "啟動命令（可選）", // Startup command (optional)
  "tree.agentArgsLabel": "啟動參數（可選）", // Launch args (optional)
  "tree.permissionSkipLabel": "跳過全部權限確認", // Skip all permission confirmations
  "tree.permissionSkipHint":
    "啟動時帶上該 agent 的「跳過確認」flag（如 Claude 的 --dangerously-skip-permissions；Codex 還會一併關閉沙箱）。每次啟動都生效，請謹慎使用。",
  "tree.permissionUnsupported":
    "OpenCode 經設定檔控制權限、沒有對應的啟動參數，此選項不適用。",
  "tree.permissionUnsupportedPi":
    "Pi 刻意不設權限確認彈窗（工具直接執行），此選項不適用。",

  // New agent-session dialog
  "newAgent.desc":
    "可選填會話名稱與自訂啟動參數（傳給 agent 命令，如 --model opus）。兩個都留空直接按 Enter 即可照常啟動。", // Optionally name the session and add custom launch args…

  // Delete confirmation
  "tree.batchDeleteTitle": "批次刪除", // Batch Delete
  "tree.deleteProjectTitle": "刪除專案", // Delete Project
  "tree.deleteGroupTitle": "刪除群組", // Delete Group
  "tree.deleteSessionTitle": "刪除會話", // Delete Session
  "tree.batchDeleteBody": (n) =>
    `確認刪除選取的 ${n} 項（專案/群組會連帶刪除其下的子群組與會話）。此操作不可復原。`, // Delete the {n} selected items…
  "tree.deleteProjectBody": (name) =>
    `確認刪除專案「${name}」，其下所有子群組與會話也會一併刪除。此操作不可復原。`, // Delete project "{name}"?…
  "tree.deleteGroupBody": (name) =>
    `確認刪除群組「${name}」，其下所有子群組與會話也會一併刪除。此操作不可復原。`, // Delete group "{name}"?…
  "tree.deleteSessionBody": (name) =>
    `確認刪除會話「${name}」（及其下所有子會話）。此操作不可復原。`, // Delete session "{name}"…
  "tree.deleteWorktrees": (n) =>
    `同時刪除關聯的 git worktree（共 ${n} 個；工作區有改動可能刪除失敗）`, // Also remove associated git worktrees…

  // Session information dialog
  "info.name": "名稱", // Name
  "info.type": "類型", // Type
  "info.status": "狀態", // Status
  "info.notYetCaptured": "尚未產生（首次執行後擷取）", // Not yet generated (captured after first run)
  "info.sessionId": "會話 ID", // Session ID
  "info.cwd": "工作目錄", // Working dir
  "info.initCmd": "啟動命令", // Startup cmd
  "info.agentArgs": "啟動參數", // Launch args
  "info.launchCmd": "完整啟動命令", // Full launch command
  "info.permission": "權限", // Permission
  "info.permissionSkip": "跳過全部確認", // Skip all confirmations
  "info.parentSessionId": "父會話 ID", // Parent ID
  "info.termTitle": "終端機標題", // Terminal title
  "info.createdAt": "建立時間", // Created at

  // Resume-session dialog
  "resume.title": "恢復會話", // Resume Session
  "resume.desc": "選 agent 類型並填入該 agent 自身的 session id，開啟後續接原對話。", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Agent 類型", // Agent type
  "resume.sessionIdPlaceholder": "對話 session id", // Conversation session id
  "resume.confirm": "恢復並開啟", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "新增 worktree 會話…", // New Worktree Session…
  "worktree.worktreeNameLabel": "worktree 名稱", // Worktree name
  "worktree.worktreeNameHint": "用作 worktree 目錄名與分支名。", // Used as the worktree directory and branch name.
  "worktree.createFailed": "建立 worktree 失敗", // Couldn't create the worktree
  "worktree.noRepoRoot": "此專案沒有可用的 git 倉庫路徑。", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "不掛", // None
  "worktreeSel.modeNew": "新建", // New
  "worktreeSel.modeExisting": "選現有", // Existing
  "worktreeSel.loading": "正在載入 worktree…", // Loading worktrees…
  "worktreeSel.empty": "此儲存庫沒有現有的 worktree。", // No existing worktrees in this repository.
  "worktreeSel.loadFailed": "無法列出 worktree（不是 git 儲存庫？）。", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint": "在此分組下新建的會話將預設使用此 worktree。", // Sessions created in this group will use this worktree by default.

  // ── Archive panel ──
  "archive.title": "已封存會話", // Archived Sessions
  "archive.empty1": "暫無封存會話。", // No archived sessions.
  "archive.empty2": "在左欄會話上按右鍵「封存會話」即可把它收進這裡。", // Right-click a session in the sidebar…
  "archive.restore": "恢復為正常會話", // Restore to normal session
  "archive.export": "匯出完整上下文為 Markdown", // Export full context as Markdown
  "archive.deleteForever": "徹底刪除（連帶錄製）", // Delete permanently (with recording)
  "archive.pickOne": "選擇左側一個封存會話查看其對話記錄", // Select an archived session on the left…
  "archive.recordingEnd": "--- 錄製結束 ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) => `讀取錄製失敗: ${err}`, // Failed to read recording: {err}
  "archive.searchRecording": "在錄製中搜尋…", // Search in recording…
  "archive.searchTranscript": "搜尋對話內容…", // Search transcript…
  "archive.searchPlaceholder": "搜尋封存內容…", // Search archived content…
  "archive.msgCountAll": (n) => `${n} 則`, // {n} messages
  "archive.msgCountFiltered": (shown, total) => `${shown} / ${total} 則`, // {shown} / {total} messages
  "archive.you": "你", // You
  "archive.toolsUsed": (tools) => `工具：${tools}`, // Tools: {tools}
  "archive.noMatch": "沒有符合的訊息", // No matching messages
  "archive.emptyTranscript": "對話記錄為空", // Transcript is empty
  "archive.loadingTranscript": "載入對話記錄…", // Loading transcript…

  // ── Global session-content search ──
  "search.allPlaceholder": "搜尋所有會話內容…", // Search across all session content…
  "search.hint": "搜尋會話內容。預設不含已封存會話，勾選「同時搜尋封存」可納入。", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "同時搜尋封存", // Include archived
  "search.includeArchivedHint": "把已封存會話也納入搜尋（預設不搜）", // Also search archived sessions (off by default)
  "search.searching": "搜尋中…", // Searching…
  "search.noResults": "找不到符合項目", // No matches found
  "search.sessionCount": (n) => `${n} 個會話`, // n sessions
  "search.matchCount": (n) => `${n} 處符合`, // n matches
  "search.pickSession": "在左側選擇一個會話以檢視符合片段", // Select a session on the left to see its matches
  "search.openSession": "開啟會話", // Open session
  "search.backToResults": "返回結果", // Back to results
  "search.archivedBadge": "已封存", // Archived
  "search.summary": (m, s) => `命中 ${m} 處 · ${s} 個會話`, // X matches · N sessions
  "search.matchPosition": (n, total) => `第 ${n} / 共 ${total}`, // N of M
  "search.roleTerminal": "終端", // Terminal
  "search.collapseGroup": "收合", // Collapse
  "search.expandGroup": "展開", // Expand
  "search.cappedNote": (l, total) => `可定位 ${l} / 共命中 ${total}`, // L of total locatable

  // ── Center pane (tabs, empty state, and background keep-alive) ──
  "center.noSession": "暫無會話", // No session
  "center.noSessionHintPre": "從左欄選擇會話，或按 ", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": " 新增終端機", // to create a terminal
  "center.createTerminal": "新增終端機", // Create Terminal
  "tab.unsavedDot": "有未儲存的修改", // Unsaved changes
  "tab.newTerminal": "新增終端機", // New terminal
  "tab.newDocument": "新增文件", // New document
  "tab.bgTitle": (n) => `背景保活分頁：${n} 個（處理程序仍在執行）`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `背景 ${n}`, // Background {n}
  "tab.scratchFallback": "（臨時終端機）", // (scratch terminal)
  "tab.killBgTab": "結束該背景分頁（處理程序隨之結束）", // Kill this background tab…
  "tab.newBrowserTab": "新分頁", // New Tab
  "tab.refreshFile": "重新整理檔案", // Refresh File
  "tab.closeOthers": "關閉其他分頁", // Close Other Tabs
  "tab.closeRight": "關閉右側分頁", // Close Tabs to the Right
  "tab.closeAll": "關閉所有分頁", // Close All Tabs
  "tab.sendToBackground": "轉入背景保活", // Send to Background

  // ── Built-in browser ──
  "browser.back": "上一頁", // Back
  "browser.forward": "下一頁", // Forward
  "browser.reload": "重新整理", // Reload
  "browser.stop": "停止載入", // Stop loading
  "browser.openExternal": "以系統瀏覽器開啟", // Open in system browser
  "browser.addressPlaceholder": "輸入網址或搜尋字詞", // Enter URL or search terms
  "browser.quickAccess": "快速存取", // Quick access
  "browser.loading": "載入中…", // Loading…
  "overlimit.title": (max) => `背景保活已超上限（${max} 個）`, // Background keep-alive over limit ({max})
  "overlimit.body": "所有背景分頁都在工作或等你回覆，請選擇要結束的分頁：", // All background tabs are working or awaiting your reply. Choose one to end:
  "overlimit.kill": "結束選取", // End Selected
  "overlimit.keep": "暫不結束", // Keep for Now
  "overlimit.earliest": "最早", // earliest
  "overlimit.statusWorking": "工作中", // working
  "overlimit.statusAsking": "待回覆", // awaiting reply
  "overlimit.statusWaiting": "等待中", // waiting

  // ── Terminal pane, context menu, and search ──
  "term.paste": "貼上", // Paste
  "term.pasteUseShortcut": "貼上（請按 ⌘V）", // Paste (press ⌘V)
  "term.selectAll": "全選", // Select All
  "term.autoCopied": (n: number) => `已自動複製 ${n} 字元 · ⌘V 貼上`,
  "term.clear": "清除畫面", // Clear
  "term.searchMenu": "搜尋…", // Search…  ⌘F
  "term.splitRight": "右分割", // Split right (⌘D)
  "term.splitDown": "下分割", // Split down (⌘⇧D)
  "term.closePane": "關閉分割", // Close split
  "term.redraw": "重繪", // Redraw
  "term.mirrorTooltip":
    "目前為鏡像顯示（尺寸由其它端主控）。點擊把 PTY 尺寸調整為本視窗大小", // Mirroring (size controlled by another client)…
  "term.mirrorBadge": (dims) => `⤢ 鏡像${dims} · 點擊適配本視窗`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) => `⤢ 鏡像${dims} · 適配本視窗`, // ⤢ Mirror{dims} · fit this window
  "term.imgUploadFailed": (n, lastError) =>
    `圖片上傳失敗 ${n} 張${lastError ? `：${lastError}` : ""}`, // Image upload failed for {n} images…
  "term.imgClipboardUnavailable": "無法從剪貼簿讀取圖片，請重新複製圖片後再試。",
  "term.starting": (agent) => `正在啟動 ${agent}…`, // Starting {agent}…
  "term.startFailed": (err) => `啟動失敗: ${err}`, // Failed to start: {err}

  // ── Agent installation guidance ──
  "agentInstall.title": (label) => `${label} 尚未安裝`, // {label} is not installed
  "agentInstall.desc": (label) =>
    `VelaTerm 沒有在 PATH 上找到 ${label}。安裝後即可啟動此工作階段。`, // couldn't find {label} on PATH
  "agentInstall.install": "一鍵安裝", // Install now
  "agentInstall.retry": "重試啟動", // Retry launch
  "agentInstall.dismiss": "我自己裝", // I'll do it myself
  "agentInstall.docs": "安裝文件", // Install docs
  "agentInstall.needsNode": "需先安裝 Node.js / npm", // Requires Node.js / npm
  "agentInstall.afterInstall": "安裝後：", // After install:
  "agentInstall.pathSaved": (label: string) => `已把 ${label} 的可執行檔路徑填入設定：`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} 已安裝`, // {label} is installed
  "agentInstall.doneDesc": "重新啟動本工作階段即可開始使用。", // Relaunch this session to start using it.
  "agentInstall.restartNow": "立即重新啟動", // Relaunch now
  "agentInstall.later": "稍後", // Later
  "search.placeholder": "在終端機中搜尋", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "所見即所得", // WYSIWYG
  "doc.source": "原始碼", // Source
  "doc.searchPlaceholder": "尋找", // Find
  "doc.searchReplacePlaceholder": "取代", // Replace
  "doc.searchReplace": "取代", // Replace
  "doc.searchReplaceAll": "全部", // All
  "doc.searchNoMatch": "無相符", // No results
  "doc.searchCaseSensitive": "區分大小寫", // Match case
  "doc.searchToggleReplace": "切換取代", // Toggle replace
  "doc.fileTree": "目錄樹", // File tree
  "doc.treeUp": "上層目錄", // Parent folder
  "doc.sidebar": "側欄", // Sidebar
  "doc.unsaved": "未儲存", // Unsaved
  "doc.saveAsTitle": "另存新檔", // Save As
  "doc.saveAsName": "檔案名稱", // File name
  "doc.outline": "大綱", // Outline
  "doc.outlineEmpty": "沒有標題", // No headings
  "doc.saving": "儲存中…", // Saving…
  "doc.overwriteConfirm": "已存在同名檔案，點「覆蓋」替換原檔案。", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "儲存（⌘S）", // Save (⌘S)
  "doc.externalChanged": "檔案已在磁碟上被修改（你有未儲存的本地修改）。", // The file was modified on disk…
  "doc.reloadDiscard": "重新載入（捨棄我的修改）", // Reload (discard my changes)
  "doc.externalChangedClean": "檔案已在磁碟上被修改。", // The file was modified on disk.
  "doc.reload": "重新載入", // Reload
  "doc.ignore": "忽略", // Ignore
  "doc.loadingFile": (title) => `正在載入 ${title}…`, // Loading {title}…
  "doc.closeTitle": "關閉文件", // Close Document
  "doc.unsavedBody": (title) => `「${title}」有未儲存的修改。`, // "{title}" has unsaved changes.
  "doc.saveAndClose": "儲存並關閉", // Save & Close
  "doc.closeNoSave": "不儲存關閉", // Close Without Saving
  "doc.conflictTitle": "儲存衝突", // Save Conflict
  "doc.conflictBody": "磁碟上的檔案已被外部修改，仍要用目前內容覆蓋嗎？", // The file on disk was modified externally…
  "doc.overwrite": "覆蓋", // Overwrite
  "doc.saveFailed": (err) => `儲存失敗：${err}`, // Save failed: {err}
  "doc.closeTab": "關閉分頁", // Close Tab
  "doc.truncatedReadonly": (size: string) =>
    `唯讀：僅顯示前 10 MB（共 ${size}）。已停用儲存，以免覆蓋檔案其餘部分。`,
  "doc.imgLoading": (title, size) => `正在載入 ${title}（${size}）…`, // Loading {title} ({size})…
  "doc.imgBeingWritten": "檔案正在寫入，待寫入穩定後將自動重新載入。", // The file is being written; it will reload automatically once it settles.
  "doc.imgDecodeFailed": "無法顯示該圖片（格式不支援或檔案已損壞）。", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "適應視窗", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "匯出 PDF", // Export PDF
  "doc.diagramError": "圖表語法錯誤", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "未選擇會話", // No session selected
  "panel.openInEditor": "在編輯器中開啟", // Open in Editor
  "panel.openInEditorTooltip": "在中欄文件編輯器中開啟（同 view 命令）", // Open in the document editor…
  "panel.preview": "預覽", // Preview
  "panel.cantRead": "（無法讀取該檔案）", // (cannot read this file)
  "panel.binary": "（二進位檔案，不預覽）", // (binary file, no preview)
  "panel.truncated": "\n…（內容過長已截斷）", // …(content truncated)
  "panel.showHidden": "顯示隱藏檔案", // Show hidden files
  "panel.hideHidden": "不顯示隱藏檔案", // Hide hidden files

  // ── File-tree actions (Files context menu and header add button) ──
  "files.newFile": "新增檔案", // New File
  "files.newFolder": "新增資料夾", // New Folder
  "files.nameLabel": "名稱", // Name
  "files.newTooltip": "新增檔案或資料夾", // New file or folder
  "files.openInTerminal": "在終端機中開啟", // Open in Terminal
  "files.revealInFinder": "在檔案管理器中顯示", // Show in File Manager
  "files.copyPath": "複製路徑", // Copy Path
  "files.copyRelPath": "複製相對路徑", // Copy Relative Path
  "files.filterPlaceholder": "篩選檔案…", // Filter files
  "files.deleteConfirm": (name) => `確定刪除「${name}」？此操作無法復原。`, // Delete "{name}"? This can't be undone.

  // ── Status bar ──
  "statusbar.sessions": (n) => `${n} 會話`, // {n} sessions
  "statusbar.filterTooltip": (label) => `點擊在左欄只看「${label}」會話（再點取消）`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `背景 ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `背景保活的分頁數（上限 ${max}，超限時自動結束最早的不活躍分頁）`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) => `已結束背景分頁：${name}（超出保活上限）`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `瀏覽器遠端存取已啟用：${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "權限：詢問", // Perms: Ask
  "statusbar.permSkip": "權限：跳過", // Perms: Skip
  "statusbar.notifyOn": "通知：開", // Notify: On
  "statusbar.notifyOff": "通知：關", // Notify: Off
  "statusbar.permTooltip": "本會話權限模式 · 點擊切換（僅影響本會話）", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "本會話權限", // This session's permissions
  "statusbar.permOptAsk": "逐步詢問（預設）", // Ask each time (default)
  "statusbar.permScopeHint": "僅對當前會話生效。全域性設定，請前往「設定 ▸ 智能體」中調整。", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg": "權限已變更，需重啟本會話才生效。重啟會接續目前對話，但會中斷進行中的任務。現在重啟？", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "立即重啟", // Restart now
  "statusbar.permRestartLater": "稍後", // Later
  "statusbar.permScopeTitle": "套用到？", // Apply to?
  "statusbar.permScopeSession": "僅目前會話", // This session only
  "statusbar.permScopeGlobal": "全域預設", // Global default
  "statusbar.permScopeGlobalHint": "本會話立即採用，並設為日後新建同類會話的預設（與設定同步）。", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

  // ── Store, notifications, and export ──
  "notify.working": "⏳ 處理中…", // ⏳ Working…
  "notify.asking": "❓ 需要你確認", // ❓ Needs your confirmation
  "notify.waiting": "✅ 已回覆", // ✅ Replied
  "store.subtask": "子任務", // Subtask
  "store.splitPane": "分割", // Split
  "export.failedTitle": "匯出會話失敗", // Failed to export session
  "export.contextSuffix": "上下文", // context

  // ── Error panel ──
  "err.renderTitle": "介面渲染出錯", // Rendering Error
  "err.renderDesc": "遇到了未預期的錯誤，以下資訊可幫助定位問題。", // An unexpected error occurred…
  "err.reload": "重新載入", // Reload
  "err.uncaughtTitle": "發生未捕獲的錯誤", // Uncaught Error
  "err.uncaughtDesc": "以下資訊可幫助定位問題。", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser": "瀏覽器端暫不支援封存回放", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `圖片上傳失敗 (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.connecting": "連線中…", // Connecting…
  "login.remoteAccess": "遠端存取", // Remote Access
  "login.desc": "輸入存取密碼以連線到該終端機。", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "存取密碼", // Access password
  "login.connect": "連線", // Connect
  "login.wrongPassword": "密碼錯誤", // Wrong password
  "login.failed": "登入失敗，請重試", // Login failed, please try again
  "login.pairingRequired": "此服務要求使用配對連結存取。請用桌面端「遠端存取」產生的配對連結開啟。", // This server requires a pairing link
  "login.authFailed": "密碼錯誤，或配對連結已失效，請用新的配對連結重新連線。", // Wrong password or pairing link expired
  "dir.title": "選擇專案目錄", // Choose Project Directory
  "dir.pathPlaceholder": "搜尋，或輸入路徑後按 Enter 跳轉（支援 ~ 開頭）", // Search, or type a path and press Enter (supports ~)
  "dir.up": "上一層", // Up one level
  "dir.newFolder": "新增資料夾", // New Folder
  "dir.newFolderPlaceholder": "資料夾名稱", // Folder name
  "dir.goInput": "前往輸入路徑", // Go to typed path
  "dir.noSubdirs": "（無子目錄）", // (no subdirectories)
  "dir.empty": "（空目錄）", // (empty folder)
  "dir.noMatch": "沒有符合的項目", // No matching items
  "dir.target": "目標目錄", // Target
  "dir.showHidden": "顯示隱藏項目", // Show hidden items
  "dir.importing": "匯入中…", // Importing…
  "dir.choose": "選擇此目錄", // Choose This Directory
  "conn.reconnecting": "連線已中斷，正在嘗試重新連線…", // Connection lost, reconnecting…
  "conn.reconnectNow": "立即重新連線", // Reconnect now
  "conn.retrying": "正在重新連線…", // Reconnecting…
  "conn.sshReconnecting": "SSH 連線已中斷，正在重建通道…", // SSH link lost, rebuilding the tunnel…
  "conn.sshDown": "SSH 連線已中斷，點擊「立即重新連線」再試一次", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "請求失敗", // Request failed
  "reqerr.dismiss": "關閉", // Dismiss
  // ── Error log panel (hidden debug entry) ──
  "errlog.title": "錯誤日誌",
  "errlog.empty": "尚無記錄的錯誤。",
  "errlog.copyAll": "全部複製",
  "errlog.clear": "清空",
  "errlog.close": "關閉",

  // ── Mobile ──
  "mobile.toDesktop": "切換到桌面版", // Switch to desktop
  "mobile.empty1": "暫無會話。", // No sessions.
  "mobile.noMatch": "沒有符合的會話", // No matching sessions
  "mobile.empty2": "在桌面端或電腦瀏覽器端建立後，這裡會自動出現。", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ 返回", // ‹ Back
  "mobile.selCopy": "複製", // Copy
  "mobile.selCancel": "取消", // Cancel

  // ── Other shared components ──
  "splitter.dragToResize": "拖曳調整大小", // Drag to resize
  "transport.wsDisconnected": "WebSocket 已斷線", // WebSocket disconnected
  "transport.wsConnectFailed": "WebSocket 連線失敗", // WebSocket connection failed
  "transport.cmdFailed": "命令失敗", // Command failed

  // ── Crepe (built-in WYSIWYG editor UI) ──
  "crepe.placeholder": "輸入內文，或鍵入 / 開啟插入選單", // Type text, or press / for the insert menu
  "crepe.textGroup": "文字", // Text
  "crepe.paragraph": "內文", // Text
  "crepe.h1": "標題 1", // Heading 1
  "crepe.h2": "標題 2", // Heading 2
  "crepe.h3": "標題 3", // Heading 3
  "crepe.h4": "標題 4", // Heading 4
  "crepe.h5": "標題 5", // Heading 5
  "crepe.h6": "標題 6", // Heading 6
  "crepe.quote": "引用", // Quote
  "crepe.divider": "分隔線", // Divider
  "crepe.listGroup": "清單", // List
  "crepe.bulletList": "項目符號清單", // Bullet List
  "crepe.orderedList": "編號清單", // Ordered List
  "crepe.taskList": "工作清單", // Task List
  "crepe.advancedGroup": "插入", // Insert
  "crepe.image": "圖片", // Image
  "crepe.codeBlock": "程式碼區塊", // Code Block
  "crepe.table": "表格", // Table
  "crepe.math": "公式", // Math
  "crepe.linkPlaceholder": "貼上或輸入連結…", // Paste or type a link…
  "crepe.upload": "上傳", // Upload
  "crepe.uploadImage": "上傳圖片", // Upload Image
  "crepe.orPasteImageLink": "或貼上圖片連結", // or paste an image link
  "crepe.imageCaption": "圖片說明", // Image caption
  "crepe.confirm": "確認", // Confirm
  "crepe.searchLanguage": "搜尋語言", // Search language
  "crepe.noResult": "無符合結果", // No results
  "crepe.edit": "編輯", // Edit
  "crepe.collapse": "收合", // Collapse
  // ── Additional right and bottom bar entries ──
  "info.project": "專案", // Project
  "panel.sessionInfo": "會話資訊", // Session info
  "panel.gitTitle": "Git 狀態", // Git status
  "panel.gitProbing": "偵測中…", // Checking…
  "panel.gitNotRepo": "非 Git 倉庫", // Not a Git repository
  "panel.gitBranch": "分支", // Branch
  "panel.gitStaged": "暫存", // Staged
  "panel.gitUnstaged": "變更", // Changed
  "panel.gitUntracked": "未追蹤", // Untracked
  "bottombar.running": "執行中", // Running
  "bottombar.collapseTasks": "收合任務區", // Collapse tasks
  "bottombar.expandTasks": "展開任務區", // Expand tasks
  "bottombar.sound": "🔔 提示音", // 🔔 Sound
  "bottombar.muted": "🔕 靜音", // 🔕 Muted
  "bottombar.overview": "會話概覽", // Sessions overview
  "bottombar.noSessions": "尚無會話", // No sessions
  "doc.pdfFilter": "PDF 檔案", // PDF file
  // ── Automatic updates ──
  "updater.title": "檢查更新",
  "updater.upToDate": "目前已是最新版本。",
  "updater.failed": (err) => `檢查更新失敗：${err}`,
  "updater.available": "發現新版本",
  "updater.versionLine": (version, current) => `版本 ${version} — 目前 ${current}`,
  "updater.noNotes": "此版本沒有提供更新說明。",
  "updater.updateNow": "立即更新",
  "updater.later": "稍後",
  "updater.skipVersion": "略過此版本",
  "updater.skipVersionHint": "不再提示這個版本。之後仍可從「檢查更新」手動安裝。",
  "updater.downloadingPct": (pct) => `正在下載… ${pct}%`,
  "updater.downloadingBytes": (mb) => `正在下載… ${mb} MB`,
  "updater.installing": "正在安裝…",
  "updater.installed": "更新已安裝，重新啟動後生效。",
  "updater.restartNow": "立即重新啟動",
  "updater.retry": "重試",
  "updater.downloadFailed": (err) => `更新失敗：${err}`,
  "updater.hide": "隱藏",
  "updater.hideHint": "在背景繼續下載，進度會留在狀態列。",
  "updater.downloadManually": "手動下載",
  "updater.downloadManuallyHint": "在瀏覽器中開啟安裝包的下載連結。",
  "updater.windowsNotice": "安裝期間 VelaTerm 會關閉，安裝完成後自動重新開啟。",
  "updater.installingWindows":
    "正在安裝… VelaTerm 即將關閉，安裝程式會完成更新並重新開啟它。",
  // The status-bar new-version segment belongs to automatic updates and stays here for centralized editing.
  "statusbar.updateAvailable": (version) => `更新 ${version}`,
  "statusbar.updateDownloading": (pct) => `正在更新… ${pct}%`,
  "statusbar.updateInstalling": "正在安裝…",
  "statusbar.updateReady": "重新啟動以完成更新",
  "statusbar.updateFailed": "更新失敗",
  "statusbar.updateTooltip": "點擊查看詳情",
};

export default zhTW;
