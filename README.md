# kimi-code-wallpaper (kimi-bg-tool)

Kimi Code 桌面应用的自定义背景工具：选一张图片、调好透明度、一键把纯色界面换成你自己的壁纸。深色（月之暗面）、浅色（月之亮面）两种主题各自独立配置。

> **免责声明**：本工具是非官方第三方工具，与月之暗面（Moonshot AI）无关。"Kimi Code" 名称仅用于描述兼容对象。

![工具界面](docs/screenshot-tool.png)

## 功能

- **自动检测安装路径**：启动即定位 Kimi Code 安装目录和主样式表，显示补丁/备份状态
- **深浅色双槽位**：深色、浅色主题可各配一张壁纸，各自独立的遮罩透明度
- **三档透明度调节**：
  - 遮罩透明度（压在壁纸上的暗/亮层，保证文字可读）
  - 侧边栏/标题栏透明度
  - 面板透明度（右侧文件预览、侧边聊天面板）
- **一键应用补丁**：自动压缩图片（2560px JPEG）、base64 内嵌进 CSS，无外部文件依赖
- **热更新**：安装热更新探针后，打补丁约 2 秒内就地生效，无需重启应用
- **一键还原原版**：首次打补丁前自动备份原样式表（`.wallpaper-bak`），随时反悔

## 使用

1. 下载 Release 里的 `kimi-bg-tool.exe`（或自行构建，见下），双击运行，免安装
2. 确认识别出的安装路径正确（不对就点「浏览…」手动选）
3. 点 **安装热更新**（一次性操作），然后**完全退出并重启 Kimi Code 一次**——此后探针永久生效
4. 在「深色背景」/「浅色背景」各选一张图，拖好三个滑块
5. 点 **应用补丁**，运行中的 Kimi Code 约 2 秒内自动换装，无需再重启

> 不装热更新也能用：直接跳到第 4 步，但每次打补丁都要重启 Kimi Code 才生效。

> **注意**：Kimi Code 应用更新会覆盖被补丁的文件（含热更新探针），更新后需重新打补丁并重装探针。「还原原版」可随时恢复未修改状态。

## 原理（简述）

Kimi Code 桌面应用是 Electron + Vue，渲染层是散文件（`resources\desktop-dist\`），主进程通过自定义 `app://` 协议每次现读磁盘文件，无完整性校验。本工具向主样式表（`assets\main-*.css`）末尾追加一段标记好的 CSS：

- `:root` 注入壁纸变量（base64 内嵌图）
- `html[data-color-scheme=dark|light|system]` 上铺「壁纸 + 半透明遮罩」背景
- 把 `body / #app / .app / .con` 透明化（否则 Chromium UA 内置背景会盖住壁纸）
- 侧边栏、右侧面板改为半透明，让壁纸透出来

补丁段有 `kimi-wallpaper-patch` 起止标记，重复打补丁会先清掉旧段再写新段；还原即恢复备份文件。

### 热更新原理

「安装热更新」向 `index.html` 注入一段探针脚本（带 `kimi-wallpaper-hot-hook` 标记，可一键卸载）：探针每 2 秒 fetch 一次版本文件 `kimi-wallpaper-hot.json`，版本号变化时才拉取 `kimi-wallpaper-hot.css` 并注入 `<style>` 就地换装。「应用补丁」只需重写这两个文件，运行中的应用约 2 秒内自动生效。由于 `app://` 协议每次请求都现读磁盘，轮询到的永远是最新内容；版本号对比保证不重绘、开销可忽略。

### 当前限制

- 动图（GIF）暂不支持：选 GIF 会被压成静态 JPEG 首帧。GIF 支持在计划中
- Kimi Code 大版本更新后若 CSS 类名变更，透明化规则可能需要跟进

## 从源码构建

需要 Rust 工具链（stable）：

```bash
cargo build --release
# 产物：target\release\kimi-bg-tool.exe（约 6.6 MB）

cargo test --release
```

技术栈：Rust + [egui/eframe](https://github.com/emilk/egui)（GUI）、image（压图）、rfd（文件选择框）、winreg（安装路径探测）。GUI 中文字体在启动时加载系统微软雅黑（`C:\Windows\Fonts\msyh.ttc`）。

## License

[MIT](LICENSE)
