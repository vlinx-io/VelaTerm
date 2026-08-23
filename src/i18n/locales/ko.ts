//! Korean dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const ko: typeof en = {
  // ── Common ──
  "common.cancel": "취소", // Cancel
  "common.confirm": "확인", // OK
  "common.delete": "삭제", // Delete
  "common.save": "저장", // Save
  "common.create": "생성", // Create
  "common.close": "닫기", // Close
  "common.copy": "복사", // Copy
  "common.cut": "잘라내기", // Cut
  "common.paste": "붙여넣기", // Paste
  "common.selectAll": "모두 선택", // Select All
  "common.copied": "복사됨", // Copied
  "common.retry": "다시 시도", // Retry
  "common.refresh": "새로 고침", // Refresh
  "common.loading": "불러오는 중…", // Loading…
  "common.prev": "이전", // Previous
  "common.next": "다음", // Next
  "common.on": "켬", // On
  "common.off": "끔", // Off
  "common.gotIt": "확인", // Got it
  "common.rename": "이름 바꾸기", // Rename
  "common.edit": "편집", // Edit
  "common.open": "열기", // Open
  "common.session": "세션", // Session

  // ── Session types and status ──
  "kind.terminal": "터미널", // Terminal
  "kind.browser": "브라우저", // Browser
  "status.idle": "대기", // Idle
  "status.running": "실행 중", // Running
  "status.exited": "종료됨", // Exited
  "status.error": "오류", // Error
  "status.working": "작업 중", // Working
  "status.asking": "확인 필요", // Needs confirmation
  "status.waiting": "확인함", // Viewed
  "status.unavailable": "상태 확인 불가",
  "indicator.unread": "읽지 않음 · 확인 대기", // Unread · awaiting review

  // ── Title bar ──
  "titlebar.builtAt": (time) => `빌드: ${time}`, // Built at {time}
  "titlebar.versionMismatch": (frontend, backend) =>
    `버전 불일치: 프런트엔드 v${frontend} ≠ 백엔드 v${backend}. 다시 빌드하거나 동기 배포하세요.`, // Version mismatch

  "titlebar.hotReloadedAt": (time) => `핫 리로드: ${time}`, // Hot reloaded at {time}
  "titlebar.themeSystem": (resolved) => `시스템 따름 (현재: ${resolved})`, // Follow system (currently {resolved})
  "titlebar.themeDark": "다크", // Dark
  "titlebar.themeLight": "라이트", // Light
  "titlebar.browser": "내장 브라우저", // Built-in Browser
  "titlebar.remoteAccess": "원격 접속 (브라우저)", // Remote Access (Browser)
  "titlebar.connectRemote": "원격 서버에 연결", // Connect to Remote Server
  "titlebar.share": "공유", // Share
  "share.title": "VelaTerm 공유", // Share VelaTerm
  "share.subtitle":
    "VelaTerm은 작은 팀이 만들고 있습니다. 마음에 드셨다면 주변에 VelaTerm을 공유해 주세요. 더 많은 분이 저희를 알게 되는 것은 팀에 정말 큰 힘이 됩니다. 감사합니다! ❤️", // We're a small team behind VelaTerm. If you enjoy it, please share VelaTerm with others…
  "share.copyLink": "링크 복사", // Copy link
  "share.copied": "복사됨!", // Copied!
  "share.wechatMoments": "WeChat 모멘트",
  "share.weibo": "Weibo",
  "share.xiaohongshu": "샤오홍슈",
  "share.xiaohongshuAction": "게시 문구와 링크를 복사하고 샤오홍슈 크리에이터 센터 열기",
  "share.wechatQrTitle": "WeChat 모멘트에 공유",
  "share.wechatQrHint":
    "WeChat으로 QR 코드를 스캔해 링크를 연 다음 모멘트에 공유를 선택하세요.",
  "share.backToPlatforms": "공유 옵션으로 돌아가기",
  "titlebar.appearance": "외관 설정", // Appearance
  "titlebar.showLeft": "사이드바 표시", // Show sidebar
  "titlebar.hideLeft": "사이드바 숨기기", // Hide sidebar
  "titlebar.showRight": "정보 패널 표시", // Show info panel
  "titlebar.hideRight": "정보 패널 숨기기", // Hide info panel

  // ── Settings ──
  "settings.title": "설정", // Settings
  "settings.catTerminal": "터미널", // Terminal
  "settings.catBehavior": "동작", // Behavior
  "settings.catAgents": "에이전트", // Agents
  "settings.permDefault": "기본", // Default
  "settings.permYolo": "YOLO", // YOLO
  "settings.yoloHint": (flag: string) =>
    `시작 시 ${flag} 를 추가해 모든 권한 확인을 건너뜁니다. 주의해서 사용하세요.`, // YOLO flag hint
  "settings.permViaEnvHint":
    "설정 파일을 통해 모든 권한 확인을 건너뜁니다 (CLI 플래그 없음). 이 세션 시작 시 적용됩니다.",
  "settings.catGeneral": "일반", // General
  "settings.cliLabel": "셸 명령",
  "settings.cliInstall": "‘vela’ 명령 설치",
  "settings.cliUninstall": "‘vela’ 명령 제거",
  "settings.cliInstalledAt": (path: string) => `${path}에 설치됨`,
  "settings.cliConflict": (path: string) =>
    `${path}에 다른 ‘vela’ 명령이 있습니다. VelaTerm은 덮어쓰지 않습니다.`,
  "settings.cliHint": "VS Code의 `code`처럼 `vela <project-path>`를 PATH에 추가합니다.",
  "settings.agentArgsHint":
    "각 에이전트 유형의 새 세션에 적용되는 기본 실행 인자. 세션 생성·편집 시 설정한 개별 인자가 우선합니다. 비워두면 없음.", // Agent default launch args hint
  "settings.agentPathLabel": "실행 파일 경로(선택)", // Executable path (optional)
  "settings.agentPathPlaceholder": "예: ~/.local/bin/claude — 비워두면 PATH에서 검색", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "설정하면 이 유형의 세션은 PATH에서 명령을 찾는 대신 이 전체 경로로 실행됩니다. 설치되어 있지만 셸 PATH에 없는 경우에 유용합니다. 원클릭 설치 성공 후 위치가 감지되면 자동으로 입력됩니다.", // Agent executable path hint
  "settings.codexHooks": "Codex 훅",
  "settings.codexHooksDesc":
    "Codex 사용자, 프로젝트, 플러그인 및 관리 구성에서 발견된 훅을 검토하고 제어합니다.",
  "settings.codexHooksEnabledCount": (enabled: number, total: number) =>
    `활성 ${enabled} / ${total}`,
  "settings.codexHooksEmpty": "Codex 훅을 찾지 못했습니다.",
  "settings.codexHooksTrust": "신뢰",
  "settings.codexHooksUser": "사용자 구성",
  "settings.codexHooksProject": "프로젝트 구성",
  "settings.codexHooksPlugin": "플러그인",
  "settings.codexHooksSession": "세션",
  "settings.codexHooksManagedSource": "관리 구성",
  "settings.codexHooksManaged": "관리됨",
  "settings.codexHooksTrusted": "신뢰됨",
  "settings.codexHooksChanged": "변경됨",
  "settings.codexHooksReview": "검토 필요",
  "settings.codexHooksDisabled": "비활성",
  "settings.codexHooksHandler": "핸들러",
  "settings.codexHooksCommand": "명령",
  "settings.codexHooksMatcher": "매처",
  "settings.codexHooksTimeout": "시간 제한",
  "settings.codexHooksStatusMessage": "상태 메시지",
  "settings.codexHooksAll": "모두",
  "settings.codexHooksNone": "없음",
  "settings.appearance": "외관", // Appearance
  "settings.accent": "강조색", // Accent
  "settings.accentAuto": "테마 따름", // Follow theme
  "settings.density": "밀도", // Density
  "settings.densityCompact": "조밀", // Compact
  "settings.densityRegular": "보통", // Regular
  "settings.densityComfy": "여유", // Comfy
  "settings.pane": "분할 창", // Panes
  "settings.paneFlush": "플랫", // Flush
  "settings.paneCard": "카드", // Card
  "settings.divider": "구분선", // Divider
  "settings.dividerSubtle": "미세", // Subtle
  "settings.dividerVisible": "표시", // Visible
  "settings.nav": "사이드바", // Sidebar
  "settings.navTree": "표준", // Tree
  "settings.navCompact": "조밀", // Compact
  "settings.tabs": "탭", // Tabs
  "settings.dynamicStatusFilter": "상태 필터 동적 추가",
  "settings.tabSingle": "단일", // Single
  "settings.tabMulti": "다중", // Multi
  "settings.maxLiveTabs": "Background limit", // Background limit
  "settings.defaultShell": "기본 셸", // Default shell
  "settings.spawnConfirm": "Confirm before spawn", // Confirm before spawn
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.cleanImages": "붙여넣은 이미지 자동 정리",
  "settings.cleanImagesHint":
    "터미널에 붙여넣거나 끌어다 놓은 이미지는 먼저 임시 파일로 저장됩니다(경로가 에이전트에 전달됩니다). 켜면 이 세션의 임시 파일은 종료 시 삭제되고, 24시간이 지난 잔여 파일은 시작 시 정리됩니다. 문서 안의 이미지는 영향을 받지 않습니다.",
  "settings.cleanImagesNow": "지금 정리",
  "settings.cleanImagesResult": (n: number, size: string) =>
    `임시 이미지 ${n}개를 정리했습니다(${size} 확보).`,
  "settings.cleanImagesEmpty": "정리할 임시 이미지가 없습니다.",
  "settings.imagePasteMode": "이미지 붙여넣기",
  "settings.imagePasteUpload": "파일 경로 붙여넣기",
  "settings.imagePasteAgent": "기본 이미지 붙여넣기",
  "settings.imagePasteHint":
    "이미지를 붙여넣을 때 입력할 내용을 선택합니다(로컬 데스크톱 전용). 파일 경로 붙여넣기: 이미지를 임시 저장하고 경로를 Claude 또는 Codex에 입력합니다. 기본 이미지 붙여넣기: Claude 또는 Codex가 시스템 클립보드를 읽고 자체 이미지 자리 표시자를 표시합니다.",
  "settings.imagePasteRemoteHint":
    "원격 세션에서는 에이전트가 자신의 컴퓨터에서 이미지를 읽을 수 있도록 항상 파일 경로를 붙여넣습니다. 기본 이미지 붙여넣기는 로컬 데스크톱에서만 사용할 수 있습니다.",
  "spawn.title": "Start spawned session?", // Start spawned session?
  "spawn.fromSession": "From", // From
  "spawn.promptLabel": "Prompt", // Prompt
  "spawn.agentLabel": "Agent", // Agent
  "spawn.worktreeLabel": "Separate git worktree", // Separate git worktree
  "spawn.modelLabel": "모델", // Model
  "spawn.effortLabel": "추론 강도", // Effort
  "spawn.launch": "Launch", // Launch
  "spawn.remaining": (n: number) => `${n} more pending`, // ${n} more pending
  "spawn.notifyTitle": "Spawn session awaiting confirmation", // Spawn session awaiting confirmation
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "변경 사항 보기…",
  "changes.title": "변경 사항",
  "changes.loading": "불러오는 중…",
  "changes.loadingDiff": "diff 불러오는 중…",
  "changes.noChanges": "변경 사항 없음",
  "changes.refresh": "새로 고침",
  "changes.notRepo": "Git 저장소가 아닙니다",
  "changes.selectFile": "파일을 선택하세요",
  "changes.binary": "바이너리 파일 — 줄 단위 diff 불가",
  "changes.commitTitle": (hash: string) => `커밋 ${hash}`,

  "git.staged": "스테이징됨",
  "git.changes": "변경 사항",
  "git.untracked": "추적되지 않는 파일",
  "git.committed": "커밋된 변경 사항",
  "git.stage": "스테이징",
  "git.unstage": "스테이징 취소",
  "git.stageAll": "모두 스테이징",
  "git.unstageAll": "모두 스테이징 취소",
  "git.discard": "변경 사항 버리기",
  "git.deleteFile": "삭제",
  "git.viewAll": "모두 보기",
  "git.detached": "(detached)",
  "git.aheadBehind": "업스트림 브랜치 대비 앞선/뒤처진 커밋 수",
  "git.commitPlaceholder": "커밋 메시지",
  "git.amend": "마지막 커밋 수정",
  "git.amendCommit": "커밋 수정",
  "git.commitCount": (n: number) => `${n}개 파일 커밋`,
  "git.commitNoFiles": "이 커밋에는 파일 변경이 없습니다",
  "git.noCommits": "아직 커밋이 없습니다",
  "git.loadMore": "더 불러오기",
  "tree.merge": "Merge…", // TODO translate
  "tree.copyWorktreePath": "Copy worktree path",
  "tree.openWorktreeDir": "Open worktree folder",
  "tree.deleteWorktreeMenu": "Delete worktree…", // TODO translate
  "tree.deleteWorktreeTitle": "Delete worktree", // TODO translate
  "tree.deleteWorktreeBody": "Choose a worktree to remove. This deletes its working directory from disk.", // TODO translate
  "tree.deleteWorktreePlaceholder": "Select a worktree…", // TODO translate
  "tree.deleteWorktreeForce": "Force delete (discard uncommitted changes)", // TODO translate
  "tree.convertToNormalSession": "Convert to normal session", // TODO translate
  "tree.convertToNormalGroup": "Convert to normal group", // TODO translate
  "merge.title": "Merge branches", // TODO translate
  "merge.desc": "Pick a source and a target branch; the source merges into the target.", // TODO translate
  "merge.notRepo": "This session's directory is not a git repository.", // TODO translate
  "merge.loadingBranches": "Loading branches…", // TODO translate
  "merge.loadingDiff": "Loading diff…", // TODO translate
  "merge.sourceLabel": "Source branch", // TODO translate
  "merge.targetLabel": "Target branch", // TODO translate
  "merge.selectBranch": "Select a branch…", // TODO translate
  "merge.swap": "Swap direction", // TODO translate
  "merge.pickHint": "Pick both branches to preview the changes this merge brings in.", // TODO translate
  "merge.changes": (target: string) => `Changes brought into "${target}"`, // TODO translate
  "merge.noChanges": "No file changes.", // TODO translate
  "merge.sameBranch": "Source and target are the same branch.", // TODO translate
  "merge.branchGone": "A selected branch no longer exists. Pick again.", // TODO translate
  "merge.upToDate": "The target branch already contains the source branch. Nothing to merge.", // TODO translate
  "merge.targetNotCheckedOut": (target: string) =>
    `Target branch "${target}" isn't checked out in any worktree, so a local merge can't run. Check it out first.`, // TODO translate
  "merge.targetDirty":
    "The target branch's working tree has uncommitted changes; the merge may be blocked.", // TODO translate
  "merge.sourceDirtyNote":
    "The source branch's working tree has uncommitted changes; they will be committed first.", // TODO translate
  "merge.commitMsgLabel": "Commit message", // TODO translate
  "merge.commitMsgPlaceholder": "Describe this change (used as the commit message)", // TODO translate
  "merge.apply": "Merge", // TODO translate
  "merge.commitAndApply": "Commit & merge", // TODO translate
  "merge.working": "Merging…", // TODO translate
  "merge.doneMsg": (source: string, target: string) => `Merged "${source}" into "${target}".`, // TODO translate
  "merge.conflictMsg": (target: string) =>
    `Merge has conflicts. Resolve them in the terminal of "${target}"'s worktree, then commit:`, // TODO translate
  "merge.close": "Close", // TODO translate
  "gitea.title": "Gitea integration", // TODO translate
  "gitea.desc":
    "Configure a Gitea server to land worktrees by opening a pull request. The token is stored in your system keychain.", // TODO translate
  "gitea.baseUrl": "Base URL", // TODO translate
  "gitea.token": "Access token", // TODO translate
  "gitea.tokenSet": "Saved (leave blank to keep)", // TODO translate
  "gitea.tokenPlaceholder": "Personal access token", // TODO translate
  "gitea.test": "Test connection", // TODO translate
  "gitea.saved": "Saved.", // TODO translate
  "settings.renderer": "터미널 렌더러", // Terminal renderer
  "settings.redrawOnReveal": "탭 전환 시 다시 그리기", // Redraw on tab switch
  "settings.catAdvanced": "고급", // Advanced
  "settings.outputScheduler": "포그라운드 우선 출력", // Foreground-priority output
  "settings.recordSessions": "세션 로그 기록", // Record session logs
  "settings.recordSessionsHint":
    "기본값은 끔. 켜면 터미널 출력을 로그 파일로 저장해 보관 재생과 검색에 사용합니다. 일반 터미널 세션은 기록하지 않으며, 에이전트 세션은 자체 대화 기록을 읽습니다.", // Record session logs hint
  "settings.fonts": "Fonts", // TODO translate
  "settings.uiFont": "Interface font", // TODO translate
  "settings.uiFontSize": "Interface size", // TODO translate
  "settings.termFont": "Terminal font", // TODO translate
  "settings.termFontSize": "Terminal size", // TODO translate
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontUnavailable": "이 기기에 설치되어 있지 않음",
  "settings.fontAuto": "Auto", // TODO translate
  "settings.fontSmaller": "Smaller", // TODO translate
  "settings.fontLarger": "Larger", // TODO translate
  "settings.fontReset": "Reset", // TODO translate
  "settings.sound": "알림음", // Notification sound
  "settings.language": "언어", // Language
  "settings.langAuto": "자동 (시스템)", // Auto (system)
  "settings.skillLabel": "Vela Skills",
  "settings.skillInstall": "Install", // Install
  "settings.skillInstalled": "Reinstall", // Reinstall
  "settings.skillInvokeHint":
    "Claude: /vspawn <task> · Codex: $vspawn <task>. If Codex does not list it after installation, start a new Codex session.",
  // Notification permission guidance
  "settings.notify": "System notifications", // TODO translate
  "settings.notifyGranted": "Enabled", // TODO translate
  "settings.notifyAllow": "Allow notifications", // TODO translate
  "settings.notifyOffHint":
    "Allow VelaTerm to alert you when an agent needs your input or finishes a task.", // TODO translate
  "settings.notifyDeniedHint": "Notifications are blocked. To turn them on:", // TODO translate
  "settings.notifyStepsMac":
    "open System Settings ▸ Notifications ▸ VelaTerm and turn on Allow Notifications (Banners or Alerts recommended).", // TODO translate
  "settings.notifyStepsWin":
    "open Settings ▸ System ▸ Notifications, enable VelaTerm, and make sure Focus assist / Do not disturb isn't blocking it.", // TODO translate
  "settings.notifyStepsLinux":
    "open your desktop's Settings ▸ Notifications and allow VelaTerm.", // TODO translate
  "settings.notifyStepsBrowser":
    "click the site-permission icon in the address bar and set Notifications to Allow.", // TODO translate
  "settings.notifyUnsupported": "Notifications aren't available in this environment.", // TODO translate
  "settings.notifyOpenSettings": "Open System Settings", // TODO translate
  // Shortcut categories
  "settings.catShortcuts": "단축키", // Shortcuts
  "settings.scOpenProject": "프로젝트 열기", // Open project
  "settings.scNewTab": "새 터미널", // New terminal
  "settings.scNewBrowserTab": "새 브라우저 탭", // New browser tab
  "settings.scClosePane": "창/탭 닫기", // Close pane / tab
  "settings.scSplitRight": "오른쪽 분할", // Split right
  "settings.scSplitDown": "아래쪽 분할", // Split down
  "settings.scSearch": "터미널에서 찾기", // Find in terminal
  "settings.scGlobalSearch": "모든 세션 검색", // Search all sessions
  "settings.scSaveDoc": "문서 저장", // Save document
  "settings.scRecording": "키를 누르세요…", // Press keys…
  "settings.scHint": "단축키를 클릭한 다음 새 조합을 누르세요(Cmd/Ctrl 필요).", // hint
  "settings.scReset": "기본값 복원", // Restore defaults
  "settings.scConflict": (label: string) => `이미 "${label}"에서 사용 중`, // conflict

  // ── Remote access panel ──
  "remote.title": "원격 접속 (브라우저)", // Remote Access (Browser)
  "remote.desc":
    "활성화하면 같은 LAN의 기기가 브라우저로 아래 주소를 열고 비밀번호를 입력해 데스크톱과 동일한 화면을 사용할 수 있습니다.", // Once enabled, devices on the same LAN…
  "remote.needPassword": "먼저 접속 비밀번호를 설정하세요", // Please set an access password first
  "remote.running": (port) => `실행 중 · 포트 ${port}`, // Running · port {port}
  "remote.urlsHint":
    "기기와 같은 WiFi / 서브넷의 주소를 브라우저로 여세요 (네트워크 인터페이스가 여러 개면 알맞은 것을 선택. VPN/터널 주소는 맨 뒤에 있으며 외부 기기에서는 대개 연결되지 않습니다):", // Open the address on the same WiFi / subnet…
  "remote.copyUrl": "클릭하여 주소 복사", // Click to copy address
  "remote.moreUrls": (n: number) => `다른 링크 ${n}개`, // N more urls
  "remote.lessUrls": "접기", // Show less
  "remote.stop": "서버 중지", // Stop Server
  "remote.passwordPlaceholder": "접속 비밀번호 설정", // Set access password
  "remote.starting": "시작 중…", // Starting…
  "remote.start": "서버 시작", // Start Server
  "remote.portLabel": "포트", // Port
  "remote.portInvalid": "포트는 1에서 65535 사이여야 합니다", // Port must be between 1 and 65535
  "remote.ipLabel": "IP 주소", // IP address
  "remote.ipAuto": "자동 (첫 번째 LAN 주소)", // Automatic (first LAN address)
  "remote.ipVpn": "VPN", // VPN
  "remote.qrHint":
    "휴대폰으로 스캔하면 선택한 주소로 페어링 링크가 열립니다.", // Scan with your phone to open the pairing link on the selected address.
  "remote.fingerprintLabel": "인증서 지문 (SHA-256)", // Certificate fingerprint (SHA-256)
  "remote.fingerprintHint":
    "처음 연결할 때 브라우저가 인증서를 신뢰할 수 없다고 경고합니다(자체 서명 인증서에서는 정상). 이 지문을 대조해 이 컴퓨터에 연결 중인지 확인하세요.", // On first connect, browsers warn the certificate is untrusted…

  "remote.pairingCreate": "페어링 링크 생성", // Create pairing link
  "remote.pairingRegenerate": "링크 재생성(모든 기기 연결 해제)", // Regenerate link (disconnects all)
  "remote.pairingCreating": "생성 중…", // Generating…
  "remote.pairingHint":
    "브라우저에서 열고 비밀번호를 입력하세요. 이 링크에는 접속 자격 증명이 포함되어 있으니 본인 기기에만 공유하세요.", // Open in a browser, then enter the password…

  "remote.devicesLabel": "페어링된 기기", // Paired devices
  "remote.lastSeen": "마지막 연결", // Last seen
  "remote.revoke": "해지", // Revoke
  "remote.deviceBlock": "차단", // Block
  "remote.deviceBlockConfirm": "차단 확인", // Confirm block
  "remote.deviceBlockHint":
    "차단된 기기는 연결이 끊기고 다시 연결할 수 없습니다(새 페어링 링크 필요). 다른 기기에는 영향이 없습니다.", // Block hint
  "remote.devicesEmpty": "페어링된 기기가 없습니다", // No paired devices yet
  "remote.autoRestartHint":
    "원격 액세스는 앱을 다시 열면 자동으로 다시 시작됩니다. \"서버 중지\"로 끌 수 있습니다.", // Remote access restarts automatically when the app is reopened. Stop Server turns this off.
  "remote.autostartFailed": "자동 시작 실패:", // Automatic start failed:
  "remote.mirror": "기기 간 레이아웃 미러링", // Mirror layout across devices
  "remote.mirrorHint":
    "탭, 분할, 활성 세션이 연결된 모든 기기에서 동일하게 유지됩니다. 키보드 포커스는 기기마다 그대로 유지됩니다.", // Tabs, splits, and the active session stay the same on every connected device. Keyboard focus stays put on each one.

  // ── Remote connection panel ──
  "connect.title": "원격 서버에 연결", // Connect to Remote Server
  "connect.pairingPlaceholder": "페어링 링크 붙여넣기", // Paste pairing link
  "connect.confirmConnect": "지문 확인 후 연결", // Fingerprint matches, connect
  "connect.desc": "원격 VelaTerm의 주소와 비밀번호를 입력하면 새 창에서 연결·조작합니다.", // Enter the address and password…
  "connect.addressPlaceholder": "IP 주소 (예: 192.168.1.100)", // IP address, e.g. 192.168.1.100
  "connect.portPlaceholder": "포트", // Port
  "connect.connecting": "연결 중…", // Connecting…
  "connect.connect": "연결", // Connect
  "connect.stagePreparing": "서버 준비 중…",
  "connect.stageTransferring": "서버 전송 중…",
  "connect.stageStarting": "서버 시작 중…",
  "connect.sshFingerprintLabel": (kt: string) => `SSH 호스트 키 지문 (${kt})`,
  "connect.sshHostNew": "이 호스트에 처음 연결합니다. 계속하기 전에 지문을 확인하세요.",
  "connect.sshHostChanged":
    "⚠ 이 호스트의 키가 변경되었습니다. 서버 재설치이거나 중간자 공격일 수 있습니다. 확실한 경우에만 계속하세요.",
  "connect.urlCertChanged":
    "⚠ 이 서버의 인증서 지문이 마지막 확인 이후 변경되었습니다. 서버 재설치이거나 중간자 공격일 수 있습니다. 확실한 경우에만 계속하세요.",
  "connect.sshPasswordLabel": "SSH 비밀번호",
  "connect.sshPasswordPlaceholder": "계정 비밀번호",
  "connect.savedHosts": "최근 호스트",
  "connect.savedHostsAll": "최근 호스트 전체",
  "connect.showAllHosts": (n: number) => `모두 보기 (${n})`,
  "connect.forgetHost": "이 호스트 삭제",
  "connect.savedHasPassword": "비밀번호 저장됨",
  "connect.rememberPassword": "비밀번호 저장",
  "connect.urlPasswordPlaceholder": "로그인 비밀번호",
  "connect.shareDesktopDb": "원격 데스크톱 앱의 데이터베이스 공유",
  "connect.shareDesktopDbHint":
    "원격 컴퓨터의 데스크톱 앱과 동일한 데이터베이스를 공유합니다(양쪽 버전을 동일하게 유지하는 것을 권장). 끄면 독립된 데이터베이스를 사용합니다.",

  // ── Sidebar ──
  "tree.newSession": "새 세션", // New Session
  "tree.newTerminalSession": "새 터미널 세션", // New Terminal Session
  "tree.newBrowserPage": "새 브라우저 페이지", // New Browser Page
  "tree.newAgentSession": (agent) => `새 ${agent} 세션`, // New {agent} Session
  "tree.newAgentSessionGroup": "더 많은 에이전트 세션", // More Agent Session
  "tree.newAgentSessionCustom": "실행 인자 지정 후 생성…", // New with launch args…
  "tree.resumeSession": "세션 재개…", // Resume Session…
  "tree.newGroup": "새 그룹", // New Group
  "tree.newSubgroup": "새 하위 그룹", // New Subgroup
  "tree.newChildSession": "새 하위 세션", // New Child Session
  "tree.openSelected": "선택한 세션 열기", // Open Selected Sessions
  "tree.archiveSelected": "선택한 세션 보관", // Archive Selected Sessions
  "tree.moveSelected": "선택 항목 이동…", // Move Selected to…
  "tree.deleteSelected": (n) => `선택한 ${n}개 항목 삭제`, // Delete {n} Selected Items
  "tree.removeProject": "프로젝트 제거", // Remove Project
  "tree.deleteGroup": "그룹 삭제", // Delete Group
  "tree.deleteSession": "세션 삭제", // Delete Session
  "tree.projectRoot": "프로젝트 루트 (그룹 없음)", // Project root (no group)
  "tree.moveToSession": "세션 아래로 이동 (하위로)", // Move under a session (as child)
  "tree.moveTo": "이동…", // Move to…
  "tree.openNewTab": "새 탭에서 열기", // Open in New Tab
  "tree.forkSession": "세션 포크", // Fork Session
  "tree.exportSession": "세션 내보내기…", // Export Session…
  "tree.sessionInfo": "세션 정보", // Session Info
  "tree.groupInfo": "그룹 정보", // Group Info
  "info.branch": "브랜치", // Branch
  "info.path": "경로", // Path
  "info.recentCommits": "최근 커밋", // Recent Commits
  "info.noCommits": "커밋 없음", // No commits
  "tree.killProcess": "프로세스 종료", // Kill Process
  "tree.archiveSession": "세션 보관", // Archive Session
  "tree.archiveGroup": "그룹 보관", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "임시", // scratch
  "tree.persistSession": "영구 세션으로 전환…", // Make Permanent Session…
  "tree.persistDoc": "디스크에 저장…", // Save to Disk…
  "tree.closeScratch": "초안 닫기", // Close Scratch
  "tree.importProject": "프로젝트 가져오기", // Import Project
  "tree.createProject": "프로젝트 만들기",
  "tree.cloneProject": "Git에서 클론", // Clone from Git
  "createProject.title": "프로젝트 만들기",
  "createProject.name": "프로젝트 이름",
  "createProject.namePlaceholder": "내-프로젝트",
  "createProject.into": "만들 위치",
  "createProject.choose": "선택…",
  "createProject.noParent": "상위 폴더를 선택하세요",
  "createProject.invalidName": "/ 또는 \\가 없는 단일 폴더 이름을 입력하세요.",
  "createProject.creating": "만드는 중…",
  "createProject.submit": "프로젝트 만들기",
  "clone.title": "Git 저장소 클론", // Clone Git Repository
  "clone.url": "저장소 URL", // Repository URL
  "clone.urlPlaceholder": "https://… 또는 git@…",
  "clone.branch": "브랜치(선택)", // Branch (optional)
  "clone.branchPlaceholder": "비우면 기본 브랜치", // Default branch if empty
  "clone.folder": "폴더 이름", // Folder name
  "clone.folderPlaceholder": "URL에서 자동", // Auto from URL
  "clone.into": "클론 위치", // Clone into
  "clone.choose": "선택…", // Choose…
  "clone.noParent": "상위 폴더를 선택하세요", // Choose a parent folder
  "clone.cloning": "클론 중…", // Cloning…
  "clone.cancelling": "취소 중…",
  "clone.stageStarting": "Git 시작 중…",
  "clone.stageConnecting": "저장소에 연결 중…",
  "clone.stagePreparing": "객체 준비 중…",
  "clone.stageReceiving": "객체 수신 중…",
  "clone.stageResolving": "델타 확인 중…",
  "clone.stageCheckout": "파일 체크아웃 중…",
  "clone.stageFinalizing": "마무리 중…",
  "clone.stageImporting": "프로젝트 가져오는 중…",
  "clone.elapsed": (seconds: number) => `${seconds}초 경과`,
  "clone.slowHint": "30초 동안 진행되지 않았습니다. 원격 컴퓨터의 네트워크 또는 프록시를 확인하거나 취소 후 다시 시도하세요.",
  "clone.submit": "클론", // Clone
  "tree.globalSearch": "모든 세션 검색", // Search All Sessions
  "tree.archivedSessions": "보관된 세션", // Archived Sessions
  "tree.searchPlaceholder": "세션 / 그룹 검색…", // Search sessions / groups…
  "tree.clearSearch": "검색 지우기", // Clear search
  "tree.filterWorking": "작업 중", // Working
  "tree.filterAsking": "처리 대기", // Pending
  "tree.filterWaiting": "확인함", // Viewed
  "tree.filterStatus": "상태로 필터", // Filter by status
  "tree.refreshStatusFilter": "상태 필터 새로 고침",
  "tree.refreshStatusMatch": "상태 새로 고침",
  "tree.filterStatusSection": "상태", // Status
  "tree.filterMarkSection": "표시", // Mark
  "tree.viewMainName": "기본",
  "tree.viewUntitled": "이름 없는 보기",
  "tree.viewDefaultName": (n) => `보기 ${n}`,
  "tree.viewPrimary": "기본 보기",
  "tree.viewManage": "보기 관리",
  "tree.viewSetPrimary": "기본 보기로 설정",
  "tree.viewRename": "보기 이름 변경",
  "tree.viewName": "보기 이름",
  "tree.viewDelete": "보기 삭제",
  "tree.viewDeletePrimary": "기본 보기는 삭제할 수 없습니다",
  "tree.viewDeleteTitle": "트리 보기 삭제",
  "tree.viewDeleteConfirm": (name) =>
    `“${name}” 보기를 삭제하시겠습니까? 저장된 검색 및 필터만 제거되며 프로젝트와 세션에는 영향을 주지 않습니다.`,
  "tree.viewSplitRight": "트리 보기를 오른쪽으로 분할",
  "tree.viewSplitDown": "트리 보기를 아래로 분할",
  "tree.viewAdd": "현재 트리 보기를 새 탭으로 복사",
  "tree.viewCount": (n) => `트리 보기 ${n}개`,
  "mark.menu": "표시", // Mark
  "mark.urgent": "긴급", // Urgent
  "mark.important": "중요", // Important
  "mark.bug": "버그", // Bug
  "mark.done": "완료", // Done
  "mark.wip": "진행 중", // In progress
  "mark.pinned": "고정", // Pinned
  "mark.idea": "아이디어", // Idea
  "mark.caution": "주의", // Caution
  "tree.clearAllNotifications": "모든 알림 배지 지우기 (세션 점과 Dock 배지)", // Clear all notification badges…
  "tree.noProjectsPre": "아직 프로젝트가 없습니다. 폴더 아이콘을 누르거나 ", // No projects yet. Click the folder button, or press
  "tree.noProjectsPost": " 로 디렉터리를 가져오세요.", // to import a directory.
  "tree.openProject": "프로젝트 열기", // Open Project
  "tree.noAttention": "상태 필터와 일치하는 세션이 없습니다", // No sessions match the status filter
  "tree.noMatch": "일치 항목 없음", // No matches

  // Dialog fields
  "tree.groupName": "그룹 이름", // Group name
  "tree.sessionNameAuto": "세션 이름 (비우면 자동 명명)", // Session name (leave empty to auto-name)
  "tree.editSession": "세션 편집", // Edit Session
  "tree.sessionName": "세션 이름", // Session name
  "tree.shellLabel": "셸 (비우면 시스템 기본값)", // Shell (leave empty for system default)
  "tree.shellMenu": "셸", // Shell
  "tree.downloadFullGitbash": "전체 Git Bash 다운로드",
  "gitbash.title": "Git Bash",
  "gitbash.downloading": "전체 Git Bash 다운로드 중…",
  "gitbash.extracting": "전체 Git Bash 압축 푸는 중…",
  "gitbash.done": "전체 Git Bash가 준비되었습니다.",
  "gitbash.failed": "Git Bash 다운로드 실패",
  "tree.shellSystemDefault": "시스템 기본값", // System default
  "form.customOption": "사용자 지정…", // Custom…
  "tree.cwdLabel": "작업 디렉터리 (비우면 프로젝트 루트)", // Working directory (leave empty for project root)
  "tree.initCmdLabel": "시작 명령 (선택)", // Startup command (optional)
  "tree.agentArgsLabel": "실행 인자 (선택)", // Launch args (optional)
  "preset.execPathLabel": "실행 파일(선택)",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint": "비워 두면 에이전트에 설정된 명령을 사용합니다. 지정하면 이 세션만 호환 대체 프로그램으로 실행됩니다.",
  "preset.saveLabel": "프리셋으로 저장",
  "preset.namePlaceholder": "프리셋 이름",
  "preset.iconChoose": "아이콘 선택",
  "preset.iconClear": "제거",
  "preset.iconHint": "정사각형 이미지가 가장 좋습니다. 나머지는 잘라서 64x64로 축소합니다.",
  "tree.permissionSkipLabel": "모든 권한 확인 건너뛰기", // Skip all permission confirmations
  "tree.permissionSkipHint":
    "시작 시 이 에이전트의 우회 플래그를 추가합니다(예: Claude의 --dangerously-skip-permissions, Codex는 샌드박스도 비활성화). 시작할 때마다 적용되므로 주의해서 사용하세요.",
  "tree.permissionUnsupported":
    "OpenCode는 설정 파일로 권한을 제어하며 해당 시작 플래그가 없어 이 옵션은 적용되지 않습니다.",
  "tree.permissionUnsupportedPi":
    "Pi는 설계상 권한 확인 프롬프트 없이 도구를 실행하므로 이 옵션은 적용되지 않습니다.",

  // 새 에이전트 세션 대화상자
  "newAgent.desc":
    "세션 이름과 사용자 지정 실행 인자(agent 명령에 전달, 예: --model opus)는 선택 사항입니다. 둘 다 비워 두고 Enter를 누르면 평소처럼 시작합니다.", // Optionally name the session and add custom launch args…

  // Delete confirmation
  "tree.batchDeleteTitle": "일괄 삭제", // Batch Delete
  "tree.deleteProjectTitle": "프로젝트 삭제", // Delete Project
  "tree.deleteGroupTitle": "그룹 삭제", // Delete Group
  "tree.deleteSessionTitle": "세션 삭제", // Delete Session
  "tree.batchDeleteBody": (n) =>
    `선택한 ${n}개 항목을 삭제합니다 (프로젝트/그룹은 하위 그룹과 세션도 함께 삭제됩니다). 되돌릴 수 없습니다.`, // Delete the {n} selected items…
  "tree.deleteProjectBody": (name) =>
    `프로젝트 "${name}"을(를) 삭제할까요? 하위 그룹과 세션도 모두 삭제됩니다. 되돌릴 수 없습니다.`, // Delete project "{name}"?…
  "tree.deleteGroupBody": (name) =>
    `그룹 "${name}"을(를) 삭제할까요? 하위 그룹과 세션도 모두 삭제됩니다. 되돌릴 수 없습니다.`, // Delete group "{name}"?…
  "tree.deleteSessionBody": (name) =>
    `세션 "${name}" (및 모든 하위 세션)을 삭제할까요? 되돌릴 수 없습니다.`, // Delete session "{name}"…
  "tree.deleteWorktrees": (n) =>
    `연결된 git worktree도 삭제 (총 ${n}개. 작업 트리에 변경이 있으면 삭제가 실패할 수 있습니다)`, // Also remove associated git worktrees…

  // Session information dialog
  "info.name": "이름", // Name
  "info.type": "종류", // Type
  "info.status": "상태", // Status
  "info.notYetCaptured": "아직 생성되지 않음 (첫 실행 후 캡처)", // Not yet generated (captured after first run)
  "info.sessionId": "세션 ID", // Session ID
  "info.cwd": "작업 디렉터리", // Working dir
  "info.initCmd": "시작 명령", // Startup cmd
  "info.agentArgs": "실행 인자", // Launch args
  "info.launchCmd": "전체 실행 명령", // Full launch command
  "info.permission": "권한", // Permission
  "info.permissionSkip": "모든 확인 건너뛰기", // Skip all confirmations
  "info.parentSessionId": "부모 세션 ID", // Parent ID
  "info.termTitle": "터미널 제목", // Terminal title
  "info.createdAt": "생성 시각", // Created at

  // Resume-session dialog
  "resume.title": "세션 재개", // Resume Session
  "resume.desc":
    "에이전트 종류를 고르고 해당 에이전트 자체의 session id를 입력하세요. 열면 원래 대화를 이어갑니다.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "에이전트 종류", // Agent type
  "resume.sessionIdPlaceholder": "대화 session id", // Conversation session id
  "resume.confirm": "재개하고 열기", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "새 worktree 세션…", // New Worktree Session…
  "worktree.worktreeNameLabel": "worktree 이름", // Worktree name
  "worktree.worktreeNameHint": "worktree 디렉터리 이름과 브랜치 이름으로 사용됩니다.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "worktree 생성 실패", // Couldn't create the worktree
  "worktree.noRepoRoot": "이 프로젝트에는 사용할 수 있는 git 저장소 경로가 없습니다.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "없음", // None
  "worktreeSel.modeNew": "새로", // New
  "worktreeSel.modeExisting": "기존", // Existing
  "worktreeSel.loading": "worktree 불러오는 중…", // Loading worktrees…
  "worktreeSel.empty": "이 저장소에 기존 worktree가 없습니다.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed": "worktree 목록을 가져올 수 없습니다 (git 저장소가 아닌가요?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint": "이 그룹에서 만든 세션은 기본적으로 이 worktree를 사용합니다.", // Sessions created in this group will use this worktree by default.

  // ── Archive panel ──
  "archive.title": "보관된 세션", // Archived Sessions
  "archive.empty1": "보관된 세션이 없습니다.", // No archived sessions.
  "archive.empty2": "사이드바의 세션에서 우클릭 후 \"세션 보관\"을 누르면 여기에 들어옵니다.", // Right-click a session in the sidebar…
  "archive.restore": "일반 세션으로 복원", // Restore to normal session
  "archive.export": "전체 컨텍스트를 Markdown으로 내보내기", // Export full context as Markdown
  "archive.deleteForever": "영구 삭제 (녹화 포함)", // Delete permanently (with recording)
  "archive.pickOne": "왼쪽에서 보관된 세션을 선택해 대화 기록을 확인하세요", // Select an archived session on the left…
  "archive.recordingEnd": "--- 녹화 끝 ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) => `녹화 읽기 실패: ${err}`, // Failed to read recording: {err}
  "archive.searchRecording": "녹화에서 검색…", // Search in recording…
  "archive.searchTranscript": "대화 내용 검색…", // Search transcript…
  "archive.searchPlaceholder": "보관 내용 검색…", // Search archived content…
  "archive.msgCountAll": (n) => `${n}개`, // {n} messages
  "archive.msgCountFiltered": (shown, total) => `${shown} / ${total}개`, // {shown} / {total} messages
  "archive.you": "나", // You
  "archive.toolsUsed": (tools) => `도구: ${tools}`, // Tools: {tools}
  "archive.noMatch": "일치하는 메시지가 없습니다", // No matching messages
  "archive.emptyTranscript": "대화 기록이 비어 있습니다", // Transcript is empty
  "archive.loadingTranscript": "대화 기록 불러오는 중…", // Loading transcript…

  // ── Global session-content search ──
  "search.allPlaceholder": "모든 세션 내용 검색…", // Search across all session content…
  "search.hint": "세션 내용을 검색합니다. 보관된 세션은 기본적으로 제외되며 '보관 세션 포함'을 선택하면 추가됩니다.", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "보관 세션 포함", // Include archived
  "search.includeArchivedHint": "보관된 세션도 검색에 포함 (기본은 꺼짐)", // Also search archived sessions (off by default)
  "search.searching": "검색 중…", // Searching…
  "search.noResults": "일치하는 항목이 없습니다", // No matches found
  "search.sessionCount": (n) => `세션 ${n}개`, // n sessions
  "search.matchCount": (n) => `일치 ${n}건`, // n matches
  "search.pickSession": "왼쪽에서 세션을 선택하면 일치 항목이 표시됩니다", // Select a session on the left to see its matches
  "search.openSession": "세션 열기", // Open session
  "search.backToResults": "결과로 돌아가기", // Back to results
  "search.archivedBadge": "보관됨", // Archived
  "search.summary": (m, s) => `일치 ${m}건 · 세션 ${s}개`, // X matches · N sessions
  "search.matchPosition": (n, total) => `${n} / ${total}`, // N of M
  "search.roleTerminal": "터미널", // Terminal
  "search.collapseGroup": "접기", // Collapse
  "search.expandGroup": "펼치기", // Expand
  "search.cappedNote": (l, total) => `${total}건 중 ${l}건 이동 가능`, // L of total locatable

  // ── Center pane ──
  "center.noSession": "세션 없음", // No session
  "center.noSessionHintPre": "사이드바에서 세션을 선택하거나 ", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": " 로 터미널을 만드세요", // to create a terminal
  "center.createTerminal": "터미널 만들기", // Create Terminal
  "tab.unsavedDot": "저장되지 않은 변경", // Unsaved changes
  "tab.newTerminal": "새 터미널", // New terminal
  "tab.newDocument": "새 문서", // New document
  "tab.bgTitle": (n) => `백그라운드 유지 탭: ${n}개 (프로세스 실행 중)`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `백그라운드 ${n}`, // Background {n}
  "tab.scratchFallback": "(임시 터미널)", // (scratch terminal)
  "tab.killBgTab": "이 백그라운드 탭 종료 (프로세스도 종료됩니다)", // Kill this background tab…
  "tab.newBrowserTab": "새 탭", // New Tab
  "tab.refreshFile": "파일 새로고침", // Refresh File
  "tab.closeOthers": "다른 탭 닫기", // Close Other Tabs
  "tab.closeRight": "오른쪽 탭 닫기", // Close Tabs to the Right
  "tab.closeAll": "모든 탭 닫기", // Close All Tabs
  "tab.sendToBackground": "백그라운드로 전환", // Send to Background

  // ── 내장 브라우저 ──
  "browser.back": "뒤로", // Back
  "browser.forward": "앞으로", // Forward
  "browser.reload": "새로고침", // Reload
  "browser.desktopOnly": "브라우저 탭은 데스크톱 앱에서만 열립니다.", // Browser tabs open in the desktop app only.
  "browser.stop": "로드 중지", // Stop loading
  "browser.openExternal": "시스템 브라우저에서 열기", // Open in system browser
  "browser.addressPlaceholder": "URL 또는 검색어 입력", // Enter URL or search terms
  "browser.quickAccess": "빠른 실행", // Quick access
  "browser.loading": "로드 중…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "VelaTerm을 종료할까요?",  // Quit VelaTerm?
  "quit.body": "실행 중인 터미널과 에이전트 세션이 모두 중지됩니다.",  // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "작업 공간 저장",  // Save workspace
  "quit.saveWorkspaceHint": "다음에 같은 탭과 분할을 복원합니다. 터미널은 복원되지만 다시 실행되지는 않습니다.",  // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "종료",  // Quit
  "dormant.body": "저장된 작업 공간에서 복원했습니다. 아직 실행 중인 프로세스가 없습니다.",  // Restored from your saved workspace. No process is running yet.
  "dormant.start": "시작",  // Start
  "overlimit.title": (max) => `백그라운드 유지가 한도를 초과했습니다 (최대 ${max}개)`, // Background keep-alive over limit ({max})
  "overlimit.body": "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
  "overlimit.kill": "End Selected", // End Selected
  "overlimit.keep": "Keep for Now", // Keep for Now
  "overlimit.earliest": "earliest", // earliest
  "overlimit.statusWorking": "working", // working
  "overlimit.statusAsking": "awaiting reply", // awaiting reply
  "overlimit.statusWaiting": "waiting", // waiting

  // ── Terminal pane ──
  "term.paste": "붙여넣기", // Paste
  "term.pasteUseShortcut": "붙여넣기 (⌘V를 누르세요)", // Paste (press ⌘V)
  "term.selectAll": "모두 선택", // Select All
  "term.autoCopied": (n: number) => `${n}자 자동 복사됨 · ⌘V 붙여넣기`,
  "term.clear": "화면 지우기", // Clear
  "term.searchMenu": "검색…", // Search…  ⌘F
  "term.splitRight": "오른쪽 분할", // Split right (⌘D)
  "term.splitDown": "아래 분할", // Split down (⌘⇧D)
  "term.closePane": "분할 닫기", // Close split
  "term.redraw": "다시 그리기", // Redraw
  "term.mirrorTooltip":
    "미러 표시 중 (크기는 다른 클라이언트가 제어). 클릭하면 PTY를 이 창 크기에 맞춥니다", // Mirroring (size controlled by another client)…
  "term.mirrorBadge": (dims) => `⤢ 미러${dims} · 클릭해 이 창에 맞춤`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) => `⤢ 미러${dims} · 이 창에 맞춤`, // ⤢ Mirror{dims} · fit this window
  "term.imgUploadFailed": (n, lastError) =>
    `이미지 업로드 실패 ${n}건${lastError ? `: ${lastError}` : ""}`, // Image upload failed for {n} images…
  "term.imgClipboardUnavailable":
    "클립보드에서 이미지를 읽지 못했습니다. 이미지를 다시 복사한 뒤 재시도하세요.",
  "term.starting": (agent) => `${agent} 시작 중…`, // Starting {agent}…
  "term.startFailed": (err) => `시작 실패: ${err}`, // Failed to start: {err}

  // ── agent 설치 안내 카드 ──
  "agentInstall.title": (label) => `${label}이(가) 설치되어 있지 않습니다`, // {label} is not installed
  "agentInstall.desc": (label) =>
    `VelaTerm이 PATH에서 ${label}을(를) 찾지 못했습니다. 설치하면 이 세션을 시작할 수 있습니다.`, // couldn't find {label} on PATH
  "agentInstall.install": "지금 설치", // Install now
  "agentInstall.retry": "다시 시작", // Retry launch
  "agentInstall.dismiss": "직접 설치", // I'll do it myself
  "agentInstall.docs": "설치 문서", // Install docs
  "agentInstall.needsNode": "Node.js / npm 필요", // Requires Node.js / npm
  "agentInstall.afterInstall": "설치 후:", // After install:
  "agentInstall.pathSaved": (label: string) => `${label} 실행 파일 경로를 설정에 저장했습니다:`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} 설치 완료`, // {label} is installed
  "agentInstall.doneDesc": "이 세션을 재시작하면 바로 사용할 수 있습니다.", // Relaunch this session to start using it.
  "agentInstall.restartNow": "지금 재시작", // Relaunch now
  "agentInstall.later": "나중에", // Later
  "search.placeholder": "터미널에서 검색", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "위지윅", // WYSIWYG
  "doc.source": "소스", // Source
  "doc.searchPlaceholder": "찾기", // Find
  "doc.searchReplacePlaceholder": "바꾸기", // Replace
  "doc.searchReplace": "바꾸기", // Replace
  "doc.searchReplaceAll": "전체", // All
  "doc.searchNoMatch": "결과 없음", // No results
  "doc.searchCaseSensitive": "대/소문자 구분", // Match case
  "doc.searchToggleReplace": "바꾸기 전환", // Toggle replace
  "doc.fileTree": "파일 트리", // File tree
  "doc.treeUp": "상위 폴더", // Parent folder
  "doc.sidebar": "사이드바", // Sidebar
  "doc.unsaved": "저장 안 됨", // Unsaved
  "doc.saveAsTitle": "다른 이름으로 저장", // Save As
  "doc.saveAsName": "파일 이름", // File name
  "doc.outline": "개요", // Outline
  "doc.outlineEmpty": "제목 없음", // No headings
  "doc.saving": "저장 중…", // Saving…
  "doc.overwriteConfirm": "같은 이름의 파일이 이미 있습니다. ‘덮어쓰기’를 누르면 대체합니다.", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "저장", // Save
  "doc.externalChanged": "디스크에서 파일이 수정되었습니다 (저장하지 않은 로컬 변경이 있습니다).", // The file was modified on disk…
  "doc.reloadDiscard": "다시 불러오기 (내 변경 버리기)", // Reload (discard my changes)
  "doc.externalChangedClean": "디스크에서 파일이 수정되었습니다.", // The file was modified on disk.
  "doc.reload": "다시 불러오기", // Reload
  "doc.ignore": "무시", // Ignore
  "doc.loadingFile": (title) => `${title} 불러오는 중…`, // Loading {title}…
  "doc.closeTitle": "문서 닫기", // Close Document
  "doc.unsavedBody": (title) => `"${title}"에 저장하지 않은 변경이 있습니다.`, // "{title}" has unsaved changes.
  "doc.saveAndClose": "저장하고 닫기", // Save & Close
  "doc.closeNoSave": "저장하지 않고 닫기", // Close Without Saving
  "doc.conflictTitle": "저장 충돌", // Save Conflict
  "doc.conflictBody": "디스크의 파일이 외부에서 수정되었습니다. 그래도 현재 내용으로 덮어쓸까요?", // The file on disk was modified externally…
  "doc.overwrite": "덮어쓰기", // Overwrite
  "doc.saveFailed": (err) => `저장 실패: ${err}`, // Save failed: {err}
  "doc.closeTab": "탭 닫기", // Close Tab
  "doc.truncatedReadonly": (size: string) =>
    `읽기 전용: 처음 10 MB만 표시 (전체 ${size}). 파일의 나머지를 덮어쓰지 않도록 저장이 비활성화되었습니다.`,
  "doc.imgLoading": (title, size) => `${title} (${size}) 불러오는 중…`, // Loading {title} ({size})…
  "doc.imgBeingWritten": "파일이 기록되는 중입니다. 안정되면 자동으로 다시 불러옵니다.", // The file is being written; it will reload automatically once it settles.
  "doc.imgDecodeFailed": "이 이미지를 표시할 수 없습니다 (지원되지 않거나 손상된 형식).", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "창에 맞춤", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "PDF로 내보내기", // Export PDF
  "doc.diagramError": "다이어그램 오류", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "선택된 세션 없음", // No session selected
  "panel.openInEditor": "편집기에서 열기", // Open in Editor
  "panel.openInEditorTooltip": "가운데 문서 편집기에서 열기 (view 명령과 동일)", // Open in the document editor…
  "panel.preview": "미리보기", // Preview
  "panel.cantRead": "(이 파일을 읽을 수 없습니다)", // (cannot read this file)
  "panel.binary": "(바이너리 파일, 미리보기 없음)", // (binary file, no preview)
  "panel.truncated": "\n…(내용이 길어 잘림)", // …(content truncated)
  "panel.showHidden": "숨김 파일 표시", // Show hidden files
  "panel.hideHidden": "숨김 파일 숨기기", // Hide hidden files

  // ── File-tree actions (Files context menu and header add button) ──
  "files.newFile": "새 파일", // New File
  "files.newFolder": "새 폴더", // New Folder
  "files.nameLabel": "이름", // Name
  "files.newTooltip": "새 파일 또는 폴더", // New file or folder
  "files.openInTerminal": "Open in Terminal",
  "files.revealInFinder": "Show in File Manager",
  "files.copyPath": "Copy Path",
  "files.copyRelPath": "Copy Relative Path",
  "files.filterPlaceholder": "Filter files…",
  "files.deleteConfirm": (name) => `"${name}"을(를) 삭제할까요? 이 작업은 되돌릴 수 없습니다.`, // Delete "{name}"? This can't be undone.

  // ── Status bar ──
  "statusbar.sessions": (n) => `세션 ${n}개`, // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `클릭하면 사이드바에 "${label}" 세션만 표시 (다시 클릭하면 해제)`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `백그라운드 ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `백그라운드 유지 탭 수 (최대 ${max}. 초과 시 가장 오래된 비활성 탭을 자동 종료)`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) => `백그라운드 탭 종료: ${name} (유지 한도 초과)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `브라우저 원격 접속 활성화: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "권한: 확인", // Perms: Ask
  "statusbar.permSkip": "권한: 건너뛰기", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip": "이 세션의 권한 모드 · 클릭하여 변경 (이 세션만)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "이 세션의 권한", // This session's permissions
  "statusbar.permOptAsk": "매번 확인 (기본값)", // Ask each time (default)
  "statusbar.permScopeHint": "현재 세션에만 적용됩니다. 전역 기본값은 설정 ▸ 에이전트에서 조정하세요.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg": "권한이 변경되었습니다. 적용하려면 이 세션을 다시 시작해야 합니다. 다시 시작하면 현재 대화는 이어지지만 진행 중인 작업은 중단됩니다. 지금 다시 시작할까요?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "지금 다시 시작", // Restart now
  "statusbar.permRestartLater": "나중에", // Later
  "statusbar.permScopeTitle": "적용 대상?", // Apply to?
  "statusbar.permScopeSession": "이 세션만", // This session only
  "statusbar.permScopeGlobal": "전역 기본값", // Global default
  "statusbar.permScopeGlobalHint": "이 세션에 즉시 적용되며, 이후 새로 만드는 동종 세션의 기본값이 됩니다 (설정과 동기화).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

  // ── Store, notifications, and export ──
  "notify.working": "⏳ 작업 중…", // ⏳ Working…
  "notify.asking": "❓ 확인이 필요합니다", // ❓ Needs your confirmation
  "notify.waiting": "✅ 응답 완료", // ✅ Replied
  "store.subtask": "하위 작업", // Subtask
  "store.splitPane": "분할", // Split
  "export.failedTitle": "세션 내보내기 실패", // Failed to export session
  "export.contextSuffix": "컨텍스트", // context

  // ── Error panel ──
  "err.renderTitle": "렌더링 오류", // Rendering Error
  "err.renderDesc": "예기치 않은 오류가 발생했습니다. 아래 정보가 문제 파악에 도움이 됩니다.", // An unexpected error occurred…
  "err.reload": "다시 불러오기", // Reload
  "err.uncaughtTitle": "잡히지 않은 오류 발생", // Uncaught Error
  "err.uncaughtDesc": "아래 정보가 문제 파악에 도움이 됩니다.", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser": "브라우저에서는 녹화 재생이 아직 지원되지 않습니다", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `이미지 업로드 실패 (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.connecting": "연결 중…", // Connecting…
  "login.remoteAccess": "원격 접속", // Remote Access
  "login.desc": "이 터미널에 연결하려면 접속 비밀번호를 입력하세요.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "접속 비밀번호", // Access password
  "login.connect": "연결", // Connect
  "login.wrongPassword": "비밀번호가 틀렸습니다", // Wrong password
  "login.rateLimited": "시도 횟수가 너무 많습니다. 1분 후 다시 시도해 주세요.", // Too many attempts. Please wait a minute and try again.
  "login.failed": "로그인 실패, 다시 시도하세요", // Login failed, please try again
  "login.pairingRequired": "이 서버는 페어링 링크가 필요합니다. 데스크톱 앱의 '원격 액세스'에서 생성한 링크로 여세요.", // This server requires a pairing link
  "login.authFailed": "인증에 실패했습니다. 액세스 비밀번호를 확인하세요. 링크를 다시 생성했다면 새 페어링 링크를 사용하세요.", // Authentication failed, check password or use a new pairing link
  "dir.title": "프로젝트 디렉터리 선택", // Choose Project Directory
  "dir.pathPlaceholder": "검색하거나 경로를 입력하고 Enter로 이동 (~ 지원)", // Search, or type a path and press Enter (supports ~)
  "dir.up": "상위 폴더로", // Up one level
  "dir.newFolder": "새 폴더", // New Folder
  "dir.newFolderPlaceholder": "폴더 이름", // Folder name
  "dir.goInput": "입력한 경로로 이동", // Go to typed path
  "dir.noSubdirs": "(하위 디렉터리 없음)", // (no subdirectories)
  "dir.empty": "(빈 폴더)", // (empty folder)
  "dir.noMatch": "일치하는 항목 없음", // No matching items
  "dir.target": "대상 폴더", // Target
  "dir.showHidden": "숨김 항목 표시", // Show hidden items
  "dir.importing": "가져오는 중…", // Importing…
  "dir.choose": "이 디렉터리 선택", // Choose This Directory
  "conn.reconnecting": "연결이 끊어졌습니다. 다시 연결하는 중…", // Connection lost, reconnecting…
  "conn.reconnectNow": "지금 다시 연결", // Reconnect now
  "conn.retrying": "다시 연결하는 중…", // Reconnecting…
  "conn.sshReconnecting": "SSH 연결이 끊어져 터널을 다시 구축하는 중…", // SSH link lost, rebuilding the tunnel…
  "conn.sshDown": "SSH 연결이 끊어졌습니다. \"지금 다시 연결\"을 눌러 다시 시도하세요", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "요청 실패", // Request failed
  "reqerr.dismiss": "닫기", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "오류 로그", // Error Log
  "errlog.empty": "기록된 오류가 없습니다.", // No errors recorded.
  "errlog.copyAll": "모두 복사", // Copy all
  "errlog.clear": "지우기", // Clear
  "errlog.close": "닫기", // Close

  // ── Mobile ──
  "mobile.toDesktop": "데스크톱 버전으로 전환", // Switch to desktop
  "mobile.empty1": "세션이 없습니다.", // No sessions.
  "mobile.noMatch": "일치하는 세션이 없습니다", // No matching sessions
  "mobile.empty2": "데스크톱 앱이나 PC 브라우저에서 만들면 여기 자동으로 나타납니다.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ 뒤로", // ‹ Back
  "mobile.selCopy": "복사", // Copy
  "mobile.selCancel": "취소", // Cancel

  // ── Other shared components ──
  "splitter.dragToResize": "드래그하여 크기 조절", // Drag to resize
  "transport.wsDisconnected": "WebSocket 연결이 끊어졌습니다", // WebSocket disconnected
  "transport.wsConnectFailed": "WebSocket 연결 실패", // WebSocket connection failed
  "transport.cmdFailed": "명령 실패", // Command failed
  "transport.remoteCmdForbidden": (cmd: string) => `원격 클라이언트에서 사용할 수 없는 명령입니다: ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) => `원격 클라이언트가 쓸 수 없는 설정 키입니다: ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) => `원격 클라이언트는 앱 데이터 디렉터리의 파일에 접근할 수 없습니다: ${path}`, // Remote clients cannot access files in the app data directory

  // ── Crepe（위지윅 편집기 내장 UI）──
  "crepe.placeholder": "본문을 입력하거나 / 로 삽입 메뉴를 여세요", // Type text, or press / for the insert menu
  "crepe.textGroup": "텍스트", // Text
  "crepe.paragraph": "본문", // Text
  "crepe.h1": "제목 1", // Heading 1
  "crepe.h2": "제목 2", // Heading 2
  "crepe.h3": "제목 3", // Heading 3
  "crepe.h4": "제목 4", // Heading 4
  "crepe.h5": "제목 5", // Heading 5
  "crepe.h6": "제목 6", // Heading 6
  "crepe.quote": "인용", // Quote
  "crepe.divider": "구분선", // Divider
  "crepe.listGroup": "목록", // List
  "crepe.bulletList": "글머리 기호 목록", // Bullet List
  "crepe.orderedList": "번호 매기기 목록", // Ordered List
  "crepe.taskList": "작업 목록", // Task List
  "crepe.advancedGroup": "삽입", // Insert
  "crepe.image": "이미지", // Image
  "crepe.codeBlock": "코드 블록", // Code Block
  "crepe.table": "표", // Table
  "crepe.math": "수식", // Math
  "crepe.linkPlaceholder": "링크를 붙여넣거나 입력…", // Paste or type a link…
  "crepe.upload": "업로드", // Upload
  "crepe.uploadImage": "이미지 업로드", // Upload Image
  "crepe.orPasteImageLink": "또는 이미지 링크 붙여넣기", // or paste an image link
  "crepe.imageCaption": "이미지 설명", // Image caption
  "crepe.confirm": "확인", // Confirm
  "crepe.searchLanguage": "언어 검색", // Search language
  "crepe.noResult": "결과 없음", // No results
  "crepe.edit": "편집", // Edit
  "crepe.collapse": "접기", // Collapse
  // ── 오른쪽 패널 / 하단 바 추가 ──
  "info.project": "프로젝트", // Project
  "panel.sessionInfo": "세션 정보", // Session info
  "panel.gitTitle": "Git 상태", // Git status
  "panel.gitProbing": "확인 중…", // Checking…
  "panel.gitNotRepo": "Git 저장소가 아닙니다", // Not a Git repository
  "panel.gitBranch": "브랜치", // Branch
  "panel.gitStaged": "스테이지됨", // Staged
  "panel.gitUnstaged": "변경됨", // Changed
  "panel.gitUntracked": "추적 안 됨", // Untracked
  "bottombar.running": "실행 중", // Running
  "bottombar.collapseTasks": "작업 영역 접기", // Collapse tasks
  "bottombar.expandTasks": "작업 영역 펼치기", // Expand tasks
  "bottombar.sound": "🔔 소리", // 🔔 Sound
  "bottombar.muted": "🔕 음소거", // 🔕 Muted
  "bottombar.overview": "세션 개요", // Sessions overview
  "bottombar.noSessions": "세션 없음", // No sessions
  "doc.pdfFilter": "PDF 파일", // PDF file
  // ── auto update ──
  "updater.title": "Check for Updates", // TODO translate
  "updater.upToDate": "You're already on the latest version.", // TODO translate
  "updater.failed": (err) => `Update check failed: ${err}`, // TODO translate
  "updater.available": "Update available", // TODO translate
  "updater.versionLine": (version, current) =>
    `Version ${version} — you're on ${current}`, // TODO translate
  "updater.noNotes": "No release notes were published for this version.", // TODO translate
  "updater.updateNow": "Update now", // TODO translate
  "updater.later": "Later", // TODO translate
  "updater.skipVersion": "Skip this version", // TODO translate
  "updater.skipVersionHint":
    "Stop reminding me about this version. You can still install it later from Check for Updates.", // TODO translate
  "updater.downloadingPct": (pct) => `Downloading… ${pct}%`, // TODO translate
  "updater.downloadingBytes": (mb) => `Downloading… ${mb} MB`, // TODO translate
  "updater.installing": "Installing…", // TODO translate
  "updater.installed": "Update installed. Restart to finish.", // TODO translate
  "updater.restartNow": "Restart now", // TODO translate
  "updater.retry": "Try again", // TODO translate
  "updater.downloadFailed": (err) => `Update failed: ${err}`, // TODO translate
  "updater.hide": "Hide", // TODO translate
  "updater.hideHint": "Keep downloading in the background. Progress stays in the status bar.", // TODO translate
  "updater.downloadManually": "Download manually", // TODO translate
  "updater.downloadManuallyHint": "Open the installer download in your browser.", // TODO translate
  "updater.windowsNotice":
    "VelaTerm will close while the installer runs, then reopen on its own.", // TODO translate
  "updater.installingWindows":
    "Installing… VelaTerm is about to close. The installer will finish the update and reopen it.", // TODO translate
  "statusbar.updateAvailable": (version) => `Update ${version}`, // TODO translate
  "statusbar.updateDownloading": (pct) => `Updating… ${pct}%`, // TODO translate
  "statusbar.updateInstalling": "Installing…", // TODO translate
  "statusbar.updateReady": "Restart to update", // TODO translate
  "statusbar.updateFailed": "Update failed", // TODO translate
  "statusbar.updateTooltip": "Click for details", // TODO translate
};

export default ko;
