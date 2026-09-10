//! Simplified Chinese dictionary based on the original UI copy; en.ts enforces the complete key set.

import type en from "./en";

const zhCN: typeof en = {
  // Project code intelligence and memory associations.
  "knowledge.title": "代码图谱",
  "knowledge.intro": "查看代码关系，并关联已保存的设计决策。",
  "knowledge.setup": "在当前后端安装 CodeGraph 后，即可启用项目索引。",
  "knowledge.downloadNotice": "从 GitHub 下载经过校验的 CodeGraph 运行时。代码索引在本机完成，遥测和更新检查均已关闭。",
  "knowledge.install": "下载 CodeGraph",
  "knowledge.installing": "正在下载并安装…",
  "knowledge.directory": "工作目录",
  "knowledge.enable": "启用索引",
  "knowledge.disable": "停用索引",
  "knowledge.sync": "同步索引",
  "knowledge.ready": "可用",
  "knowledge.disabled": "已停用",
  "knowledge.indexing": "正在建立索引…",
  "knowledge.syncing": "正在同步…",
  "knowledge.failed": "失败",
  "knowledge.symbols": "符号",
  "knowledge.files": "文件",
  "knowledge.edges": "关系",
  "knowledge.search": "搜索符号或文件路径…",
  "knowledge.searchButton": "搜索",
  "knowledge.noResults": "没有匹配的符号。",
  "knowledge.selectSymbol": "选择一个符号，查看源码、关联关系和相关记忆。",
  "knowledge.source": "源码",
  "knowledge.incoming": "传入关系",
  "knowledge.outgoing": "传出关系",
  "knowledge.noEdges": "索引中没有相关关系。",
  "knowledge.analysisNote": "关系来自静态分析，可能不完整或存在不确定性。",
  "knowledge.changed": "查询期间文件发生了变化。请再次同步，再使用行号或确认复核结果。",
  "knowledge.truncated": "当前视图已限量显示，部分关系或源码行未展示。",
  "knowledge.linkMemory": "关联记忆",
  "knowledge.chooseMemory": "选择记忆条目",
  "knowledge.noLinks": "暂无代码关联。可在符号详情中关联记忆。",
  "knowledge.inspect": "核对代码与记忆",
  "knowledge.unlink": "移除关联",
  "knowledge.codeReferences": "代码引用",
  "knowledge.refresh": "刷新",
  "knowledge.current": "未变更",
  "knowledge.review": "需要复核",
  "knowledge.unavailable": "不可用",
  "knowledge.reviewHelp": "请将此记忆与所显示的代码进行核对。确认后仅记录当前文件版本，不修改记忆正文。",
  "knowledge.confirmReview": "确认已复核",
  "knowledge.agentHint": "智能体可在此工作目录运行 vknowledge search \"主题\"。查询会同步已启用的索引，并分别返回代码和记忆。",
  "knowledge.busy": "索引任务正在运行。可以关闭此页面，或停用索引以停止任务。",
  "knowledge.disabledHelp": "启用此目录的索引后即可查询代码。停用会保留索引和记忆关联。",
  "knowledge.conflict": "代码或记忆已发生变化。请重新加载后再保存关联。",
  "knowledge.symbolMissing": "符号或源码已不可用。请同步索引后重新搜索。",
  "knowledge.directoryMissing": "工作目录不存在或已发生变化。请检查项目和会话的路径。",
  "knowledge.partial": "索引不完整。请再次同步，并检查源码文件是否可读。",
  "knowledge.interrupted": "上一次任务被中断。请同步索引以重试。",
  "knowledge.checksum": "下载文件的校验值不匹配，未安装运行时。",
  "knowledge.downloadFailed": "无法下载 CodeGraph。请检查后端与 GitHub 的连接后重试。",
  "knowledge.timeout": "索引任务超时。请检查仓库大小后重试。",
  "knowledge.error": "操作失败。请检查后端的目录访问权限和运行时后重试。",

  // Global Memory: a thematic LLM Wiki shared across sessions.
  "memory.title": "全局记忆",
  "memory.add": "加入到全局记忆",
  "memory.intro": "以持续更新的 Wiki 组织知识、决策和经验，形成跨会话共享的长期记忆。",
  "memory.entries": "记忆条目",
  "memory.emptyJobs": "暂无整理记录。",
  "memory.jobs": "整理记录",
  "memory.search": "检索记忆标题与正文…",
  "memory.empty": "暂无匹配的记忆。可从会话中加入内容，逐步建立你的 Wiki。",
  "memory.emptyDetail": "选择一个条目，查阅知识内容、关联与来源。",
  "memory.new": "新建记忆",
  "memory.titleField": "标题",
  "memory.summary": "摘要",
  "memory.content": "正文（Markdown）",
  "memory.tags": "标签（用逗号分隔）",
  "memory.related": "关联记忆",
  "memory.backlinks": "引用此条目的记忆",
  "memory.sources": "来源",
  "memory.history": "修订历史",
  "memory.restore": "恢复此版本",
  "memory.restoreConfirm": "将此修订恢复为新版本？当前版本仍会保留在历史中。",
  "memory.deleteConfirm": "删除此记忆及其修订历史？来源会话不受影响。",
  "memory.export": "导出 Markdown",
  "memory.selectAgent": "智能体",
  "memory.model": "模型（可选）",
  "memory.modelHint": "留空时使用智能体已配置的模型。",
  "memory.compile": "整理并保存",
  "memory.compileHelp": "所选智能体会按主题梳理此会话，并与已有记忆合并。会话文本与相关记忆将通过你配置的智能体发送给模型。",
  "memory.unavailable": "未安装或未配置",
  "memory.allTags": "全部标签",
  "memory.updated": "最近更新",
  "memory.titleSort": "按标题排序",
  "memory.sourceNote": "此快照保留整理时使用的会话文本；原会话删除后仍可查阅。",
  "memory.noKnowledge": "未发现可复用的知识，没有更改记忆条目。",
  "memory.running": "进行中",
  "memory.completed": "已完成",
  "memory.failed": "失败",
  "memory.cancelled": "已取消",
  "memory.extract": "提取主题",
  "memory.merge": "合并知识",
  "memory.commit": "保存记忆",
  "memory.done": "已保存",
  "memory.closeHint": "整理期间可关闭此窗口，稍后在整理记录中查看进度。",
  "memory.conflict": "操作期间此记忆已发生变更。请重新加载后再试；本次修改尚未保存。",
  "memory.duplicate": "已存在同名记忆，请打开该条目合并内容。",
  "memory.busy": "已有整理任务正在运行。请等待完成，或在整理记录中取消。",
  "memory.notFound": "此记忆、来源或任务已不存在。",
  "memory.noTranscript": "此会话暂无可读取的对话内容。",
  "memory.agentUnavailable": "所选智能体不可用，请在设置中检查其可执行文件路径。",
  "memory.invalid": "部分字段或链接无效，请检查标题、正文及关联记忆。",
  "memory.processFailed": "智能体未能完成整理。请检查登录状态、模型及 CLI 配置后重试。",
  "memory.timeout": "智能体调用超时，请更换可用模型或缩短会话后重试。",
  "memory.interrupted": "整理任务已中断，可重试处理已保存的来源快照。",
  "memory.tooLarge": "来源、上下文或输出超出支持的大小，未截断内容，也未写入记忆。",
  "memory.invalidOutput": "智能体返回的结构化数据无效，未写入记忆。请重试或更换智能体。",
  "memory.loadError": "无法加载记忆数据，请检查连接后重试。",
  "memory.unsaved": "放弃尚未保存的修改？",
  "memory.source": "来源快照",

  // ── Common ──
  "common.cancel": "取消",
  "common.confirm": "确定",
  "common.delete": "删除",
  "common.save": "保存",
  "common.create": "新建",
  "common.close": "关闭",
  "chat.imageViewOriginal": "查看原图",
  "chat.imageCopy": "复制图片",
  "chat.imageSave": "保存图片",
  "chat.imageActionFailed": "图片操作失败，请重试。",
  "common.copy": "复制",
  "common.cut": "剪切",
  "common.paste": "粘贴",
  "common.selectAll": "全选",
  "common.copied": "已复制",
  "chat.sync.loading": "正在同步会话…",
  "chat.sync.failed": "同步失败，已加载的消息仍可查看。",
  "chat.sync.history": "加载更早的消息",
  "chat.submission.updateRequired": "请先更新服务端，再使用此客户端发送消息。",
  "chat.submission.sending": "发送中…",
  "chat.submission.sent": "已发送",
  "chat.submission.queued": "已排队",
  "chat.submission.failed": "发送失败",
  "chat.submission.unknown": "发送结果待确认",
  "chat.submission.check": "确认状态",
  "common.retry": "重试",
  "common.experimental": "实验性功能",
  "common.refresh": "刷新",
  "common.loading": "加载中…",
  "common.prev": "上一个",
  "common.next": "下一个",
  "common.on": "开",
  "common.off": "关",
  "common.gotIt": "知道了",
  "common.rename": "重命名",
  "common.edit": "编辑",
  "common.open": "打开",
  "common.session": "会话",

  // ── Session types and status ──
  "kind.terminal": "终端",
  "kind.browser": "浏览器",
  "status.idle": "空闲",
  "status.running": "运行中",
  "status.exited": "已退出",
  "status.error": "异常",
  "status.working": "处理中",
  "status.asking": "待确认",
  "status.waiting": "已查看",
  "status.unavailable": "状态不可用",
  "indicator.unread": "未读 · 待查看",

  // ── Title bar ──
  "titlebar.builtAt": (time) => `构建于 ${time}`,
  "titlebar.versionMismatch": (frontend, backend) =>
    `版本不一致：前端 v${frontend} ≠ 后端 v${backend}，请重新构建或同步部署。`,
  "titlebar.hotReloadedAt": (time) => `热更新于 ${time}`,
  "titlebar.themeSystem": (resolved) => `跟随系统（当前${resolved}）`,
  "titlebar.themeDark": "暗色",
  "titlebar.themeLight": "亮色",
  "titlebar.browser": "内置浏览器",
  "titlebar.remoteAccess": "远程访问（浏览器）",
  "titlebar.connectRemote": "连接远程服务",
  "titlebar.mirrored": "镜像中", // Mirrored
  "titlebar.mirroredHint":
    "镜像已开启：标签页、分屏和当前会话跟随主机。开关在主机那边。", // Mirroring is on: tabs, splits, and the active session follow the host. The switch is on the host.
  "titlebar.mirroredBy": (n: number) => `被 ${n} 端镜像`, // Mirrored by {n}
  "titlebar.mirroredByHint": (n: number) =>
    `有 ${n} 个远程端连着。标签页、分屏和当前会话是共用的，两边都能改。`, // {n} remote clients are connected. Tabs, splits, and the active session are shared, and either side can rearrange them.
  "titlebar.clientsTitle": "已连接的客户端", // Attached clients
  "titlebar.clientUnnamed": "未命名客户端", // Unnamed client
  "titlebar.clientSince": (time: string) => `${time} 起`, // since {time}
  "titlebar.share": "分享", // Share
  // ── Alt-triggered menu bar (Windows/Linux) ──
  "menubar.file": "文件", // File
  "menubar.terminal": "终端", // Terminal
  "menubar.help": "帮助", // Help
  "menubar.newTerminal": "新建终端", // New Terminal
  "menubar.visitWebsite": "访问官网", // Visit Website
  "menubar.sendFeedback": "发送反馈", // Send Feedback
  "menubar.clearBadges": "清除通知标识", // Clear Notification Badges
  "share.title": "分享 VelaTerm", // Share VelaTerm
  "share.subtitle":
    "我们是 VelaTerm 背后的一个小团队。如果你喜欢它，欢迎把 VelaTerm 分享给更多人。让更多人知道我们，对我们真的很重要。谢谢你的支持！❤️", // We're a small team behind VelaTerm. If you enjoy it, please share VelaTerm with others…
  "share.copyLink": "复制链接", // Copy link
  "share.copied": "已复制！", // Copied!
  "share.wechatMoments": "微信朋友圈",
  "share.weibo": "微博",
  "share.xiaohongshu": "小红书",
  "share.xiaohongshuAction": "复制分享文案和链接，然后打开小红书创作中心",
  "share.wechatQrTitle": "分享到微信朋友圈",
  "share.wechatQrHint": "请使用微信扫码打开链接，再选择“分享到朋友圈”。",
  "share.backToPlatforms": "返回分享平台",
  "titlebar.appearance": "外观设置",
  "titlebar.showLeft": "显示左栏",
  "titlebar.hideLeft": "隐藏左栏",
  "titlebar.showRight": "显示信息面板",
  "titlebar.hideRight": "隐藏信息面板",

  // ── Settings ──
  "settings.title": "设置",
  "settings.catTerminal": "终端",
  "settings.catBehavior": "行为",
  "settings.catAgents": "智能体",
  "settings.permDefault": "默认",
  "settings.permYolo": "YOLO",
  "settings.yoloHint": (flag: string) =>
    `启动时附加 ${flag}，跳过全部权限确认，请谨慎使用。`,
  "settings.permViaEnvHint":
    "通过配置文件注入跳过全部权限确认（无命令行 flag）。仅影响该会话启动时的行为。",
  "settings.catGeneral": "通用",
  "settings.cliLabel": "Shell 命令",
  "settings.cliInstall": "安装 ‘vela’ 命令",
  "settings.cliUninstall": "卸载 ‘vela’ 命令",
  "settings.cliInstalledAt": (path: string) => `已安装到 ${path}`,
  "settings.cliConflict": (path: string) =>
    `${path} 已存在其他 ‘vela’ 命令，VelaTerm 不会覆盖它。`,
  "settings.cliHint":
    "像 VS Code 的 `code` 一样，把 `vela <项目路径>` 添加到 PATH。",
  "settings.agentArgsHint":
    "各类型智能体新建会话时套用的默认启动参数。新建或编辑单个会话时设的参数会覆盖这里的默认。留空表示不带参数。",
  "settings.agentPathLabel": "可执行文件路径（可选）",
  "settings.agentPathPlaceholder": "如 ~/.local/bin/claude——留空则从 PATH 查找",
  "settings.agentPathHint":
    "设置后，该类型会话一律按这条完整路径启动，不再从 PATH 查找命令。适用于「装了但不在 shell PATH 上」的情况。一键安装成功且能探测到落点时会自动填入。",
  "settings.appearance": "外观",
  "settings.accent": "强调色",
  "settings.accentAuto": "跟随明暗",
  "settings.density": "密度",
  "settings.densityCompact": "紧凑",
  "settings.densityRegular": "标准",
  "settings.densityComfy": "宽松",
  "settings.pane": "分屏",
  "settings.paneFlush": "无缝",
  "settings.paneCard": "卡片",
  "settings.divider": "分隔条",
  "settings.dividerSubtle": "极细",
  "settings.dividerVisible": "可见",
  "settings.nav": "左栏",
  "settings.navTree": "标准",
  "settings.navCompact": "紧凑",
  "settings.tabs": "标签页",
  "settings.defaultSessionEngine": "新建会话的默认视图",
  "settings.defaultSessionEngineHint": "已有会话保持创建时的视图。",
  "settings.dynamicStatusFilter": "状态筛选动态增加",
  "settings.tabSingle": "单标签",
  "settings.tabMulti": "多标签",
  "settings.maxLiveTabs": "后台保活上限",
  "settings.defaultShell": "默认 Shell",
  "settings.spawnConfirm": "派生前确认",
  "settings.usageAuto": "额度自动刷新",
  "settings.usageRefresh": "额度刷新",
  "settings.cleanImages": "自动清理粘贴图片",
  "settings.cleanImagesHint":
    "粘贴或拖入终端的图片会先存成临时文件（把路径发给 agent）。开启后：退出时删除本次会话产生的这些临时图，启动时清理超过 24 小时的残留。文档里的图片不受影响。",
  "settings.cleanImagesNow": "立即清理",
  "settings.cleanImagesResult": (n: number, size: string) =>
    `已清理 ${n} 个临时图片（释放 ${size}）。`,
  "settings.cleanImagesEmpty": "没有需要清理的临时图片。",
  "settings.imagePasteMode": "图片粘贴",
  "settings.imagePasteUpload": "粘贴文件路径",
  "settings.imagePasteAgent": "原生图片粘贴",
  "settings.imagePasteHint":
    "选择粘贴图片时写入的内容（仅本地桌面端）。粘贴文件路径：把图片存成临时文件，在输入框显示可读路径（Codex 显示 image_path: …）。原生图片粘贴：触发 Claude 或 Codex 读取系统剪贴板并显示自身的图片占位符。",
  "settings.imagePasteRemoteHint":
    "远程会话固定粘贴文件路径，让智能体能在其所在机器读取图片；原生图片粘贴仅在本地桌面端可用。",
  "spawn.title": "启动派生会话？",
  "spawn.fromSession": "来自",
  "spawn.promptLabel": "提示词",
  "spawn.agentLabel": "智能体",
  "spawn.worktreeLabel": "独立 git worktree",
  "spawn.modelLabel": "模型",
  "spawn.effortLabel": "推理强度",
  "spawn.modelDefault": "默认", // Default
  "spawn.modelLoading": "正在获取模型…", // Listing models…
  "spawn.modelListUnavailable": "取不到模型列表 — 在上面直接填模型名", // No model list available — type an identifier above
  "spawn.launch": "启动",
  "spawn.remaining": (n: number) => `还有 ${n} 个待确认`,
  "spawn.notifyTitle": "派生会话待确认",
  "orch.title": "启动这些智能体？",
  "orch.notifyTitle": "编排等待确认",
  "orch.coordinatorName": "进度",
  "orch.sharedSettings": "总设置",
  "orch.agentLabel": "类型",
  "orch.modelLabel": "模型",
  "orch.effortLabel": "思考程度",
  "orch.nameLabel": "名称",
  "orch.promptLabel": "任务",
  "orch.worktreeLabel": "工作树",
  "orch.worktreeNone": "就用当前目录",
  "orch.worktreeShared": "共用一个工作树",
  "orch.worktreeEach": "每人一个工作树",
  "orch.follow": "跟随总设置",
  "orch.overridden": "已改",
  "orch.remove": "删除",
  "orch.launch": (n: number) => `启动 ${n} 个`,
  "orch.modelPlaceholder": "智能体默认",
  "orch.effortPlaceholder": "智能体默认",
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "查看改动…",
  "changes.title": "改动",
  "changes.loading": "加载中…",
  "changes.loadingDiff": "加载 diff…",
  "changes.noChanges": "没有改动",
  "changes.refresh": "刷新",
  "changes.notRepo": "不是 git 仓库",
  "changes.selectFile": "选择文件查看",
  "changes.binary": "二进制文件，无法逐行 diff",
  "changes.commitTitle": (hash: string) => `提交 ${hash}`,

  "git.staged": "已暂存",
  "git.changes": "更改",
  "git.untracked": "未跟踪文件",
  "git.committed": "已提交的更改",
  "git.stage": "暂存",
  "git.unstage": "取消暂存",
  "git.stageAll": "全部暂存",
  "git.unstageAll": "全部取消暂存",
  "git.discard": "放弃更改",
  "git.deleteFile": "删除",
  "git.viewAll": "查看全部",
  "git.detached": "（游离 HEAD）",
  "git.aheadBehind": "相对上游分支领先和落后的提交数",
  "git.commitPlaceholder": "提交说明",
  "git.amend": "修改上一次提交",
  "git.amendCommit": "修改提交",
  "git.commitCount": (n: number) => `提交 ${n} 个文件`,
  "git.commitNoFiles": "这个提交没有文件变化",
  "git.noCommits": "还没有提交",
  "git.loadMore": "加载更多",
  "tree.merge": "合并…",
  "tree.copyWorktreePath": "复制 worktree 路径",
  "tree.openWorktreeDir": "打开 worktree 目录",
  "tree.deleteWorktreeMenu": "删除 worktree…",
  "tree.deleteWorktreeTitle": "删除 worktree",
  "tree.deleteWorktreeBody":
    "选择要删除的 worktree，会从磁盘上删掉它的工作目录。",
  "tree.deleteWorktreePlaceholder": "选择一个 worktree…",
  "tree.deleteWorktreeForce": "强制删除（丢弃未提交的改动）",
  "tree.convertToNormalSession": "转为普通会话",
  "tree.moveGroupToWorktree": "转移到 Worktree…",
  "tree.convertToNormalGroup": "转为普通分组",
  "merge.title": "合并分支",
  "merge.desc":
    "选好来源分支与目标分支，把来源合并进目标；方向可用中间按钮调换。",
  "merge.notRepo": "该会话目录不是 git 仓库。",
  "merge.loadingBranches": "正在读取分支…",
  "merge.loadingDiff": "正在加载差异…",
  "merge.sourceLabel": "来源分支",
  "merge.targetLabel": "目标分支",
  "merge.selectBranch": "选择分支…",
  "merge.swap": "调换方向",
  "merge.pickHint": "选好来源与目标分支后，这里会显示合并将带入的改动。",
  "merge.changes": (target: string) => `将带入「${target}」的改动`,
  "merge.noChanges": "没有文件改动。",
  "merge.sameBranch": "来源与目标是同一条分支。",
  "merge.branchGone": "所选分支已不存在，请重新选择。",
  "merge.upToDate": "目标分支已包含来源分支的改动，无需合并。",
  "merge.targetNotCheckedOut": (target: string) =>
    `目标分支「${target}」没有被任何工作树 checkout，无法本地合并。请先在某个工作树切到该分支。`,
  "merge.targetDirty": "目标分支所在工作树有未提交改动，合并可能受阻。",
  "merge.sourceDirtyNote": "来源分支所在工作树有未提交改动，会先提交再合并。",
  "merge.commitMsgLabel": "提交信息",
  "merge.commitMsgPlaceholder": "描述这次改动（作为提交信息）",
  "merge.apply": "合并",
  "merge.commitAndApply": "提交并合并",
  "merge.working": "正在合并…",
  "merge.doneMsg": (source: string, target: string) =>
    `已把「${source}」合并进「${target}」。`,
  "merge.conflictMsg": (target: string) =>
    `合并出现冲突，请到「${target}」所在工作树的终端里解决后提交：`,
  "merge.close": "关闭",
  "gitea.title": "Gitea 集成",
  "gitea.desc":
    "配置 Gitea 服务器后，可用「开 PR」的方式落地 worktree。token 存进系统钥匙串（不可用时退回明文）。",
  "gitea.baseUrl": "服务器地址",
  "gitea.token": "访问 token",
  "gitea.tokenSet": "已保存（留空则保留）",
  "gitea.tokenPlaceholder": "个人访问 token",
  "gitea.test": "测试连接",
  "gitea.saved": "已保存。",
  "settings.renderer": "终端渲染器",
  "settings.redrawOnReveal": "切回标签时重绘",
  "settings.catAdvanced": "高级",
  "settings.outputScheduler": "前台优先输出",
  "settings.recordSessions": "记录会话日志",
  "settings.recordSessionsHint":
    "默认关。开启后会把会话的终端输出存成日志文件，供归档回放与搜索。普通终端会话一律不录；agent 会话归档读自己的对话记录。",
  "settings.fonts": "字体",
  "settings.uiFont": "界面字体",
  "settings.uiFontSize": "界面字号",
  "settings.termFont": "终端字体",
  "settings.termFontSize": "终端字号",
  "settings.termLineHeight": "终端行高",
  "settings.chatTypography": "会话视图",
  "settings.chatTypographyHint": "字体设置独立于终端视图，修改后立即生效。",
  "settings.chatFont": "会话字体",
  "settings.chatFontSize": "会话字号",
  "settings.chatLineHeight": "会话行高",
  "settings.fontDefault": "默认",
  "settings.fontCustom": "自定义…",
  "settings.fontUnavailable": "本机未安装该字体",
  "settings.fontAuto": "自动",
  "settings.fontSmaller": "缩小",
  "settings.fontLarger": "放大",
  "settings.fontReset": "复位",
  "settings.sound": "通知提示音",
  "settings.language": "语言",
  "settings.langAuto": "自动（跟随系统）",
  "settings.skillLabel": "Vela 技能",
  "settings.skillInstall": "安装",
  "settings.skillInstalled": "重新安装",
  "settings.skillInvokeHint":
    "Claude：/vspawn <任务>；Codex：$vspawn <任务>。安装后若 Codex 未列出技能，请新建 Codex 会话。",
  // Notification permission guidance
  "settings.notify": "系统通知",
  "settings.notifyGranted": "已开启",
  "settings.notifyAllow": "允许通知",
  "settings.notifyOffHint":
    "允许 VelaTerm 在智能体需要你输入或任务完成时通知你。",
  "settings.notifyDeniedHint": "通知已被系统屏蔽。开启方法：",
  "settings.notifyStepsMac":
    "打开「系统设置 ▸ 通知 ▸ VelaTerm」，开启「允许通知」（建议样式选横幅或提醒）。",
  "settings.notifyStepsWin":
    "打开「设置 ▸ 系统 ▸ 通知」，启用 VelaTerm，并确认「专注助手 / 勿扰」没有屏蔽它。",
  "settings.notifyStepsLinux": "在桌面环境的「设置 ▸ 通知」里允许 VelaTerm。",
  "settings.notifyStepsBrowser":
    "点击地址栏的站点权限图标，把通知设为「允许」。",
  "settings.notifyUnsupported": "当前环境不支持系统通知。",
  "settings.notifyOpenSettings": "打开系统设置",
  // Shortcut categories
  "settings.catShortcuts": "快捷键",
  "settings.scOpenProject": "打开项目",
  "settings.scNewTab": "新建终端",
  "settings.scNewBrowserTab": "新建浏览器标签",
  "settings.scClosePane": "关闭分屏/标签",
  "settings.scSplitRight": "向右分屏",
  "settings.scSplitDown": "向下分屏",
  "settings.scSearch": "在终端内搜索",
  "settings.scGlobalSearch": "搜索所有会话",
  "settings.scSaveDoc": "保存文档",
  "settings.scRecording": "请按下组合键…",
  "settings.scHint": "点一下某个快捷键，再按下新的组合键（需含 Cmd/Ctrl）。",
  "settings.scReset": "恢复默认",
  "settings.scConflict": (label: string) => `已被「${label}」占用`,

  // ── Remote access panel ──
  "remote.title": "远程访问（浏览器）",
  "remote.desc":
    "开启后，同一局域网的设备用浏览器打开下方地址、输入密码，即可获得与桌面一致的界面。",
  "remote.needPassword": "请先设置访问密码",
  "remote.running": (port) => `运行中 · 端口 ${port}`,
  "remote.urlsHint":
    "用浏览器打开下面和你设备同一 WiFi / 网段的地址（多张网卡时挑对的那个；VPN/隧道地址排在最后，外部设备多半连不上）：",
  "remote.copyUrl": "点击复制地址",
  "remote.moreUrls": (n: number) => `其它 ${n} 个链接`,
  "remote.lessUrls": "收起",
  "remote.stop": "停止服务",
  "remote.passwordPlaceholder": "设置访问密码",
  "remote.starting": "启动中…",
  "remote.start": "开启服务",
  "remote.portLabel": "端口",
  "remote.portInvalid": "端口必须是 1 到 65535 之间的数字",
  "remote.ipLabel": "IP", // IP address
  "remote.ipAuto": "自动（第一个局域网地址）", // Automatic (first LAN address)
  "remote.ipVpn": "VPN", // VPN
  "remote.qrHint": "用手机扫码即可在所选地址上打开配对链接。", // Scan with your phone to open the pairing link on the selected address.
  "remote.fingerprintLabel": "证书指纹（SHA-256）",
  "remote.fingerprintHint":
    "首次连接时浏览器会提示证书不受信任，这是自签证书的正常现象；核对此指纹可确认连接的是本机。",

  "remote.pairingCreate": "生成配对链接", // Create pairing link
  "remote.pairingRegenerate": "重新生成链接（踢掉全部设备）", // Regenerate link (disconnects all)
  "remote.pairingCreating": "生成中…", // Generating…
  "remote.pairingHint":
    "用浏览器打开后输入密码。链接含访问凭据，只分享给自己的设备。", // Open in a browser, then enter the password…

  "remote.devicesLabel": "已配对设备", // Paired devices
  "remote.lastSeen": "最后连接", // Last seen
  "remote.revoke": "吊销", // Revoke
  "remote.deviceBlock": "禁止访问", // Block
  "remote.deviceBlockConfirm": "确认禁止", // Confirm block
  "remote.deviceBlockHint":
    "被禁设备会被断开且无法重连（需重新用配对链接），其他设备不受影响。", // Block hint
  "remote.devicesEmpty": "暂无已配对设备", // No paired devices yet
  "remote.autoRestartHint":
    "重新打开应用时远程访问会自动恢复，「停止服务」可关闭此功能。", // Remote access restarts automatically when the app is reopened. Stop Server turns this off.
  "remote.autostartFailed": "自动启动失败：", // Automatic start failed:
  "remote.mirror": "多端界面镜像", // Mirror layout across devices
  "remote.mirrorHint":
    "标签、分屏和当前会话在所有已连接设备上保持一致，各端的键盘焦点互不打扰。", // Tabs, splits, and the active session stay the same on every connected device. Keyboard focus stays put on each one.

  // ── Remote connection panel ──
  "connect.title": "连接远程服务",
  "connect.pairingPlaceholder": "粘贴配对链接",
  "connect.confirmConnect": "指纹无误，连接",
  "connect.desc": "输入远程 VelaTerm 的地址和密码，在新窗口中连接并操控。",
  "connect.addressPlaceholder": "IP 地址，如 192.168.1.100",
  "connect.portPlaceholder": "端口",
  "connect.connecting": "连接中…",
  "connect.connect": "连接",
  "connect.stagePreparing": "准备服务端…",
  "connect.stageTransferring": "传输服务端…",
  "connect.stageStarting": "启动服务端…",
  "connect.sshFingerprintLabel": (kt: string) => `SSH 主机指纹（${kt}）`,
  "connect.sshHostNew": "首次连接这台主机，请核对指纹一致后再继续。",
  "connect.sshHostChanged":
    "⚠ 这台主机的密钥变了：可能是服务器重装，也可能是中间人攻击。确认无误再继续。",
  "connect.urlCertChanged":
    "⚠ 这台服务器的证书指纹自你上次确认后变了：可能是服务器重装，也可能是中间人攻击。确认无误再继续。",
  "connect.sshPasswordLabel": "SSH 密码",
  "connect.sshPasswordPlaceholder": "账户密码",
  "connect.savedHosts": "最近连接",
  "connect.savedHostsAll": "全部最近主机",
  "connect.showAllHosts": (n: number) => `查看全部 (${n})`,
  "connect.forgetHost": "忘记此主机",
  "connect.savedHasPassword": "已保存密码",
  "connect.rememberPassword": "记住密码",
  "connect.showPassword": "显示密码",
  "connect.hidePassword": "隐藏密码",
  "connect.urlPasswordPlaceholder": "登录密码",
  "connect.mirror": "镜像远端桌面版", // Mirror the remote desktop app
  "connect.mirrorHint":
    "标签、分屏和当前会话与远端机器上的桌面版保持一致，任一边的改动两边同时可见。桌面版未运行时，本次连接直接打开它的数据库；没有数据库则使用独立数据库。", // Same tabs, splits, and active session as the desktop app on the remote machine; changes on either side show on both. If the desktop app is not running, this connection opens its database directly, or a separate database when there is none.
  "connect.shareDesktopDb": "复用远端桌面版的数据库",
  "connect.shareDesktopDbHint":
    "与远端机器的桌面版共用同一数据库（建议两边同版本）。不勾则使用独立数据库。",

  // ── Sidebar (project tree, menus, and dialogs) ──
  "tree.newSession": "新建会话",
  "tree.newTerminalSession": "新建终端",
  "tree.newBrowserPage": "新建浏览器页面",
  "tree.newAgentSession": (agent) => `新建 ${agent} 会话`,
  "tree.newAgentSessionGroup": "更多智能体会话",
  "tree.newAgentSessionCustom": "自定义参数新建…",
  "tree.resumeSession": "恢复会话…",
  "tree.newGroup": "新建分组",
  "tree.newSubgroup": "新建子分组",
  "tree.newChildSession": "新建子会话",
  "tree.openSelected": "打开选中会话",
  "tree.archiveSelected": "归档选中的会话",
  "tree.moveSelected": "移动所选到…", // Move Selected to…
  "tree.deleteSelected": (n) => `删除选中的 ${n} 项`,
  "tree.removeProject": "移除项目",
  "tree.deleteGroup": "删除分组",
  "tree.deleteSession": "删除会话",
  "tree.projectRoot": "项目根（无分组）",
  "tree.moveToSession": "移到会话下（成为子会话）",
  "tree.moveTo": "移动到…",
  "tree.openNewTab": "在新标签打开",
  "tree.forkSession": "Fork 会话",
  "tree.exportSession": "导出会话…",
  "tree.sessionInfo": "会话信息",
  "tree.groupInfo": "分组信息",
  "info.branch": "分支",
  "info.path": "路径",
  "info.recentCommits": "最近提交",
  "info.noCommits": "无提交",
  "tree.killProcess": "结束进程",
  "tree.archiveSession": "归档会话",
  "tree.archiveGroup": "归档分组",
  // Temporary (draft) sessions
  "tree.scratchTag": "临时",
  "tree.persistSession": "转为永久会话…",
  "tree.persistDoc": "保存到磁盘…",
  "tree.closeScratch": "关闭草稿",
  "tree.importProject": "导入项目",
  "tree.createProject": "创建项目",
  // New Collection / Collection name / research / Create Collection / No folder / Delete Collection
  "tree.newCollection": "新建集合",
  "tree.deleteCollection": "删除集合",
  "collection.title": "新建集合",
  "collection.name": "集合名称",
  "collection.namePlaceholder": "research",
  "collection.submit": "创建集合",
  "collection.tag": "无文件夹",
  "collection.deleteTitle": "删除集合",
  "collection.deleteBody": (name) =>
    `删除集合“${name}”？其中的分组和会话也会一并删除，且无法撤销。`,
  "tree.cloneProject": "从 Git 克隆",
  "createProject.title": "创建项目",
  "createProject.name": "项目名称",
  "createProject.namePlaceholder": "我的项目",
  "createProject.into": "创建位置",
  "createProject.choose": "选择…",
  "createProject.noParent": "请选择父目录",
  "createProject.invalidName": "请输入不含 / 或 \\ 的单个目录名称。",
  "createProject.creating": "正在创建…",
  "createProject.submit": "创建项目",
  "clone.title": "克隆 Git 仓库",
  "clone.url": "仓库地址",
  "clone.urlPlaceholder": "https://… 或 git@…",
  "clone.branch": "分支（可选）",
  "clone.branchPlaceholder": "留空则用默认分支",
  "clone.folder": "目录名",
  "clone.folderPlaceholder": "留空则自动取仓库名",
  "clone.into": "克隆到",
  "clone.choose": "选择…",
  "clone.noParent": "请选择一个父目录",
  "clone.cloning": "克隆中…",
  "clone.cancelling": "正在取消…",
  "clone.stageStarting": "正在启动 Git…",
  "clone.stageConnecting": "正在连接仓库…",
  "clone.stagePreparing": "正在准备对象…",
  "clone.stageReceiving": "正在接收对象…",
  "clone.stageResolving": "正在解析增量…",
  "clone.stageCheckout": "正在检出文件…",
  "clone.stageFinalizing": "正在完成克隆…",
  "clone.stageImporting": "正在导入项目…",
  "clone.elapsed": (seconds: number) => `已用时 ${seconds} 秒`,
  "clone.slowHint":
    "已连续 30 秒没有进度，请检查远程机器的网络或代理；你也可以取消后重试。",
  "clone.submit": "克隆",
  "tree.globalSearch": "搜索所有会话",
  "tree.archivedSessions": "已归档会话",
  "tree.searchPlaceholder": "搜索会话 / 分组…",
  "tree.clearSearch": "清空搜索",
  "tree.filterWorking": "工作中",
  "tree.filterAsking": "等待处理",
  "tree.filterWaiting": "已查看",
  "tree.filterStatus": "状态筛选",
  "tree.refreshStatusFilter": "刷新状态筛选",
  "tree.refreshStatusMatch": "刷新状态",
  "tree.filterStatusSection": "状态",
  "tree.filterMarkSection": "标记",
  "tree.viewMainName": "主分身",
  "tree.viewUntitled": "未命名分身",
  "tree.viewDefaultName": (n) => `分身 ${n}`,
  "tree.viewPrimary": "主分身",
  "tree.viewManage": "管理分身",
  "tree.viewSetPrimary": "设为主分身",
  "tree.viewRename": "重命名分身",
  "tree.viewName": "分身名称",
  "tree.viewDelete": "删除分身",
  "tree.viewDeletePrimary": "主分身不能删除",
  "tree.viewDeleteTitle": "删除树分身",
  "tree.viewDeleteConfirm": (name) =>
    `确定删除“${name}”吗？它保存的搜索与筛选条件会被移除，项目和会话不会受影响。`,
  "tree.viewSplitRight": "向右切分树分身",
  "tree.viewSplitDown": "向下切分树分身",
  "tree.viewAdd": "复制当前树分身到新标签页",
  "tree.viewCount": (n) => `${n} 个树分身`,
  "mark.menu": "标记",
  "mark.urgent": "紧急",
  "mark.important": "重要",
  "mark.bug": "缺陷",
  "mark.done": "已完成",
  "mark.wip": "进行中",
  "mark.pinned": "置顶关注",
  "mark.idea": "想法",
  "mark.caution": "注意",
  "tree.clearAllNotifications": "清除全部通知标识（会话小点与 Dock 角标）",
  "tree.noProjectsPre": "还没有项目。点击文件夹图标，或按 ",
  "tree.noProjectsPost": " 导入一个目录开始。",
  "tree.openProject": "打开项目",
  "tree.noAttention": "没有符合状态筛选的会话",
  "tree.noMatch": "无匹配结果",

  // Dialog fields
  "tree.groupName": "分组名称",
  "tree.sessionNameAuto": "会话名称（留空自动命名）",
  "tree.editSession": "编辑会话",
  "tree.sessionName": "会话名称",
  "tree.shellLabel": "Shell（留空用系统默认）",
  "tree.shellMenu": "Shell",
  "tree.downloadFullGitbash": "下载完整 Git Bash",
  "gitbash.title": "Git Bash",
  "gitbash.downloading": "正在下载完整 Git Bash…",
  "gitbash.extracting": "正在解压完整 Git Bash…",
  "gitbash.done": "完整 Git Bash 已就绪。",
  "gitbash.failed": "下载 Git Bash 失败",
  "tree.shellSystemDefault": "系统默认",
  "form.customOption": "自定义…",
  "tree.cwdLabel": "工作目录（留空用项目根）",
  "tree.initCmdLabel": "启动命令（可选）",
  // Run as / Terminal / Conversation
  "tree.engineLabel": "打开方式",
  "tree.engineTui": "终端视图",
  "tree.engineChat": "会话视图",
  // The agent runs its own terminal interface.
  "tree.engineTuiHint": "运行智能体自带的终端界面。",
  // Messages and tool cards, with buttons for permission questions.
  "tree.engineChatHint": "以消息和工具卡片呈现，权限请求可直接在界面中确认。",
  "tree.agentArgsLabel": "启动参数（可选）",
  // Working directory / Leave empty for the default
  "tree.workingDirLabel": "工作目录",
  "tree.workingDirPlaceholder": "留空则用默认目录",
  "preset.execPathLabel": "可执行文件（可选）",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint":
    "留空则用该智能体已配置的命令。填了就只有这个会话用它，可以跑兼容的替代程序。",
  "preset.saveLabel": "存为预设",
  "preset.namePlaceholder": "给这个预设起个名",
  "preset.iconChoose": "选择图标",
  "preset.iconClear": "移除",
  "preset.iconHint": "方形图片效果最好，其他图片会裁剪并缩放到 64x64。",
  "tree.permissionSkipLabel": "跳过全部权限确认",
  "tree.permissionSkipHint":
    "启动时带上该 agent 的「跳过确认」flag（如 Claude 的 --dangerously-skip-permissions；Codex 还会一并关闭沙箱）。每次启动都生效，请谨慎使用。",
  "tree.permissionUnsupported":
    "OpenCode 经配置文件控制权限、没有对应的启动参数，此选项不适用。",
  "tree.permissionUnsupportedPi":
    "Pi 刻意不设权限确认弹窗（工具直接执行），此选项不适用。",

  // New agent-session dialog
  "newAgent.desc":
    "可选填会话名和自定义启动参数（传给 agent 命令，如 --model opus）。两个都留空直接回车即可照常启动。",

  // Delete confirmation
  "tree.batchDeleteTitle": "批量删除",
  "tree.deleteProjectTitle": "删除项目",
  "tree.deleteGroupTitle": "删除分组",
  "tree.deleteSessionTitle": "删除会话",
  "tree.batchDeleteBody": (n) =>
    `确认删除选中的 ${n} 项（项目/分组会级联删除其下的子分组与会话）。此操作不可撤销。`,
  "tree.deleteProjectBody": (name) =>
    `确认删除项目「${name}」，其下所有子分组与会话也会一并删除。此操作不可撤销。`,
  "tree.deleteGroupBody": (name) =>
    `确认删除分组「${name}」，其下所有子分组与会话也会一并删除。此操作不可撤销。`,
  "tree.deleteSessionBody": (name) =>
    `确认删除会话「${name}」（及其下所有子会话）。此操作不可撤销。`,
  "tree.deleteWorktrees": (n) =>
    `同时删除关联的 git worktree（共 ${n} 个；工作区有改动可能删除失败）`,

  // Session information dialog
  "info.name": "名称",
  "info.type": "类型",
  "info.status": "状态",
  "info.notYetCaptured": "尚未生成（首次运行后捕获）",
  "info.sessionId": "会话 ID",
  "info.cwd": "工作目录",
  "info.initCmd": "启动命令",
  "info.agentArgs": "启动参数",
  "info.launchCmd": "完整启动命令",
  "info.permission": "权限",
  "info.permissionSkip": "跳过全部确认",
  "info.parentSessionId": "父会话 ID",
  "info.termTitle": "终端标题",
  "info.createdAt": "创建时间",

  // Resume-session dialog
  "importSessions.results": ({ count }: { count: number }) => `${count} 条结果`,
  "importSessions.selected": ({ count }: { count: number }) => `已选 ${count} 条`,
  "importSessions.clearSelection": "清空选择",
  "importSessions.clearSearch": "清除搜索",
  "importSessions.noHistory": "此项目目录下没有可导入的历史会话。",
  "importSessions.title": "导入会话",
  "importSessions.description": "查找工作目录与本项目一致的 Codex、Claude 和 OpenCode 历史会话。选择会话并将其加入项目后，即可打开并继续对话。",
  "importSessions.search": "搜索标题、Agent 名称或会话 ID",
  "importSessions.empty": "未找到匹配的会话。",
  "importSessions.imported": "已导入",
  "importSessions.confirm": ({ count }: { count: number }) => `导入（${count}）`,
  "importSessions.success": ({ count }: { count: number }) => `已将 ${count} 个会话加入项目。`,
  "resume.title": "恢复会话",
  "resume.desc":
    "选 agent 类型并填入该 agent 自身的 session id，打开后续接原对话。",
  "resume.agentType": "Agent 类型",
  "resume.sessionIdPlaceholder": "对话 session id",
  "resume.confirm": "恢复并打开",

  // New worktree-session dialog
  "tree.newWorktreeSession": "新建 worktree 会话…",
  "worktree.worktreeNameLabel": "worktree 名称",
  "worktree.worktreeNameHint": "用作 worktree 目录名与分支名。",
  "worktree.createFailed": "创建 worktree 失败",
  "worktree.noRepoRoot": "该项目没有可用的 git 仓库路径。",
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "不挂",
  "worktreeSel.modeNew": "新建",
  "worktreeSel.modeExisting": "选已有",
  "worktreeSel.loading": "正在加载 worktree…",
  "worktreeSel.empty": "该仓库没有已存在的 worktree。",
  "worktreeSel.loadFailed": "无法列出 worktree（不是 git 仓库？）。",
  "group.worktreeHint": "在该分组下新建的会话将默认使用此 worktree。",
  "worktree.moveGroupTitle": "把分组转移到 Worktree",
  "worktree.moveGroupHint":
    "之后在这个分组里新建的会话会用这个 worktree；已有的会话还是留在原来的目录。",

  // ── Archive panel ──
  "archive.title": "已归档会话",
  "archive.empty1": "暂无归档会话。",
  "archive.empty2": "在左栏会话上右键「归档会话」即可把它收进这里。",
  "archive.restore": "恢复为正常会话",
  "archive.export": "导出完整上下文为 Markdown",
  "archive.deleteForever": "彻底删除（连带录制）",
  "archive.pickOne": "选择左侧一个归档会话查看其对话记录",
  "archive.recordingEnd": "--- 录制结束 ---",
  "archive.readRecordingFailed": (err) => `读取录制失败: ${err}`,
  "archive.searchRecording": "在录制中搜索…",
  "archive.searchTranscript": "搜索对话内容…",
  "archive.searchPlaceholder": "搜索归档内容…",
  "archive.msgCountAll": (n) => `${n} 条`,
  "archive.msgCountFiltered": (shown, total) => `${shown} / ${total} 条`,
  "archive.you": "你",
  "archive.toolsUsed": (tools) => `工具：${tools}`,
  "archive.noMatch": "没有匹配的消息",
  "archive.emptyTranscript": "对话记录为空",
  "archive.loadingTranscript": "加载对话记录…",

  // ── Global session-content search ──
  "search.allPlaceholder": "搜索所有会话内容…",
  "search.hint":
    "搜索会话内容。默认不含已归档会话，勾选「同时搜索归档」可纳入。",
  "search.includeArchived": "同时搜索归档",
  "search.includeArchivedHint": "把已归档会话也纳入搜索（默认不搜）",
  "search.searching": "搜索中…",
  "search.noResults": "未找到匹配",
  "search.sessionCount": (n) => `${n} 个会话`,
  "search.matchCount": (n) => `${n} 处命中`,
  "search.pickSession": "在左侧选择一个会话以查看命中片段",
  "search.openSession": "打开会话",
  "search.backToResults": "返回结果",
  "search.archivedBadge": "已归档",
  "search.summary": (m, s) => `命中 ${m} 处 · ${s} 个会话`,
  "search.matchPosition": (n, total) => `第 ${n} / 共 ${total}`,
  "search.roleTerminal": "终端",
  "search.collapseGroup": "折叠",
  "search.expandGroup": "展开",
  "search.cappedNote": (l, total) => `可定位 ${l} / 共命中 ${total}`,

  // ── Center pane (tabs, empty state, and background keep-alive) ──
  "center.noSession": "暂无会话",
  "center.noSessionHintPre": "从左栏选择会话，或按 ",
  "center.noSessionHintPost": " 新建终端",
  "center.createTerminal": "新建终端",
  "tab.unsavedDot": "有未保存修改",
  "tab.newTerminal": "新建终端",
  "tab.newDocument": "新建文档",
  "tab.bgTitle": (n) => `后台保活标签：${n} 个（进程仍在运行）`,
  "tab.bgLabel": (n) => `后台 ${n}`,
  "tab.scratchFallback": "（临时终端）",
  "tab.killBgTab": "结束该后台标签（进程随之结束）",
  "tab.newBrowserTab": "新标签页",
  "tab.refreshFile": "刷新文件",
  "tab.closeOthers": "关闭其他标签",
  "tab.closeRight": "关闭右侧标签",
  "tab.closeAll": "关闭所有标签",
  "tab.sendToBackground": "转入后台保活", // Send to Background

  // ── Built-in browser ──
  "browser.back": "后退",
  "browser.forward": "前进",
  "browser.reload": "刷新",
  "browser.desktopOnly": "浏览器标签只能在桌面端打开。", // Browser tabs open in the desktop app only.
  "browser.stop": "停止加载",
  "browser.openExternal": "在系统浏览器中打开",
  "browser.addressPlaceholder": "输入网址或搜索词",
  "browser.quickAccess": "快速访问",
  "browser.loading": "加载中…",
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "退出 VelaTerm？", // Quit VelaTerm?
  "quit.body": "正在运行的终端会话与智能体会话都会被停止。", // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "保存工作空间", // Save workspace
  "quit.saveWorkspaceHint":
    "下次打开时恢复相同的标签页和分屏。终端会恢复出来，但不会自动重启。", // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "退出", // Quit
  "dormant.body": "已从保存的工作空间恢复，进程还没有启动。", // Restored from your saved workspace. No process is running yet.
  "dormant.start": "启动", // Start
  "overlimit.title": (max) => `后台保活已超上限（${max} 个）`,
  "overlimit.body": "所有后台标签都在工作或等你回复，请选择要结束的标签：",
  "overlimit.kill": "结束选中",
  "overlimit.keep": "暂不结束",
  "overlimit.earliest": "最早",
  "overlimit.statusWorking": "工作中",
  "overlimit.statusAsking": "待回复",
  "overlimit.statusWaiting": "等待中",

  // ── Terminal pane, context menu, and search ──
  "term.paste": "粘贴",
  "term.pasteUseShortcut": "粘贴（请按 ⌘V）",
  "term.selectAll": "全选",
  "term.autoCopied": (n: number) => `已自动复制 ${n} 字符 · ⌘V 粘贴`,
  "term.clear": "清屏",
  "term.searchMenu": "搜索…",
  "term.splitRight": "右分屏",
  "term.splitDown": "下分屏",
  "term.closePane": "关闭分屏",
  "term.redraw": "重绘",
  "term.mirrorTooltip":
    "当前为镜像显示（尺寸由其它端主控）。点击把 PTY 尺寸调整为本窗口大小",
  "term.mirrorBadge": (dims) => `⤢ 镜像${dims} · 点击适配本窗口`,
  "term.mirrorBadgeMobile": (dims) => `⤢ 镜像${dims} · 适配本窗口`,
  "term.imgUploadFailed": (n, lastError) =>
    `图片上传失败 ${n} 张${lastError ? `：${lastError}` : ""}`,
  "term.imgClipboardUnavailable":
    "无法从剪贴板读取图片，请重新复制图片后再试。",
  "term.starting": (agent) => `正在启动 ${agent}…`,
  "term.startFailed": (err) => `启动失败: ${err}`,

  // ── Agent installation guidance ──
  "agentInstall.title": (label) => `${label} 未安装`,
  "agentInstall.desc": (label) =>
    `VelaTerm 没有在 PATH 上找到 ${label}。安装后即可启动该会话。`,
  "agentInstall.install": "一键安装",
  "agentInstall.retry": "重试启动",
  "agentInstall.dismiss": "我自己装",
  "agentInstall.docs": "安装文档",
  "agentInstall.needsNode": "需先安装 Node.js / npm",
  "agentInstall.afterInstall": "安装后：",
  "agentInstall.pathSaved": (label: string) =>
    `已把 ${label} 的可执行文件路径填入设置：`,
  "agentInstall.doneTitle": (label: string) => `${label} 已安装`,
  "agentInstall.doneDesc": "重启本会话即可开始使用。",
  "agentInstall.restartNow": "立即重启",
  "agentInstall.later": "稍后",
  "search.placeholder": "在终端中搜索",

  // ── Document tabs ──
  "doc.wysiwyg": "所见即所得",
  "doc.source": "源码",
  "doc.searchPlaceholder": "查找",
  "doc.searchReplacePlaceholder": "替换",
  "doc.searchReplace": "替换",
  "doc.searchReplaceAll": "全部",
  "doc.searchNoMatch": "无匹配",
  "doc.searchCaseSensitive": "区分大小写",
  "doc.searchToggleReplace": "切换替换",
  "doc.fileTree": "目录树",
  "doc.treeUp": "上级目录",
  "doc.sidebar": "侧栏",
  "doc.unsaved": "未保存",
  "doc.saveAsTitle": "另存为",
  "doc.saveAsName": "文件名",
  "doc.outline": "大纲",
  "doc.outlineEmpty": "没有标题",
  "doc.saving": "保存中…",
  "doc.overwriteConfirm": "已存在同名文件，点「覆盖」替换原文件。",
  "doc.saveTooltip": "保存",
  "doc.externalChanged": "文件已在磁盘上被修改（你有未保存的本地修改）。",
  "doc.reloadDiscard": "重新加载（丢弃我的修改）",
  "doc.externalChangedClean": "文件已在磁盘上被修改。",
  "doc.reload": "重新加载",
  "doc.ignore": "忽略",
  "doc.loadingFile": (title) => `正在加载 ${title}…`,
  "doc.closeTitle": "关闭文档",
  "doc.unsavedBody": (title) => `「${title}」有未保存的修改。`,
  "doc.saveAndClose": "保存并关闭",
  "doc.closeNoSave": "不保存关闭",
  "doc.conflictTitle": "保存冲突",
  "doc.conflictBody": "磁盘上的文件已被外部修改，仍要用当前内容覆盖吗？",
  "doc.overwrite": "覆盖",
  "doc.saveFailed": (err) => `保存失败：${err}`,
  "doc.closeTab": "关闭标签",
  "doc.truncatedReadonly": (size: string) =>
    `只读：仅显示前 10 MB（共 ${size}）。已禁用保存，以免覆盖文件其余部分。`,
  "doc.imgLoading": (title, size) => `正在加载 ${title}（${size}）…`,
  "doc.imgBeingWritten": "文件正在被写入，待写入稳定后将自动重新加载。",
  "doc.imgDecodeFailed": "无法显示该图片（格式不支持或文件已损坏）。",
  "doc.imgFit": "适应窗口",
  "doc.imgActual": "1:1",
  "doc.exportPdf": "导出 PDF",
  "doc.diagramError": "图表语法错误",

  // ── Right information panel ──
  "panel.noSession": "未选择会话",
  "panel.openInEditor": "在编辑器中打开",
  "panel.openInEditorTooltip": "在中栏文档编辑器中打开（同 view 命令）",
  "panel.preview": "预览", // Preview
  "panel.cantRead": "（无法读取该文件）",
  "panel.binary": "（二进制文件，不预览）",
  "panel.truncated": "\n…（内容过长已截断）",
  "panel.showHidden": "显示隐藏文件", // Show hidden files
  "panel.hideHidden": "不显示隐藏文件", // Hide hidden files

  // ── File-tree actions (Files context menu and header add button) ──
  "files.newFile": "新建文件", // New File
  "files.newFolder": "新建文件夹", // New Folder
  "files.nameLabel": "名称", // Name
  "files.newTooltip": "新建文件或文件夹", // New file or folder
  "files.openInTerminal": "在终端中打开", // Open in Terminal
  "files.revealInFinder": "在文件管理器中显示", // Show in File Manager
  "files.copyPath": "复制路径", // Copy Path
  "files.copyRelPath": "复制相对路径", // Copy Relative Path
  "files.filterPlaceholder": "过滤文件…", // Filter files
  "files.dblClickOpen": "双击打开", // Double-click to open
  "files.deleteConfirm": (name) => `确定删除“${name}”？此操作不可撤销。`, // Delete "{name}"? This can't be undone.

  // ── File transfer (remote access) ──
  "transfer.uploadsTitle": "上传", // Uploads
  "transfer.download": "下载", // Download
  "transfer.upload": "上传文件…", // Upload Files…
  "transfer.uploadTooltip": "把文件上传到这个目录", // Upload files to this folder
  "transfer.clear": "清空", // Clear
  "transfer.cancelled": "已取消", // Cancelled
  "transfer.failed": "失败", // Failed
  "transfer.stalled": "正在重连…", // Reconnecting…
  "transfer.foldersUnsupported": "文件夹传不了。", // Folders can't be uploaded.

  // ── Status bar ──
  "statusbar.sessions": (n) => `${n} 会话`,
  "statusbar.filterTooltip": (label) =>
    `点击在左栏只看「${label}」会话（再点取消）`,
  "statusbar.bgCount": (n, max) => `后台 ${n}/${max}`,
  "statusbar.bgTooltip": (max) =>
    `后台保活的标签数（上限 ${max}，超限时自动结束最早的不活跃标签）`,
  "statusbar.bgEvicted": (name) => `已结束后台标签：${name}（超出保活上限）`,
  "statusbar.webTooltip": (url) => `浏览器远程访问已开启：${url}`,
  "statusbar.permAsk": "权限：询问",
  "statusbar.permSkip": "权限：跳过",
  "statusbar.notifyOn": "通知：开",
  "statusbar.notifyOff": "通知：关",
  "statusbar.permTooltip": "本会话权限模式 · 点击切换（仅影响本会话）",
  "statusbar.permMenuTitle": "本会话权限",
  "statusbar.permOptAsk": "逐步询问（默认）",
  "statusbar.permScopeHint":
    "仅对当前会话生效。全局性设置，请前往「设置 ▸ 智能体」中调整。",
  "statusbar.permRestartMsg":
    "权限已更改，需重启本会话才生效。重启会续接当前对话，但会打断正在进行的任务。现在重启？",
  "statusbar.permRestartNow": "立即重启",
  "statusbar.permRestartLater": "稍后",
  "statusbar.permScopeTitle": "应用到？",
  "statusbar.permScopeSession": "仅当前会话",
  "statusbar.permScopeGlobal": "全局默认",
  "statusbar.permScopeGlobalHint":
    "本会话立即采用，并设为以后新建同类会话的默认（与设置同步）。",

  // ── Store, notifications, and export ──
  "notify.working": "⏳ 处理中…",
  "notify.asking": "❓ 需要你确认",
  "notify.waiting": "✅ 已回复",
  "store.subtask": "子任务",
  "store.splitPane": "分屏",
  "export.failedTitle": "导出会话失败",
  "export.contextSuffix": "上下文",

  // ── Error panel ──
  "err.renderTitle": "界面渲染出错",
  "err.renderDesc": "遇到了未预期的错误，以下信息可帮助定位问题。",
  "err.reload": "重新加载",
  "err.uncaughtTitle": "发生未捕获的错误",
  "err.uncaughtDesc": "以下信息可帮助定位问题。",

  // ── transport ──
  "transport.noReplayInBrowser": "浏览器端暂不支持归档回放",
  "transport.imgUploadHttp": (status) => `图片上传失败 (${status})`,

  // ── Login gate, directory selection, and connection banner ──
  "login.connecting": "连接中…",
  "login.remoteAccess": "远程访问",
  "login.desc": "输入访问密码以连接到该终端。",
  "login.passwordPlaceholder": "访问密码",
  "login.connect": "连接",
  "login.wrongPassword": "密码错误",
  "login.rateLimited": "尝试次数过多，请稍等一分钟后重试。", // Too many attempts. Please wait a minute and try again.
  "login.failed": "登录失败，请重试",
  "login.pairingRequired":
    "此服务要求使用配对链接访问。请用桌面端「远程访问」生成的配对链接打开。",
  "login.authFailed":
    "认证失败。请确认访问密码；如果配对链接已重新生成，请改用新链接。",
  "dir.title": "选择项目目录",
  "dir.pathPlaceholder": "搜索，或输入路径后回车跳转（支持 ~ 开头）",
  "dir.up": "上一级",
  "dir.newFolder": "新建文件夹",
  "dir.newFolderPlaceholder": "文件夹名称",
  "dir.goInput": "前往输入路径",
  "dir.noSubdirs": "（无子目录）",
  "dir.empty": "（空目录）",
  "dir.noMatch": "没有匹配项",
  "dir.target": "目标目录",
  "dir.showHidden": "显示隐藏项",
  "dir.importing": "导入中…",
  "dir.choose": "选择此目录",
  "conn.reconnecting": "连接已断开，正在尝试重连…",
  "conn.reconnectNow": "立即重连",
  "conn.retrying": "正在重连…",
  "conn.sshReconnecting": "SSH 链路已断，正在重建隧道…",
  "conn.sshDown": "SSH 链路已断开，点击「立即重连」再试一次",
  "reqerr.title": "请求失败",
  "reqerr.dismiss": "关闭",
  // ── Error log panel (hidden debug entry) ──
  "errlog.title": "错误日志",
  "errlog.empty": "暂无记录的错误。",
  "errlog.copyAll": "全部复制",
  "errlog.clear": "清空",
  "errlog.close": "关闭",

  // ── Mobile ──
  "mobile.toDesktop": "切换到桌面版",
  "mobile.empty1": "暂无会话。",
  "mobile.noMatch": "没有匹配的会话",
  "mobile.empty2": "在桌面端或电脑浏览器端创建后，这里会自动出现。",
  "mobile.back": "‹ 返回",
  "mobile.selCopy": "复制",
  "mobile.selCancel": "取消",

  // ── Other shared components ──
  "splitter.dragToResize": "拖拽调整大小",
  "transport.wsDisconnected": "WebSocket 已断开",
  "transport.wsConnectFailed": "WebSocket 连接失败",
  "transport.cmdFailed": "命令失败",
  "transport.remoteCmdForbidden": (cmd: string) =>
    `远程客户端不可使用该命令：${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) =>
    `远程客户端不可写入该设置项：${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) =>
    `远程客户端无法访问应用数据目录中的文件：${path}`, // Remote clients cannot access files in the app data directory

  // ── Crepe (built-in WYSIWYG editor UI) ──
  "crepe.placeholder": "输入正文，或键入 / 唤起插入菜单",
  "crepe.textGroup": "文本",
  "crepe.paragraph": "正文",
  "crepe.h1": "一级标题",
  "crepe.h2": "二级标题",
  "crepe.h3": "三级标题",
  "crepe.h4": "四级标题",
  "crepe.h5": "五级标题",
  "crepe.h6": "六级标题",
  "crepe.quote": "引用",
  "crepe.divider": "分割线",
  "crepe.listGroup": "列表",
  "crepe.bulletList": "无序列表",
  "crepe.orderedList": "有序列表",
  "crepe.taskList": "任务列表",
  "crepe.advancedGroup": "插入",
  "crepe.image": "图片",
  "crepe.codeBlock": "代码块",
  "crepe.table": "表格",
  "crepe.math": "公式",
  "crepe.linkPlaceholder": "粘贴或输入链接…",
  "crepe.upload": "上传",
  "crepe.uploadImage": "上传图片",
  "crepe.orPasteImageLink": "或粘贴图片链接",
  "crepe.imageCaption": "图片说明",
  "crepe.confirm": "确认",
  "crepe.searchLanguage": "搜索语言",
  "crepe.noResult": "无匹配结果",
  "crepe.edit": "编辑",
  "crepe.collapse": "收起",
  // ── Additional right and bottom bar entries ──
  "info.project": "项目",
  "panel.sessionInfo": "会话信息",
  "panel.gitTitle": "Git 状态",
  "panel.gitProbing": "探测中…",
  "panel.gitNotRepo": "非 Git 仓库",
  "panel.gitBranch": "分支",
  "panel.gitStaged": "暂存",
  "panel.gitUnstaged": "改动",
  "panel.gitUntracked": "未跟踪",
  "bottombar.running": "运行中",
  "bottombar.collapseTasks": "收起任务区",
  "bottombar.expandTasks": "展开任务区",
  "bottombar.sound": "🔔 提示音",
  "bottombar.muted": "🔕 静音",
  "bottombar.overview": "会话概览",
  "bottombar.noSessions": "暂无会话",
  "doc.pdfFilter": "PDF 文件",
  // ── Automatic updates ──
  "updater.title": "检查更新",
  "updater.upToDate": "当前已是最新版本。",
  "updater.failed": (err) => `检查更新失败：${err}`,
  "updater.available": "发现新版本",
  "updater.versionLine": (version, current) =>
    `版本 ${version} — 当前 ${current}`,
  "updater.noNotes": "该版本没有提供更新说明。",
  "updater.updateNow": "立即更新",
  "updater.later": "稍后",
  "updater.skipVersion": "跳过此版本",
  "updater.skipVersionHint":
    "不再提示这个版本。之后仍可从「检查更新」手动安装。",
  "updater.downloadingPct": (pct) => `正在下载… ${pct}%`,
  "updater.downloadingBytes": (mb) => `正在下载… ${mb} MB`,
  "updater.installing": "正在安装…",
  "updater.installed": "更新已安装，重启后生效。",
  "updater.restartNow": "立即重启",
  "updater.retry": "重试",
  "updater.downloadFailed": (err) => `更新失败：${err}`,
  "updater.hide": "隐藏",
  "updater.hideHint": "在后台继续下载，进度会留在状态栏。",
  "updater.downloadManually": "手动下载",
  "updater.downloadManuallyHint": "在浏览器中打开下载页面。",
  "updater.windowsNotice": "安装期间 VelaTerm 会关闭，安装完成后自动重新打开。",
  "updater.installingWindows":
    "正在安装… VelaTerm 即将关闭，安装程序会完成更新并重新打开它。",
  // The status-bar new-version segment belongs to automatic updates and stays here for centralized editing.
  "statusbar.updateAvailable": (version) => `更新 ${version}`,
  "statusbar.updateDownloading": (pct) => `正在更新… ${pct}%`,
  "statusbar.updateInstalling": "正在安装…",
  "statusbar.updateReady": "重启以完成更新",
  "statusbar.updateFailed": "更新失败",
  "statusbar.updateTooltip": "点击查看详情",

  // ── 会话视图（把智能体会话读成对话） ──
  "session.showConversation": "会话视图",
  "session.showTerminal": "终端视图",
  "session.switchTitle": "切换视图将重新启动智能体",
  "session.switchBody": "当前进行中的回合会中断，对话内容不会丢失。",
  "session.switchConfirm": "切换",
  "session.loading": "正在读取对话…",
  "session.unavailable": "暂时无法读取该会话的对话内容",
  "session.working": "处理中…",
  "session.thinking": "思考过程",
  "session.toolRunning": "进行中",
  "session.toolUnknown": "工具",
  "session.toolFailed": "执行失败",
  "session.toolNoDetail": "没有更多记录",
  "session.showMore": (n: number) => `显示其余 ${n} 个字符`,
  "session.showLess": "收起",
  "session.composerHint": "给智能体发消息 · 回车发送，Shift+回车换行",
  "session.send": "发送",

  // ── 对话引擎（以协议方式驱动的会话） ──
  "chat.empty": "在下方输入内容，开始对话。",
  "chat.interrupt": "停止",
  "chat.interruptTooltip": "停止 · Esc",
  "chat.allow": "允许",
  "chat.deny": "拒绝",
  "chat.permissionAsk": (tool: string) => `${tool} 请求执行`,
  "chat.exited": (code: number) => `智能体已退出（代码 ${code}）`,
  "chat.modeNextTurn": "下一轮生效",
  "chat.modePendingHint": (current: string, next: string) =>
    `当前权限：${current}；下一轮权限：${next}。当前回合继续使用原权限。`,
  "chat.modeTooltip": "权限模式",
  "chat.collaborationModeTooltip": "协作模式",
  "chat.collaborationMode.default": "默认模式",
  "chat.collaborationMode.defaultHint": "直接推进，仅在需要你决定时提问",
  "chat.collaborationMode.plan": "计划模式",
  "chat.collaborationMode.planHint": "先调查并制定计划，可用交互卡片提问",
  "chat.modelTooltip": "模型",
  "chat.keepChoice": "设为默认",
  "chat.keepChoiceFor": (model) => `设为 ${model} 的默认`,
  "chat.modelDefault": "默认模型",
  "chat.mode.default": "每次询问",
  "chat.mode.acceptEdits": "自动接受改动",
  "chat.mode.plan": "计划模式",
  "chat.mode.bypassPermissions": "全部放行",
  "chat.mode.readOnly": "只读",
  "chat.mode.fullAccess": "完全访问",
  "chat.placeholder": "给智能体发消息，可用 /命令、/技能 与 @文件",
  "chat.command.clearDescription": "归档当前会话并开始一个全新对话",
  "chat.command.rewindDescription": "从最近一条用户消息选择要回退的内容",
  "chat.command.rewindUnavailable":
    "需要有已完成的用户消息，且当前没有正在处理的回合、排队消息或权限请求。",
  "chat.effortTooltip": "思考程度",
  "chat.effortDefault": "思考",
  "chat.effort.auto": "自动",
  "chat.effort.low": "低",
  "chat.effort.medium": "中",
  "chat.effort.high": "高",
  "chat.effort.xhigh": "很高",
  "chat.effort.max": "最高",
  "chat.effort.ultra": "极致",
  "chat.effort.ultracode": "Ultra Code",
  "chat.agentTooltip": "智能体",
  "chat.effort.minimal": "最低",
  "chat.filterPlaceholder": "筛选",
  "chat.placeholderOpencode": "给智能体发消息，可用 /命令 与 @文件；以 ! 开头可执行 Shell 命令",
  "chat.command.compactDescription": "总结对话内容，释放上下文空间",
  "chat.command.undoDescription": "撤销最后一条消息及其造成的文件改动",
  "chat.command.redoDescription": "恢复上一次撤销的内容",
  "chat.command.shareDescription": "生成分享链接",
  "chat.command.unshareDescription": "取消分享",
  "chat.mode.auto": "自动判断",

  // ── 智能体提出的问题，用表单作答 ──
  "chat.question.heading": "智能体提出了一个问题",
  "chat.question.submit": "提交",
  "chat.question.next": "下一题",
  "chat.question.dismiss": "关闭",
  "chat.question.answerPlaceholder": "输入你的回答",
  "chat.question.otherPlaceholder": "其他回答",
  "chat.question.answeredHeading": (n: number) => `已回答 ${n} 个问题`,
  "chat.question.blankAnswer": "未填写",

  // ── 等待批准的计划 ──
  "chat.plan.heading": "计划已就绪，等待批准",
  "chat.plan.implement": "批准并执行",
  "chat.plan.reject": "拒绝",

  // ── 智能体忙碌时排队的消息 ──
  "chat.placeholderBusy": "输入消息，本轮结束后将自动发送",
  "chat.queueTooltip": (combo: string) => `本轮结束后发送 · ${combo} 立即发送`,
  "chat.queue.pending": "待发送",
  "chat.queue.edit": "编辑",
  "chat.queue.remove": "删除",

  // ── 粘贴或拖进输入框的图片 ──
  "chat.attach.remove": "移除这张图片",
  "chat.attach.tooMany": (max: number) => `一条消息最多可附带 ${max} 张图片`,
  "chat.attach.tooLarge": (name: string, mb: number) => `${name} 超过 ${mb} MB，未添加为附件`,
  "chat.attach.unreadable": (name: string) => `无法读取 ${name}`,
  // Compacting the conversation… / Context compacted / Context compacted automatically
  "chat.compaction.running": "正在压缩上下文…",
  "chat.compaction.manual": "上下文已压缩",
  "chat.compaction.auto": "上下文已自动压缩",
  "chat.compaction.from": (tokens: string) => `压缩前 ${tokens} tokens`,
  // N steps
  "chat.subagent.steps": (n: number) => `${n} 步`,
  "chat.subagent.tokens": (tokens: string) => `${tokens} 个 token`,
  "chat.rewind.title": "从这里回退",
  "chat.rewind.warning": "此操作无法撤销。",
  "chat.rewind.conversation": "回退对话",
  "chat.rewind.files": "恢复文件",
  "chat.rewind.both": "回退对话并恢复文件",
  "chat.rewind.confirm.conversation": "删除这条消息及其之后的全部内容？",
  "chat.rewind.confirm.files": "将文件恢复到这条消息之前的状态？",
  "chat.rewind.confirm.both": "删除这一回合，并恢复它修改过的文件？",
  "chat.rewind.unavailable": "这条消息没有对应的文件检查点。",
  "chat.rewind.previewing": "正在检查文件恢复点…",
  "chat.rewind.cancel": "保持现状",
  "chat.rewind.apply": "回退",
  "chat.rewind.applying": "正在回退…",
  "chat.rewind.fileSummary": (files: number, insertions: number, deletions: number) =>
    `将改动 ${files} 个文件：+${insertions} −${deletions}。此操作无法撤销。`,
  // ── 权限卡上的「以后不用再问」按钮，点一下就采纳 ──
  "chat.suggest.modeSession": (mode: string) => `本会话改为${mode}`,
  "chat.suggest.mode": (mode: string) => `改为${mode}`,
  "chat.suggest.allowSession": (rule: string) => `本会话允许 ${rule}`,
  "chat.suggest.allowAlways": (rule: string) => `始终允许 ${rule}`,
  "chat.suggest.dirSession": (dirs: string) => `本会话允许访问 ${dirs}`,
  "chat.suggest.dirAlways": (dirs: string) => `始终允许访问 ${dirs}`,
  // ── Codex：网络放行规则、插话、自带命令，以及速度与语气控件 ──
  "chat.suggest.networkAlways": (host: string) => `始终允许访问网络主机 ${host}`,
  "chat.steer": "插话",
  "chat.stopping": "正在停止当前回合…",
  "chat.stopped": "当前回合已停止",
  "chat.steerAccepted": "插话已发送",
  "chat.steerTooltip": (combo: string) => `${combo} 追加到当前回合`,
  "chat.command.reviewDescription": "审查代码并指出需要处理的问题",
  "chat.command.reviewHint": "[branch <分支名> | commit <提交号> | 说明]",
  "chat.command.startTimeout": "智能体未能及时打开会话",
  "chat.serviceTierTooltip": "速度",
  "chat.serviceTier.default": "标准速度",
  "chat.personalityTooltip": "语气",
  "chat.personality.default": "默认语气",
  "chat.personality.none": "中性",
  "chat.personality.friendly": "友好",
  "chat.personality.pragmatic": "务实",
  // ── 长对话：连续的工具调用折成一行，以及回到末尾的入口 ──
  "chat.toolRun.count": (n: number) => `${n} 个工具调用`,
  "chat.toolRun.tooltip": "逐个查看",
  "chat.backToEnd": "回到最新消息",
  "chat.elicitation.heading": (server: string) => `${server} 请求输入`,
  "chat.elicitation.cancel": "取消",
  "chat.elicitation.decline": "拒绝",
  "chat.elicitation.submit": "提交",
  "chat.elicitation.done": "完成",
  "chat.elicitation.choose": "请选择…",
  "chat.effort.off": "关闭",
  "chat.effort.offHint": "不进行扩展思考",
  "chat.fastMode.label": "快速",
  "chat.fastMode.on": "快速模式已开启",
  "chat.fastMode.off": "快速模式已关闭",
  "chat.usage.context": (used: string, max: string, pct: number) =>
    `上下文：已用 ${used}，上限 ${max} token（${pct}%）`,
  "chat.usage.cost": (usd: string) => `本次会话费用：$${usd}`,
  "chat.usage.rateLimited": (resets: string) => `已达到用量上限，${resets} 重置`,
  "chat.usage.rateWarning": (pct: number, resets: string) => `用量上限：已用 ${pct}%，${resets} 重置`,
  "chat.mcp.codexScope": "此操作将修改 Codex 用户配置，影响使用该配置的其他会话。是否继续？",
  "chat.mcp.tooltip": "MCP 服务器",
  "chat.mcp.loading": "正在读取服务器列表…",
  "chat.mcp.backendUnsupported": "当前连接的 VelaTerm 后端不支持 MCP 管理。请更新并重启该后端，然后重试。",
  "chat.mcp.none": "未配置 MCP 服务器",
  "chat.mcp.tools": (n: number) => `${n} 个工具`,
  "chat.mcp.reconnect": "重新连接",
  "chat.mcp.disable": "禁用",
  "chat.mcp.enable": "启用",
  "chat.mcp.status.connected": "已连接",
  "chat.mcp.status.disabled": "已禁用",
  "chat.mcp.status.failed": "连接失败",
  "chat.mcp.status.pending": "连接中",
  "chat.mcp.status.disconnected": "已断开",
  "chat.mcp.status.other": "未知",
  "chat.tasks.label": "任务",
  "chat.tasks.tooltip": "后台任务",
  "chat.tasks.backgroundAll": "将正在运行的工作转入后台",
  "chat.tasks.none": "没有后台任务",
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
    `${seconds} 秒后重试（${attempt}/${max}）：${message}`,
  "chat.notify.dismiss": "关闭",
};

export default zhCN;
