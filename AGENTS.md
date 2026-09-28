# AGENTS.md — kimi-bg-tool（Kimi Code 背景更换工具）

给 Kimi Code 桌面应用（Electron）换壁纸/字体的 Windows GUI 小工具。Rust 单文件实现，选图/裁剪/调透明度/选系统字体（或导入本地字体文件）后把图片压成 base64 JPEG 内嵌进主样式表补丁，支持热更新免重启。

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
- 首次写入前备份为 `主样式表名.wallpaper-bak`；「还原原版」只能从备份恢复，**永远不要无备份覆盖**；还原时除恢复文件外还推送反向补丁（见热更新节），运行中的应用免重启恢复原版外观
- 主样式表文件名带 hash（如 `main-c0uVXxKo.css`），由 `locate_main_css` 从 index.html 里解析，**不要硬编码文件名**
- 透明化名单（`.app/.con/body/#app` 全透明、`.sidebar-actions` 侧边栏顶部「新建会话/搜索」按钮区容器（div.sidebar-actions 实色 rgb(249,251,252)，CDP 实测，所在 .side 已半透明故全透明即可）、`.side-footer` 侧边栏底部用户区（头像/用户名/徽章/设置齿轮，`background:var(--color-sidebar-bg)` 实色，同理全透明即可）、`.side/.windows-titlebar` 侧边栏透明度、`.global-preview/.agent-panel/.file-preview/.fp-body/.ui-panel-header/.chat-header/.topbar` 面板透明度、`.sc` 侧边聊天面板（/btw，aside.global-preview > .pt-shell > .pt-body 内的白底根容器，与面板组同款 {PA}）、`.chat-dock:before`）是反复用 CDP 探针排出来的，dark/light/system 三套镜像必须同步改
- **字体覆盖**（`build_font_block` 纯函数）：界面字体全走 `:root` 的 `--font-ui`（别名 `--sans`/`--font-display`/`--markdown-font-family`）、代码字体走 `--font-mono`（别名 `--mono`/`--markdown-code-font-family`），改这两个变量即改全界面。用 `:root:root` 提特异性（全应用样式表 `font-family !important` 数量为 0，追加末尾即可覆盖）；**不要碰字号**（应用自带 data-font-scale），**不要用 `*{font-family:!important}`**（会砸 KaTeX 数学字体）。字体与主题无关不分镜像，插在 dark/light 块之后、结束标记之前。槽设置用 `FontSlotSetting`（None/Name/File，`From<&str>` 空串→None，故测试可直接传字符串字面量）：
  - **名字模式（方案 B）**：字体名经 `sanitize_font_name` 消毒（剔除 `"` `'` `\` `{}` `<>` `;` `/` `*` `()` 与控制字符）防 CSS 注入；进 `--font-ui-latin` 首位（系统字体拉丁+CJK 分开管）
  - **文件模式（方案 A，优先于名字模式，可按槽混合）**：先生成 `@font-face`（family 固定 `KimiFontUI`/`KimiFontMono`，url **绝对路径** `/kimi-wallpaper-font-{ui|mono}-<毫秒>.<ext>`，`format()` 映射 woff2/woff/ttf→truetype，`font-display:swap`），再把自定义名直接放 `--font-ui`/`--font-mono` 首位（**不进 latin 槽**——导入的可能是 CJK 字体）
  - 两个槽都无效（None 或消毒后为空）→ 不产出字体块

### 热更新
- 探针块（`kimi-wallpaper-hot-hook`）插在 `index.html` 最后一个 `</body>` 前；index.html 也有 `.wallpaper-bak` 备份。当前为 **v3**（块内 `<!-- version: v3 -->` 标记，`HOOK_VERSION_MARK`）
- v3 探针维护两层：`<video>` 背景层（z-index:-2，muted/loop/autoplay/playsInline，object-fit:cover）+ 独立遮罩 `<div>`（z-index:-1，`background:var(--kimi-wallpaper-mask,transparent)`，该 CSS 变量由补丁按 dark/light/system 定义，遮罩随主题自动切换，探针不感知主题）
- **视频必须走 fetch→blob→objectURL**（v3 相对 v2 的唯一改动）：根因是 app:// 自定义协议对 `.mp4` 返回 **application/octet-stream**（app.asar 内 `protocol-*.cjs` 的 MIME 表无 .mp4 条目）且**无 Range/206 支持**（整文件流式返回），Chromium 媒体栈对直接 `video.src` 直接报 MEDIA_ERR_SRC_NOT_SUPPORTED（error 4，CDP 实测）；fetch 拿 blob 再 `URL.createObjectURL` 喂给 video 立刻正常播放（27MB 实测 readyState 4）。探针已改为 fetch→blob→objectURL 完全绕开协议层，切换视频时 revoke 旧 blobUrl
- 热更文件：`kimi-wallpaper-hot.css`（=补丁全文）+ `kimi-wallpaper-hot.json`（`{"v":N,"video":"kimi-wallpaper-video-<时间戳>.mp4"|null}`）。**必须先写 css 后写 json**，版本号 `max(旧+1, unix毫秒)`，探针以 json 为准；video 字段驱动视频层挂载/移除
- 视频文件不内嵌：应用时部署为**版本化文件名** `desktop-dist\kimi-wallpaper-video-<毫秒时间戳>.mp4`（不用固定名——Windows 不允许写入被播放实例占用的文件，直接覆盖会 OS error 32 部署失败），探针按 json 里的名字 fetch。部署成功后惰性清理其余 `kimi-wallpaper-video*.mp4`（含历史固定名），删除失败（旧文件仍被占用）忽略并提示「下次应用时自动清理」；清除视频时删全部版本化文件，json 照常写 null
- **字体文件部署同款模式**（`deploy_font_file`/`cleanup_old_font_files`）：导入的字体复制为 `desktop-dist\kimi-wallpaper-font-{ui|mono}-<毫秒时间戳>.<ext>`（同毫秒自增），@font-face 按补丁里的绝对路径 `/<名字>` 直接引用（字体走 CSS `url()` 加载，无需 fetch→blob）；部署成功后惰性清理同前缀旧版，删除失败忽略并提示下次清理；槽退出文件模式时删该前缀全部文件。只接受 ttf/woff/woff2（`font_file_format` 映射 format()），**拒绝 .otf**（协议 MIME 表无此条目）；>8MB 仅日志警告（CJK 字体常见）；部署失败日志报错并回退名字模式，不影响其余补丁
- faststart 重封装**保留**（`mp4_moov_before_mdat` 检测 + `faststart_remux` 纯搬盒子：moov 移到 mdat 前 + stco/co64 chunk 偏移修正）：当前 blob 路径下非必需，但有益无害，且对未来协议修复后可直放有意义
- **音轨剥除**（`mp4_track_handlers` 检测 + `strip_audio_remux`）：探针 muted 播放、音轨纯浪费，部署时剥除音轨/字幕/数据轨——保留第一个 vide trak，按 stsc/stsz/stco 样本表从原 mdat 提取视频样本重建 mdat（一样本一 chunk），moov 只含视频轨（mvhd/tkhd/stsd/stts/ctts/stss 原样），输出天然 faststart（ftyp+moov+mdat）。无音轨视频不做重建直接走 faststart 路径；分片 MP4（mvex）/stz2/表不一致一律 Err 回退原样部署并日志
- 探针 fetch **必须用绝对路径** `/kimi-wallpaper-hot.*`——相对路径在 `/sessions/<id>` 路由下会 404 静默失效（踩过的坑）
- 升级检测：`hook_needs_upgrade` = 含起始标记但缺 `HOOK_VERSION_MARK`（v1/v2 等无当前标记的旧块）。`install_hook` 幂等策略：未安装→插入；旧块→remove 后再插当前块；已当前→原样返回。UI 三态：绿「已启用 v3」/ 黄「需升级」（按钮「升级热更新」）/ 灰「未启用」
- **反向还原补丁**（`RESTORE_TEMPLATE`/`build_restore_patch`）：运行中的应用启动时已把带补丁的主样式表读进内存，磁盘还原**删不掉内存里的规则**，热更层只能覆盖不能删除——所以 `do_restore` 在恢复文件后往 hot.css 写反向补丁：与补丁**同选择器**（保证特异性 ≥ 补丁，`.file-preview`/`.fp-body`/`.ui-panel-header` 必须用最长限定 `.global-preview .xx`，图片模板里它们是 `html[...] .global-preview .xx` (0,3,1)）+ 原版值 + `!important`（探针 style 元素在 head 末尾，同特异性同 !important 后写者赢）。原版值全部实测自 .wallpaper-bak 且都是主题变量引用（html `background:none`；`.app`/`.con`/`.sc`/`.global-preview`/`.file-preview` → `var(--bg)`；`.agent-panel`/`.chat-header` → `var(--color-bg)`；`.topbar` → `var(--color-topbar-bg-frost)`；`.side`/`.windows-titlebar`/`.sidebar-actions`/`.side-footer` → `var(--color-sidebar-bg)`；`.fp-body`/`.ui-panel-header` → `transparent`；`.chat-dock:before` → `opacity:1`），自动跟随 dark/light/system 无需写死颜色。字体变量（`FONT_VAR_NAMES` 8 个）由 `extract_original_font_vars` 从 `assets/*.css`（从不打补丁，即原版来源）现提取，**必须在 restore_patch 之后调用**（此时主样式表已是纯净备份）；提取不到的跳过（主变量还原后孤立别名无人引用，无害）
- **纯视频白屏修复**：图片槽全空但视频激活时，`build_patch` 返回 None 会把热更 CSS 清空 → 界面恢复不透明背景盖住视频层（白屏）。`do_apply` 在该分支改发 `build_video_patch`（`VIDEO_TEMPLATE`：透明化规则 + `--kimi-wallpaper-mask` 变量 + html `background:transparent`，无图片；字体块照常追加）。**图槽全空但设了字体**时 `build_patch` 只产出字体块（不附带透明化规则——只改字体的用户不一定要壁纸）；图片/视频/字体全空才返回 None 走纯还原
- Kimi Code 应用更新会冲掉 index.html（探针）和 main-*.css（补丁），热更文件可能幸存但成孤儿。更新后的恢复流程见 `T:\KCD BackGrond\热更新探针-档案与恢复指南.md`

### 图片管线
- `process_image(path, crop)` 返回 `ProcessedImage{b64, orig_len, comp_len, cropped, mime}`：普通图走 load_from_memory → 可选 crop_imm 裁剪（归一化 CropRect 转像素，round+clamp，宽高≥1）→ 最长边压到 2560 → JPEG（先 q82，超 900KB 降 q70）→ base64，`mime="image/jpeg"`
- **GIF 透传**：扩展名 `.gif` 不走 image crate，原始字节直接 base64（保留动画），`mime="image/gif"`；与裁剪冲突时忽略裁剪并日志「GIF 动画不支持裁剪，已整图嵌入」；原始字节 > 1.5MB（`GIF_WARN_BYTES`）日志体积警告
- `MAX_JPEG_BYTES = 900KB`：base64 内嵌进 CSS，太大样式表会膨胀。补丁模板的 `data:{MIME};base64,{B64}` 占位符按槽位 mime 替换
- 模板另定义 `--kimi-wallpaper-mask` 变量（dark 两处 `rgba(7,7,13,{DA})`、light 两处 `rgba(250,250,252,{LA})`，各含 scheme/system 选择器），给视频遮罩层用；原有背景渐变遮罩保留作兜底
- 裁剪选区 `CropRect` 是归一化坐标（0..1），随配置持久化到 `kimi-bg-tool.conf`（见下节），启动时恢复
- **耗电警告**：GIF 槽选中、视频槽常驻显示橙色警告「持续占用 CPU/GPU，会增加耗电」；视频部署/GIF 嵌入时日志同步警告

### 配置持久化
- `kimi-bg-tool.conf`（exe 同目录，`conf_path()` 基于 `current_exe`，失败则静默跳过）：`key=value` 每行一条，持久化安装路径、side/panel 透明度、两槽位的 path/alpha/裁剪选区（`x,y,w,h` 逗号分隔，None 不写行）、视频路径（`video.path`）、界面/代码字体名（`font.ui`/`font.mono`，空串不写行）与导入字体文件路径（`font.ui.file`/`font.mono.file`，None 不写行；有值即文件模式，优先于字体名）。解析用 `splitn(2, '=')`，路径含 `=`/中文安全
- `SettingsSnapshot::to_conf/from_conf` 纯函数（有单测），from_conf 对缺行/坏行/未知键容错，缺字段取默认值（alpha 0.80/0.78/0.55/0.25）
- 防抖 500ms 写盘：`update_persistence` 每帧比对 `snapshot().to_conf()`，变了记 `save_due`，到期才 `fs::write`；`request_repaint_after` 保证工具闲置时到期帧被唤醒落盘。写失败仅记一行日志不重试刷屏
- 启动恢复顺序：注册表自动检测在前 → conf 覆盖（conf 字段优先，用户手改的安装路径高于注册表）→ 槽位图片/视频/字体文件失效时清槽并记日志「上次选择的…已失效: <路径>」

### GUI
- egui 无 CJK 字体，`install_cjk_font` 从 `C:\Windows\Fonts` 加载微软雅黑，别删
- 裁剪编辑器（`open_crop_editor`/`show_crop_editor`）：红框拖拽，角命中 ≤14px 存进 `editor.drag`，一次拖拽期间不重判；`centered_max_crop`/`clamp_crop` 是纯函数，有单测覆盖
- 「界面字体」卡（`font_slot_row`）：每槽一个 `ComboBox` 可搜索下拉（按钮显示当前值/「默认」，弹层顶部过滤框输入即过滤、大小写不敏感子串匹配，首项固定「默认（不设置）」，过滤内容不精确匹配任何字体时给「使用「<输入>」」手填项；`ui.close_menu()` 关弹层——egui 0.31 无 `ui.close()`）；字体列表来自启动时 `enumerate_system_fonts`（winreg 读 HKLM+HKCU `...\CurrentVersion\Fonts` 值名，`clean_font_value_names` 纯函数剥 ` (TrueType)`/` (OpenType)`/` (所有版本)` 后缀、` & ` 多 face 取首、去重排序；读取失败降级空列表仍可手填）。「选文件...」（rfd）导入 ttf/woff/woff2 进文件模式（otf 拒绝并日志提示转 WOFF2），文件模式下下拉禁用、显示文件名+「清除文件」。编辑实时进 `SettingsSnapshot` 走同一套 500ms 防抖落盘

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
