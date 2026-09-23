# 网站模型目录同步

> 自 2026-09-22 起，本页描述的网站目录（不可用时为随客户端提供的目录）是 Claude 模型列表的基础，本机安装的 Claude Code CLI 报告的模型合并其中：CLI 条目更新对应模型的能力，目录尚未列出的模型追加到末尾，目录中的其他模型仍可选择。详见[Claude 模型目录：网站目录与本机 CLI 合并](claude-model-catalog_20260922.md)。

Claude 完整目录由 velaterm.com 后端维护。客户端默认读取 `https://velaterm.com/api/model-catalog/claude`，作为模型列表的基础，合并本机 CLI 报告的模型并追加用户自定义模型；设置了 Claude Code 的 `availableModels` 时，菜单按其过滤。目录中的模型元数据不代表当前账号一定拥有调用权限。

客户端启动时恢复本机数据库缓存并检查网站，此后每六小时检查一次；失败后每五分钟重试。菜单支持手动刷新，并显示网站目录、缓存目录或随客户端提供的初始目录，以及目录版本和检查时间。网络或目录校验失败时保留上次有效目录。桌面与浏览器共用同一后端缓存，更新事件同步到所有连接。

Codex 继续使用其 app-server 的 `model/list` 获取当前账号目录，不用网站静态目录覆盖这一结果。

## 网站维护

网站仓库为 `vlx-term-server`。初始目录位于 `backend/src/main/resources/catalogs/claude.json`，包含14项；首次部署后可以独立更新目录，无须再次发布客户端或重启网站。

- 公开读取：`GET /api/model-catalog/claude`，支持 ETag 和 304。
- 管理更新：`POST /api/admin/model-catalog/claude`，使用既有 `ADMIN_TOKEN` 的 Bearer 认证，提交完整目录 JSON。
- 持久文件：`MODEL_CATALOG_PATH`，默认 `data/claude-models.json`；生产环境必须指向持久卷，并纳入备份。

目录字段为 `schemaVersion: 1`、正整数 `revision` 和非空 `models`。每项包含唯一 `id`、`label`、`description`、可空的 `contextWindow`、`effortLevels`，以及可选的 `minVersion` 和 `supportsFastMode`。`minVersion` 为三段数字形式的 Claude CLI 最低版本。完整请求最大1 MiB，最多1000项。

发布时读取当前目录，保留需继续提供的模型，修改条目并增加 revision，再提交完整文件。服务端校验成功后原子替换文件；旧版本号、重复 ID 和非法字段均被拒绝。回滚也必须使用更大的 revision，模型内容可恢复为之前备份。禁止同一 revision 发布不同内容，客户端会拒绝此类响应。

初次上线需要部署网站接口和新版客户端。此后仅更新目录即可让已升级客户端收到新增模型。本次本地验证使用的 `test-new-model` 仅存在于测试目录，不属于发布资源。

## 本地诊断

`VLX_MODEL_CATALOG_URL` 可覆盖下载地址，仅允许 HTTPS 或本机回环 HTTP。客户端 RPC `model_catalog_status` 返回来源、revision、checkedAt、error 和 refreshing；`model_catalog_refresh` 触发检查。缓存保存在应用数据库 `app_settings` 的 `model-catalog.claude.v1` 项。

同步日志位于应用数据目录的 `logs/runtime-*.log`，可用 `VLX_LOG_DIR` 覆盖目录，`VLX_MODEL_CATALOG_LOG_LEVEL` 支持 INFO、WARN、ERROR、OFF。日志不含认证信息或完整目录内容。

本地联调端口固定记录在 `.dev-data/model-catalog-qa/ports.json`。测试用网站进程使用真实控制器、目录存储和鉴权，排除了与目录无关的数据库服务。真实菜单验证通过 vlx-browser 的“本机自动化测试”Profile 执行。

通用日志目录、级别优先级、轮转和隐私边界见[运行日志与隐私保护](runtime-diagnostics_20260909.md)。
