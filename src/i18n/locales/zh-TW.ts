//! Traditional Chinese dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const zhTW: typeof en = {
  // Project code intelligence and memory associations.
  "knowledge.title": "程式碼圖譜",
  "knowledge.intro": "查看程式碼關係，並連結已儲存的設計決策。",
  "knowledge.setup": "在目前後端安裝 CodeGraph 後，即可啟用專案索引。",
  "knowledge.downloadNotice": "從 GitHub 下載經過驗證的 CodeGraph 執行環境。程式碼索引在本機完成，遙測和更新檢查均已停用。",
  "knowledge.install": "下載 CodeGraph",
  "knowledge.installing": "正在下載並安裝…",
  "knowledge.directory": "工作目錄",
  "knowledge.enable": "啟用索引",
  "knowledge.disable": "停用索引",
  "knowledge.sync": "同步索引",
  "knowledge.ready": "可用",
  "knowledge.disabled": "已停用",
  "knowledge.indexing": "正在建立索引…",
  "knowledge.syncing": "正在同步…",
  "knowledge.failed": "失敗",
  "knowledge.symbols": "符號",
  "knowledge.files": "檔案",
  "knowledge.edges": "關係",
  "knowledge.search": "搜尋符號或檔案路徑…",
  "knowledge.searchButton": "搜尋",
  "knowledge.noResults": "沒有符合的符號。",
  "knowledge.selectSymbol": "選擇一個符號，查看原始碼、關係和相關記憶。",
  "knowledge.source": "原始碼",
  "knowledge.incoming": "傳入關係",
  "knowledge.outgoing": "傳出關係",
  "knowledge.noEdges": "索引中沒有相關關係。",
  "knowledge.analysisNote": "關係來自靜態分析，可能不完整或存在不確定性。",
  "knowledge.changed": "查詢期間檔案已變更。請再次同步，再使用行號或確認檢閱結果。",
  "knowledge.truncated": "目前檢視已限制顯示數量，部分關係或原始碼行未顯示。",
  "knowledge.linkMemory": "連結記憶",
  "knowledge.chooseMemory": "選擇記憶條目",
  "knowledge.noLinks": "尚無程式碼連結。可在符號詳細資料中連結記憶。",
  "knowledge.inspect": "核對程式碼與記憶",
  "knowledge.unlink": "移除連結",
  "knowledge.codeReferences": "程式碼參照",
  "knowledge.refresh": "重新整理",
  "knowledge.current": "未變更",
  "knowledge.review": "需要檢閱",
  "knowledge.unavailable": "無法使用",
  "knowledge.reviewHelp": "請將此記憶與顯示的程式碼核對。確認後僅記錄目前檔案版本，不修改記憶內文。",
  "knowledge.confirmReview": "確認已檢閱",
  "knowledge.agentHint": "代理程式可在此工作目錄執行 vknowledge search \"主題\"。查詢會同步已啟用的索引，並分別傳回程式碼和記憶。",
  "knowledge.busy": "索引工作正在執行。可以關閉此頁面，或停用索引以停止工作。",
  "knowledge.disabledHelp": "啟用此目錄的索引後即可查詢程式碼。停用會保留索引和記憶連結。",
  "knowledge.conflict": "程式碼或記憶已變更。請重新載入後再儲存連結。",
  "knowledge.symbolMissing": "符號或原始碼已無法使用。請同步索引後重新搜尋。",
  "knowledge.directoryMissing": "工作目錄不存在或已變更。請檢查專案和工作階段的路徑。",
  "knowledge.partial": "索引不完整。請再次同步，並確認原始碼檔案可以讀取。",
  "knowledge.interrupted": "上一次工作已中斷。請同步索引以重試。",
  "knowledge.checksum": "下載檔案的校驗值不符，未安裝執行環境。",
  "knowledge.downloadFailed": "無法下載 CodeGraph。請檢查後端與 GitHub 的連線後重試。",
  "knowledge.timeout": "索引工作逾時。請檢查儲存庫大小後重試。",
  "knowledge.error": "操作失敗。請檢查後端的目錄存取權限和執行環境後重試。",

  // Global Memory: a thematic LLM Wiki shared across sessions.
  "memory.title": "全域記憶",
  "memory.add": "加入全域記憶",
  "memory.intro": "以持續更新的 Wiki 組織知識、決策與經驗，建立跨工作階段共享的長期記憶。",
  "memory.entries": "記憶條目",
  "memory.emptyJobs": "尚無整理紀錄。",
  "memory.jobs": "整理紀錄",
  "memory.search": "搜尋記憶標題與內文…",
  "memory.empty": "沒有符合的記憶。可從工作階段加入內容，逐步建立你的 Wiki。",
  "memory.emptyDetail": "選擇一個條目，查閱知識內容、關聯與來源。",
  "memory.new": "新增記憶",
  "memory.titleField": "標題",
  "memory.summary": "摘要",
  "memory.content": "內文（Markdown）",
  "memory.tags": "標籤（以逗號分隔）",
  "memory.related": "相關記憶",
  "memory.backlinks": "連結至此條目的記憶",
  "memory.sources": "來源",
  "memory.history": "修訂紀錄",
  "memory.restore": "還原此版本",
  "memory.restoreConfirm": "將此修訂還原為新版本？目前版本仍會保留在歷史紀錄中。",
  "memory.deleteConfirm": "刪除此記憶及其修訂紀錄？來源工作階段不受影響。",
  "memory.export": "匯出 Markdown",
  "memory.selectAgent": "代理程式",
  "memory.model": "模型（選填）",
  "memory.modelHint": "留空時使用代理程式已設定的模型。",
  "memory.compile": "整理並儲存",
  "memory.compileHelp": "所選代理程式會依主題整理此工作階段，並與現有記憶合併。對話文字與相關記憶將透過你設定的代理程式傳送給模型。",
  "memory.unavailable": "尚未安裝或設定",
  "memory.allTags": "所有標籤",
  "memory.updated": "最近更新",
  "memory.titleSort": "依標題排序",
  "memory.sourceNote": "此快照保留整理時使用的對話文字；即使原工作階段已刪除，仍可查閱。",
  "memory.noKnowledge": "未發現可重複運用的知識，未變更任何記憶條目。",
  "memory.running": "進行中",
  "memory.completed": "已完成",
  "memory.failed": "失敗",
  "memory.cancelled": "已取消",
  "memory.extract": "擷取主題",
  "memory.merge": "合併知識",
  "memory.commit": "儲存記憶",
  "memory.done": "已儲存",
  "memory.closeHint": "整理期間可關閉此視窗，稍後在整理紀錄中查看進度。",
  "memory.conflict": "操作期間此記憶已變更。請重新載入後再試；本次修改尚未儲存。",
  "memory.duplicate": "已有同名記憶，請開啟該條目合併內容。",
  "memory.busy": "已有整理工作正在執行。請等待完成，或在整理紀錄中取消。",
  "memory.notFound": "此記憶、來源或工作已不存在。",
  "memory.noTranscript": "此工作階段目前沒有可讀取的對話內容。",
  "memory.agentUnavailable": "所選代理程式無法使用，請在設定中檢查其執行檔路徑。",
  "memory.invalid": "部分欄位或連結無效，請檢查標題、內文及相關記憶。",
  "memory.processFailed": "代理程式未能完成整理。請檢查登入狀態、模型及 CLI 設定後重試。",
  "memory.timeout": "代理程式呼叫逾時，請更換可用模型或縮短對話後重試。",
  "memory.interrupted": "整理工作已中斷，可重試處理已儲存的來源快照。",
  "memory.tooLarge": "來源、上下文或輸出超出支援的大小，未截斷內容，也未寫入記憶。",
  "memory.invalidOutput": "代理程式傳回的結構化資料無效，未寫入記憶。請重試或更換代理程式。",
  "memory.loadError": "無法載入記憶資料，請檢查連線後重試。",
  "memory.unsaved": "放棄尚未儲存的修改？",
  "memory.source": "來源快照",

  // ── Common ──
  "common.cancel": "取消", // Cancel
  "common.confirm": "確定", // OK
  "common.delete": "刪除", // Delete
  "common.save": "儲存", // Save
  "common.create": "建立", // Create
  "common.close": "關閉", // Close
  "chat.imageViewOriginal": "檢視原圖",
  "chat.imageCopy": "複製圖片",
  "chat.imageSave": "儲存圖片",
  "chat.imageActionFailed": "圖片操作失敗，請重試。",
  "common.copy": "複製", // Copy
  "common.cut": "剪下", // Cut
  "common.paste": "貼上", // Paste
  "common.selectAll": "全選", // Select All
  "common.copied": "已複製", // Copied
  "chat.sync.loading": "正在同步對話…",
  "chat.sync.failed": "同步失敗，仍可查看已載入的訊息。",
  "chat.sync.history": "載入更早的訊息",
  "chat.submission.updateRequired": "請先更新伺服器，再使用此用戶端傳送訊息。",
  "chat.submission.sending": "傳送中…",
  "chat.submission.sent": "已傳送",
  "chat.submission.queued": "已排入佇列",
  "chat.submission.failed": "傳送失敗",
  "chat.submission.unknown": "傳送結果待確認",
  "chat.submission.check": "確認狀態",
  "common.retry": "重試", // Retry
  "common.experimental": "實驗性功能",
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
  "titlebar.browser": "內建瀏覽器", // Built-in Browser
  "titlebar.remoteAccess": "遠端存取（瀏覽器）", // Remote Access (Browser)
  "titlebar.connectRemote": "連線到遠端服務", // Connect to Remote Server
  "titlebar.mirrored": "鏡像中", // Mirrored
  "titlebar.mirroredHint":
    "鏡像已開啟：分頁、分割與目前會話跟隨主機。開關在主機端。", // Mirroring is on: tabs, splits, and the active session follow the host. The switch is on the host.
  "titlebar.mirroredBy": (n: number) => `被 ${n} 端鏡像`, // Mirrored by {n}
  "titlebar.mirroredByHint": (n: number) =>
    `有 ${n} 個遠端連著。分頁、分割和目前的會話是共用的，兩邊都能改。`, // {n} remote clients are connected. Tabs, splits, and the active session are shared, and either side can rearrange them.
  "titlebar.clientsTitle": "已連線的用戶端", // Attached clients
  "titlebar.clientUnnamed": "未命名用戶端", // Unnamed client
  "titlebar.clientSince": (time: string) => `${time} 起`, // since {time}
  "titlebar.share": "分享", // Share
  // ── Alt-triggered menu bar (Windows/Linux) ──
  "menubar.file": "檔案", // File
  "menubar.terminal": "終端機", // Terminal
  "menubar.help": "說明", // Help
  "menubar.newTerminal": "新增終端機", // New Terminal
  "menubar.visitWebsite": "造訪官網", // Visit Website
  "menubar.sendFeedback": "傳送意見回饋", // Send Feedback
  "menubar.clearBadges": "清除通知標識", // Clear Notification Badges
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
  "settings.catAgents": "智慧體", // Agents
  "settings.permDefault": "預設", // Default
  "settings.permYolo": "YOLO", // YOLO
  "settings.yoloHint": (flag: string) =>
    `啟動時附加 ${flag}，跳過全部權限確認，請謹慎使用。`,
  "settings.permViaEnvHint":
    "透過設定檔注入跳過全部權限確認（無命令列旗標）。僅影響該會話啟動時的行為。", // YOLO flag hint
  "settings.catGeneral": "一般", // General
  "settings.cliLabel": "Shell 指令",
  "settings.cliInstall": "安裝 ‘vela’ 指令",
  "settings.cliUninstall": "解除安裝 ‘vela’ 指令",
  "settings.cliInstalledAt": (path: string) => `已安裝至 ${path}`,
  "settings.cliConflict": (path: string) =>
    `${path} 已存在其他 ‘vela’ 指令，VelaTerm 不會覆寫它。`,
  "settings.cliHint":
    "像 VS Code 的 `code` 一樣，將 `vela <專案路徑>` 加入 PATH。",
  "settings.agentArgsHint":
    "各類型智慧體新建會話時套用的預設啟動參數。新建或編輯單個會話時設定的參數會覆寫此處的預設。留空表示不帶參數。", // Agent default launch args hint
  "settings.agentPathLabel": "可執行檔路徑（可選）", // Executable path (optional)
  "settings.agentPathPlaceholder": "如 ~/.local/bin/claude——留空則從 PATH 尋找", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "設定後，該類型會話一律按這條完整路徑啟動，不再從 PATH 尋找命令。適用於「已安裝但不在 shell PATH 上」的情況。一鍵安裝成功且能偵測到安裝位置時會自動填入。", // Agent executable path hint
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
  "settings.defaultSessionEngine": "新建會話的預設檢視",
  "settings.defaultSessionEngineHint": "既有會話維持建立時的檢視。",
  "settings.dynamicStatusFilter": "狀態篩選動態增加",
  "settings.tabSingle": "單分頁", // Single
  "settings.tabMulti": "多分頁", // Multi
  "settings.maxLiveTabs": "背景保活上限", // Background limit
  "settings.defaultShell": "預設 Shell", // Default shell
  "settings.spawnConfirm": "派生前確認", // Confirm before spawn
  "settings.usageAuto": "額度自動刷新", // Usage auto-refresh
  "settings.usageRefresh": "額度刷新", // Usage refresh
  "settings.cleanImages": "自動清理貼上的圖片",
  "settings.cleanImagesHint":
    "貼上或拖入終端的圖片會先存成暫存檔（把路徑傳給 agent）。開啟後：結束時刪除本次會話產生的這些暫存圖，啟動時清理超過 24 小時的殘留。文件內的圖片不受影響。",
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
    "遠端會話固定貼上檔案路徑，讓智慧體能在其所在機器讀取圖片；原生圖片貼上僅在本機桌面端可用。",
  "spawn.title": "啟動派生會話？", // Start spawned session?
  "spawn.fromSession": "來自", // From
  "spawn.promptLabel": "提示詞", // Prompt
  "spawn.agentLabel": "智慧體", // Agent
  "spawn.worktreeLabel": "獨立 git worktree", // Separate git worktree
  "spawn.modelLabel": "模型", // Model
  "spawn.effortLabel": "推理強度", // Effort
  "spawn.modelDefault": "預設", // Default
  "spawn.modelLoading": "正在取得模型…", // Listing models…
  "spawn.modelListUnavailable": "取不到模型清單 — 在上面直接填模型名", // No model list available — type an identifier above
  "spawn.launch": "啟動", // Launch
  "spawn.remaining": (n: number) => `還有 ${n} 個待確認`, // ${n} more pending
  "spawn.notifyTitle": "派生會話待確認", // Spawn session awaiting confirmation
  "orch.title": "啟動這些智慧體？",
  "orch.notifyTitle": "編排等待確認",
  "orch.coordinatorName": "進度",
  "orch.sharedSettings": "總設定",
  "orch.agentLabel": "類型",
  "orch.modelLabel": "模型",
  "orch.effortLabel": "思考程度",
  "orch.nameLabel": "名稱",
  "orch.promptLabel": "任務",
  "orch.worktreeLabel": "工作樹",
  "orch.worktreeNone": "沿用目前目錄",
  "orch.worktreeShared": "共用一個工作樹",
  "orch.worktreeEach": "每個各一個工作樹",
  "orch.follow": "跟隨總設定",
  "orch.overridden": "已改",
  "orch.remove": "刪除",
  "orch.launch": (n: number) => `啟動 ${n} 個`,
  "orch.modelPlaceholder": "智慧體預設",
  "orch.effortPlaceholder": "智慧體預設",
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "查看變更…",
  "changes.title": "變更",
  "changes.loading": "載入中…",
  "changes.loadingDiff": "載入 diff…",
  "changes.noChanges": "沒有變更",
  "changes.refresh": "重新整理",
  "changes.notRepo": "不是 git 儲存庫",
  "changes.selectFile": "選擇檔案查看",
  "changes.binary": "二進位檔案，無法逐行 diff",
  "changes.commitTitle": (hash: string) => `提交 ${hash}`,

  "git.staged": "已暫存",
  "git.changes": "變更",
  "git.untracked": "未追蹤檔案",
  "git.committed": "已提交的變更",
  "git.stage": "暫存",
  "git.unstage": "取消暫存",
  "git.stageAll": "全部暫存",
  "git.unstageAll": "全部取消暫存",
  "git.discard": "捨棄變更",
  "git.deleteFile": "刪除",
  "git.viewAll": "檢視全部",
  "git.detached": "（分離 HEAD）",
  "git.aheadBehind": "相對上游分支領先和落後的提交數",
  "git.commitPlaceholder": "提交說明",
  "git.amend": "修改上一次提交",
  "git.amendCommit": "修改提交",
  "git.commitCount": (n: number) => `提交 ${n} 個檔案`,
  "git.commitNoFiles": "這個提交沒有檔案變更",
  "git.noCommits": "尚未有提交",
  "git.loadMore": "載入更多",
  "tree.merge": "合併…",
  "tree.copyWorktreePath": "複製 worktree 路徑",
  "tree.openWorktreeDir": "開啟 worktree 目錄",
  "tree.deleteWorktreeMenu": "刪除 worktree…",
  "tree.deleteWorktreeTitle": "刪除 worktree",
  "tree.deleteWorktreeBody":
    "選擇要刪除的 worktree，會從磁碟上刪掉它的工作目錄。",
  "tree.deleteWorktreePlaceholder": "選擇一個 worktree…",
  "tree.deleteWorktreeForce": "強制刪除（捨棄未提交的變更）",
  "tree.convertToNormalSession": "轉為普通會話",
  "tree.moveGroupToWorktree": "轉移到 Worktree…",
  "tree.convertToNormalGroup": "轉為普通群組",
  "merge.title": "合併分支",
  "merge.desc":
    "選好來源分支與目標分支，把來源合併進目標；方向可用中間按鈕調換。",
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
  "merge.doneMsg": (source: string, target: string) =>
    `已把「${source}」合併進「${target}」。`,
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
  "settings.termLineHeight": "終端機行高",
  "settings.chatTypography": "對話檢視",
  "settings.chatTypographyHint": "字型設定與終端機分開儲存，變更後立即生效。",
  "settings.chatFont": "對話字型",
  "settings.chatFontSize": "對話字級",
  "settings.chatLineHeight": "對話行高",
  "settings.fontDefault": "預設", // Default
  "settings.fontCustom": "自訂…", // Custom
  "settings.fontUnavailable": "本機未安裝此字型",
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
    "Claude：/vspawn <任務>；Codex：$vspawn <任務>。安裝後若 Codex 未列出技能，請建立新的 Codex 會話。",
  // Notification permission guidance
  "settings.notify": "系統通知", // System notifications
  "settings.notifyGranted": "已開啟", // Enabled
  "settings.notifyAllow": "允許通知", // Allow notifications
  "settings.notifyOffHint":
    "允許 VelaTerm 在智慧體需要你輸入或任務完成時通知你。", // Allow VelaTerm to alert you when an agent needs your input or finishes a task.
  "settings.notifyDeniedHint": "通知已被系統封鎖。開啟方法：", // Notifications are blocked. To turn them on:
  "settings.notifyStepsMac":
    "開啟「系統設定 ▸ 通知 ▸ VelaTerm」，開啟「允許通知」（建議樣式選橫幅或提醒）。", // open System Settings ▸ Notifications ▸ VelaTerm and turn on Allow Notifications (Banners or Alerts recommended).
  "settings.notifyStepsWin":
    "開啟「設定 ▸ 系統 ▸ 通知」，啟用 VelaTerm，並確認「專注輔助 / 勿擾」沒有封鎖它。", // open Settings ▸ System ▸ Notifications, enable VelaTerm, and make sure Focus assist / Do not disturb isn't blocking it.
  "settings.notifyStepsLinux": "在桌面環境的「設定 ▸ 通知」裡允許 VelaTerm。", // open your desktop's Settings ▸ Notifications and allow VelaTerm.
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
  "settings.scGlobalSearch": "搜尋所有會話", // Search all sessions
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
  "remote.ipLabel": "IP", // IP address
  "remote.ipAuto": "自動（第一個區域網路位址）", // Automatic (first LAN address)
  "remote.ipVpn": "VPN", // VPN
  "remote.qrHint": "用手機掃描即可在所選位址上開啟配對連結。", // Scan with your phone to open the pairing link on the selected address.
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
  "remote.autoRestartHint":
    "重新開啟應用程式時遠端存取會自動恢復，「停止伺服器」可關閉此功能。", // Remote access restarts automatically when the app is reopened. Stop Server turns this off.
  "remote.autostartFailed": "自動啟動失敗：", // Automatic start failed:
  "remote.mirror": "多端介面鏡像", // Mirror layout across devices
  "remote.mirrorHint":
    "分頁、分割與目前會話在所有已連線裝置上保持一致，各端的鍵盤焦點互不打擾。", // Tabs, splits, and the active session stay the same on every connected device. Keyboard focus stays put on each one.

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
  "connect.showPassword": "顯示密碼",
  "connect.hidePassword": "隱藏密碼",
  "connect.urlPasswordPlaceholder": "登入密碼",
  "connect.mirror": "鏡像遠端桌面版", // Mirror the remote desktop app
  "connect.mirrorHint":
    "分頁、分割與目前會話均與遠端機器上的桌面版保持一致，任一邊的變更兩邊同時可見。桌面版未執行時，本次連線會直接開啟它的資料庫；沒有資料庫則使用獨立資料庫。", // Same tabs, splits, and active session as the desktop app on the remote machine; changes on either side show on both. If the desktop app is not running, this connection opens its database directly, or a separate database when there is none.
  "connect.shareDesktopDb": "共用遠端桌面版的資料庫",
  "connect.shareDesktopDbHint":
    "與遠端機器的桌面版共用同一資料庫（建議兩邊同版本）。不勾則使用獨立資料庫。",

  // ── Sidebar (project tree, menus, and dialogs) ──
  "tree.newSession": "新增會話", // New Session
  "tree.newTerminalSession": "新增終端機", // New Terminal
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
  // New Collection / Collection name / research / Create Collection / No folder / Delete Collection
  "tree.newCollection": "新增集合",
  "tree.deleteCollection": "刪除集合",
  "collection.title": "新增集合",
  "collection.name": "集合名稱",
  "collection.namePlaceholder": "research",
  "collection.submit": "建立集合",
  "collection.tag": "無資料夾",
  "collection.deleteTitle": "刪除集合",
  "collection.deleteBody": (name) =>
    `刪除集合「${name}」？其中的分組與會話也會一併刪除，且無法復原。`,
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
  "clone.slowHint":
    "已連續 30 秒沒有進度，請檢查遠端機器的網路或 Proxy；你也可以取消後重試。",
  "clone.submit": "複製", // Clone
  "tree.globalSearch": "搜尋所有會話", // Search All Sessions
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
    `確定刪除「${name}」嗎？其儲存的搜尋與篩選條件會被移除，專案和會話不受影響。`,
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
  // Run as / Terminal / Conversation
  "tree.engineLabel": "開啟方式",
  "tree.engineTui": "終端機檢視",
  "tree.engineChat": "會話檢視",
  // The agent runs its own terminal interface.
  "tree.engineTuiHint": "執行智慧體自帶的終端介面。",
  // Messages and tool cards, with buttons for permission questions.
  "tree.engineChatHint": "以訊息和工具卡片呈現，權限請求可直接在介面中確認。",
  "tree.agentArgsLabel": "啟動參數（可選）", // Launch args (optional)
  // Working directory / Leave empty for the default
  "tree.workingDirLabel": "工作目錄",
  "tree.workingDirPlaceholder": "留空則使用預設目錄",
  "preset.execPathLabel": "可執行檔（選填）",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint":
    "留空則使用該智慧體已設定的指令。填了就只有這個會話用它，可以跑相容的替代程式。",
  "preset.saveLabel": "儲存為預設組合",
  "preset.namePlaceholder": "為這個預設組合命名",
  "preset.iconChoose": "選擇圖示",
  "preset.iconClear": "移除",
  "preset.iconHint": "方形圖片效果最好，其他圖片會裁切並縮放到 64x64。",
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
  "importSessions.results": ({ count }: { count: number }) => `${count} 筆結果`,
  "importSessions.selected": ({ count }: { count: number }) => `已選取 ${count} 筆`,
  "importSessions.clearSelection": "清除選取",
  "importSessions.clearSearch": "清除搜尋",
  "importSessions.noHistory": "此專案目錄下沒有可匯入的歷史會話。",
  "importSessions.title": "匯入會話",
  "importSessions.description": "尋找工作目錄與本專案一致的 Codex、Claude 和 OpenCode 歷史會話。選取並加入專案後，即可開啟並繼續對話。",
  "importSessions.search": "搜尋標題、Agent 名稱或會話 ID",
  "importSessions.empty": "找不到符合條件的會話。",
  "importSessions.imported": "已匯入",
  "importSessions.confirm": ({ count }: { count: number }) => `匯入（${count}）`,
  "importSessions.success": ({ count }: { count: number }) => `已將 ${count} 個會話加入專案。`,
  "resume.title": "恢復會話", // Resume Session
  "resume.desc":
    "選 agent 類型並填入該 agent 自身的 session id，開啟後續接原對話。", // Pick the agent type and enter the agent's own session id…
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
  "worktree.moveGroupTitle": "把分組轉移到 Worktree",
  "worktree.moveGroupHint":
    "之後在這個分組裡新建的會話會用這個 worktree；已有的則留在原來的目錄。",

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
  "search.hint":
    "搜尋會話內容。預設不含已封存會話，勾選「同時搜尋封存」可納入。", // Search session content. Archived sessions are excluded by default.
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
  "browser.desktopOnly": "瀏覽器分頁只能在桌面端開啟。", // Browser tabs open in the desktop app only.
  "browser.stop": "停止載入", // Stop loading
  "browser.openExternal": "以系統瀏覽器開啟", // Open in system browser
  "browser.addressPlaceholder": "輸入網址或搜尋字詞", // Enter URL or search terms
  "browser.quickAccess": "快速存取", // Quick access
  "browser.loading": "載入中…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "結束 VelaTerm？", // Quit VelaTerm?
  "quit.body": "正在執行的終端機會話與智慧體會話都會被停止。", // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "儲存工作區", // Save workspace
  "quit.saveWorkspaceHint":
    "下次開啟時還原相同的分頁和分割。終端機會還原出來，但不會自動重新啟動。", // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "結束", // Quit
  "dormant.body": "已從儲存的工作區還原，程序尚未啟動。", // Restored from your saved workspace. No process is running yet.
  "dormant.start": "啟動", // Start
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
  "term.imgClipboardUnavailable":
    "無法從剪貼簿讀取圖片，請重新複製圖片後再試。",
  "term.starting": (agent) => `正在啟動 ${agent}…`, // Starting {agent}…
  "term.startFailed": (err) => `啟動失敗: ${err}`, // Failed to start: {err}

  // ── Agent installation guidance ──
  "agentInstall.title": (label) => `${label} 尚未安裝`, // {label} is not installed
  "agentInstall.desc": (label) =>
    `VelaTerm 沒有在 PATH 上找到 ${label}。安裝後即可啟動此會話。`, // couldn't find {label} on PATH
  "agentInstall.install": "一鍵安裝", // Install now
  "agentInstall.retry": "重試啟動", // Retry launch
  "agentInstall.dismiss": "我自己裝", // I'll do it myself
  "agentInstall.docs": "安裝文件", // Install docs
  "agentInstall.needsNode": "需先安裝 Node.js / npm", // Requires Node.js / npm
  "agentInstall.afterInstall": "安裝後：", // After install:
  "agentInstall.pathSaved": (label: string) =>
    `已把 ${label} 的可執行檔路徑填入設定：`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} 已安裝`, // {label} is installed
  "agentInstall.doneDesc": "重新啟動本會話即可開始使用。", // Relaunch this session to start using it.
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
  "doc.saveTooltip": "儲存", // Save
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
  "files.dblClickOpen": "雙擊開啟", // Double-click to open
  "files.deleteConfirm": (name) => `確定刪除「${name}」？此操作無法復原。`, // Delete "{name}"? This can't be undone.

  // ── File transfer (remote access) ──
  "transfer.uploadsTitle": "上傳", // Uploads
  "transfer.download": "下載", // Download
  "transfer.upload": "上傳檔案…", // Upload Files…
  "transfer.uploadTooltip": "把檔案上傳到這個目錄", // Upload files to this folder
  "transfer.clear": "清空", // Clear
  "transfer.cancelled": "已取消", // Cancelled
  "transfer.failed": "失敗", // Failed
  "transfer.stalled": "正在重新連線…", // Reconnecting…
  "transfer.foldersUnsupported": "資料夾傳不了。", // Folders can't be uploaded.

  // ── Status bar ──
  "statusbar.sessions": (n) => `${n} 會話`, // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `點擊在左欄只看「${label}」會話（再點取消）`, // Click to show only "X" sessions…
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
  "statusbar.permScopeHint":
    "僅對當前會話生效。全域性設定，請前往「設定 ▸ 智慧體」中調整。", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg":
    "權限已變更，需重啟本會話才生效。重啟會接續目前對話，但會中斷進行中的任務。現在重啟？", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "立即重啟", // Restart now
  "statusbar.permRestartLater": "稍後", // Later
  "statusbar.permScopeTitle": "套用到？", // Apply to?
  "statusbar.permScopeSession": "僅目前會話", // This session only
  "statusbar.permScopeGlobal": "全域預設", // Global default
  "statusbar.permScopeGlobalHint":
    "本會話立即採用，並設為日後新建同類會話的預設（與設定同步）。", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

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
  "login.rateLimited": "嘗試次數過多，請稍候一分鐘後再試。", // Too many attempts. Please wait a minute and try again.
  "login.failed": "登入失敗，請重試", // Login failed, please try again
  "login.pairingRequired":
    "此服務要求使用配對連結存取。請用桌面端「遠端存取」產生的配對連結開啟。", // This server requires a pairing link
  "login.authFailed":
    "認證失敗。請確認存取密碼；若配對連結已重新產生，請改用新連結。", // Authentication failed, check password or use a new pairing link
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
  "transport.remoteCmdForbidden": (cmd: string) =>
    `遠端用戶端無法使用此命令：${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) =>
    `遠端用戶端無法寫入此設定鍵：${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) =>
    `遠端用戶端無法存取應用程式資料目錄中的檔案：${path}`, // Remote clients cannot access files in the app data directory

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
  "updater.versionLine": (version, current) =>
    `版本 ${version} — 目前 ${current}`,
  "updater.noNotes": "此版本沒有提供更新說明。",
  "updater.updateNow": "立即更新",
  "updater.later": "稍後",
  "updater.skipVersion": "略過此版本",
  "updater.skipVersionHint":
    "不再提示這個版本。之後仍可從「檢查更新」手動安裝。",
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
  "updater.downloadManuallyHint": "在瀏覽器中開啟下載頁面。",
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

  // ── 會話檢視（把智慧體會話讀成對話） ──
  "session.showConversation": "會話檢視",
  "session.showTerminal": "終端機檢視",
  "session.switchTitle": "切換檢視將重新啟動智慧體",
  "session.switchBody": "目前進行中的回合會中斷，對話內容不會遺失。",
  "session.switchConfirm": "切換",
  "session.loading": "正在讀取對話…",
  "session.unavailable": "暫時無法讀取此會話的對話內容",
  "session.working": "處理中…",
  "session.thinking": "思考過程",
  "session.toolRunning": "進行中",
  "session.toolUnknown": "工具",
  "session.toolFailed": "執行失敗",
  "session.toolNoDetail": "沒有更多記錄",
  "session.showMore": (n: number) => `顯示其餘 ${n} 個字元`,
  "session.showLess": "收合",
  "session.composerHint": "傳訊息給智慧體 · Enter 送出，Shift+Enter 換行",
  "session.send": "送出",

  // ── 對話引擎（以協定方式驅動的會話） ──
  "chat.empty": "在下方輸入內容，開始對話。",
  "chat.interrupt": "停止",
  "chat.interruptTooltip": "停止 · Esc",
  "chat.allow": "允許",
  "chat.deny": "拒絕",
  "chat.permissionAsk": (tool: string) => `${tool} 請求執行`,
  "chat.exited": (code: number) => `智慧體已結束（代碼 ${code}）`,
  "chat.modeNextTurn": "下一輪生效",
  "chat.modePendingHint": (current: string, next: string) =>
    `目前權限：${current}；下一輪權限：${next}。目前回合繼續使用原權限。`,
  "chat.modeTooltip": "權限模式",
  "chat.collaborationModeTooltip": "協作模式",
  "chat.collaborationMode.default": "預設模式",
  "chat.collaborationMode.defaultHint": "直接推進，僅在需要你決定時提問",
  "chat.collaborationMode.plan": "計畫模式",
  "chat.collaborationMode.planHint": "先調查並制定計畫，可用互動卡片提問",
  "chat.modelTooltip": "模型",
  "chat.keepChoice": "設為預設",
  "chat.keepChoiceFor": (model) => `設為 ${model} 的預設`,
  "chat.modelDefault": "預設模型",
  "chat.mode.default": "每次詢問",
  "chat.mode.acceptEdits": "自動接受變更",
  "chat.mode.plan": "計畫模式",
  "chat.mode.bypassPermissions": "全部放行",
  "chat.mode.readOnly": "唯讀",
  "chat.mode.fullAccess": "完整存取",
  "chat.placeholder": "傳訊息給智慧體，可用 /命令、/技能 與 @檔案",
  "chat.command.clearDescription": "封存目前工作階段並開始全新對話",
  "chat.command.rewindDescription": "從最近一則使用者訊息選擇要回復的內容",
  "chat.command.rewindUnavailable":
    "必須先有已完成的使用者訊息，且目前沒有進行中的回合、佇列訊息或權限要求，才能回復。",
  "chat.effortTooltip": "思考程度",
  "chat.effortDefault": "思考",
  "chat.effort.auto": "自動",
  "chat.effort.low": "低",
  "chat.effort.medium": "中",
  "chat.effort.high": "高",
  "chat.effort.xhigh": "很高",
  "chat.effort.max": "最高",
  "chat.effort.ultra": "極致",
  "chat.effort.ultracode": "Ultra Code",
  "chat.agentTooltip": "智慧體",
  "chat.effort.minimal": "最低",
  "chat.filterPlaceholder": "篩選",
  "chat.placeholderOpencode": "向智慧體傳送訊息，可使用 /命令 與 @檔案；以 ! 開頭可執行 Shell 命令",
  "chat.command.compactDescription": "摘要對話內容，釋放上下文空間",
  "chat.command.undoDescription": "復原最後一則訊息及其造成的檔案變更",
  "chat.command.redoDescription": "恢復上一次復原的內容",
  "chat.command.shareDescription": "建立分享連結",
  "chat.command.unshareDescription": "取消分享",
  "chat.mode.auto": "自動判斷",

  // ── 智慧體提出的問題，用表單作答 ──
  "chat.question.heading": "智慧體提出了一個問題",
  "chat.question.submit": "提交",
  "chat.question.next": "下一題",
  "chat.question.dismiss": "關閉",
  "chat.question.answerPlaceholder": "輸入你的回答",
  "chat.question.otherPlaceholder": "其他回答",
  "chat.question.answeredHeading": (n: number) => `已回答 ${n} 個問題`,
  "chat.question.blankAnswer": "未填寫",

  // ── 等待核准的計畫 ──
  "chat.plan.heading": "計畫已就緒，等待核准",
  "chat.plan.implement": "核准並執行",
  "chat.plan.reject": "拒絕",

  // ── 智慧體忙碌時排隊的訊息 ──
  "chat.placeholderBusy": "輸入訊息，本輪結束後將自動傳送",
  "chat.queueTooltip": (combo: string) => `本輪結束後傳送 · ${combo} 立即傳送`,
  "chat.queue.pending": "待傳送",
  "chat.queue.edit": "編輯",
  "chat.queue.remove": "刪除",

  // ── 貼上或拖進輸入框的圖片 ──
  "chat.attach.remove": "移除這張圖片",
  "chat.attach.tooMany": (max: number) => `一則訊息最多可附帶 ${max} 張圖片`,
  "chat.attach.tooLarge": (name: string, mb: number) => `${name} 超過 ${mb} MB，未加入附件`,
  "chat.attach.unreadable": (name: string) => `無法讀取 ${name}`,
  // Compacting the conversation… / Context compacted / Context compacted automatically
  "chat.compaction.running": "正在壓縮上下文…",
  "chat.compaction.manual": "上下文已壓縮",
  "chat.compaction.auto": "上下文已自動壓縮",
  "chat.compaction.from": (tokens: string) => `壓縮前 ${tokens} tokens`,
  // N steps
  "chat.subagent.steps": (n: number) => `${n} 步`,
  "chat.subagent.tokens": (tokens: string) => `${tokens} 個 token`,
  "chat.rewind.title": "從這裡回退",
  "chat.rewind.warning": "此操作無法復原。",
  "chat.rewind.conversation": "回退對話",
  "chat.rewind.files": "還原檔案",
  "chat.rewind.both": "回退對話並還原檔案",
  "chat.rewind.confirm.conversation": "刪除這則訊息及其之後的全部內容？",
  "chat.rewind.confirm.files": "將檔案還原到這則訊息之前的狀態？",
  "chat.rewind.confirm.both": "刪除這一回合，並還原它修改過的檔案？",
  "chat.rewind.unavailable": "這則訊息沒有對應的檔案檢查點。",
  "chat.rewind.previewing": "正在檢查檔案還原點…",
  "chat.rewind.cancel": "維持現狀",
  "chat.rewind.apply": "回退",
  "chat.rewind.applying": "正在回退…",
  "chat.rewind.fileSummary": (files: number, insertions: number, deletions: number) =>
    `將變更 ${files} 個檔案：+${insertions} −${deletions}。此操作無法復原。`,
  // ── 權限卡上的「以後不用再問」按鈕，點一下就採納 ──
  "chat.suggest.modeSession": (mode: string) => `本次會話改為${mode}`,
  "chat.suggest.mode": (mode: string) => `改為${mode}`,
  "chat.suggest.allowSession": (rule: string) => `本次會話允許 ${rule}`,
  "chat.suggest.allowAlways": (rule: string) => `一律允許 ${rule}`,
  "chat.suggest.dirSession": (dirs: string) => `本次會話允許存取 ${dirs}`,
  "chat.suggest.dirAlways": (dirs: string) => `一律允許存取 ${dirs}`,
  // ── Codex：網路放行規則、插話、內建命令，以及速度與語氣控制項 ──
  "chat.suggest.networkAlways": (host: string) => `一律允許存取網路主機 ${host}`,
  "chat.steer": "插話",
  "chat.stopping": "正在停止目前回合…",
  "chat.stopped": "目前回合已停止",
  "chat.steerAccepted": "插話已傳送",
  "chat.steerTooltip": (combo: string) => `${combo} 加入目前回合`,
  "chat.command.reviewDescription": "審查程式碼並指出需要處理的問題",
  "chat.command.reviewHint": "[branch <分支名稱> | commit <提交編號> | 說明]",
  "chat.command.startTimeout": "智慧體未能及時開啟會話",
  "chat.serviceTierTooltip": "速度",
  "chat.serviceTier.default": "標準速度",
  "chat.personalityTooltip": "語氣",
  "chat.personality.default": "預設語氣",
  "chat.personality.none": "中性",
  "chat.personality.friendly": "友善",
  "chat.personality.pragmatic": "務實",
  // ── 長對話：連續的工具呼叫摺成一行，以及回到結尾的入口 ──
  "chat.toolRun.count": (n: number) => `${n} 個工具呼叫`,
  "chat.toolRun.tooltip": "逐一檢視",
  "chat.backToEnd": "回到最新訊息",
  "chat.elicitation.heading": (server: string) => `${server} 要求輸入`,
  "chat.elicitation.cancel": "取消",
  "chat.elicitation.decline": "拒絕",
  "chat.elicitation.submit": "送出",
  "chat.elicitation.done": "完成",
  "chat.elicitation.choose": "請選擇…",
  "chat.effort.off": "關閉",
  "chat.effort.offHint": "不進行延伸思考",
  "chat.fastMode.label": "快速",
  "chat.fastMode.on": "快速模式已開啟",
  "chat.fastMode.off": "快速模式已關閉",
  "chat.usage.context": (used: string, max: string, pct: number) =>
    `上下文：已用 ${used}，上限 ${max} token（${pct}%）`,
  "chat.usage.cost": (usd: string) => `本次會話費用：$${usd}`,
  "chat.usage.rateLimited": (resets: string) => `已達到用量上限，${resets} 重設`,
  "chat.usage.rateWarning": (pct: number, resets: string) => `用量上限：已用 ${pct}%，${resets} 重設`,
  "chat.mcp.codexScope": "此操作將修改 Codex 使用者設定，影響使用該設定的其他工作階段。是否繼續？",
  "chat.mcp.tooltip": "MCP 伺服器",
  "chat.mcp.loading": "正在讀取伺服器清單…",
  "chat.mcp.backendUnsupported": "目前連線的 VelaTerm 後端不支援 MCP 管理。請更新並重新啟動該後端，然後重試。",
  "chat.mcp.none": "尚未設定 MCP 伺服器",
  "chat.mcp.tools": (n: number) => `${n} 個工具`,
  "chat.mcp.reconnect": "重新連線",
  "chat.mcp.disable": "停用",
  "chat.mcp.enable": "啟用",
  "chat.mcp.status.connected": "已連線",
  "chat.mcp.status.disabled": "已停用",
  "chat.mcp.status.failed": "連線失敗",
  "chat.mcp.status.pending": "連線中",
  "chat.mcp.status.disconnected": "已中斷",
  "chat.mcp.status.other": "未知",
  "chat.tasks.label": "工作",
  "chat.tasks.tooltip": "背景工作",
  "chat.tasks.backgroundAll": "將執行中的作業移至背景",
  "chat.tasks.none": "沒有背景工作",
  "chat.tasks.stop": "停止",
  "chat.tasks.open": "Open task", // TODO translate
  "chat.tasks.tabTooltip": "Background task", // TODO translate
  "chat.tasks.status.running": "Running", // TODO translate
  "chat.tasks.status.completed": "Completed", // TODO translate
  "chat.tasks.status.failed": "Failed", // TODO translate
  "chat.tasks.status.canceled": "Stopped", // TODO translate
  "chat.tasks.status.ended": "Ended", // TODO translate
  "chat.tasks.stale": "No longer reported by the agent", // TODO translate
  "chat.tasks.elapsed": "Elapsed", // TODO translate
  "chat.tasks.tokens": "Tokens", // TODO translate
  "chat.tasks.toolUses": "Tool calls", // TODO translate
  "chat.tasks.currentAgent": "Current agent", // TODO translate
  "chat.tasks.started": "Started", // TODO translate
  "chat.tasks.finished": "Finished", // TODO translate
  "chat.tasks.summary": "Summary", // TODO translate
  "chat.tasks.outputFile": "Output file", // TODO translate
  "chat.tasks.phases": "Phases", // TODO translate
  "chat.tasks.noProgress": "This task reports no per-agent progress.", // TODO translate
  "chat.tasks.attempt": (n: number) => `attempt ${n}`, // TODO translate
  "chat.tasks.prompt": "Prompt", // TODO translate
  "chat.tasks.result": "Result", // TODO translate
  "chat.tasks.agentState.start": "running", // TODO translate
  "chat.tasks.agentState.done": "done", // TODO translate
  "chat.retry.line": (attempt: number, max: number, seconds: number, message: string) =>
    `${seconds} 秒後重試（${attempt}/${max}）：${message}`,
  "chat.notify.dismiss": "關閉",
};

export default zhTW;
