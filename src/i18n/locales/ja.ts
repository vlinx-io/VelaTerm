//! Japanese dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const ja: typeof en = {
  // ── Common ──
  "common.cancel": "キャンセル", // Cancel
  "common.confirm": "OK", // OK
  "common.delete": "削除", // Delete
  "common.save": "保存", // Save
  "common.create": "作成", // Create
  "common.close": "閉じる", // Close
  "common.copy": "コピー", // Copy
  "common.cut": "切り取り", // Cut
  "common.paste": "貼り付け", // Paste
  "common.selectAll": "すべて選択", // Select All
  "common.copied": "コピーしました", // Copied
  "common.retry": "再試行", // Retry
  "common.refresh": "更新", // Refresh
  "common.loading": "読み込み中…", // Loading…
  "common.prev": "前へ", // Previous
  "common.next": "次へ", // Next
  "common.on": "オン", // On
  "common.off": "オフ", // Off
  "common.gotIt": "OK", // Got it
  "common.rename": "名前を変更", // Rename
  "common.edit": "編集", // Edit
  "common.open": "開く", // Open
  "common.session": "セッション", // Session

  // ── Session types and status ──
  "kind.terminal": "ターミナル", // Terminal
  "kind.browser": "ブラウザ", // Browser
  "status.idle": "待機", // Idle
  "status.running": "実行中", // Running
  "status.exited": "終了", // Exited
  "status.error": "エラー", // Error
  "status.working": "処理中", // Working
  "status.asking": "要確認", // Needs confirmation
  "status.waiting": "確認済み", // Viewed
  "status.unavailable": "ステータス取得不可",
  "indicator.unread": "未読 · 確認待ち", // Unread · awaiting review

  // ── Title bar ──
  "titlebar.builtAt": (time) => `ビルド: ${time}`, // Built at {time}
  "titlebar.versionMismatch": (frontend, backend) =>
    `バージョン不一致：フロント v${frontend} ≠ バック v${backend}。再ビルドまたは同期デプロイしてください。`, // Version mismatch

  "titlebar.hotReloadedAt": (time) => `ホットリロード: ${time}`, // Hot reloaded at {time}
  "titlebar.themeSystem": (resolved) => `システムに従う（現在: ${resolved}）`, // Follow system (currently {resolved})
  "titlebar.themeDark": "ダーク", // Dark
  "titlebar.themeLight": "ライト", // Light
  "titlebar.browser": "内蔵ブラウザ", // Built-in Browser
  "titlebar.remoteAccess": "リモートアクセス（ブラウザ）", // Remote Access (Browser)
  "titlebar.connectRemote": "リモートサーバーに接続", // Connect to Remote Server
  "titlebar.mirrored": "ミラー中", // Mirrored
  "titlebar.mirroredHint":
    "ミラーが有効です。タブ・分割・アクティブなセッションはホストに追従します。切り替えはホスト側にあります。", // Mirroring is on: tabs, splits, and the active session follow the host. The switch is on the host.
  "titlebar.mirroredBy": (n: number) => `${n} 台がミラー中`, // Mirrored by {n}
  "titlebar.mirroredByHint": (n: number) =>
    `リモート ${n} 台が接続中です。タブ・分割・アクティブなセッションは共有され、どちら側からでも変更できます。`, // {n} remote clients are connected. Tabs, splits, and the active session are shared, and either side can rearrange them.
  "titlebar.clientsTitle": "接続中のクライアント", // Attached clients
  "titlebar.clientUnnamed": "名前のないクライアント", // Unnamed client
  "titlebar.clientSince": (time: string) => `${time} から`, // since {time}
  "titlebar.share": "共有", // Share
  // ── Alt-triggered menu bar (Windows/Linux) ──
  "menubar.file": "ファイル", // File
  "menubar.terminal": "ターミナル", // Terminal
  "menubar.help": "ヘルプ", // Help
  "menubar.newTerminal": "新しいターミナル", // New Terminal
  "menubar.visitWebsite": "ウェブサイトを開く", // Visit Website
  "menubar.sendFeedback": "フィードバックを送信", // Send Feedback
  "menubar.clearBadges": "通知バッジを消去", // Clear Notification Badges
  "share.title": "VelaTerm を共有", // Share VelaTerm
  "share.subtitle":
    "VelaTerm は小さなチームで開発しています。気に入っていただけたら、ぜひ周りの方にシェアしてください。より多くの方に私たちを知っていただくことは、チームにとって大きな支えになります。ありがとうございます！❤️", // We're a small team behind VelaTerm. If you enjoy it, please share VelaTerm with others…
  "share.copyLink": "リンクをコピー", // Copy link
  "share.copied": "コピーしました", // Copied!
  "share.wechatMoments": "WeChat モーメンツ",
  "share.weibo": "Weibo",
  "share.xiaohongshu": "小紅書",
  "share.xiaohongshuAction":
    "投稿文とリンクをコピーして、小紅書クリエイターセンターを開く",
  "share.wechatQrTitle": "WeChat モーメンツにシェア",
  "share.wechatQrHint":
    "WeChat で QR コードを読み取り、リンクを開いてから「モーメンツにシェア」を選択してください。",
  "share.backToPlatforms": "共有先に戻る",
  "titlebar.appearance": "外観設定", // Appearance
  "titlebar.showLeft": "サイドバーを表示", // Show sidebar
  "titlebar.hideLeft": "サイドバーを隠す", // Hide sidebar
  "titlebar.showRight": "情報パネルを表示", // Show info panel
  "titlebar.hideRight": "情報パネルを隠す", // Hide info panel

  // ── Settings ──
  "settings.title": "設定", // Settings
  "settings.catTerminal": "ターミナル", // Terminal
  "settings.catBehavior": "動作", // Behavior
  "settings.catAgents": "エージェント", // Agents
  "settings.permDefault": "デフォルト", // Default
  "settings.permYolo": "YOLO", // YOLO
  "settings.yoloHint": (flag: string) =>
    `起動時に ${flag} を付与し、すべての権限確認をスキップします。慎重に使用してください。`, // YOLO flag hint
  "settings.permViaEnvHint":
    "設定ファイル経由ですべての権限確認をスキップします（CLI フラグなし）。このセッションの起動時に適用されます。",
  "settings.catGeneral": "一般", // General
  "settings.cliLabel": "シェルコマンド",
  "settings.cliInstall": "‘vela’ コマンドをインストール",
  "settings.cliUninstall": "‘vela’ コマンドをアンインストール",
  "settings.cliInstalledAt": (path: string) => `${path} にインストール済み`,
  "settings.cliConflict": (path: string) =>
    `${path} に別の ‘vela’ コマンドがあります。VelaTerm は上書きしません。`,
  "settings.cliHint":
    "VS Code の `code` と同様に `vela <project-path>` を PATH に追加します。",
  "settings.agentArgsHint":
    "各エージェントタイプの新規セッションに適用される既定の起動引数。セッションの作成・編集時に設定した個別の引数が優先されます。空欄で引数なし。", // Agent default launch args hint
  "settings.agentPathLabel": "実行ファイルパス（任意）", // Executable path (optional)
  "settings.agentPathPlaceholder":
    "例: ~/.local/bin/claude — 空欄なら PATH から検索", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "設定すると、このタイプのセッションは PATH でコマンドを探す代わりに、この完全パスで起動します。インストール済みなのにシェルの PATH に無い場合に便利です。ワンクリックインストール成功後、場所を検出できた場合は自動で入力されます。", // Agent executable path hint
  "settings.appearance": "外観", // Appearance
  "settings.accent": "アクセント", // Accent
  "settings.accentAuto": "テーマに従う", // Follow theme
  "settings.density": "密度", // Density
  "settings.densityCompact": "コンパクト", // Compact
  "settings.densityRegular": "標準", // Regular
  "settings.densityComfy": "ゆったり", // Comfy
  "settings.pane": "ペイン", // Panes
  "settings.paneFlush": "フラット", // Flush
  "settings.paneCard": "カード", // Card
  "settings.divider": "区切り線", // Divider
  "settings.dividerSubtle": "極細", // Subtle
  "settings.dividerVisible": "表示", // Visible
  "settings.nav": "サイドバー", // Sidebar
  "settings.navTree": "標準", // Tree
  "settings.navCompact": "コンパクト", // Compact
  "settings.tabs": "タブ", // Tabs
  "settings.dynamicStatusFilter": "状態フィルターへの動的追加",
  "settings.tabSingle": "シングル", // Single
  "settings.tabMulti": "マルチ", // Multi
  "settings.maxLiveTabs": "Background limit", // Background limit
  "settings.defaultShell": "既定のシェル", // Default shell
  "settings.spawnConfirm": "Confirm before spawn", // Confirm before spawn
  "settings.usageAuto": "Usage auto-refresh", // Usage auto-refresh
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.cleanImages": "貼り付け画像の自動クリーンアップ",
  "settings.cleanImagesHint":
    "ターミナルに貼り付け／ドロップした画像は、まず一時ファイルとして保存されます（パスがエージェントに渡されます）。オンにすると、このセッションの一時ファイルは終了時に削除され、24 時間以上前の残りは起動時に整理されます。ドキュメント内の画像には影響しません。",
  "settings.cleanImagesNow": "今すぐクリーンアップ",
  "settings.cleanImagesResult": (n: number, size: string) =>
    `一時画像を ${n} 件削除しました（${size} 解放）。`,
  "settings.cleanImagesEmpty": "クリーンアップする一時画像はありません。",
  "settings.imagePasteMode": "画像の貼り付け",
  "settings.imagePasteUpload": "ファイルパスを貼り付け",
  "settings.imagePasteAgent": "ネイティブ画像貼り付け",
  "settings.imagePasteHint":
    "画像を貼り付けたときに入力する内容を選びます（ローカルデスクトップのみ）。ファイルパスを貼り付け：画像を一時保存し、そのパスを Claude または Codex に入力します。ネイティブ画像貼り付け：Claude または Codex がシステムのクリップボードを読み、独自の画像プレースホルダーを表示します。",
  "settings.imagePasteRemoteHint":
    "リモートセッションでは、エージェント側で画像を読めるよう常にファイルパスを貼り付けます。ネイティブ画像貼り付けはローカルデスクトップでのみ使用できます。",
  "spawn.title": "Start spawned session?", // Start spawned session?
  "spawn.fromSession": "From", // From
  "spawn.promptLabel": "Prompt", // Prompt
  "spawn.agentLabel": "Agent", // Agent
  "spawn.worktreeLabel": "Separate git worktree", // Separate git worktree
  "spawn.modelLabel": "モデル", // Model
  "spawn.effortLabel": "推論強度", // Effort
  "spawn.modelDefault": "デフォルト", // Default
  "spawn.modelLoading": "モデルを取得中…", // Listing models…
  "spawn.modelListUnavailable":
    "モデル一覧を取得できません — 上の欄に識別子を入力してください", // No model list available — type an identifier above
  "spawn.launch": "Launch", // Launch
  "spawn.remaining": (n: number) => `${n} more pending`, // ${n} more pending
  "spawn.notifyTitle": "Spawn session awaiting confirmation", // Spawn session awaiting confirmation
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "変更を表示…",
  "changes.title": "変更",
  "changes.loading": "読み込み中…",
  "changes.loadingDiff": "差分を読み込み中…",
  "changes.noChanges": "変更なし",
  "changes.refresh": "更新",
  "changes.notRepo": "Git リポジトリではありません",
  "changes.selectFile": "ファイルを選択してください",
  "changes.binary": "バイナリファイル — 行差分は表示できません",
  "changes.commitTitle": (hash: string) => `コミット ${hash}`,

  "git.staged": "ステージ済み",
  "git.changes": "変更",
  "git.untracked": "未追跡ファイル",
  "git.committed": "コミット済みの変更",
  "git.stage": "ステージ",
  "git.unstage": "ステージ解除",
  "git.stageAll": "すべてステージ",
  "git.unstageAll": "すべてステージ解除",
  "git.discard": "変更を破棄",
  "git.deleteFile": "削除",
  "git.viewAll": "すべて表示",
  "git.detached": "(detached)",
  "git.aheadBehind": "上流ブランチに対する先行・遅延コミット数",
  "git.commitPlaceholder": "コミットメッセージ",
  "git.amend": "直前のコミットを修正",
  "git.amendCommit": "コミットを修正",
  "git.commitCount": (n: number) => `${n} ファイルをコミット`,
  "git.commitNoFiles": "このコミットにファイルの変更はありません",
  "git.noCommits": "コミットがありません",
  "git.loadMore": "さらに読み込む",
  "tree.merge": "Merge…", // TODO translate
  "tree.copyWorktreePath": "Copy worktree path",
  "tree.openWorktreeDir": "Open worktree folder",
  "tree.deleteWorktreeMenu": "Delete worktree…", // TODO translate
  "tree.deleteWorktreeTitle": "Delete worktree", // TODO translate
  "tree.deleteWorktreeBody":
    "Choose a worktree to remove. This deletes its working directory from disk.", // TODO translate
  "tree.deleteWorktreePlaceholder": "Select a worktree…", // TODO translate
  "tree.deleteWorktreeForce": "Force delete (discard uncommitted changes)", // TODO translate
  "tree.convertToNormalSession": "Convert to normal session", // TODO translate
  "tree.moveGroupToWorktree": "Worktree に移動…",
  "tree.convertToNormalGroup": "Convert to normal group", // TODO translate
  "merge.title": "Merge branches", // TODO translate
  "merge.desc":
    "Pick a source and a target branch; the source merges into the target.", // TODO translate
  "merge.notRepo": "This session's directory is not a git repository.", // TODO translate
  "merge.loadingBranches": "Loading branches…", // TODO translate
  "merge.loadingDiff": "Loading diff…", // TODO translate
  "merge.sourceLabel": "Source branch", // TODO translate
  "merge.targetLabel": "Target branch", // TODO translate
  "merge.selectBranch": "Select a branch…", // TODO translate
  "merge.swap": "Swap direction", // TODO translate
  "merge.pickHint":
    "Pick both branches to preview the changes this merge brings in.", // TODO translate
  "merge.changes": (target: string) => `Changes brought into "${target}"`, // TODO translate
  "merge.noChanges": "No file changes.", // TODO translate
  "merge.sameBranch": "Source and target are the same branch.", // TODO translate
  "merge.branchGone": "A selected branch no longer exists. Pick again.", // TODO translate
  "merge.upToDate":
    "The target branch already contains the source branch. Nothing to merge.", // TODO translate
  "merge.targetNotCheckedOut": (target: string) =>
    `Target branch "${target}" isn't checked out in any worktree, so a local merge can't run. Check it out first.`, // TODO translate
  "merge.targetDirty":
    "The target branch's working tree has uncommitted changes; the merge may be blocked.", // TODO translate
  "merge.sourceDirtyNote":
    "The source branch's working tree has uncommitted changes; they will be committed first.", // TODO translate
  "merge.commitMsgLabel": "Commit message", // TODO translate
  "merge.commitMsgPlaceholder":
    "Describe this change (used as the commit message)", // TODO translate
  "merge.apply": "Merge", // TODO translate
  "merge.commitAndApply": "Commit & merge", // TODO translate
  "merge.working": "Merging…", // TODO translate
  "merge.doneMsg": (source: string, target: string) =>
    `Merged "${source}" into "${target}".`, // TODO translate
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
  "settings.renderer": "ターミナルレンダラー", // Terminal renderer
  "settings.redrawOnReveal": "タブ復帰時に再描画", // Redraw on tab switch
  "settings.catAdvanced": "詳細設定", // Advanced
  "settings.outputScheduler": "フォアグラウンド優先出力", // Foreground-priority output
  "settings.recordSessions": "セッションログを記録", // Record session logs
  "settings.recordSessionsHint":
    "既定はオフ。オンにするとターミナル出力をログファイルに保存し、アーカイブ再生と検索に使います。通常のターミナルセッションは記録しません。エージェントセッションは独自の会話記録を読み込みます。", // Record session logs hint
  "settings.fonts": "Fonts", // TODO translate
  "settings.uiFont": "Interface font", // TODO translate
  "settings.uiFontSize": "Interface size", // TODO translate
  "settings.termFont": "Terminal font", // TODO translate
  "settings.termFontSize": "Terminal size", // TODO translate
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontUnavailable": "このデバイスにインストールされていません",
  "settings.fontAuto": "Auto", // TODO translate
  "settings.fontSmaller": "Smaller", // TODO translate
  "settings.fontLarger": "Larger", // TODO translate
  "settings.fontReset": "Reset", // TODO translate
  "settings.sound": "通知音", // Notification sound
  "settings.language": "言語", // Language
  "settings.langAuto": "自動（システム）", // Auto (system)
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
  "settings.notifyUnsupported":
    "Notifications aren't available in this environment.", // TODO translate
  "settings.notifyOpenSettings": "Open System Settings", // TODO translate
  // Shortcut categories
  "settings.catShortcuts": "ショートカット", // Shortcuts
  "settings.scOpenProject": "プロジェクトを開く", // Open project
  "settings.scNewTab": "新規ターミナル", // New terminal
  "settings.scNewBrowserTab": "新規ブラウザタブ", // New browser tab
  "settings.scClosePane": "ペイン／タブを閉じる", // Close pane / tab
  "settings.scSplitRight": "右に分割", // Split right
  "settings.scSplitDown": "下に分割", // Split down
  "settings.scSearch": "ターミナル内を検索", // Find in terminal
  "settings.scGlobalSearch": "全セッションを検索", // Search all sessions
  "settings.scSaveDoc": "ドキュメントを保存", // Save document
  "settings.scRecording": "キーを押してください…", // Press keys…
  "settings.scHint":
    "ショートカットをクリックし、新しい組み合わせを押します（Cmd/Ctrl が必要）。", // hint
  "settings.scReset": "デフォルトに戻す", // Restore defaults
  "settings.scConflict": (label: string) =>
    `「${label}」で既に使用されています`, // conflict

  // ── Remote access panel ──
  "remote.title": "リモートアクセス（ブラウザ）", // Remote Access (Browser)
  "remote.desc":
    "有効にすると、同じ LAN 上のデバイスがブラウザで下記アドレスを開き、パスワードを入力すれば、デスクトップと同じ画面を利用できます。", // Once enabled, devices on the same LAN…
  "remote.needPassword": "先にアクセスパスワードを設定してください", // Please set an access password first
  "remote.running": (port) => `稼働中 · ポート ${port}`, // Running · port {port}
  "remote.urlsHint":
    "お使いのデバイスと同じ WiFi / サブネットのアドレスをブラウザで開いてください（NIC が複数ある場合は適切なものを選択。VPN/トンネルのアドレスは末尾に表示され、外部デバイスからは通常接続できません）：", // Open the address on the same WiFi / subnet…
  "remote.copyUrl": "クリックしてアドレスをコピー", // Click to copy address
  "remote.moreUrls": (n: number) => `他 ${n} 件のリンク`, // N more urls
  "remote.lessUrls": "折りたたむ", // Show less
  "remote.stop": "サーバーを停止", // Stop Server
  "remote.passwordPlaceholder": "アクセスパスワードを設定", // Set access password
  "remote.starting": "起動中…", // Starting…
  "remote.start": "サーバーを開始", // Start Server
  "remote.portLabel": "ポート", // Port
  "remote.portInvalid": "ポートは 1〜65535 の範囲で指定してください", // Port must be between 1 and 65535
  "remote.ipLabel": "IP", // IP address
  "remote.ipAuto": "自動（最初の LAN アドレス）", // Automatic (first LAN address)
  "remote.ipVpn": "VPN", // VPN
  "remote.qrHint":
    "スマートフォンでスキャンすると、選択したアドレスでペアリングリンクを開けます。", // Scan with your phone to open the pairing link on the selected address.
  "remote.fingerprintLabel": "証明書フィンガープリント (SHA-256)", // Certificate fingerprint (SHA-256)
  "remote.fingerprintHint":
    "初回接続時、ブラウザは証明書が信頼されていないと警告します（自己署名証明書では正常です）。このフィンガープリントを照合し、この端末への接続であることを確認してください。", // On first connect, browsers warn the certificate is untrusted…

  "remote.pairingCreate": "ペアリングリンクを作成", // Create pairing link
  "remote.pairingRegenerate": "リンクを再生成（全端末を切断）", // Regenerate link (disconnects all)
  "remote.pairingCreating": "生成中…", // Generating…
  "remote.pairingHint":
    "ブラウザで開いてパスワードを入力します。このリンクには認証情報が含まれます。自分のデバイスにのみ共有してください。", // Open in a browser, then enter the password…

  "remote.devicesLabel": "ペア済みデバイス", // Paired devices
  "remote.lastSeen": "最終接続", // Last seen
  "remote.revoke": "失効", // Revoke
  "remote.deviceBlock": "ブロック", // Block
  "remote.deviceBlockConfirm": "ブロックする", // Confirm block
  "remote.deviceBlockHint":
    "ブロックした端末は切断され、再接続できません（再度ペアリングリンクが必要）。他の端末には影響しません。", // Block hint
  "remote.devicesEmpty": "ペアリング済みの端末はありません", // No paired devices yet
  "remote.autoRestartHint":
    "リモートアクセスはアプリの再起動時に自動的に再開されます。「サーバーを停止」で無効になります。", // Remote access restarts automatically when the app is reopened. Stop Server turns this off.
  "remote.autostartFailed": "自動起動に失敗しました:", // Automatic start failed:
  "remote.mirror": "デバイス間でレイアウトを同期", // Mirror layout across devices
  "remote.mirrorHint":
    "タブ・分割・アクティブなセッションが接続中のすべての端末で一致します。キーボードフォーカスは各端末でそのまま保たれます。", // Tabs, splits, and the active session stay the same on every connected device. Keyboard focus stays put on each one.

  // ── Remote connection panel ──
  "connect.title": "リモートサーバーに接続", // Connect to Remote Server
  "connect.pairingPlaceholder": "ペアリングリンクを貼り付け", // Paste pairing link
  "connect.confirmConnect": "指紋を確認して接続", // Fingerprint matches, connect
  "connect.desc":
    "リモート VelaTerm のアドレスとパスワードを入力し、新しいウィンドウで接続・操作します。", // Enter the address and password…
  "connect.addressPlaceholder": "IPアドレス（例: 192.168.1.100）", // IP address, e.g. 192.168.1.100
  "connect.portPlaceholder": "ポート", // Port
  "connect.connecting": "接続中…", // Connecting…
  "connect.connect": "接続", // Connect
  "connect.stagePreparing": "サーバーを準備中…",
  "connect.stageTransferring": "サーバーを転送中…",
  "connect.stageStarting": "サーバーを起動中…",
  "connect.sshFingerprintLabel": (kt: string) =>
    `SSH ホスト鍵のフィンガープリント（${kt}）`,
  "connect.sshHostNew":
    "このホストへの初回接続です。続行する前にフィンガープリントを確認してください。",
  "connect.sshHostChanged":
    "⚠ このホストの鍵が変更されました。サーバーの再インストール、または中間者攻撃の可能性があります。確信がある場合のみ続行してください。",
  "connect.urlCertChanged":
    "⚠ このサーバーの証明書フィンガープリントが前回の確認から変更されました。サーバーの再インストール、または中間者攻撃の可能性があります。確信がある場合のみ続行してください。",
  "connect.sshPasswordLabel": "SSH パスワード",
  "connect.sshPasswordPlaceholder": "アカウントのパスワード",
  "connect.savedHosts": "最近の接続先",
  "connect.savedHostsAll": "最近の接続先すべて",
  "connect.showAllHosts": (n: number) => `すべて表示 (${n})`,
  "connect.forgetHost": "このホストを削除",
  "connect.savedHasPassword": "パスワード保存済み",
  "connect.rememberPassword": "パスワードを保存",
  "connect.showPassword": "パスワードを表示",
  "connect.hidePassword": "パスワードを非表示",
  "connect.urlPasswordPlaceholder": "ログインパスワード",
  "connect.mirror": "デバイス間でレイアウトを同期", // Mirror layout across devices
  "connect.mirrorHint":
    "このリモートサービスに接続したすべての端末で、タブ・分割・アクティブなセッションが一致します。オフなら各端末が自分のレイアウトを保ちます。", // Tabs, splits, and the active session stay the same on every device connected to this remote service. Off = each device keeps its own layout.
  "connect.shareDesktopDb": "リモートのデスクトップ版のデータベースを共用",
  "connect.shareDesktopDbHint":
    "リモートマシンのデスクトップ版と同じデータベースを共有します（両方を同じバージョンに揃えることを推奨）。オフにすると独立したデータベースを使用します。",

  // ── Sidebar ──
  "tree.newSession": "新規セッション", // New Session
  "tree.newTerminalSession": "新規ターミナルセッション", // New Terminal Session
  "tree.newBrowserPage": "新規ブラウザページ", // New Browser Page
  "tree.newAgentSession": (agent) => `新規 ${agent} セッション`, // New {agent} Session
  "tree.newAgentSessionGroup": "その他のエージェントセッション", // More Agent Session
  "tree.newAgentSessionCustom": "起動引数を指定して新規…", // New with launch args…
  "tree.resumeSession": "セッションを再開…", // Resume Session…
  "tree.newGroup": "新規グループ", // New Group
  "tree.newSubgroup": "新規サブグループ", // New Subgroup
  "tree.newChildSession": "新規子セッション", // New Child Session
  "tree.openSelected": "選択したセッションを開く", // Open Selected Sessions
  "tree.archiveSelected": "選択したセッションをアーカイブ", // Archive Selected Sessions
  "tree.moveSelected": "選択項目を移動…", // Move Selected to…
  "tree.deleteSelected": (n) => `選択した ${n} 件を削除`, // Delete {n} Selected Items
  "tree.removeProject": "プロジェクトを削除", // Remove Project
  "tree.deleteGroup": "グループを削除", // Delete Group
  "tree.deleteSession": "セッションを削除", // Delete Session
  "tree.projectRoot": "プロジェクトルート（グループなし）", // Project root (no group)
  "tree.moveToSession": "セッションの下へ移動（子にする）", // Move under a session (as child)
  "tree.moveTo": "移動先…", // Move to…
  "tree.openNewTab": "新しいタブで開く", // Open in New Tab
  "tree.forkSession": "セッションをフォーク", // Fork Session
  "tree.exportSession": "セッションをエクスポート…", // Export Session…
  "tree.sessionInfo": "セッション情報", // Session Info
  "tree.groupInfo": "グループ情報", // Group Info
  "info.branch": "ブランチ", // Branch
  "info.path": "パス", // Path
  "info.recentCommits": "最近のコミット", // Recent Commits
  "info.noCommits": "コミットなし", // No commits
  "tree.killProcess": "プロセスを終了", // Kill Process
  "tree.archiveSession": "セッションをアーカイブ", // Archive Session
  "tree.archiveGroup": "グループをアーカイブ", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "一時", // scratch
  "tree.persistSession": "永続セッションに変換…", // Make Permanent Session…
  "tree.persistDoc": "ディスクに保存…", // Save to Disk…
  "tree.closeScratch": "下書きを閉じる", // Close Scratch
  "tree.importProject": "プロジェクトをインポート", // Import Project
  "tree.createProject": "プロジェクトを作成",
  // New Collection / Collection name / research / Create Collection / No folder / Delete Collection
  "tree.newCollection": "新規コレクション",
  "tree.deleteCollection": "コレクションを削除",
  "collection.title": "新規コレクション",
  "collection.name": "コレクション名",
  "collection.namePlaceholder": "research",
  "collection.submit": "コレクションを作成",
  "collection.tag": "フォルダーなし",
  "collection.deleteTitle": "コレクションを削除",
  "collection.deleteBody": (name) =>
    `コレクション「${name}」を削除しますか？中のグループとセッションもすべて削除され、元に戻せません。`,
  "tree.cloneProject": "Git からクローン", // Clone from Git
  "createProject.title": "プロジェクトを作成",
  "createProject.name": "プロジェクト名",
  "createProject.namePlaceholder": "my-project",
  "createProject.into": "作成先",
  "createProject.choose": "選択…",
  "createProject.noParent": "親フォルダーを選択してください",
  "createProject.invalidName":
    "/ または \\ を含まない単一のフォルダー名を入力してください。",
  "createProject.creating": "作成中…",
  "createProject.submit": "プロジェクトを作成",
  "clone.title": "Git リポジトリをクローン", // Clone Git Repository
  "clone.url": "リポジトリ URL", // Repository URL
  "clone.urlPlaceholder": "https://… または git@…",
  "clone.branch": "ブランチ（任意）", // Branch (optional)
  "clone.branchPlaceholder": "空欄なら既定のブランチ", // Default branch if empty
  "clone.folder": "フォルダ名", // Folder name
  "clone.folderPlaceholder": "URL から自動取得", // Auto from URL
  "clone.into": "クローン先", // Clone into
  "clone.choose": "選択…", // Choose…
  "clone.noParent": "親フォルダを選択してください", // Choose a parent folder
  "clone.cloning": "クローン中…", // Cloning…
  "clone.cancelling": "キャンセル中…",
  "clone.stageStarting": "Git を起動しています…",
  "clone.stageConnecting": "リポジトリに接続しています…",
  "clone.stagePreparing": "オブジェクトを準備しています…",
  "clone.stageReceiving": "オブジェクトを受信しています…",
  "clone.stageResolving": "差分を解決しています…",
  "clone.stageCheckout": "ファイルをチェックアウトしています…",
  "clone.stageFinalizing": "完了処理中…",
  "clone.stageImporting": "プロジェクトをインポートしています…",
  "clone.elapsed": (seconds: number) => `経過 ${seconds} 秒`,
  "clone.slowHint":
    "30 秒間進捗がありません。リモートマシンのネットワークまたはプロキシを確認するか、キャンセルして再試行してください。",
  "clone.submit": "クローン", // Clone
  "tree.globalSearch": "すべてのセッションを検索", // Search All Sessions
  "tree.archivedSessions": "アーカイブ済みセッション", // Archived Sessions
  "tree.searchPlaceholder": "セッション / グループを検索…", // Search sessions / groups…
  "tree.clearSearch": "検索をクリア", // Clear search
  "tree.filterWorking": "作業中", // Working
  "tree.filterAsking": "対応待ち", // Pending
  "tree.filterWaiting": "確認済み", // Viewed
  "tree.filterStatus": "ステータスで絞り込み", // Filter by status
  "tree.refreshStatusFilter": "ステータスフィルターを更新",
  "tree.refreshStatusMatch": "ステータスを更新",
  "tree.filterStatusSection": "ステータス", // Status
  "tree.filterMarkSection": "マーク", // Mark
  "tree.viewMainName": "メイン",
  "tree.viewUntitled": "名称未設定ビュー",
  "tree.viewDefaultName": (n) => `ビュー ${n}`,
  "tree.viewPrimary": "メインビュー",
  "tree.viewManage": "ビューを管理",
  "tree.viewSetPrimary": "メインに設定",
  "tree.viewRename": "ビュー名を変更",
  "tree.viewName": "ビュー名",
  "tree.viewDelete": "ビューを削除",
  "tree.viewDeletePrimary": "メインビューは削除できません",
  "tree.viewDeleteTitle": "ツリービューを削除",
  "tree.viewDeleteConfirm": (name) =>
    `「${name}」を削除しますか？保存された検索とフィルターは削除されますが、プロジェクトとセッションには影響しません。`,
  "tree.viewSplitRight": "ツリービューを右に分割",
  "tree.viewSplitDown": "ツリービューを下に分割",
  "tree.viewAdd": "現在のツリービューを新しいタブにコピー",
  "tree.viewCount": (n) => `${n} 個のツリービュー`,
  "mark.menu": "マーク", // Mark
  "mark.urgent": "緊急", // Urgent
  "mark.important": "重要", // Important
  "mark.bug": "バグ", // Bug
  "mark.done": "完了", // Done
  "mark.wip": "進行中", // In progress
  "mark.pinned": "ピン留め", // Pinned
  "mark.idea": "アイデア", // Idea
  "mark.caution": "注意", // Caution
  "tree.clearAllNotifications":
    "通知バッジをすべてクリア（セッションのドットと Dock バッジ）", // Clear all notification badges…
  "tree.noProjectsPre":
    "プロジェクトがまだありません。フォルダーアイコンをクリック、または ", // No projects yet. Click the folder button, or press
  "tree.noProjectsPost": " でディレクトリをインポートしてください。", // to import a directory.
  "tree.openProject": "プロジェクトを開く", // Open Project
  "tree.noAttention": "ステータス絞り込みに一致するセッションはありません", // No sessions match the status filter
  "tree.noMatch": "一致なし", // No matches

  // Dialog fields
  "tree.groupName": "グループ名", // Group name
  "tree.sessionNameAuto": "セッション名（空欄で自動命名）", // Session name (leave empty to auto-name)
  "tree.editSession": "セッションを編集", // Edit Session
  "tree.sessionName": "セッション名", // Session name
  "tree.shellLabel": "シェル（空欄でシステム既定）", // Shell (leave empty for system default)
  "tree.shellMenu": "シェル", // Shell
  "tree.downloadFullGitbash": "完全版 Git Bash をダウンロード",
  "gitbash.title": "Git Bash",
  "gitbash.downloading": "完全版 Git Bash をダウンロード中…",
  "gitbash.extracting": "完全版 Git Bash を展開中…",
  "gitbash.done": "完全版 Git Bash の準備ができました。",
  "gitbash.failed": "Git Bash のダウンロードに失敗しました",
  "tree.shellSystemDefault": "システム既定", // System default
  "form.customOption": "カスタム…", // Custom…
  "tree.cwdLabel": "作業ディレクトリ（空欄でプロジェクトルート）", // Working directory (leave empty for project root)
  "tree.initCmdLabel": "起動コマンド（任意）", // Startup command (optional)
  "tree.agentArgsLabel": "起動引数（任意）", // Launch args (optional)
  // Working directory / Leave empty for the default
  "tree.workingDirLabel": "作業ディレクトリ",
  "tree.workingDirPlaceholder": "空欄なら既定のディレクトリ",
  "preset.execPathLabel": "実行ファイル（任意）",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint":
    "空欄ならエージェントに設定済みのコマンドを使います。指定するとこのセッションだけが互換の別プログラムで動きます。",
  "preset.saveLabel": "プリセットとして保存",
  "preset.namePlaceholder": "プリセット名",
  "preset.iconChoose": "アイコンを選択",
  "preset.iconClear": "削除",
  "preset.iconHint":
    "正方形の画像が最適です。それ以外は切り取って 64x64 に縮小します。",
  "tree.permissionSkipLabel": "すべての権限確認をスキップ", // Skip all permission confirmations
  "tree.permissionSkipHint":
    "起動時にこのエージェントのバイパス用フラグを付与します（例: Claude の --dangerously-skip-permissions。Codex はサンドボックスも無効化）。毎回の起動で有効になるため、慎重に使用してください。",
  "tree.permissionUnsupported":
    "OpenCode は設定ファイルで権限を管理し、対応する起動フラグがないため、この項目は適用されません。",
  "tree.permissionUnsupportedPi":
    "Pi は設計上、権限確認のプロンプトなしでツールを実行するため、この項目は適用されません。",

  // New agent-session dialog
  "newAgent.desc":
    "セッション名とカスタム起動引数（agent コマンドに渡されます。例: --model opus）は任意です。両方空のまま Enter を押すと通常どおり起動します。", // Optionally name the session and add custom launch args…

  // Delete confirmation
  "tree.batchDeleteTitle": "一括削除", // Batch Delete
  "tree.deleteProjectTitle": "プロジェクトを削除", // Delete Project
  "tree.deleteGroupTitle": "グループを削除", // Delete Group
  "tree.deleteSessionTitle": "セッションを削除", // Delete Session
  "tree.batchDeleteBody": (n) =>
    `選択した ${n} 件を削除します（プロジェクト/グループは配下のサブグループとセッションも連鎖削除されます）。この操作は取り消せません。`, // Delete the {n} selected items…
  "tree.deleteProjectBody": (name) =>
    `プロジェクト「${name}」を削除しますか？配下のサブグループとセッションもすべて削除されます。この操作は取り消せません。`, // Delete project "{name}"?…
  "tree.deleteGroupBody": (name) =>
    `グループ「${name}」を削除しますか？配下のサブグループとセッションもすべて削除されます。この操作は取り消せません。`, // Delete group "{name}"?…
  "tree.deleteSessionBody": (name) =>
    `セッション「${name}」（およびすべての子セッション）を削除しますか？この操作は取り消せません。`, // Delete session "{name}"…
  "tree.deleteWorktrees": (n) =>
    `関連する git worktree も削除する（計 ${n} 件。作業ツリーに変更があると削除に失敗することがあります）`, // Also remove associated git worktrees…

  // Session information dialog
  "info.name": "名前", // Name
  "info.type": "種類", // Type
  "info.status": "状態", // Status
  "info.notYetCaptured": "未生成（初回実行後に取得）", // Not yet generated (captured after first run)
  "info.sessionId": "セッション ID", // Session ID
  "info.cwd": "作業ディレクトリ", // Working dir
  "info.initCmd": "起動コマンド", // Startup cmd
  "info.agentArgs": "起動引数", // Launch args
  "info.launchCmd": "完全な起動コマンド", // Full launch command
  "info.permission": "権限", // Permission
  "info.permissionSkip": "すべての確認をスキップ", // Skip all confirmations
  "info.parentSessionId": "親セッション ID", // Parent ID
  "info.termTitle": "ターミナルタイトル", // Terminal title
  "info.createdAt": "作成日時", // Created at

  // Resume-session dialog
  "resume.title": "セッションを再開", // Resume Session
  "resume.desc":
    "エージェントの種類を選び、そのエージェント自身の session id を入力します。開くと元の会話を引き継ぎます。", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "エージェント種別", // Agent type
  "resume.sessionIdPlaceholder": "会話の session id", // Conversation session id
  "resume.confirm": "再開して開く", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "新規 worktree セッション…", // New Worktree Session…
  "worktree.worktreeNameLabel": "worktree 名", // Worktree name
  "worktree.worktreeNameHint":
    "worktree のディレクトリ名とブランチ名に使われます。", // Used as the worktree directory and branch name.
  "worktree.createFailed": "worktree の作成に失敗しました", // Couldn't create the worktree
  "worktree.noRepoRoot":
    "このプロジェクトには利用可能な git リポジトリのパスがありません。", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "なし", // None
  "worktreeSel.modeNew": "新規", // New
  "worktreeSel.modeExisting": "既存", // Existing
  "worktreeSel.loading": "worktree を読み込み中…", // Loading worktrees…
  "worktreeSel.empty": "このリポジトリには既存の worktree がありません。", // No existing worktrees in this repository.
  "worktreeSel.loadFailed":
    "worktree を一覧できませんでした（git リポジトリではない？）。", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint":
    "このグループで作成したセッションは既定でこの worktree を使用します。", // Sessions created in this group will use this worktree by default.
  "worktree.moveGroupTitle": "グループを Worktree に移動",
  "worktree.moveGroupHint":
    "以後このグループで作成するセッションはこの worktree を使います。既存のセッションは今のディレクトリのままです。",

  // ── Archive panel ──
  "archive.title": "アーカイブ済みセッション", // Archived Sessions
  "archive.empty1": "アーカイブ済みセッションはありません。", // No archived sessions.
  "archive.empty2":
    "サイドバーのセッションを右クリックして「セッションをアーカイブ」を選ぶとここに収納されます。", // Right-click a session in the sidebar…
  "archive.restore": "通常のセッションに戻す", // Restore to normal session
  "archive.export": "完全なコンテキストを Markdown でエクスポート", // Export full context as Markdown
  "archive.deleteForever": "完全に削除（録画も含む）", // Delete permanently (with recording)
  "archive.pickOne": "左側のアーカイブ済みセッションを選んで会話履歴を表示", // Select an archived session on the left…
  "archive.recordingEnd": "--- 録画終了 ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) => `録画の読み込みに失敗: ${err}`, // Failed to read recording: {err}
  "archive.searchRecording": "録画内を検索…", // Search in recording…
  "archive.searchTranscript": "会話内容を検索…", // Search transcript…
  "archive.searchPlaceholder": "アーカイブ内容を検索…", // Search archived content…
  "archive.msgCountAll": (n) => `${n} 件`, // {n} messages
  "archive.msgCountFiltered": (shown, total) => `${shown} / ${total} 件`, // {shown} / {total} messages
  "archive.you": "あなた", // You
  "archive.toolsUsed": (tools) => `ツール: ${tools}`, // Tools: {tools}
  "archive.noMatch": "一致するメッセージはありません", // No matching messages
  "archive.emptyTranscript": "会話履歴は空です", // Transcript is empty
  "archive.loadingTranscript": "会話履歴を読み込み中…", // Loading transcript…

  // ── Global session-content search ──
  "search.allPlaceholder": "すべてのセッション内容を検索…", // Search across all session content…
  "search.hint":
    "セッション内容を検索します。アーカイブ済みは既定で除外され、「アーカイブも検索」で追加できます。", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "アーカイブも検索", // Include archived
  "search.includeArchivedHint":
    "アーカイブ済みセッションも検索対象に含める（既定ではオフ）", // Also search archived sessions (off by default)
  "search.searching": "検索中…", // Searching…
  "search.noResults": "一致する項目が見つかりません", // No matches found
  "search.sessionCount": (n) => `${n} 件のセッション`, // n sessions
  "search.matchCount": (n) => `${n} 件の一致`, // n matches
  "search.pickSession": "左側のセッションを選択すると一致箇所が表示されます", // Select a session on the left to see its matches
  "search.openSession": "セッションを開く", // Open session
  "search.backToResults": "結果に戻る", // Back to results
  "search.archivedBadge": "アーカイブ済み", // Archived
  "search.summary": (m, s) => `${m} 件の一致 · ${s} 件のセッション`, // X matches · N sessions
  "search.matchPosition": (n, total) => `${n} / ${total}`, // N of M
  "search.roleTerminal": "ターミナル", // Terminal
  "search.collapseGroup": "折りたたむ", // Collapse
  "search.expandGroup": "展開", // Expand
  "search.cappedNote": (l, total) => `${total} 件中 ${l} 件に移動可能`, // L of total locatable

  // ── Center pane ──
  "center.noSession": "セッションなし", // No session
  "center.noSessionHintPre": "サイドバーからセッションを選ぶか、", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": " でターミナルを作成", // to create a terminal
  "center.createTerminal": "ターミナルを作成", // Create Terminal
  "tab.unsavedDot": "未保存の変更あり", // Unsaved changes
  "tab.newTerminal": "新規ターミナル", // New terminal
  "tab.newDocument": "新規ドキュメント", // New document
  "tab.bgTitle": (n) => `バックグラウンド常駐タブ: ${n} 件（プロセスは実行中）`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `バックグラウンド ${n}`, // Background {n}
  "tab.scratchFallback": "（一時ターミナル）", // (scratch terminal)
  "tab.killBgTab": "このバックグラウンドタブを終了（プロセスも終了します）", // Kill this background tab…
  "tab.newBrowserTab": "新しいタブ", // New Tab
  "tab.refreshFile": "ファイルを再読み込み", // Refresh File
  "tab.closeOthers": "他のタブを閉じる", // Close Other Tabs
  "tab.closeRight": "右側のタブを閉じる", // Close Tabs to the Right
  "tab.closeAll": "すべてのタブを閉じる", // Close All Tabs
  "tab.sendToBackground": "バックグラウンドに移動", // Send to Background

  // ── Built-in browser ──
  "browser.back": "戻る", // Back
  "browser.forward": "進む", // Forward
  "browser.reload": "再読み込み", // Reload
  "browser.desktopOnly": "ブラウザタブはデスクトップアプリでのみ開けます。", // Browser tabs open in the desktop app only.
  "browser.stop": "読み込みを中止", // Stop loading
  "browser.openExternal": "システムのブラウザで開く", // Open in system browser
  "browser.addressPlaceholder": "URL または検索語を入力", // Enter URL or search terms
  "browser.quickAccess": "クイックアクセス", // Quick access
  "browser.loading": "読み込み中…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "VelaTerm を終了しますか？", // Quit VelaTerm?
  "quit.body":
    "実行中のターミナルとエージェントのセッションはすべて停止します。", // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "ワークスペースを保存", // Save workspace
  "quit.saveWorkspaceHint":
    "次回、同じタブと分割を復元します。ターミナルは復元されますが再起動はされません。", // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "終了", // Quit
  "dormant.body":
    "保存したワークスペースから復元しました。プロセスはまだ起動していません。", // Restored from your saved workspace. No process is running yet.
  "dormant.start": "起動", // Start
  "overlimit.title": (max) =>
    `バックグラウンド常駐が上限を超えています（最大 ${max} 件）`, // Background keep-alive over limit ({max})
  "overlimit.body":
    "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
  "overlimit.kill": "End Selected", // End Selected
  "overlimit.keep": "Keep for Now", // Keep for Now
  "overlimit.earliest": "earliest", // earliest
  "overlimit.statusWorking": "working", // working
  "overlimit.statusAsking": "awaiting reply", // awaiting reply
  "overlimit.statusWaiting": "waiting", // waiting

  // ── Terminal pane ──
  "term.paste": "貼り付け", // Paste
  "term.pasteUseShortcut": "貼り付け（⌘V を押してください）", // Paste (press ⌘V)
  "term.selectAll": "すべて選択", // Select All
  "term.autoCopied": (n: number) => `${n} 文字を自動コピー · ⌘V で貼り付け`,
  "term.clear": "画面をクリア", // Clear
  "term.searchMenu": "検索…", // Search…  ⌘F
  "term.splitRight": "右に分割", // Split right (⌘D)
  "term.splitDown": "下に分割", // Split down (⌘⇧D)
  "term.closePane": "分割を閉じる", // Close split
  "term.redraw": "再描画", // Redraw
  "term.mirrorTooltip":
    "ミラー表示中（サイズは他のクライアントが制御）。クリックすると PTY をこのウィンドウのサイズに合わせます", // Mirroring (size controlled by another client)…
  "term.mirrorBadge": (dims) =>
    `⤢ ミラー${dims} · クリックでこのウィンドウに合わせる`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) =>
    `⤢ ミラー${dims} · このウィンドウに合わせる`, // ⤢ Mirror{dims} · fit this window
  "term.imgUploadFailed": (n, lastError) =>
    `画像のアップロードに失敗: ${n} 件${lastError ? `（${lastError}）` : ""}`, // Image upload failed for {n} images…
  "term.imgClipboardUnavailable":
    "クリップボードから画像を読み取れませんでした。画像をコピーし直して再試行してください。",
  "term.starting": (agent) => `${agent} を起動中…`, // Starting {agent}…
  "term.startFailed": (err) => `起動に失敗: ${err}`, // Failed to start: {err}

  // ── Agent installation guidance ──
  "agentInstall.title": (label) => `${label} がインストールされていません`, // {label} is not installed
  "agentInstall.desc": (label) =>
    `VelaTerm は PATH 上で ${label} を見つけられませんでした。インストールするとこのセッションを起動できます。`, // couldn't find {label} on PATH
  "agentInstall.install": "今すぐインストール", // Install now
  "agentInstall.retry": "起動を再試行", // Retry launch
  "agentInstall.dismiss": "自分でインストールする", // I'll do it myself
  "agentInstall.docs": "インストール手順", // Install docs
  "agentInstall.needsNode": "Node.js / npm が必要です", // Requires Node.js / npm
  "agentInstall.afterInstall": "インストール後：", // After install:
  "agentInstall.pathSaved": (label: string) =>
    `${label} の実行ファイルパスを設定に保存しました:`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) =>
    `${label} はインストール済みです`, // {label} is installed
  "agentInstall.doneDesc": "このセッションを再起動すると使い始められます。", // Relaunch this session to start using it.
  "agentInstall.restartNow": "今すぐ再起動", // Relaunch now
  "agentInstall.later": "後で", // Later
  "search.placeholder": "ターミナル内を検索", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "WYSIWYG", // WYSIWYG
  "doc.source": "ソース", // Source
  "doc.searchPlaceholder": "検索", // Find
  "doc.searchReplacePlaceholder": "置換", // Replace
  "doc.searchReplace": "置換", // Replace
  "doc.searchReplaceAll": "すべて", // All
  "doc.searchNoMatch": "一致なし", // No results
  "doc.searchCaseSensitive": "大文字小文字を区別", // Match case
  "doc.searchToggleReplace": "置換を切り替え", // Toggle replace
  "doc.fileTree": "ファイルツリー", // File tree
  "doc.treeUp": "親フォルダ", // Parent folder
  "doc.sidebar": "サイドバー", // Sidebar
  "doc.unsaved": "未保存", // Unsaved
  "doc.saveAsTitle": "名前を付けて保存", // Save As
  "doc.saveAsName": "ファイル名", // File name
  "doc.outline": "アウトライン", // Outline
  "doc.outlineEmpty": "見出しなし", // No headings
  "doc.saving": "保存中…", // Saving…
  "doc.overwriteConfirm":
    "同名のファイルが既に存在します。「上書き」で置き換えます。", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "保存", // Save
  "doc.externalChanged":
    "ファイルがディスク上で変更されました（未保存のローカル変更があります）。", // The file was modified on disk…
  "doc.reloadDiscard": "再読み込み（自分の変更を破棄）", // Reload (discard my changes)
  "doc.externalChangedClean": "ファイルがディスク上で変更されました。", // The file was modified on disk.
  "doc.reload": "再読み込み", // Reload
  "doc.ignore": "無視", // Ignore
  "doc.loadingFile": (title) => `${title} を読み込み中…`, // Loading {title}…
  "doc.closeTitle": "ドキュメントを閉じる", // Close Document
  "doc.unsavedBody": (title) => `「${title}」には未保存の変更があります。`, // "{title}" has unsaved changes.
  "doc.saveAndClose": "保存して閉じる", // Save & Close
  "doc.closeNoSave": "保存せずに閉じる", // Close Without Saving
  "doc.conflictTitle": "保存の競合", // Save Conflict
  "doc.conflictBody":
    "ディスク上のファイルが外部で変更されています。それでも現在の内容で上書きしますか？", // The file on disk was modified externally…
  "doc.overwrite": "上書き", // Overwrite
  "doc.saveFailed": (err) => `保存に失敗: ${err}`, // Save failed: {err}
  "doc.closeTab": "タブを閉じる", // Close Tab
  "doc.truncatedReadonly": (size: string) =>
    `読み取り専用：先頭 10 MB のみ表示（全 ${size}）。ファイルの残りを上書きしないよう保存は無効です。`,
  "doc.imgLoading": (title, size) => `${title}（${size}）を読み込み中…`, // Loading {title} ({size})…
  "doc.imgBeingWritten":
    "ファイルは書き込み中です。安定したら自動で再読み込みします。", // The file is being written; it will reload automatically once it settles.
  "doc.imgDecodeFailed":
    "この画像を表示できません（未対応の形式か破損しています）。", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "ウィンドウに合わせる", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "PDFとして書き出す", // Export PDF
  "doc.diagramError": "図の構文エラー", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "セッション未選択", // No session selected
  "panel.openInEditor": "エディタで開く", // Open in Editor
  "panel.openInEditorTooltip":
    "中央のドキュメントエディタで開く（view コマンドと同じ）", // Open in the document editor…
  "panel.preview": "プレビュー", // Preview
  "panel.cantRead": "（このファイルを読み取れません）", // (cannot read this file)
  "panel.binary": "（バイナリファイルのためプレビューなし）", // (binary file, no preview)
  "panel.truncated": "\n…（長すぎるため省略）", // …(content truncated)
  "panel.showHidden": "隠しファイルを表示", // Show hidden files
  "panel.hideHidden": "隠しファイルを非表示", // Hide hidden files

  // ── File-tree actions (Files context menu and header add button) ──
  "files.newFile": "新規ファイル", // New File
  "files.newFolder": "新規フォルダ", // New Folder
  "files.nameLabel": "名前", // Name
  "files.newTooltip": "新規ファイル / フォルダ", // New file or folder
  "files.openInTerminal": "Open in Terminal",
  "files.revealInFinder": "Show in File Manager",
  "files.copyPath": "Copy Path",
  "files.copyRelPath": "Copy Relative Path",
  "files.filterPlaceholder": "Filter files…",
  "files.dblClickOpen": "ダブルクリックで開く",
  "files.deleteConfirm": (name) =>
    `「${name}」を削除しますか？この操作は元に戻せません。`, // Delete "{name}"? This can't be undone.

  // ── File transfer (remote access) ──
  "transfer.uploadsTitle": "アップロード", // Uploads
  "transfer.download": "ダウンロード", // Download
  "transfer.upload": "ファイルをアップロード…", // Upload Files…
  "transfer.uploadTooltip": "このフォルダーにファイルをアップロード", // Upload files to this folder
  "transfer.clear": "クリア", // Clear
  "transfer.cancelled": "キャンセル済み", // Cancelled
  "transfer.failed": "失敗", // Failed
  "transfer.stalled": "再接続中…", // Reconnecting…
  "transfer.foldersUnsupported": "フォルダーはアップロードできません。", // Folders can't be uploaded.

  // ── Status bar ──
  "statusbar.sessions": (n) => `${n} セッション`, // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `クリックでサイドバーを「${label}」のみに絞り込み（再クリックで解除）`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `バックグラウンド ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `バックグラウンド常駐タブ数（上限 ${max}。超過時は最も古い非アクティブなタブを自動終了）`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) =>
    `バックグラウンドタブを終了しました: ${name}（上限超過）`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `ブラウザリモートアクセス有効: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "権限：確認", // Perms: Ask
  "statusbar.permSkip": "権限：スキップ", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip":
    "このセッションの権限モード · クリックで切り替え（このセッションのみ）", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "このセッションの権限", // This session's permissions
  "statusbar.permOptAsk": "毎回確認（既定）", // Ask each time (default)
  "statusbar.permScopeHint":
    "現在のセッションにのみ適用されます。全体の既定値は「設定 ▸ エージェント」で調整できます。", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg":
    "権限を変更しました。反映にはこのセッションの再起動が必要です。再起動すると現在の会話は継続されますが、進行中のタスクは中断されます。今すぐ再起動しますか？", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "今すぐ再起動", // Restart now
  "statusbar.permRestartLater": "後で", // Later
  "statusbar.permScopeTitle": "適用先は？", // Apply to?
  "statusbar.permScopeSession": "このセッションのみ", // This session only
  "statusbar.permScopeGlobal": "グローバル既定", // Global default
  "statusbar.permScopeGlobalHint":
    "このセッションに即時適用し、今後新規作成する同種セッションの既定値になります（設定と同期）。", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

  // ── Store, notifications, and export ──
  "notify.working": "⏳ 処理中…", // ⏳ Working…
  "notify.asking": "❓ 確認が必要です", // ❓ Needs your confirmation
  "notify.waiting": "✅ 返信済み", // ✅ Replied
  "store.subtask": "サブタスク", // Subtask
  "store.splitPane": "分割", // Split
  "export.failedTitle": "セッションのエクスポートに失敗", // Failed to export session
  "export.contextSuffix": "コンテキスト", // context

  // ── Error panel ──
  "err.renderTitle": "レンダリングエラー", // Rendering Error
  "err.renderDesc":
    "予期しないエラーが発生しました。以下の情報が問題の特定に役立ちます。", // An unexpected error occurred…
  "err.reload": "再読み込み", // Reload
  "err.uncaughtTitle": "未捕捉のエラーが発生", // Uncaught Error
  "err.uncaughtDesc": "以下の情報が問題の特定に役立ちます。", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser":
    "ブラウザでは録画再生はまだサポートされていません", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `画像のアップロードに失敗 (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.connecting": "接続中…", // Connecting…
  "login.remoteAccess": "リモートアクセス", // Remote Access
  "login.desc":
    "このターミナルに接続するにはアクセスパスワードを入力してください。", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "アクセスパスワード", // Access password
  "login.connect": "接続", // Connect
  "login.wrongPassword": "パスワードが違います", // Wrong password
  "login.rateLimited":
    "試行回数が多すぎます。1分ほど待ってから再度お試しください。", // Too many attempts. Please wait a minute and try again.
  "login.failed": "ログインに失敗しました。もう一度お試しください", // Login failed, please try again
  "login.pairingRequired":
    "このサーバーはペアリングリンクが必要です。デスクトップアプリの「リモートアクセス」で生成したリンクを開いてください。", // This server requires a pairing link
  "login.authFailed":
    "認証に失敗しました。アクセスパスワードを確認してください。リンクを再生成した場合は新しいペアリングリンクを使用してください。", // Authentication failed, check password or use a new pairing link
  "dir.title": "プロジェクトディレクトリを選択", // Choose Project Directory
  "dir.pathPlaceholder":
    "検索、またはパスを入力して Enter で移動（~ 始まりに対応）", // Search, or type a path and press Enter (supports ~)
  "dir.up": "一つ上へ", // Up one level
  "dir.newFolder": "新しいフォルダ", // New Folder
  "dir.newFolderPlaceholder": "フォルダ名", // Folder name
  "dir.goInput": "入力したパスへ移動", // Go to typed path
  "dir.noSubdirs": "（サブディレクトリなし）", // (no subdirectories)
  "dir.empty": "（空のフォルダ）", // (empty folder)
  "dir.noMatch": "一致する項目がありません", // No matching items
  "dir.target": "対象フォルダ", // Target
  "dir.showHidden": "隠しファイルを表示", // Show hidden items
  "dir.importing": "インポート中…", // Importing…
  "dir.choose": "このディレクトリを選択", // Choose This Directory
  "conn.reconnecting": "接続が切断されました。再接続しています…", // Connection lost, reconnecting…
  "conn.reconnectNow": "今すぐ再接続", // Reconnect now
  "conn.retrying": "再接続しています…", // Reconnecting…
  "conn.sshReconnecting":
    "SSH接続が切断されました。トンネルを再構築しています…", // SSH link lost, rebuilding the tunnel…
  "conn.sshDown":
    "SSH接続が切断されました。「今すぐ再接続」を押して再試行してください", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "リクエストに失敗しました", // Request failed
  "reqerr.dismiss": "閉じる", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "エラーログ", // Error Log
  "errlog.empty": "記録されたエラーはありません。", // No errors recorded.
  "errlog.copyAll": "すべてコピー", // Copy all
  "errlog.clear": "クリア", // Clear
  "errlog.close": "閉じる", // Close

  // ── Mobile ──
  "mobile.toDesktop": "デスクトップ版に切り替え", // Switch to desktop
  "mobile.empty1": "セッションがありません。", // No sessions.
  "mobile.noMatch": "一致するセッションがありません", // No matching sessions
  "mobile.empty2":
    "デスクトップ版か PC ブラウザで作成すると、ここに自動で表示されます。", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ 戻る", // ‹ Back
  "mobile.selCopy": "コピー", // Copy
  "mobile.selCancel": "キャンセル", // Cancel

  // ── Other shared components ──
  "splitter.dragToResize": "ドラッグでサイズを調整", // Drag to resize
  "transport.wsDisconnected": "WebSocket が切断されました", // WebSocket disconnected
  "transport.wsConnectFailed": "WebSocket 接続に失敗しました", // WebSocket connection failed
  "transport.cmdFailed": "コマンドが失敗しました", // Command failed
  "transport.remoteCmdForbidden": (cmd: string) =>
    `このコマンドはリモートクライアントでは利用できません: ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) =>
    `この設定キーはリモートクライアントからは書き込めません: ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) =>
    `リモートクライアントはアプリのデータディレクトリ内のファイルにアクセスできません: ${path}`, // Remote clients cannot access files in the app data directory

  // ── Crepe (built-in WYSIWYG editor UI) ──
  "crepe.placeholder": "本文を入力するか、/ で挿入メニューを開きます", // Type text, or press / for the insert menu
  "crepe.textGroup": "テキスト", // Text
  "crepe.paragraph": "本文", // Text
  "crepe.h1": "見出し 1", // Heading 1
  "crepe.h2": "見出し 2", // Heading 2
  "crepe.h3": "見出し 3", // Heading 3
  "crepe.h4": "見出し 4", // Heading 4
  "crepe.h5": "見出し 5", // Heading 5
  "crepe.h6": "見出し 6", // Heading 6
  "crepe.quote": "引用", // Quote
  "crepe.divider": "区切り線", // Divider
  "crepe.listGroup": "リスト", // List
  "crepe.bulletList": "箇条書き", // Bullet List
  "crepe.orderedList": "番号付きリスト", // Ordered List
  "crepe.taskList": "タスクリスト", // Task List
  "crepe.advancedGroup": "挿入", // Insert
  "crepe.image": "画像", // Image
  "crepe.codeBlock": "コードブロック", // Code Block
  "crepe.table": "表", // Table
  "crepe.math": "数式", // Math
  "crepe.linkPlaceholder": "リンクを貼り付けまたは入力…", // Paste or type a link…
  "crepe.upload": "アップロード", // Upload
  "crepe.uploadImage": "画像をアップロード", // Upload Image
  "crepe.orPasteImageLink": "または画像リンクを貼り付け", // or paste an image link
  "crepe.imageCaption": "画像の説明", // Image caption
  "crepe.confirm": "確認", // Confirm
  "crepe.searchLanguage": "言語を検索", // Search language
  "crepe.noResult": "一致なし", // No results
  "crepe.edit": "編集", // Edit
  "crepe.collapse": "折りたたむ", // Collapse
  // ── Additional right and bottom bar entries ──
  "info.project": "プロジェクト", // Project
  "panel.sessionInfo": "セッション情報", // Session info
  "panel.gitTitle": "Git ステータス", // Git status
  "panel.gitProbing": "確認中…", // Checking…
  "panel.gitNotRepo": "Git リポジトリではありません", // Not a Git repository
  "panel.gitBranch": "ブランチ", // Branch
  "panel.gitStaged": "ステージ済み", // Staged
  "panel.gitUnstaged": "変更", // Changed
  "panel.gitUntracked": "未追跡", // Untracked
  "bottombar.running": "実行中", // Running
  "bottombar.collapseTasks": "タスク欄を閉じる", // Collapse tasks
  "bottombar.expandTasks": "タスク欄を開く", // Expand tasks
  "bottombar.sound": "🔔 サウンド", // 🔔 Sound
  "bottombar.muted": "🔕 ミュート", // 🔕 Muted
  "bottombar.overview": "セッション概要", // Sessions overview
  "bottombar.noSessions": "セッションがありません", // No sessions
  "doc.pdfFilter": "PDF ファイル", // PDF file
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
  "updater.hideHint":
    "Keep downloading in the background. Progress stays in the status bar.", // TODO translate
  "updater.downloadManually": "Download manually", // TODO translate
  "updater.downloadManuallyHint": "Open the download page in your browser.", // TODO translate
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

export default ja;
