# AGENTS.md — kimi-bg-tool（Kimi Code 背景更换工具）

给 Kimi Code 桌面应用（Electron）换自定义壁纸的 Windows GUI 小工具。Rust 单文件实现，选图/裁剪/调透明度后把图片压成 base64 JPEG 内嵌进主样式表补丁，支持热更新免重启。

## 构建与测试

```bash
taskkill //F //IM kimi-bg-tool.exe 2>/dev/null; true   # 用户常开着旧 exe，会锁文件报 os error 5
cargo build --release    # 必须零 error 零 warning
cargo test --release     # 全部测试必须通过
```

- 产物：`target\release\kimi-bg-tool.exe`（双击即用，免安装）
- 依赖镜像：`.cargo\config.toml` 已配 rsproxy，不要动
- 技术栈：eframe/egui 0.31.1 + image 0.25 + rfd + winreg + regex + anyhow + base64。**不引入新 crate** 除非确实必要
- 代码与日志文本用中文；`src\main.rs` 单文件，按注释分节（数据结构 / 安装路径检测 / 补丁 / 热更新 / 图片处理 / UI / 测试）

## 架构与硬约束（改动前必读）

### 补丁段
- 写入 `<安装根>\resources\desktop-dist\assets\main-*.css` 末尾，以 `/* === kimi-wallpaper-patch === */` 开头、`kimi-wallpaper-patch end === */` 结尾，`strip_patch` 靠这两个标记幂等移除
- 首次写入前备份为 `主样式表名.wallpaper-bak`；「还原原版」只能从备份恢复，**永远不要无备份覆盖**
- 主样式表文件名带 hash（如 `main-c0uVXxKo.css`），由 `locate_main_css` 从 index.html 里解析，**不要硬编码文件名**
- 透明化名单（`.app/.con/body/#app` 全透明、`.side/.windows-titlebar` 侧边栏透明度、`.global-preview/.agent-panel/.file-preview/.fp-body/.ui-panel-header/.chat-header/.topbar` 面板透明度、`.chat-dock:before`）是反复用 CDP 探针排出来的，dark/light/system 三套镜像必须同步改

### 热更新
- 探针块（`kimi-wallpaper-hot-hook`）插在 `index.html` 最后一个 `</body>` 前；index.html 也有 `.wallpaper-bak` 备份
- 热更文件：`kimi-wallpaper-hot.css`（=补丁全文）+ `kimi-wallpaper-hot.json`（`{"v":N}`）。**必须先写 css 后写 json**，版本号 `max(旧+1, unix毫秒)`，探针以 json 为准
- 探针 fetch **必须用绝对路径** `/kimi-wallpaper-hot.*`——相对路径在 `/sessions/<id>` 路由下会 404 静默失效（踩过的坑）
- Kimi Code 应用更新会冲掉 index.html（探针）和 main-*.css（补丁），热更文件可能幸存但成孤儿。更新后的恢复流程见 `T:\KCD BackGrond\热更新探针-档案与恢复指南.md`

### 图片管线
- `process_image(path, crop)`：load_from_memory → 可选 crop_imm 裁剪（归一化 CropRect 转像素，round+clamp，宽高≥1）→ 最长边压到 2560 → JPEG（先 q82，超 900KB 降 q70）→ base64
- `MAX_JPEG_BYTES = 900KB`：base64 内嵌进 CSS，太大样式表会膨胀
- 裁剪选区 `CropRect` 是归一化坐标（0..1），不落盘，仅 GUI 会话内有效

### GUI
- egui 无 CJK 字体，`install_cjk_font` 从 `C:\Windows\Fonts` 加载微软雅黑，别删
- 裁剪编辑器（`open_crop_editor`/`show_crop_editor`）：红框拖拽，角命中 ≤14px 存进 `editor.drag`，一次拖拽期间不重判；`centered_max_crop`/`clamp_crop` 是纯函数，有单测覆盖

## 验证手法

- 工具自身 GUI：用 kimi-cu MCP 截图 `get_app_state(app="kimi-bg-tool", mode="image")`
- Kimi Code 端效果：改热更 css（如遮罩改红）+ json 版本+1，等 4 秒截图 Kimi Code 窗口确认，然后还原再+1
- **绝不主动杀 `Kimi Code.exe`**——agent 自己就跑在它里面，杀了会中断会话。重启只能让用户双击桌面「重启KimiCode-应用壁纸.bat」

## Git 约定

- 仓库：`https://github.com/LJF-0125/kimi-code-wallpaper`，gh CLI 已登录
- 提交：`git -c user.name="LJF-0125" -c user.email="LJF-0125@users.noreply.github.com" commit ...`
- `target/`、`test-fixture/` 已 gitignore，不要提交构建产物

## 相关文档（项目外）

- `T:\KCD BackGrond\热更新探针-档案与恢复指南.md` — 探针原理、Kimi Code 更新后的恢复三步、排障实录
- `T:\KCD BackGrond\probe\kimi-wallpaper-hot-hook.html` — 探针代码纯净存档
- `T:\KCD BackGrond\Kimi Code 自定义背景改造总结.md` — 早期调查总结（部分过时，以本文件和恢复指南为准）
