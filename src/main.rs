#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context};
use eframe::egui;

const PATCH_START: &str = "/* === kimi-wallpaper-patch";
const PATCH_HEADER: &str = "/* === kimi-wallpaper-patch === */";
const PATCH_END_MARK: &str = "kimi-wallpaper-patch end === */";
const MAX_TEXTURE_SIDE: u32 = 2560;
const MAX_JPEG_BYTES: usize = 900 * 1024;
const CONF_FILE_NAME: &str = "kimi-bg-tool.conf";
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

const HOT_HOOK_START: &str = "<!-- === kimi-wallpaper-hot-hook === -->";
const HOT_HOOK_END: &str = "<!-- === kimi-wallpaper-hot-hook end === -->";
const HOOK_VERSION_MARK: &str = "<!-- version: v2 -->";
const VIDEO_FILE_NAME: &str = "kimi-wallpaper-video.mp4";
const GIF_WARN_BYTES: usize = 3 * 512 * 1024; // 1.5MB

const HOT_HOOK_BLOCK: &str = r#"    <!-- === kimi-wallpaper-hot-hook === -->
    <!-- version: v2 -->
    <script>
      (function () {
        var STYLE_ID = 'kimi-wallpaper-hot-style';
        var VIDEO_ID = 'kimi-wallpaper-hot-video';
        var MASK_ID = 'kimi-wallpaper-hot-mask';
        var cur = null, curVideo;
        function applyCss(css) {
          var el = document.getElementById(STYLE_ID);
          if (!el) { el = document.createElement('style'); el.id = STYLE_ID; document.head.appendChild(el); }
          if (el.textContent !== css) el.textContent = css;
        }
        function applyVideo(name) {
          if (name === curVideo) return; curVideo = name;
          var v = document.getElementById(VIDEO_ID), m = document.getElementById(MASK_ID);
          if (!name) { if (v) v.remove(); if (m) m.remove(); return; }
          if (!v) { v = document.createElement('video'); v.id = VIDEO_ID; document.body.appendChild(v); }
          v.muted = true; v.loop = true; v.autoplay = true; v.playsInline = true;
          v.setAttribute('style', 'position:fixed;left:0;top:0;width:100%;height:100%;object-fit:cover;z-index:-2;pointer-events:none');
          v.src = '/' + name + '?v=' + Date.now();
          v.play().catch(function () {});
          if (!m) { m = document.createElement('div'); m.id = MASK_ID; document.body.appendChild(m); }
          m.setAttribute('style', 'position:fixed;left:0;top:0;width:100%;height:100%;z-index:-1;pointer-events:none;background:var(--kimi-wallpaper-mask,transparent)');
        }
        function tick() {
          fetch('/kimi-wallpaper-hot.json?_=' + Date.now(), { cache: 'no-store' })
            .then(function (r) { if (!r.ok) return null; return r.json(); })
            .then(function (j) {
              if (!j || typeof j.v === 'undefined') return;
              applyVideo(typeof j.video === 'string' && j.video ? j.video : null);
              if (j.v === cur) return;
              return fetch('/kimi-wallpaper-hot.css?v=' + encodeURIComponent(j.v), { cache: 'no-store' })
                .then(function (rc) { return rc.ok ? rc.text() : ''; })
                .then(function (css) { applyCss(css); cur = j.v; });
            })
            .catch(function () { });
        }
        tick();
        setInterval(tick, 2000);
      })();
    </script>
    <!-- === kimi-wallpaper-hot-hook end === -->"#;

const DARK_TEMPLATE: &str = r#":root{--kimi-wallpaper-dark:url("data:{MIME};base64,{B64}")}
html[data-color-scheme=dark]{--kimi-wallpaper-mask:rgba(7,7,13,{DA})}
html[data-color-scheme=dark]{background-color:#0a0a10;background-image:linear-gradient(rgba(7,7,13,{DA}),rgba(7,7,13,{DA})),var(--kimi-wallpaper-dark) !important;background-size:cover,cover;background-position:center,center;background-repeat:no-repeat,no-repeat;background-attachment:fixed,fixed}
html[data-color-scheme=dark] .app,html[data-color-scheme=dark] .con,html[data-color-scheme=dark] body,html[data-color-scheme=dark] #app{background:transparent !important}
html[data-color-scheme=dark] .side,html[data-color-scheme=dark] .windows-titlebar{background:rgba(10,10,17,{SA}) !important}
html[data-color-scheme=dark] .global-preview,html[data-color-scheme=dark] .agent-panel,html[data-color-scheme=dark] .global-preview .file-preview,html[data-color-scheme=dark] .global-preview .fp-body,html[data-color-scheme=dark] .global-preview .ui-panel-header{background:rgba(10,10,17,{PA}) !important}
html[data-color-scheme=dark] .chat-header,html[data-color-scheme=dark] .topbar{background:rgba(10,10,17,{PA}) !important}
html[data-color-scheme=dark] .chat-dock:before{opacity:.45 !important}
@media (prefers-color-scheme:dark){
html[data-color-scheme=system]{--kimi-wallpaper-mask:rgba(7,7,13,{DA})}
html[data-color-scheme=system]{background-color:#0a0a10;background-image:linear-gradient(rgba(7,7,13,{DA}),rgba(7,7,13,{DA})),var(--kimi-wallpaper-dark) !important;background-size:cover,cover;background-position:center,center;background-repeat:no-repeat,no-repeat;background-attachment:fixed,fixed}
html[data-color-scheme=system] .app,html[data-color-scheme=system] .con,html[data-color-scheme=system] body,html[data-color-scheme=system] #app{background:transparent !important}
html[data-color-scheme=system] .side,html[data-color-scheme=system] .windows-titlebar{background:rgba(10,10,17,{SA}) !important}
html[data-color-scheme=system] .global-preview,html[data-color-scheme=system] .agent-panel,html[data-color-scheme=system] .global-preview .file-preview,html[data-color-scheme=system] .global-preview .fp-body,html[data-color-scheme=system] .global-preview .ui-panel-header{background:rgba(10,10,17,{PA}) !important}
html[data-color-scheme=system] .chat-header,html[data-color-scheme=system] .topbar{background:rgba(10,10,17,{PA}) !important}
html[data-color-scheme=system] .chat-dock:before{opacity:.45 !important}
}
"#;

const LIGHT_TEMPLATE: &str = r#":root{--kimi-wallpaper-light:url("data:{MIME};base64,{B64}")}
html[data-color-scheme=light]{--kimi-wallpaper-mask:rgba(250,250,252,{LA})}
html[data-color-scheme=light]{background-color:#f5f5f7;background-image:linear-gradient(rgba(250,250,252,{LA}),rgba(250,250,252,{LA})),var(--kimi-wallpaper-light) !important;background-size:cover,cover;background-position:center,center;background-repeat:no-repeat,no-repeat;background-attachment:fixed,fixed}
html[data-color-scheme=light] .app,html[data-color-scheme=light] .con,html[data-color-scheme=light] body,html[data-color-scheme=light] #app{background:transparent !important}
html[data-color-scheme=light] .side,html[data-color-scheme=light] .windows-titlebar{background:rgba(255,255,255,{SA}) !important}
html[data-color-scheme=light] .global-preview,html[data-color-scheme=light] .agent-panel,html[data-color-scheme=light] .global-preview .file-preview,html[data-color-scheme=light] .global-preview .fp-body,html[data-color-scheme=light] .global-preview .ui-panel-header{background:rgba(255,255,255,{PA}) !important}
html[data-color-scheme=light] .chat-header,html[data-color-scheme=light] .topbar{background:rgba(255,255,255,{PA}) !important}
html[data-color-scheme=light] .chat-dock:before{opacity:.45 !important}
@media (prefers-color-scheme:light){
html[data-color-scheme=system]{--kimi-wallpaper-mask:rgba(250,250,252,{LA})}
html[data-color-scheme=system]{background-color:#f5f5f7;background-image:linear-gradient(rgba(250,250,252,{LA}),rgba(250,250,252,{LA})),var(--kimi-wallpaper-light) !important;background-size:cover,cover;background-position:center,center;background-repeat:no-repeat,no-repeat;background-attachment:fixed,fixed}
html[data-color-scheme=system] .app,html[data-color-scheme=system] .con,html[data-color-scheme=system] body,html[data-color-scheme=system] #app{background:transparent !important}
html[data-color-scheme=system] .side,html[data-color-scheme=system] .windows-titlebar{background:rgba(255,255,255,{SA}) !important}
html[data-color-scheme=system] .global-preview,html[data-color-scheme=system] .agent-panel,html[data-color-scheme=system] .global-preview .file-preview,html[data-color-scheme=system] .global-preview .fp-body,html[data-color-scheme=system] .global-preview .ui-panel-header{background:rgba(255,255,255,{PA}) !important}
html[data-color-scheme=system] .chat-header,html[data-color-scheme=system] .topbar{background:rgba(255,255,255,{PA}) !important}
html[data-color-scheme=system] .chat-dock:before{opacity:.45 !important}
}
"#;

// ---------- 数据结构 ----------

#[derive(Clone, Copy, PartialEq, Debug)]
struct CropRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum CropAspect {
    Ratio16x9,
    Ratio21x9,
    Free,
}

impl CropAspect {
    const ALL: [CropAspect; 3] = [CropAspect::Ratio16x9, CropAspect::Ratio21x9, CropAspect::Free];

    fn ratio(self) -> Option<f32> {
        match self {
            CropAspect::Ratio16x9 => Some(16.0 / 9.0),
            CropAspect::Ratio21x9 => Some(21.0 / 9.0),
            CropAspect::Free => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            CropAspect::Ratio16x9 => "16:9（宽屏）",
            CropAspect::Ratio21x9 => "21:9（超宽屏）",
            CropAspect::Free => "自由比例",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SlotKind {
    Dark,
    Light,
}

#[derive(Clone, Copy, PartialEq)]
enum CropDrag {
    None,
    Move,
    Nw,
    Ne,
    Sw,
    Se,
}

struct CropEditor {
    slot: SlotKind,
    texture: egui::TextureHandle,
    rect: CropRect,
    aspect: CropAspect,
    drag: CropDrag,
}

/// 归一化空间内的居中最大选区：宽≤1、高≤1 约束下取该比例的居中最大框。
fn centered_max_crop(aspect: Option<f32>) -> CropRect {
    match aspect {
        None => CropRect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 },
        Some(r) if r >= 1.0 => {
            let h = 1.0 / r;
            CropRect { x: 0.0, y: (1.0 - h) / 2.0, w: 1.0, h }
        }
        Some(r) => {
            let w = r;
            CropRect { x: (1.0 - w) / 2.0, y: 0.0, w, h: 1.0 }
        }
    }
}

/// 保证选区在 0..1 图内，且宽/高不小于 min。
fn clamp_crop(r: CropRect, min: f32) -> CropRect {
    let min = min.clamp(0.0, 1.0);
    let mut out = r;
    out.w = out.w.clamp(min, 1.0);
    out.h = out.h.clamp(min, 1.0);
    out.x = out.x.clamp(0.0, 1.0 - out.w);
    out.y = out.y.clamp(0.0, 1.0 - out.h);
    out
}

/// 按指针位置判定拖拽命中：距某角 ≤14px 拖该角，框内移动，框外忽略。
fn hit_test(p: egui::Pos2, r: egui::Rect) -> CropDrag {
    let corners = [
        (r.left_top(), CropDrag::Nw),
        (r.right_top(), CropDrag::Ne),
        (r.left_bottom(), CropDrag::Sw),
        (r.right_bottom(), CropDrag::Se),
    ];
    for (c, d) in corners {
        if p.distance(c) <= 14.0 {
            return d;
        }
    }
    if r.contains(p) {
        CropDrag::Move
    } else {
        CropDrag::None
    }
}

/// 被拖拽角的对角锚点（归一化坐标）。
fn drag_anchor(r: CropRect, corner: CropDrag) -> (f32, f32) {
    match corner {
        CropDrag::Nw => (r.x + r.w, r.y + r.h),
        CropDrag::Ne => (r.x, r.y + r.h),
        CropDrag::Sw => (r.x + r.w, r.y),
        CropDrag::Se => (r.x, r.y),
        _ => (r.x, r.y),
    }
}

/// 被拖拽角相对锚点的方向：选区左上角 = 锚点 + (-sx*w, -sy*h)。
fn drag_sign(corner: CropDrag) -> (f32, f32) {
    match corner {
        CropDrag::Nw => (-1.0, -1.0),
        CropDrag::Ne => (1.0, -1.0),
        CropDrag::Sw => (-1.0, 1.0),
        CropDrag::Se => (1.0, 1.0),
        _ => (1.0, 1.0),
    }
}

struct Slot {
    path: Option<PathBuf>,
    texture: Option<egui::TextureHandle>,
    alpha: f32,
    crop: Option<CropRect>,
}

impl Slot {
    fn new(alpha: f32) -> Self {
        Self {
            path: None,
            texture: None,
            alpha,
            crop: None,
        }
    }
}

// ---------- 配置持久化 ----------

/// 配置文件路径：exe 同目录。current_exe 失败则返回 None（静默跳过持久化）。
fn conf_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join(CONF_FILE_NAME)))
}

#[derive(Clone, PartialEq, Debug)]
struct SettingsSnapshot {
    install_path: String,
    side_alpha: f32,
    panel_alpha: f32,
    dark_path: Option<PathBuf>,
    dark_alpha: f32,
    dark_crop: Option<CropRect>,
    light_path: Option<PathBuf>,
    light_alpha: f32,
    light_crop: Option<CropRect>,
    video_path: Option<PathBuf>,
}

impl Default for SettingsSnapshot {
    fn default() -> Self {
        Self {
            install_path: String::new(),
            side_alpha: 0.55,
            panel_alpha: 0.25,
            dark_path: None,
            dark_alpha: 0.80,
            light_path: None,
            light_alpha: 0.78,
            dark_crop: None,
            light_crop: None,
            video_path: None,
        }
    }
}

impl SettingsSnapshot {
    /// 序列化为 key=value 每行一条的文本。path/crop 为 None 时不写对应行。
    fn to_conf(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("install_path={}\n", self.install_path));
        out.push_str(&format!("side_alpha={:.2}\n", self.side_alpha));
        out.push_str(&format!("panel_alpha={:.2}\n", self.panel_alpha));
        out.push_str(&format!("dark.alpha={:.2}\n", self.dark_alpha));
        if let Some(p) = &self.dark_path {
            out.push_str(&format!("dark.path={}\n", p.display()));
        }
        if let Some(c) = &self.dark_crop {
            out.push_str(&format!("dark.crop={},{},{},{}\n", c.x, c.y, c.w, c.h));
        }
        out.push_str(&format!("light.alpha={:.2}\n", self.light_alpha));
        if let Some(p) = &self.light_path {
            out.push_str(&format!("light.path={}\n", p.display()));
        }
        if let Some(c) = &self.light_crop {
            out.push_str(&format!("light.crop={},{},{},{}\n", c.x, c.y, c.w, c.h));
        }
        if let Some(p) = &self.video_path {
            out.push_str(&format!("video.path={}\n", p.display()));
        }
        out
    }

    /// 解析 conf 文本；缺行/坏行/未知键一律容错，缺失字段取默认值。
    fn from_conf(text: &str) -> Self {
        let mut snap = Self::default();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut it = line.splitn(2, '=');
            let key = it.next().unwrap_or_default().trim();
            let val = it.next().unwrap_or_default().trim();
            match key {
                "install_path" => snap.install_path = val.to_string(),
                "side_alpha" => {
                    if let Ok(x) = val.parse::<f32>() {
                        snap.side_alpha = x;
                    }
                }
                "panel_alpha" => {
                    if let Ok(x) = val.parse::<f32>() {
                        snap.panel_alpha = x;
                    }
                }
                "dark.alpha" => {
                    if let Ok(x) = val.parse::<f32>() {
                        snap.dark_alpha = x;
                    }
                }
                "light.alpha" => {
                    if let Ok(x) = val.parse::<f32>() {
                        snap.light_alpha = x;
                    }
                }
                "dark.path" => {
                    snap.dark_path = (!val.is_empty()).then(|| PathBuf::from(val));
                }
                "light.path" => {
                    snap.light_path = (!val.is_empty()).then(|| PathBuf::from(val));
                }
                "dark.crop" => snap.dark_crop = parse_crop(val),
                "light.crop" => snap.light_crop = parse_crop(val),
                "video.path" => {
                    snap.video_path = (!val.is_empty()).then(|| PathBuf::from(val));
                }
                _ => {} // 未知键忽略
            }
        }
        snap
    }
}

/// `x,y,w,h` 四个 f32 逗号分隔；任一部分解析失败或不是 4 段则返回 None。
fn parse_crop(val: &str) -> Option<CropRect> {
    let parts: Vec<&str> = val.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let nums: Option<Vec<f32>> = parts
        .iter()
        .map(|p| p.trim().parse::<f32>().ok())
        .collect();
    let n = nums?;
    Some(CropRect { x: n[0], y: n[1], w: n[2], h: n[3] })
}

struct BgToolApp {
    install_path: String,
    root: Option<PathBuf>,
    css_name: Option<String>,
    patched: bool,
    bak_exists: bool,
    hot_enabled: bool,
    hot_needs_upgrade: bool,
    status_err: Option<String>,
    dark: Slot,
    light: Slot,
    video_path: Option<PathBuf>,
    side_alpha: f32,
    panel_alpha: f32,
    log: String,
    crop_editor: Option<CropEditor>,
    saved_conf: String,
    pending_conf: Option<String>,
    save_due: Option<Instant>,
}

impl BgToolApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            install_path: String::new(),
            root: None,
            css_name: None,
            patched: false,
            bak_exists: false,
            hot_enabled: false,
            hot_needs_upgrade: false,
            status_err: None,
            dark: Slot::new(0.80),
            light: Slot::new(0.78),
            video_path: None,
            side_alpha: 0.55,
            panel_alpha: 0.25,
            log: String::new(),
            crop_editor: None,
            saved_conf: String::new(),
            pending_conf: None,
            save_due: None,
        };
        app.log_push("Kimi Code 背景更换工具已启动");
        match detect_install_path() {
            Some(p) => {
                app.log_push(&format!("自动检测到安装路径: {}", p.display()));
                app.install_path = p.display().to_string();
            }
            None => app.log_push("未自动检测到 Kimi Code 安装路径，请手动选择"),
        }
        // 恢复 conf：conf 里有的字段以 conf 为准（用户手改的安装路径优先于注册表检测）
        if let Some(text) = conf_path().and_then(|p| fs::read_to_string(p).ok()) {
            let snap = SettingsSnapshot::from_conf(&text);
            app.apply_snapshot(&snap, &cc.egui_ctx);
            app.saved_conf = app.snapshot().to_conf();
            app.log_push("已从 kimi-bg-tool.conf 恢复上次配置");
        }
        app.refresh();
        app
    }

    fn log_push(&mut self, msg: &str) {
        self.log.push_str(msg);
        if !msg.ends_with('\n') {
            self.log.push('\n');
        }
    }

    fn snapshot(&self) -> SettingsSnapshot {
        SettingsSnapshot {
            install_path: self.install_path.clone(),
            side_alpha: self.side_alpha,
            panel_alpha: self.panel_alpha,
            dark_path: self.dark.path.clone(),
            dark_alpha: self.dark.alpha,
            dark_crop: self.dark.crop,
            light_path: self.light.path.clone(),
            light_alpha: self.light.alpha,
            light_crop: self.light.crop,
            video_path: self.video_path.clone(),
        }
    }

    fn apply_snapshot(&mut self, s: &SettingsSnapshot, ctx: &egui::Context) {
        self.install_path = s.install_path.clone();
        self.side_alpha = s.side_alpha;
        self.panel_alpha = s.panel_alpha;
        let msgs = [
            Self::restore_slot(&mut self.dark, &s.dark_path, s.dark_alpha, s.dark_crop, ctx),
            Self::restore_slot(&mut self.light, &s.light_path, s.light_alpha, s.light_crop, ctx),
        ];
        for m in msgs.into_iter().flatten() {
            self.log_push(&m);
        }
        self.video_path = None;
        if let Some(p) = &s.video_path {
            if p.is_file() {
                self.video_path = Some(p.clone());
            } else {
                self.log_push(&format!("上次选择的视频已失效: {}", p.display()));
            }
        }
    }

    /// 恢复单个槽位：文件缺失或预览加载失败时清掉该槽并返回提示信息。
    fn restore_slot(
        slot: &mut Slot,
        path: &Option<PathBuf>,
        alpha: f32,
        crop: Option<CropRect>,
        ctx: &egui::Context,
    ) -> Option<String> {
        slot.alpha = alpha;
        slot.crop = None;
        slot.path = None;
        slot.texture = None;
        let Some(p) = path else { return None };
        if !p.is_file() {
            return Some(format!("上次选择的图片已失效: {}", p.display()));
        }
        match load_preview(ctx, p, 240) {
            Ok(tex) => {
                slot.path = Some(p.clone());
                slot.texture = Some(tex);
                slot.crop = crop;
                None
            }
            Err(_) => Some(format!("上次选择的图片已失效: {}", p.display())),
        }
    }

    /// 防抖持久化：每帧比对序列化结果，变化后记 save_due，到期才写盘；
    /// request_repaint_after 保证工具闲置时到期帧也会被唤醒执行保存。
    fn update_persistence(&mut self, ctx: &egui::Context) {
        let current = self.snapshot().to_conf();
        let dirty = match &self.pending_conf {
            Some(p) => *p != current,
            None => current != self.saved_conf,
        };
        if dirty {
            self.pending_conf = Some(current);
            self.save_due = Some(Instant::now() + SAVE_DEBOUNCE);
        }
        let Some(due) = self.save_due else { return };
        let now = Instant::now();
        if now < due {
            ctx.request_repaint_after(due.saturating_duration_since(now));
            return;
        }
        if let Some(content) = self.pending_conf.take() {
            self.save_due = None;
            match conf_path() {
                Some(p) => match fs::write(&p, &content) {
                    Ok(()) => self.saved_conf = content,
                    Err(e) => {
                        // 标记为已保存避免反复重试刷屏，仅记一行日志
                        self.saved_conf = content;
                        self.log_push(&format!("配置保存失败: {e:#}"));
                    }
                },
                None => self.saved_conf = content,
            }
        }
    }

    fn refresh(&mut self) {
        self.css_name = None;
        self.patched = false;
        self.bak_exists = false;
        self.hot_enabled = false;
        self.hot_needs_upgrade = false;
        self.status_err = None;
        self.root = None;

        if self.install_path.trim().is_empty() {
            self.status_err = Some("未设置安装路径".to_string());
            return;
        }
        let input = PathBuf::from(self.install_path.trim());
        let root = match normalize_root(&input) {
            Some(r) => r,
            None => {
                self.status_err = Some("路径无效：找不到 resources\\desktop-dist\\index.html".to_string());
                return;
            }
        };
        self.root = Some(root.clone());
        let dist = root.join("resources").join("desktop-dist");
        if let Ok(index_html) = fs::read_to_string(dist.join("index.html")) {
            self.hot_enabled = hook_installed(&index_html);
            self.hot_needs_upgrade = hook_needs_upgrade(&index_html);
        }
        match locate_main_css(&root) {
            Ok(css) => {
                self.css_name = Some(
                    css.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                );
                if let Ok(content) = fs::read_to_string(&css) {
                    self.patched = content.contains(PATCH_START);
                }
                self.bak_exists = backup_path(&css).exists();
            }
            Err(e) => self.status_err = Some(format!("定位主样式表失败: {e:#}")),
        }
    }

    fn process_slot(
        path: &Option<PathBuf>,
        crop: Option<CropRect>,
    ) -> Result<Option<ProcessedImage>, String> {
        match path {
            None => Ok(None),
            Some(p) => process_image(p, crop).map(Some).map_err(|e| format!("{e:#}")),
        }
    }

    fn log_processed(&mut self, label: &str, p: &ProcessedImage, had_crop: bool) {
        if p.mime == "image/gif" {
            self.log_push(&format!("{label}: GIF 动画原样嵌入，原始 {} 字节", p.orig_len));
            if had_crop {
                self.log_push(&format!("{label}: GIF 动画不支持裁剪，已整图嵌入"));
            }
            if p.comp_len > GIF_WARN_BYTES {
                self.log_push(&format!(
                    "{label}: GIF 体积较大（{:.1} MB），将显著增大样式表并增加内存占用",
                    p.comp_len as f64 / 1024.0 / 1024.0
                ));
            }
            return;
        }
        self.log_push(&format!("{label}: 原始 {} 字节 -> JPEG {} 字节", p.orig_len, p.comp_len));
        if let Some((w, h)) = p.cropped {
            self.log_push(&format!("{label}: 已按选区裁剪为 {}×{} px", w, h));
        }
        if p.comp_len > MAX_JPEG_BYTES {
            self.log_push(&format!("{label}: 警告压缩后仍超过 900KB，可能导致样式表过大"));
        }
    }

    /// 部署视频：检测 moov 位置，非 faststart 自动重封装（不转码）后写入 dest；
    /// 检测/重封装失败回退原样拷贝并日志提示。成功返回 Some(VIDEO_FILE_NAME)。
    fn deploy_video(&mut self, src: &Path, dest: &Path) -> Option<String> {
        let data = match fs::read(src) {
            Ok(d) => d,
            Err(e) => {
                self.log_push(&format!("视频部署失败: 读取 {} 失败: {e}", src.display()));
                return None;
            }
        };
        let mut remuxed = false;
        let out = match mp4_moov_before_mdat(&data) {
            Ok(true) => data,
            Ok(false) => match faststart_remux(&data) {
                Ok(v) => {
                    remuxed = true;
                    v
                }
                Err(e) => {
                    self.log_push(&format!(
                        "faststart 重封装失败，已按原样部署；若背景卡住不动，请用 ffmpeg -movflags faststart 转一下: {e:#}"
                    ));
                    data
                }
            },
            Err(e) => {
                self.log_push(&format!(
                    "faststart 检测失败，已按原样部署；若背景卡住不动，请用 ffmpeg -movflags faststart 转一下: {e:#}"
                ));
                data
            }
        };
        let mb = out.len() as f64 / 1024.0 / 1024.0;
        if let Err(e) = fs::write(dest, &out) {
            self.log_push(&format!("视频部署失败: 写入 {} 失败: {e}", dest.display()));
            return None;
        }
        self.log_push(&format!("视频已部署: {mb:.1} MB"));
        if remuxed {
            self.log_push("视频 moov 在文件尾，已自动重封装为 faststart（不转码，画质无损）");
        }
        self.log_push("警告: 视频背景持续解码播放，会增加耗电（笔记本用电池时更明显）");
        Some(VIDEO_FILE_NAME.to_string())
    }

    fn do_apply(&mut self) {
        let root = match self.root.clone() {
            Some(r) => r,
            None => {
                self.log_push("错误: 安装路径无效，无法应用补丁");
                return;
            }
        };
        let dist = root.join("resources").join("desktop-dist");
        let dark = match Self::process_slot(&self.dark.path, self.dark.crop) {
            Ok(v) => v,
            Err(e) => {
                self.log_push(&format!("深色图片处理失败: {e}"));
                return;
            }
        };
        let light = match Self::process_slot(&self.light.path, self.light.crop) {
            Ok(v) => v,
            Err(e) => {
                self.log_push(&format!("浅色图片处理失败: {e}"));
                return;
            }
        };
        if let Some(p) = &dark {
            self.log_processed("深色图", p, self.dark.crop.is_some());
        }
        if let Some(p) = &light {
            self.log_processed("浅色图", p, self.light.crop.is_some());
        }
        // 视频部署：app:// 协议不支持 Range 请求，MP4 必须 faststart（moov 在文件头），
        // 非 faststart 自动重封装（纯搬盒子、不转码）后再写入 desktop-dist 固定名
        let video_name = match self.video_path.clone() {
            Some(src) => {
                let dest = dist.join(VIDEO_FILE_NAME);
                self.deploy_video(&src, &dest)
            }
            None => {
                let dest = dist.join(VIDEO_FILE_NAME);
                if dest.exists() {
                    if let Err(e) = fs::remove_file(&dest) {
                        self.log_push(&format!("清理旧视频文件失败: {e:#}"));
                    }
                }
                None
            }
        };
        let patch = build_patch(
            dark.as_ref().map(|t| (t.b64.as_str(), self.dark.alpha, t.mime)),
            light.as_ref().map(|t| (t.b64.as_str(), self.light.alpha, t.mime)),
            self.side_alpha,
            self.panel_alpha,
        );
        match apply_patch(&root, patch.as_deref()) {
            Ok(css) => {
                if patch.is_some() {
                    self.log_push(&format!("补丁已写入: {}", css.display()));
                } else {
                    self.log_push("两个槽位均为空，已移除补丁（纯还原）");
                }
                match write_hot_files(&dist, patch.as_deref().unwrap_or(""), video_name.as_deref()) {
                    Ok(_) => {
                        if self.hot_enabled {
                            self.log_push("热更新已推送：运行中的 Kimi Code 约 2 秒内自动生效，无需重启");
                        } else {
                            self.log_push("需重启 Kimi Code 生效（或点「安装热更新」，之后无需重启）");
                        }
                    }
                    Err(e) => self.log_push(&format!("热更文件写入失败: {e:#}")),
                }
            }
            Err(e) => self.log_push(&format!("应用补丁失败: {e:#}")),
        }
        self.refresh();
    }

    fn do_restore(&mut self) {
        let root = match self.root.clone() {
            Some(r) => r,
            None => {
                self.log_push("错误: 安装路径无效，无法还原");
                return;
            }
        };
        match restore_patch(&root) {
            Ok(css) => {
                self.log_push(&format!("已从备份还原: {}", css.display()));
                let dist = root.join("resources").join("desktop-dist");
                match write_hot_files(&dist, "", None) {
                    Ok(_) => {
                        if self.hot_enabled {
                            self.log_push("热更新已推送：运行中的 Kimi Code 约 2 秒内自动生效，无需重启");
                        } else {
                            self.log_push("需重启 Kimi Code 生效（或点「安装热更新」，之后无需重启）");
                        }
                    }
                    Err(e) => self.log_push(&format!("热更文件写入失败: {e:#}")),
                }
            }
            Err(e) => self.log_push(&format!("还原失败: {e:#}")),
        }
        self.refresh();
    }

    fn do_install_hook(&mut self) {
        let root = match self.root.clone() {
            Some(r) => r,
            None => {
                self.log_push("错误: 安装路径无效，无法安装热更新");
                return;
            }
        };
        let dist = root.join("resources").join("desktop-dist");
        let index = dist.join("index.html");
        // 先探测是否旧版升级，用于区分结果日志
        let upgrading = fs::read_to_string(&index)
            .map(|c| hook_needs_upgrade(&c))
            .unwrap_or(false);
        let result = (|| -> anyhow::Result<()> {
            let content = fs::read_to_string(&index)
                .with_context(|| format!("读取 {} 失败", index.display()))?;
            let new_content = install_hook(&content)?;
            if new_content != content {
                let bak = backup_path(&index);
                if !bak.exists() {
                    fs::copy(&index, &bak)
                        .with_context(|| format!("备份 index.html 失败: {}", bak.display()))?;
                }
                fs::write(&index, new_content)
                    .with_context(|| format!("写入 index.html 失败: {}", index.display()))?;
            }
            // 放好初始热更文件，探针启动即有可拉取内容
            write_hot_files(&dist, "", None)?;
            Ok(())
        })();
        match result {
            Ok(_) if upgrading => {
                self.log_push("热更新已升级到 v2（支持视频背景），重启 Kimi Code 一次后生效")
            }
            Ok(_) => self.log_push("热更新已安装，重启 Kimi Code 一次后永久生效，之后打补丁无需重启"),
            Err(e) => self.log_push(&format!("安装热更新失败: {e:#}")),
        }
        self.refresh();
    }

    fn do_remove_hook(&mut self) {
        let root = match self.root.clone() {
            Some(r) => r,
            None => {
                self.log_push("错误: 安装路径无效，无法卸载热更新");
                return;
            }
        };
        let dist = root.join("resources").join("desktop-dist");
        let index = dist.join("index.html");
        let bak = backup_path(&index);
        let result = if bak.exists() {
            fs::copy(&bak, &index)
                .with_context(|| format!("从备份恢复 index.html 失败: {}", index.display()))
                .map(|_| ())
        } else {
            (|| -> anyhow::Result<()> {
                let content = fs::read_to_string(&index)
                    .with_context(|| format!("读取 {} 失败", index.display()))?;
                fs::write(&index, remove_hook(&content))
                    .with_context(|| format!("写入 index.html 失败: {}", index.display()))
            })()
        };
        match result {
            Ok(_) => self.log_push("热更新已卸载"),
            Err(e) => self.log_push(&format!("卸载热更新失败: {e:#}")),
        }
        self.refresh();
    }

    fn open_crop_editor(&mut self, kind: SlotKind, ctx: &egui::Context) {
        let slot = match kind {
            SlotKind::Dark => &self.dark,
            SlotKind::Light => &self.light,
        };
        let Some(path) = slot.path.clone() else { return };
        let texture = match load_preview(ctx, &path, 1024) {
            Ok(t) => t,
            Err(e) => {
                self.log_push(&format!("打开裁剪编辑器失败: {e:#}"));
                return;
            }
        };
        let aspect = CropAspect::Ratio16x9;
        let rect = slot.crop.unwrap_or_else(|| centered_max_crop(aspect.ratio()));
        self.crop_editor = Some(CropEditor {
            slot: kind,
            texture,
            rect,
            aspect,
            drag: CropDrag::None,
        });
    }

    fn show_crop_editor(&mut self, ctx: &egui::Context) {
        #[derive(PartialEq)]
        enum CropAction {
            None,
            Confirm,
            Clear,
        }
        let Some(mut editor) = self.crop_editor.take() else { return };
        let title = match editor.slot {
            SlotKind::Dark => "裁剪壁纸 - 深色背景",
            SlotKind::Light => "裁剪壁纸 - 浅色背景",
        };
        let mut open = true;
        let mut cancel = false;
        let mut action = CropAction::None;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                let prev_aspect = editor.aspect;
                egui::ComboBox::from_label("裁剪比例")
                    .selected_text(editor.aspect.label())
                    .show_ui(ui, |ui| {
                        for a in CropAspect::ALL {
                            ui.selectable_value(&mut editor.aspect, a, a.label());
                        }
                    });
                // 显示区：按图片比例适配到最大约 680×420
                let tex_size = editor.texture.size_vec2();
                let scale = (680.0 / tex_size.x).min(420.0 / tex_size.y).min(1.0);
                let disp_size = tex_size * scale;
                let min_norm = 40.0 / disp_size.x.min(disp_size.y);
                if editor.aspect != prev_aspect {
                    // 保持选区中心，按新比例重新适配并 clamp
                    let cx = editor.rect.x + editor.rect.w / 2.0;
                    let cy = editor.rect.y + editor.rect.h / 2.0;
                    editor.rect = match editor.aspect.ratio() {
                        Some(r) => {
                            let mut w = editor.rect.w.min(1.0);
                            let mut h = w / r;
                            if h > 1.0 {
                                h = 1.0;
                                w = h * r;
                            }
                            clamp_crop(
                                CropRect { x: cx - w / 2.0, y: cy - h / 2.0, w, h },
                                min_norm,
                            )
                        }
                        None => clamp_crop(editor.rect, min_norm),
                    };
                }
                ui.label("拖动红框移动，拖动四角缩放");
                let (img_rect, response) =
                    ui.allocate_exact_size(disp_size, egui::Sense::drag());
                let to_screen = |r: &CropRect| {
                    egui::Rect::from_min_size(
                        img_rect.min + egui::vec2(r.x * img_rect.width(), r.y * img_rect.height()),
                        egui::vec2(r.w * img_rect.width(), r.h * img_rect.height()),
                    )
                };
                // 拖拽：开始时判定命中区，一次拖拽期间不重新判定
                if response.drag_started() {
                    editor.drag = match response.interact_pointer_pos() {
                        Some(p) => hit_test(p, to_screen(&editor.rect)),
                        None => CropDrag::None,
                    };
                }
                if response.dragged() {
                    let delta = ctx.input(|i| i.pointer.delta());
                    if let Some(p) = response.interact_pointer_pos() {
                        match editor.drag {
                            CropDrag::Move => {
                                editor.rect.x += delta.x / img_rect.width();
                                editor.rect.y += delta.y / img_rect.height();
                            }
                            corner @ (CropDrag::Nw | CropDrag::Ne | CropDrag::Sw | CropDrag::Se) => {
                                let (ax, ay) = drag_anchor(editor.rect, corner);
                                let (sx, sy) = drag_sign(corner);
                                let px = (p.x - img_rect.min.x) / img_rect.width();
                                let py = (p.y - img_rect.min.y) / img_rect.height();
                                let dx = (px - ax).abs();
                                let dy = (py - ay).abs();
                                let ratio = editor.aspect.ratio();
                                let (mut w, mut h) = match ratio {
                                    Some(r) => {
                                        // 取与指针偏移更吻合的轴驱动，保持比例
                                        let (w1, h1) = (dx, dx / r);
                                        let (w2, h2) = (dy * r, dy);
                                        if (dy - h1).abs() <= (dx - w2).abs() {
                                            (w1, h1)
                                        } else {
                                            (w2, h2)
                                        }
                                    }
                                    None => (dx, dy),
                                };
                                if w < min_norm {
                                    w = min_norm;
                                    if let Some(r) = ratio {
                                        h = w / r;
                                    }
                                }
                                if h < min_norm {
                                    h = min_norm;
                                    if let Some(r) = ratio {
                                        w = h * r;
                                    }
                                }
                                if w > 1.0 {
                                    w = 1.0;
                                    if let Some(r) = ratio {
                                        h = w / r;
                                    }
                                }
                                if h > 1.0 {
                                    h = 1.0;
                                    if let Some(r) = ratio {
                                        w = h * r;
                                    }
                                }
                                let x = if sx > 0.0 { ax } else { ax - w };
                                let y = if sy > 0.0 { ay } else { ay - h };
                                editor.rect = clamp_crop(CropRect { x, y, w, h }, min_norm);
                            }
                            CropDrag::None => {}
                        }
                        editor.rect = clamp_crop(editor.rect, min_norm);
                    }
                }
                if response.drag_stopped() {
                    editor.drag = CropDrag::None;
                }
                // 绘制：纹理、框外四块半透明遮罩、红框描边、四角把手
                let painter = ui.painter_at(img_rect);
                painter.image(
                    editor.texture.id(),
                    img_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                let cr = to_screen(&editor.rect);
                let mask = egui::Color32::from_black_alpha(140);
                let lr = egui::Rect::from_min_max(img_rect.min, egui::pos2(cr.min.x, img_rect.max.y));
                let rr = egui::Rect::from_min_max(egui::pos2(cr.max.x, img_rect.min.y), img_rect.max);
                let tr = egui::Rect::from_min_max(egui::pos2(cr.min.x, img_rect.min.y), egui::pos2(cr.max.x, cr.min.y));
                let br = egui::Rect::from_min_max(egui::pos2(cr.min.x, cr.max.y), egui::pos2(cr.max.x, img_rect.max.y));
                for m in [tr, br, lr, rr] {
                    if m.is_positive() {
                        painter.rect_filled(m, 0.0, mask);
                    }
                }
                painter.rect_stroke(
                    cr,
                    0.0,
                    egui::Stroke::new(2.0_f32, egui::Color32::RED),
                    egui::StrokeKind::Inside,
                );
                for c in [cr.left_top(), cr.right_top(), cr.left_bottom(), cr.right_bottom()] {
                    painter.rect_filled(
                        egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0)),
                        0.0,
                        egui::Color32::RED,
                    );
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("确定").clicked() {
                        action = CropAction::Confirm;
                    }
                    if ui.button("取消").clicked() {
                        cancel = true;
                    }
                    if ui.button("重置选区").clicked() {
                        editor.rect = centered_max_crop(editor.aspect.ratio());
                        editor.drag = CropDrag::None;
                    }
                    if ui.button("清除裁剪").clicked() {
                        action = CropAction::Clear;
                    }
                });
            });
        match action {
            CropAction::Confirm => {
                let slot = match editor.slot {
                    SlotKind::Dark => &mut self.dark,
                    SlotKind::Light => &mut self.light,
                };
                slot.crop = Some(editor.rect);
                self.log_push("已保存裁剪选区");
                self.crop_editor = None;
            }
            CropAction::Clear => {
                let slot = match editor.slot {
                    SlotKind::Dark => &mut self.dark,
                    SlotKind::Light => &mut self.light,
                };
                slot.crop = None;
                self.log_push("已清除裁剪选区");
                self.crop_editor = None;
            }
            CropAction::None => {
                if open && !cancel {
                    self.crop_editor = Some(editor);
                }
            }
        }
    }
}

// ---------- 安装路径检测 ----------

fn is_valid_root(root: &Path) -> bool {
    root.join("resources").join("desktop-dist").join("index.html").is_file()
}

fn detect_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let mut found_loc: Option<PathBuf> = None;

    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for flags in [KEY_READ, KEY_READ | KEY_WOW64_32KEY] {
            let key = RegKey::predef(hive);
            let uninstall = match key.open_subkey_with_flags(
                "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
                flags,
            ) {
                Ok(k) => k,
                Err(_) => continue,
            };
            for subname in uninstall.enum_keys().flatten() {
                let sub = match uninstall.open_subkey(&subname) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let display: Result<String, _> = sub.get_value("DisplayName");
                if let Ok(name) = display {
                    if name.to_lowercase().contains("kimi") {
                        if let Ok(loc) = sub.get_value::<String, _>("InstallLocation") {
                            let loc = loc.trim().trim_matches('"').to_string();
                            if !loc.is_empty() {
                                found_loc = Some(PathBuf::from(loc));
                            }
                        }
                    }
                }
            }
        }
    }
    found_loc
}

fn common_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(la) = std::env::var("LOCALAPPDATA") {
        v.push(PathBuf::from(la).join("Programs").join("Kimi Code"));
    }
    v.push(PathBuf::from(r"C:\Program Files\Kimi Code"));
    v.push(PathBuf::from(r"C:\Program Files (x86)\Kimi Code"));
    v.push(PathBuf::from(r"T:\Kimi Code"));
    v
}

fn detect_install_path() -> Option<PathBuf> {
    if let Some(p) = detect_from_registry() {
        if is_valid_root(&p) {
            return Some(p);
        }
    }
    for c in common_candidates() {
        if is_valid_root(&c) {
            return Some(c);
        }
    }
    None
}

/// 用户可能选中 exe、resources 目录或 desktop-dist 目录，统一规范化到安装根目录。
fn normalize_root(input: &Path) -> Option<PathBuf> {
    let start: PathBuf = if input.is_file() {
        input.parent()?.to_path_buf()
    } else {
        input.to_path_buf()
    };
    let mut cur = Some(start.as_path());
    let mut levels = 0;
    while let Some(dir) = cur {
        if levels > 4 {
            break;
        }
        if is_valid_root(dir) {
            return Some(dir.to_path_buf());
        }
        if dir.join("desktop-dist").join("index.html").is_file() {
            if dir.file_name().map(|n| n == "resources").unwrap_or(false) {
                return dir.parent().map(|p| p.to_path_buf());
            }
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
        levels += 1;
    }
    None
}

// ---------- CSS 定位 / 补丁 ----------

fn locate_main_css(root: &Path) -> anyhow::Result<PathBuf> {
    let index = root
        .join("resources")
        .join("desktop-dist")
        .join("index.html");
    let html = fs::read_to_string(&index)
        .with_context(|| format!("读取 {} 失败", index.display()))?;
    let re = regex::Regex::new(r#"href="/assets/(main-[^"]+\.css)""#)?;
    let name = re
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());
    let name = match name {
        Some(n) => n,
        None => {
            let re2 = regex::Regex::new(r#"href=["']/assets/(main-[^"']+\.css)["']"#)?;
            re2.captures(&html)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| anyhow!("index.html 中未找到 /assets/main-*.css 引用"))?
        }
    };
    let css = root
        .join("resources")
        .join("desktop-dist")
        .join("assets")
        .join(&name);
    if !css.is_file() {
        bail!("主样式表文件不存在: {}", css.display());
    }
    Ok(css)
}

fn backup_path(css: &Path) -> PathBuf {
    let mut s = css.as_os_str().to_os_string();
    s.push(".wallpaper-bak");
    PathBuf::from(s)
}

fn strip_patch(css: &str) -> String {
    let mut out = css.to_string();
    while let Some(start) = out.find(PATCH_START) {
        match out[start..].find(PATCH_END_MARK) {
            Some(rel) => {
                let end = start + rel + PATCH_END_MARK.len();
                out.replace_range(start..end, "");
            }
            None => break,
        }
    }
    out
}

fn build_patch(
    dark: Option<(&str, f32, &str)>,
    light: Option<(&str, f32, &str)>,
    side_alpha: f32,
    panel_alpha: f32,
) -> Option<String> {
    if dark.is_none() && light.is_none() {
        return None;
    }
    let sa = format!("{side_alpha:.2}");
    let pa = format!("{panel_alpha:.2}");
    let mut out = String::from(PATCH_HEADER);
    out.push('\n');
    if let Some((b64, da, mime)) = dark {
        out.push_str(
            &DARK_TEMPLATE
                .replace("{B64}", b64)
                .replace("{DA}", &format!("{da:.2}"))
                .replace("{SA}", &sa)
                .replace("{PA}", &pa)
                .replace("{MIME}", mime),
        );
    }
    if let Some((b64, la, mime)) = light {
        out.push_str(
            &LIGHT_TEMPLATE
                .replace("{B64}", b64)
                .replace("{LA}", &format!("{la:.2}"))
                .replace("{SA}", &sa)
                .replace("{PA}", &pa)
                .replace("{MIME}", mime),
        );
    }
    out.push_str("/* === kimi-wallpaper-patch end === */\n");
    Some(out)
}

/// patch 为 None 时只移除现有补丁（纯还原）。
fn apply_patch(root: &Path, patch: Option<&str>) -> anyhow::Result<PathBuf> {
    let css = locate_main_css(root)?;
    let bak = backup_path(&css);
    if !bak.exists() {
        fs::copy(&css, &bak).with_context(|| format!("创建备份失败: {}", bak.display()))?;
    }
    let content = fs::read_to_string(&css)
        .with_context(|| format!("读取样式表失败: {}", css.display()))?;
    let stripped = strip_patch(&content);
    let mut new = stripped.trim_end().to_string();
    match patch {
        Some(p) => {
            new.push_str("\n\n");
            new.push_str(p);
            if !new.ends_with('\n') {
                new.push('\n');
            }
        }
        None => {
            new.push('\n');
        }
    }
    fs::write(&css, new).with_context(|| format!("写入样式表失败: {}", css.display()))?;
    Ok(css)
}

fn restore_patch(root: &Path) -> anyhow::Result<PathBuf> {
    let css = locate_main_css(root)?;
    let bak = backup_path(&css);
    if !bak.exists() {
        bail!("未找到备份文件: {}", bak.display());
    }
    fs::copy(&bak, &css).with_context(|| format!("还原失败: {}", css.display()))?;
    Ok(css)
}

// ---------- 热更新 ----------

fn hot_css_path(dist_root: &Path) -> PathBuf {
    dist_root.join("kimi-wallpaper-hot.css")
}

fn hot_json_path(dist_root: &Path) -> PathBuf {
    dist_root.join("kimi-wallpaper-hot.json")
}

/// 写热更 css（空字符串=无壁纸）和版本文件 {"v":N,"video":"name"|null}。
/// 先写 css 后写 json（探针以 json 为准）。
fn write_hot_files(dist_root: &Path, patch_css: &str, video: Option<&str>) -> anyhow::Result<()> {
    let re = regex::Regex::new(r#""v"\s*:\s*(\d+)"#)?;
    let old_v: u64 = fs::read_to_string(hot_json_path(dist_root))
        .ok()
        .and_then(|s| {
            re.captures(&s)?
                .get(1)?
                .as_str()
                .parse::<u64>()
                .ok()
        })
        .unwrap_or(0);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let new_v = old_v.saturating_add(1).max(now_ms);
    let json = match video {
        Some(name) => format!("{{\"v\":{new_v},\"video\":\"{name}\"}}"),
        None => format!("{{\"v\":{new_v},\"video\":null}}"),
    };
    fs::write(hot_css_path(dist_root), patch_css).context("写入热更 css 失败")?;
    fs::write(hot_json_path(dist_root), json).context("写入热更版本文件失败")?;
    Ok(())
}

fn hook_installed(index_html_content: &str) -> bool {
    index_html_content.contains(HOT_HOOK_START)
}

/// 已装探针但缺 v2 版本标记（旧版 v1）时需要升级。
fn hook_needs_upgrade(index_html_content: &str) -> bool {
    hook_installed(index_html_content) && !index_html_content.contains(HOOK_VERSION_MARK)
}

/// 在最后一个 </body> 前插入热更新探针；已是最新 v2 时原样返回（幂等），
/// 已装旧版则先移除旧块再插入新块（升级）。
fn install_hook(index_html_content: &str) -> anyhow::Result<String> {
    if hook_installed(index_html_content) && !hook_needs_upgrade(index_html_content) {
        return Ok(index_html_content.to_string());
    }
    let base = if hook_needs_upgrade(index_html_content) {
        remove_hook(index_html_content)
    } else {
        index_html_content.to_string()
    };
    let pos = base
        .rfind("</body>")
        .ok_or_else(|| anyhow!("index.html 中未找到 </body>，无法植入热更新探针"))?;
    let mut out = String::with_capacity(base.len() + HOT_HOOK_BLOCK.len() + 1);
    out.push_str(&base[..pos]);
    out.push_str(HOT_HOOK_BLOCK);
    out.push('\n');
    out.push_str(&base[pos..]);
    Ok(out)
}

/// 删除整段探针（含标记行），幂等。
fn remove_hook(index_html_content: &str) -> String {
    let Some(start) = index_html_content.find(HOT_HOOK_START) else {
        return index_html_content.to_string();
    };
    let Some(rel) = index_html_content[start..].find(HOT_HOOK_END) else {
        return index_html_content.to_string();
    };
    let end = start + rel + HOT_HOOK_END.len();
    // 探针块首行自带缩进（插入时就在标记之前），连同其后插入的换行一起删除，
    // 这样 remove(install(x)) == x 精确还原
    let block_indent = HOT_HOOK_BLOCK.len() - HOT_HOOK_BLOCK.trim_start().len();
    let removal_start = start.saturating_sub(block_indent);
    let mut out = String::with_capacity(index_html_content.len());
    out.push_str(&index_html_content[..removal_start]);
    let rest = index_html_content[end..].strip_prefix('\n').unwrap_or(&index_html_content[end..]);
    out.push_str(rest);
    out
}

// ---------- MP4 faststart ----------

/// MP4 容器 box 类型（嵌套子 box；其余视为叶子）。
const MP4_CONTAINERS: [[u8; 4]; 8] = [
    *b"moov", *b"trak", *b"mdia", *b"minf", *b"stbl", *b"edts", *b"udta", *b"dinf",
];

/// 解析顶层 box 序列：每个 box 为 4 字节大端 size + 4 字节 type；
/// size==1 时紧跟 8 字节 largesize；size==0 表示到文件尾。
/// 返回 (type, 起始偏移, 总大小含头)。
fn parse_top_boxes(data: &[u8]) -> anyhow::Result<Vec<([u8; 4], usize, u64)>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < data.len() {
        let head = data
            .get(pos..pos + 8)
            .ok_or_else(|| anyhow!("MP4 顶层 box 头部不完整（偏移 {pos}）"))?;
        let size32 = u32::from_be_bytes(head[0..4].try_into().unwrap());
        let typ: [u8; 4] = head[4..8].try_into().unwrap();
        let (header, size) = match size32 {
            1 => {
                let ext = data
                    .get(pos + 8..pos + 16)
                    .ok_or_else(|| anyhow!("MP4 largesize box 头部不完整（偏移 {pos}）"))?;
                (16u64, u64::from_be_bytes(ext.try_into().unwrap()))
            }
            0 => (8u64, data.len() as u64 - pos as u64),
            s => (8u64, s as u64),
        };
        if size < header {
            bail!("MP4 box {:?} size 非法: {size} < 头 {header}", String::from_utf8_lossy(&typ));
        }
        let end = pos as u64 + size;
        if end > data.len() as u64 {
            bail!(
                "MP4 box {:?} 越界: 结束 {end} > 文件长度 {}",
                String::from_utf8_lossy(&typ),
                data.len()
            );
        }
        out.push((typ, pos, size));
        pos = end as usize;
    }
    Ok(out)
}

/// moov 是否在 mdat 之前（faststart 判定）。找不到任一个则 Err。
fn mp4_moov_before_mdat(data: &[u8]) -> anyhow::Result<bool> {
    let boxes = parse_top_boxes(data)?;
    let moov = boxes
        .iter()
        .find(|b| b.0 == *b"moov")
        .map(|b| b.1)
        .ok_or_else(|| anyhow!("未找到 moov box（可能不是 MP4）"))?;
    let mdat = boxes
        .iter()
        .find(|b| b.0 == *b"mdat")
        .map(|b| b.1)
        .ok_or_else(|| anyhow!("未找到 mdat box（可能不是 MP4）"))?;
    Ok(moov < mdat)
}

/// 修正单个 stco/co64 box 的 chunk 偏移。body 含 8 字节 box 头，
/// 其后 version/flags 4 字节 + entry_count 4 字节 + N 个 u32/u64 大端 entry。
fn fix_chunk_offsets(body: &mut [u8], delta: i64, wide: bool) -> anyhow::Result<()> {
    let esz = if wide { 8usize } else { 4 };
    if body.len() < 16 {
        bail!("stco/co64 box 过短: {} 字节", body.len());
    }
    let entry_count = u32::from_be_bytes(body[12..16].try_into().unwrap()) as usize;
    let need = entry_count
        .checked_mul(esz)
        .and_then(|n| n.checked_add(16))
        .ok_or_else(|| anyhow!("stco/co64 entry_count 非法"))?;
    if body.len() < need {
        bail!(
            "stco/co64 entry 越界: 声明 {entry_count} 条需 {need} 字节，实际 {} 字节",
            body.len()
        );
    }
    for i in 0..entry_count {
        let off = 16 + i * esz;
        if wide {
            let v = u64::from_be_bytes(body[off..off + 8].try_into().unwrap());
            let nv = (v as i64)
                .checked_add(delta)
                .ok_or_else(|| anyhow!("co64 chunk 偏移 {v} 加 delta {delta} 溢出"))?;
            if nv < 0 {
                bail!("co64 chunk 偏移 {v} 加 delta {delta} 为负");
            }
            body[off..off + 8].copy_from_slice(&(nv as u64).to_be_bytes());
        } else {
            let v = u32::from_be_bytes(body[off..off + 4].try_into().unwrap());
            let nv = (v as i64)
                .checked_add(delta)
                .ok_or_else(|| anyhow!("stco chunk 偏移 {v} 加 delta {delta} 溢出"))?;
            if !(0..=u32::MAX as i64).contains(&nv) {
                bail!("stco chunk 偏移 {v} 加 delta {delta} 越界");
            }
            body[off..off + 4].copy_from_slice(&(nv as u32).to_be_bytes());
        }
    }
    Ok(())
}

/// 递归修正 moov 子树里的 stco/co64：容器 box 进树，叶子 stco/co64 修偏移。
fn fix_moov_tree(region: &mut [u8], delta: i64) -> anyhow::Result<()> {
    let mut pos = 0usize;
    while pos + 8 <= region.len() {
        let size32 = u32::from_be_bytes(region[pos..pos + 4].try_into().unwrap());
        let typ: [u8; 4] = region[pos + 4..pos + 8].try_into().unwrap();
        let (header, size) = match size32 {
            1 => {
                if pos + 16 > region.len() {
                    bail!("moov 内嵌 largesize box 头部不完整（偏移 {pos}）");
                }
                let ext: [u8; 8] = region[pos + 8..pos + 16].try_into().unwrap();
                (16usize, u64::from_be_bytes(ext))
            }
            0 => (8usize, region.len() as u64 - pos as u64),
            s => (8usize, s as u64),
        };
        if size < header as u64 || pos as u64 + size > region.len() as u64 {
            bail!(
                "moov 内嵌 box {:?} 大小非法（偏移 {pos}）",
                String::from_utf8_lossy(&typ)
            );
        }
        let size = size as usize;
        let body = &mut region[pos..pos + size];
        if typ == *b"stco" {
            fix_chunk_offsets(body, delta, false)?;
        } else if typ == *b"co64" {
            fix_chunk_offsets(body, delta, true)?;
        } else if MP4_CONTAINERS.contains(&typ) {
            fix_moov_tree(&mut body[header..], delta)?;
        }
        pos += size;
    }
    Ok(())
}

/// 重封装为 faststart 布局：[mdat 之前的非 moov boxes] + moov + mdat + [其余 boxes]，
/// 各 box 原始字节不变；moov 内的 stco/co64 chunk 偏移按 mdat 位移量修正（delta 可正可负）。
/// moov 已在 mdat 前则直接返回原数据拷贝。
fn faststart_remux(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let boxes = parse_top_boxes(data)?;
    let moov = boxes
        .iter()
        .find(|b| b.0 == *b"moov")
        .ok_or_else(|| anyhow!("未找到 moov box"))?;
    let mdat = boxes
        .iter()
        .find(|b| b.0 == *b"mdat")
        .ok_or_else(|| anyhow!("未找到 mdat box"))?;
    if moov.1 < mdat.1 {
        return Ok(data.to_vec());
    }
    // mdat 新起始偏移 = 原 mdat 之前所有 box 的字节和（moov 在其后，天然不含）+ moov 大小
    let mdat_new_off: u64 = boxes
        .iter()
        .filter(|b| b.1 < mdat.1)
        .map(|b| b.2)
        .sum::<u64>()
        + moov.2;
    let delta = mdat_new_off as i64 - mdat.1 as i64;
    // 修正 moov 副本内的 chunk 偏移
    let mut moov_bytes = data[moov.1..moov.1 + moov.2 as usize].to_vec();
    let moov_header = if u32::from_be_bytes(moov_bytes[0..4].try_into().unwrap()) == 1 {
        16usize
    } else {
        8usize
    };
    fix_moov_tree(&mut moov_bytes[moov_header..], delta)?;
    // 组装新布局：[mdat 之前的 boxes] + moov + mdat + [其余 boxes]
    let mut out = Vec::with_capacity(data.len());
    for b in &boxes {
        if b.1 < mdat.1 {
            out.extend_from_slice(&data[b.1..b.1 + b.2 as usize]);
        }
    }
    out.extend_from_slice(&moov_bytes);
    out.extend_from_slice(&data[mdat.1..mdat.1 + mdat.2 as usize]);
    for b in &boxes {
        if b.1 > mdat.1 && b.0 != *b"moov" {
            out.extend_from_slice(&data[b.1..b.1 + b.2 as usize]);
        }
    }
    Ok(out)
}

// ---------- 图片处理 ----------

struct ProcessedImage {
    b64: String,
    orig_len: usize,
    comp_len: usize,
    cropped: Option<(u32, u32)>,
    mime: &'static str,
}

fn process_image(path: &Path, crop: Option<CropRect>) -> anyhow::Result<ProcessedImage> {
    use base64::Engine as _;
    use image::ImageEncoder;

    let data = fs::read(path).with_context(|| format!("读取图片失败: {}", path.display()))?;
    let orig_len = data.len();
    // GIF 不解码、不压缩：原始字节直接 base64 内嵌，保留动画
    if path.extension().map(|e| e.eq_ignore_ascii_case("gif")).unwrap_or(false) {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
        return Ok(ProcessedImage {
            b64,
            orig_len,
            comp_len: orig_len,
            cropped: None,
            mime: "image/gif",
        });
    }
    let img = image::load_from_memory(&data).context("解析图片失败（格式不支持或文件损坏）")?;
    // 先按归一化选区裁剪，再走现有缩放/JPEG 流程
    let (img, cropped) = if let Some(c) = crop {
        let iw = img.width();
        let ih = img.height();
        let x = (c.x * iw as f32).round().clamp(0.0, iw.saturating_sub(1) as f32) as u32;
        let y = (c.y * ih as f32).round().clamp(0.0, ih.saturating_sub(1) as f32) as u32;
        let w = (c.w * iw as f32).round().clamp(1.0, iw.saturating_sub(x) as f32) as u32;
        let h = (c.h * ih as f32).round().clamp(1.0, ih.saturating_sub(y) as f32) as u32;
        let buf = image::imageops::crop_imm(&img, x, y, w, h).to_image();
        (image::DynamicImage::ImageRgba8(buf), Some((w, h)))
    } else {
        (img, None)
    };
    let (w, h) = (img.width(), img.height());
    let rgba = if w.max(h) > MAX_TEXTURE_SIDE {
        let (nw, nh) = if w >= h {
            (
                MAX_TEXTURE_SIDE,
                ((h as u64 * MAX_TEXTURE_SIDE as u64) / w as u64).max(1) as u32,
            )
        } else {
            (
                ((w as u64 * MAX_TEXTURE_SIDE as u64) / h as u64).max(1) as u32,
                MAX_TEXTURE_SIDE,
            )
        };
        image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3)
    } else {
        img.to_rgba8()
    };
    let rgb = image::DynamicImage::ImageRgba8(rgba).into_rgb8();
    let (w, h) = (rgb.width(), rgb.height());
    let encode = |q: u8| -> anyhow::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, q);
        enc.write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)?;
        Ok(buf)
    };
    let mut buf = encode(82)?;
    if buf.len() > MAX_JPEG_BYTES {
        buf = encode(70)?;
    }
    let comp_len = buf.len();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    Ok(ProcessedImage { b64, orig_len, comp_len, cropped, mime: "image/jpeg" })
}

fn load_preview(ctx: &egui::Context, path: &Path, max_side: u32) -> anyhow::Result<egui::TextureHandle> {
    let img = image::open(path).context("预览加载失败")?;
    let (w, h) = (img.width(), img.height());
    let scale = max_side as f32 / w.max(h) as f32;
    let (nw, nh) = if scale < 1.0 {
        (
            ((w as f32 * scale).round() as u32).max(1),
            ((h as f32 * scale).round() as u32).max(1),
        )
    } else {
        (w, h)
    };
    let small = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle);
    let ci = egui::ColorImage::from_rgba_unmultiplied(
        [nw as usize, nh as usize],
        small.as_raw(),
    );
    Ok(ctx.load_texture(
        path.display().to_string(),
        ci,
        egui::TextureOptions::default(),
    ))
}

// ---------- UI ----------

fn slot_ui(ui: &mut egui::Ui, title: &str, slot: &mut Slot, ctx: &egui::Context, log: &mut String) -> bool {
    let mut crop_requested = false;
    ui.group(|ui| {
        ui.label(egui::RichText::new(title).strong());
        let size = egui::vec2(230.0, 132.0);
        if let Some(tex) = &slot.texture {
            ui.image((tex.id(), size));
        } else {
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter().rect_filled(
                rect,
                4.0,
                egui::Color32::from_gray(28),
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "未选择图片",
                egui::FontId::proportional(13.0),
                egui::Color32::GRAY,
            );
        }
        if slot.crop.is_some() {
            ui.colored_label(egui::Color32::from_rgb(120, 220, 120), "已裁剪");
        }
        let is_gif = slot
            .path
            .as_ref()
            .and_then(|p| p.extension())
            .map(|e| e.eq_ignore_ascii_case("gif"))
            .unwrap_or(false);
        if is_gif {
            ui.colored_label(
                egui::Color32::from_rgb(230, 160, 60),
                "GIF 动图持续解码播放，会增加耗电（笔记本用电池时更明显）",
            );
        }
        ui.horizontal(|ui| {
            if ui.button("选择图片").clicked() {
                let picked = rfd::FileDialog::new()
                    .add_filter("图片文件", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
                    .pick_file();
                if let Some(f) = picked {
                    match load_preview(ctx, &f, 240) {
                        Ok(tex) => {
                            slot.path = Some(f.clone());
                            slot.texture = Some(tex);
                            slot.crop = None;
                            log.push_str(&format!("已选择{}图片: {}\n", title, f.display()));
                        }
                        Err(e) => log.push_str(&format!("加载预览失败: {e:#}\n")),
                    }
                }
            }
            if ui.button("清除").clicked() {
                slot.path = None;
                slot.texture = None;
                slot.crop = None;
            }
            if ui
                .add_enabled(slot.path.is_some() && !is_gif, egui::Button::new("裁剪..."))
                .clicked()
            {
                crop_requested = true;
            }
        });
        ui.add(
            egui::Slider::new(&mut slot.alpha, 0.0..=0.95)
                .text("遮罩透明度")
                .fixed_decimals(2),
        );
    });
    crop_requested
}

impl eframe::App for BgToolApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("安装路径:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.install_path)
                        .desired_width(340.0)
                        .hint_text("Kimi Code 安装目录"),
                );
                if ui.button("浏览...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.install_path = dir.display().to_string();
                        self.refresh();
                    }
                }
                if ui.button("选 exe...").clicked() {
                    if let Some(f) = rfd::FileDialog::new()
                        .add_filter("可执行文件", &["exe"])
                        .pick_file()
                    {
                        self.install_path = f.display().to_string();
                        self.refresh();
                    }
                }
            });

            ui.horizontal(|ui| {
                match &self.css_name {
                    Some(name) => {
                        ui.label(format!("主样式表: {}", name));
                        if self.patched {
                            ui.colored_label(egui::Color32::from_rgb(120, 220, 120), "已打补丁");
                        } else {
                            ui.colored_label(egui::Color32::LIGHT_GRAY, "未打补丁");
                        }
                    }
                    None => {
                        let msg = self
                            .status_err
                            .clone()
                            .unwrap_or_else(|| "未检测到 Kimi Code 安装".to_string());
                        ui.colored_label(egui::Color32::from_rgb(230, 200, 90), msg);
                    }
                }
                if self.bak_exists {
                    ui.colored_label(egui::Color32::LIGHT_BLUE, "备份存在");
                } else {
                    ui.colored_label(egui::Color32::DARK_GRAY, "无备份");
                }
                if self.hot_enabled {
                    if self.hot_needs_upgrade {
                        ui.colored_label(egui::Color32::from_rgb(230, 200, 90), "热更新: 需升级");
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(120, 220, 120), "热更新: 已启用 v2");
                    }
                } else {
                    ui.colored_label(egui::Color32::DARK_GRAY, "热更新: 未启用");
                }
            });

            ui.separator();

            let mut log = std::mem::take(&mut self.log);
            let mut crop_req: Option<SlotKind> = None;
            ui.columns(2, |cols| {
                if slot_ui(&mut cols[0], "深色背景", &mut self.dark, ctx, &mut log) {
                    crop_req = Some(SlotKind::Dark);
                }
                if slot_ui(&mut cols[1], "浅色背景", &mut self.light, ctx, &mut log) {
                    crop_req = Some(SlotKind::Light);
                }
            });
            self.log = log;
            if let Some(kind) = crop_req {
                self.open_crop_editor(kind, ctx);
            }

            ui.group(|ui| {
                ui.label(egui::RichText::new("视频背景（MP4）").strong());
                match &self.video_path {
                    Some(p) => {
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        ui.label(format!("已选择: {name}"));
                    }
                    None => {
                        ui.label("未选择视频");
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("选择视频").clicked() {
                        let picked = rfd::FileDialog::new()
                            .add_filter("MP4 视频", &["mp4"])
                            .pick_file();
                        if let Some(f) = picked {
                            self.log_push(&format!("已选择视频: {}", f.display()));
                            self.video_path = Some(f);
                        }
                    }
                    if ui
                        .add_enabled(self.video_path.is_some(), egui::Button::new("清除"))
                        .clicked()
                    {
                        self.video_path = None;
                    }
                });
                ui.colored_label(
                    egui::Color32::from_rgb(230, 160, 60),
                    "视频/动图背景持续占用 CPU/GPU，会增加耗电（笔记本用电池时更明显）",
                );
            });

            ui.add(
                egui::Slider::new(&mut self.side_alpha, 0.0..=0.95)
                    .text("侧边栏/标题栏透明度")
                    .fixed_decimals(2),
            );
            ui.add(
                egui::Slider::new(&mut self.panel_alpha, 0.0..=0.95)
                    .text("面板透明度（右侧预览/聊天面板）")
                    .fixed_decimals(2),
            );

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let apply_btn = egui::Button::new(egui::RichText::new("应用补丁").strong())
                    .min_size(egui::vec2(120.0, 30.0));
                if ui.add(apply_btn).clicked() {
                    self.do_apply();
                }
                if ui
                    .add(egui::Button::new("还原原版").min_size(egui::vec2(100.0, 30.0)))
                    .clicked()
                {
                    self.do_restore();
                }
                if self.hot_enabled && !self.hot_needs_upgrade {
                    if ui
                        .add(egui::Button::new("卸载热更新").min_size(egui::vec2(100.0, 30.0)))
                        .clicked()
                    {
                        self.do_remove_hook();
                    }
                } else {
                    let label = if self.hot_needs_upgrade {
                        "升级热更新"
                    } else {
                        "安装热更新"
                    };
                    if ui
                        .add(egui::Button::new(label).min_size(egui::vec2(100.0, 30.0)))
                        .clicked()
                    {
                        self.do_install_hook();
                    }
                }
            });

            ui.separator();
            ui.label("日志:");
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(f32::INFINITY)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.log)
                            .desired_width(f32::INFINITY)
                            .interactive(false)
                            .font(egui::TextStyle::Monospace),
                    );
                });
        });

        self.show_crop_editor(ctx);
        self.update_persistence(ctx);
    }
}

/// 加载 Windows 系统中文字体，修复 egui 默认字体无 CJK 字形导致的方框问题。
fn install_cjk_font(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};
    use std::sync::Arc;

    // 依次尝试：微软雅黑(TTC) / 微软雅黑(TTF) / 黑体 / 宋体
    for path in [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            let mut data = FontData::from_owned(bytes);
            data.index = 0; // ttc 集合取第一个字体
            let mut fonts = FontDefinitions::default();
            fonts.font_data.insert("cjk".to_owned(), Arc::new(data));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "cjk".to_owned());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .push("cjk".to_owned());
            ctx.set_fonts(fonts);
            return;
        }
    }
    // 找不到中文字体：静默保持默认字体，不 panic
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Kimi Code 背景更换工具")
            .with_inner_size([760.0, 700.0])
            .with_min_inner_size([680.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "kimi-bg-tool",
        options,
        Box::new(|cc| {
            install_cjk_font(&cc.egui_ctx);
            Ok(Box::new(BgToolApp::new(cc)))
        }),
    )
}

// ---------- 单元测试 ----------

#[cfg(test)]
mod tests {
    use super::*;

    const ORIG_CSS: &str = "body { color: red; }\n";

    fn make_fixture(tag: &str) -> (PathBuf, PathBuf) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-fixture")
            .join(tag);
        let _ = fs::remove_dir_all(&root);
        let dd = root.join("resources").join("desktop-dist");
        let assets = dd.join("assets");
        fs::create_dir_all(&assets).unwrap();
        fs::write(
            dd.join("index.html"),
            r#"<html><head><link rel="stylesheet" href="/assets/main-abc123.css"></head><body></body></html>"#,
        )
        .unwrap();
        let css_path = assets.join("main-abc123.css");
        fs::write(&css_path, ORIG_CSS).unwrap();
        (root, css_path)
    }

    fn make_test_image(path: &Path) {
        let mut img = image::RgbImage::new(32, 32);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgb([(x * 8) as u8, (y * 8) as u8, 128]);
        }
        img.save(path).unwrap();
    }

    #[test]
    fn test_apply_patch_dark_only() {
        let (root, css_path) = make_fixture("dark-only");
        let img_path = root.join("dark.png");
        make_test_image(&img_path);

        let p = process_image(&img_path, None).unwrap();
        assert!(p.orig_len > 0 && p.comp_len > 0);
        assert!(!p.b64.is_empty());

        let patch = build_patch(Some((&p.b64, 0.80, "image/jpeg")), None, 0.55, 0.25).unwrap();
        apply_patch(&root, Some(&patch)).unwrap();

        let bak = backup_path(&css_path);
        assert!(bak.is_file(), "备份文件应被创建");
        assert_eq!(fs::read_to_string(&bak).unwrap(), ORIG_CSS, "备份应等于原始内容");

        let css = fs::read_to_string(&css_path).unwrap();
        assert!(css.contains("--kimi-wallpaper-dark"));
        assert!(!css.contains("--kimi-wallpaper-light"));
        assert_eq!(css.matches(PATCH_HEADER).count(), 1, "补丁段应只有一段");
        assert!(css.contains("rgba(7,7,13,0.80)"), "深色遮罩透明度应格式化两位小数");
        assert!(css.contains("rgba(10,10,17,0.55)"), "侧边栏透明度");
        assert!(css.contains(".global-preview"), "深色补丁应包含面板规则");
        assert!(css.contains(".agent-panel"), "深色补丁应包含 agent 面板规则");
        assert!(css.contains(".fp-body"), "深色补丁应包含 fp-body 规则");
        assert!(css.contains(".ui-panel-header"), "深色补丁应包含面板标题栏规则");
        assert!(css.contains("rgba(10,10,17,0.25)"), "深色面板透明度 PA=0.25");
        assert!(css.contains("html[data-color-scheme=dark]"));
        assert!(css.contains("@media (prefers-color-scheme:dark)"));

        // 再次应用：不重复追加补丁段，备份不被覆盖
        let patch2 = build_patch(Some((&p.b64, 0.80, "image/jpeg")), None, 0.55, 0.25).unwrap();
        apply_patch(&root, Some(&patch2)).unwrap();
        let css2 = fs::read_to_string(&css_path).unwrap();
        assert_eq!(css2.matches(PATCH_HEADER).count(), 1, "重复应用不应产生重复补丁段");
        assert_eq!(fs::read_to_string(&bak).unwrap(), ORIG_CSS, "重复应用不应覆盖备份");
    }

    #[test]
    fn test_apply_patch_both_slots() {
        let (root, css_path) = make_fixture("both-slots");
        let dark_png = root.join("dark.png");
        let light_png = root.join("light.png");
        make_test_image(&dark_png);
        make_test_image(&light_png);
        let d = process_image(&dark_png, None).unwrap();
        let l = process_image(&light_png, None).unwrap();

        let patch = build_patch(Some((&d.b64, 0.80, "image/jpeg")), Some((&l.b64, 0.78, "image/jpeg")), 0.55, 0.25).unwrap();
        apply_patch(&root, Some(&patch)).unwrap();

        let css = fs::read_to_string(&css_path).unwrap();
        assert!(css.contains("--kimi-wallpaper-dark"));
        assert!(css.contains("--kimi-wallpaper-light"));
        assert_eq!(css.matches(":root{--kimi-wallpaper-").count(), 2);
        assert_eq!(css.matches(PATCH_HEADER).count(), 1);
        assert!(css.contains("rgba(250,250,252,0.78)"));
        assert!(css.contains("rgba(255,255,255,0.55)"));
        assert!(css.contains(".global-preview"), "浅色补丁应包含面板规则");
        assert!(css.contains("rgba(255,255,255,0.25)"), "浅色面板透明度 PA=0.25");
        assert!(css.contains(".chat-header"), "补丁应包含顶部标签条半透明规则");
        assert!(css.contains(".chat-dock:before"), "补丁应包含底部输入区遮帘规则");
    }

    #[test]
    fn test_patch_fixed_cover() {
        let patch = build_patch(Some((&"x".repeat(8), 0.80, "image/jpeg")), None, 0.55, 0.25).unwrap();
        assert!(patch.contains("background-size:cover,cover"));
        assert!(patch.contains("background-position:center,center"));
        assert!(!patch.contains("{BS}") && !patch.contains("{BP}"), "不应再有填充模式占位符");
    }

    #[test]
    fn test_empty_apply_removes_patch_and_restore() {
        let (root, css_path) = make_fixture("restore");
        let img_path = root.join("dark.png");
        make_test_image(&img_path);
        let p = process_image(&img_path, None).unwrap();
        let patch = build_patch(Some((&p.b64, 0.80, "image/jpeg")), None, 0.55, 0.25).unwrap();
        apply_patch(&root, Some(&patch)).unwrap();
        assert!(fs::read_to_string(&css_path).unwrap().contains(PATCH_START));

        // 两个槽位都为空 -> 只删不加
        apply_patch(&root, None).unwrap();
        let css = fs::read_to_string(&css_path).unwrap();
        assert!(!css.contains(PATCH_START), "空槽位应用应移除补丁段");
        assert_eq!(css, ORIG_CSS, "移除补丁后应恢复原始内容");

        // 重新打上再还原
        apply_patch(&root, Some(&patch)).unwrap();
        restore_patch(&root).unwrap();
        assert_eq!(fs::read_to_string(&css_path).unwrap(), ORIG_CSS, "还原应恢复备份内容");
    }

    #[test]
    fn test_normalize_root_variants() {
        let (root, _) = make_fixture("normalize");
        let exe = root.join("Kimi.exe");
        fs::write(&exe, b"fake").unwrap();
        assert_eq!(normalize_root(&exe).unwrap(), root, "选中 exe 应定位到安装根目录");
        assert_eq!(normalize_root(&root.join("resources")).unwrap(), root, "选中 resources 目录");
        assert_eq!(
            normalize_root(&root.join("resources").join("desktop-dist")).unwrap(),
            root,
            "选中 desktop-dist 目录"
        );
        assert_eq!(normalize_root(&root).unwrap(), root);
        assert!(normalize_root(Path::new("C:\\\\no-such-dir-xyz")).is_none());
    }

    #[test]
    fn test_locate_main_css_from_index_html() {
        let (root, css_path) = make_fixture("locate");
        assert_eq!(locate_main_css(&root).unwrap(), css_path);
    }

    #[test]
    fn test_install_hook_and_remove() {
        let original = "<html>\n  <body>\n    <div>x</div>\n  </body>\n</html>\n";
        let installed = install_hook(original).unwrap();
        assert!(installed.contains(HOT_HOOK_START));
        assert!(installed.contains(HOT_HOOK_END));
        assert!(installed.contains(HOOK_VERSION_MARK), "新装探针应为 v2");
        assert!(installed.contains("setInterval(tick, 2000)"));
        assert!(
            installed.contains("fetch('/kimi-wallpaper-hot.json"),
            "探针必须用绝对路径拉取版本文件，否则进入 /sessions/ 路由后会 404 失效"
        );
        assert!(installed.contains("fetch('/kimi-wallpaper-hot.css"));
        assert_eq!(
            installed.matches("kimi-wallpaper-hot-hook").count(),
            2,
            "起始/结束两个标记各出现一次"
        );
        assert!(hook_installed(&installed));

        // 重复 install 幂等
        let again = install_hook(&installed).unwrap();
        assert_eq!(again, installed, "重复安装应原样返回");
        assert_eq!(again.matches("kimi-wallpaper-hot-hook").count(), 2);

        // remove 后与原文件一致，且幂等
        let removed = remove_hook(&installed);
        assert!(!removed.contains("kimi-wallpaper-hot-hook"));
        assert_eq!(removed, original, "卸载探针后应恢复原内容");
        assert_eq!(remove_hook(&removed), removed, "重复卸载不应改变内容");
        assert!(!hook_installed(&removed));
    }

    #[test]
    fn test_install_hook_no_body_errors() {
        let r = install_hook("<html><head><title>t</title></head></html>");
        assert!(r.is_err(), "找不到 </body> 时应报错");
    }

    #[test]
    fn test_write_hot_files_bumps_version() {
        let dist = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-fixture")
            .join("hot-files");
        let _ = fs::remove_dir_all(&dist);
        fs::create_dir_all(&dist).unwrap();
        let re = regex::Regex::new(r#""v"\s*:\s*(\d+)"#).unwrap();
        let read_v = || -> u64 {
            let j = fs::read_to_string(hot_json_path(&dist)).unwrap();
            re.captures(&j).unwrap()[1].parse().unwrap()
        };

        // 首次写入：旧 v 视为 0，新 v = max(1, now_ms) = now_ms
        write_hot_files(&dist, "cssA", None).unwrap();
        assert_eq!(fs::read_to_string(hot_css_path(&dist)).unwrap(), "cssA");
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let v1 = read_v();
        assert!(v1 >= now_ms - 5000, "首个版本应取当前 unix 毫秒（允许时钟误差）");
        assert!(v1 >= 1);

        // 手写旧大版本 -> 新 v = 旧v+1（旧v+1 > now_ms，取旧v+1 分支）
        fs::write(hot_json_path(&dist), r#"{"v":99999999999999}"#).unwrap();
        write_hot_files(&dist, "cssB", None).unwrap();
        assert_eq!(fs::read_to_string(hot_css_path(&dist)).unwrap(), "cssB");
        assert_eq!(read_v(), 100000000000000, "应取 旧v+1");

        // 空 css 也写入，v 继续递增
        write_hot_files(&dist, "", None).unwrap();
        assert_eq!(fs::read_to_string(hot_css_path(&dist)).unwrap(), "");
        assert_eq!(read_v(), 100000000000001, "空 css 后 v 应继续 +1");
    }

    #[test]
    fn test_centered_max_crop_16x9() {
        let r = centered_max_crop(Some(16.0 / 9.0));
        assert!((r.w - 1.0).abs() < 1e-6, "16:9 时宽应铺满");
        assert!((r.h - 9.0 / 16.0).abs() < 1e-6, "16:9 时高应为 9/16");
        assert_eq!(r.x, 0.0);
        assert!(
            (r.y - (1.0 - 9.0 / 16.0) / 2.0).abs() < 1e-6,
            "y 应垂直居中"
        );
    }

    #[test]
    fn test_centered_max_crop_21x9() {
        let r = centered_max_crop(CropAspect::Ratio21x9.ratio());
        assert!((r.w - 1.0).abs() < 1e-6);
        assert!((r.h - 9.0 / 21.0).abs() < 1e-6, "21:9 时高应为 9/21");
        assert!(r.h < 9.0 / 16.0, "21:9 应比 16:9 更扁");
        assert!((r.y - (1.0 - r.h) / 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_centered_max_crop_free() {
        let r = centered_max_crop(CropAspect::Free.ratio());
        assert_eq!(r, CropRect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 });
        assert!(CropAspect::Free.ratio().is_none());
    }

    #[test]
    fn test_clamp_crop_pulls_back_inside() {
        let r = clamp_crop(CropRect { x: -0.2, y: 0.9, w: 0.5, h: 0.5 }, 0.05);
        assert_eq!(r.x, 0.0, "负坐标应回缩到 0");
        assert!((r.y + r.h) <= 1.0 + 1e-6, "底部越界应回缩");
        assert!(r.y >= 0.0);
    }

    #[test]
    fn test_clamp_crop_min_size_and_bounds() {
        let r = clamp_crop(CropRect { x: 0.5, y: 0.5, w: 0.01, h: 0.01 }, 0.2);
        assert!((r.w - 0.2).abs() < 1e-6, "宽不应小于 min");
        assert!((r.h - 0.2).abs() < 1e-6, "高不应小于 min");
        assert!(r.x + r.w <= 1.0 + 1e-6);
        assert!(r.y + r.h <= 1.0 + 1e-6);
        let full = clamp_crop(CropRect { x: 0.0, y: 0.0, w: 1.5, h: 2.0 }, 0.05);
        assert_eq!(full.w, 1.0);
        assert_eq!(full.h, 1.0);
    }

    #[test]
    fn test_process_image_with_crop() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-fixture").join("crop-img");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let img_path = dir.join("src.png");
        make_test_image(&img_path); // 32×32

        let p = process_image(&img_path, Some(CropRect { x: 0.25, y: 0.25, w: 0.5, h: 0.5 })).unwrap();
        assert_eq!(p.cropped, Some((16, 16)), "应裁剪为中央 16×16");
        assert_eq!(p.mime, "image/jpeg");

        let p_full = process_image(&img_path, Some(CropRect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 })).unwrap();
        assert_eq!(p_full.cropped, Some((32, 32)), "全图裁剪等于原尺寸");
        assert!(!p_full.b64.is_empty());

        let p_none = process_image(&img_path, None).unwrap();
        assert!(p_none.cropped.is_none());
        assert!(!p_none.b64.is_empty());

        // 越界选区应被 clamp，不 panic
        let p_clamped = process_image(&img_path, Some(CropRect { x: 0.9, y: 0.9, w: 0.5, h: 0.5 })).unwrap();
        assert!(p_clamped.cropped.is_some());
        let (w, h) = p_clamped.cropped.unwrap();
        assert!(w >= 1 && h >= 1);
        assert!(w <= 32 && h <= 32);
        assert!(!p_clamped.b64.is_empty());
    }

    #[test]
    fn test_conf_round_trip_with_crop() {
        let s = SettingsSnapshot {
            install_path: "T:\\Kimi Code".to_string(),
            side_alpha: 0.55,
            panel_alpha: 0.25,
            dark_path: Some(PathBuf::from("C:\\图 片\\dark=1.png")),
            dark_alpha: 0.80,
            dark_crop: Some(CropRect { x: 0.1, y: 0.2, w: 0.5, h: 0.4 }),
            light_path: None,
            light_alpha: 0.78,
            light_crop: None,
            video_path: None,
        };
        let back = SettingsSnapshot::from_conf(&s.to_conf());
        assert_eq!(back, s, "含 crop 的完整快照应 round-trip（含中文/空格/= 路径）");
    }

    #[test]
    fn test_conf_round_trip_no_crop_and_equals_path() {
        let s = SettingsSnapshot {
            install_path: String::new(),
            side_alpha: 0.30,
            panel_alpha: 0.10,
            dark_path: Some(PathBuf::from("D:\\a=b\\壁纸.png")),
            dark_alpha: 0.90,
            dark_crop: None,
            light_path: Some(PathBuf::from("E:\\light.jpg")),
            light_alpha: 0.70,
            light_crop: Some(CropRect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }),
            video_path: Some(PathBuf::from("F:\\视频\\bg.mp4")),
        };
        let back = SettingsSnapshot::from_conf(&s.to_conf());
        assert_eq!(back.dark_path, s.dark_path);
        assert_eq!(back.light_path, s.light_path);
        assert_eq!(back.video_path, s.video_path, "视频路径（含中文）应 round-trip");
        assert!(back.dark_crop.is_none());
        assert_eq!(back.light_crop, s.light_crop);
        assert!((back.side_alpha - 0.30).abs() < 1e-6);
        assert!((back.dark_alpha - 0.90).abs() < 1e-6);
        assert!((back.light_alpha - 0.70).abs() < 1e-6);
        // conf 文本本身不应含 dark.crop 行
        assert!(!s.to_conf().contains("dark.crop"));
    }

    #[test]
    fn test_from_conf_tolerates_garbage_and_missing() {
        let text = "\
# 注释行应被跳过
完全乱写的一行
side_alpha=不是数字
unknown_key=zzz

dark.crop=0.1,0.2,bad
dark.crop=0.1,0.2,0.3
dark.alpha=0.66
dark.path=
light.crop=,,
";
        let s = SettingsSnapshot::from_conf(text);
        assert_eq!(s.install_path, "", "缺 install_path 行应为空串");
        assert!((s.side_alpha - 0.55).abs() < 1e-6, "坏 alpha 应回落默认值");
        assert!((s.panel_alpha - 0.25).abs() < 1e-6);
        assert!((s.dark_alpha - 0.66).abs() < 1e-6, "合法行仍应生效");
        assert_eq!(s.dark_crop, None, "坏 crop 应为 None");
        assert_eq!(s.dark_path, None, "空 path 应为 None");
        assert_eq!(s.light_crop, None);
        assert!((s.light_alpha - 0.78).abs() < 1e-6);
    }

    #[test]
    fn test_gif_passthrough() {
        use base64::Engine as _;
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-fixture").join("gif-passthrough");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let gif_path = dir.join("anim.gif");
        // 假 GIF 头即可：process_image 对 .gif 不应尝试解码
        let bytes = b"GIF89a\x10\x00\x10\x00\x91\x00\x00";
        fs::write(&gif_path, bytes).unwrap();

        let p = process_image(&gif_path, None).unwrap();
        assert_eq!(p.mime, "image/gif");
        assert_eq!(p.orig_len, bytes.len());
        assert_eq!(p.comp_len, bytes.len(), "GIF 不压缩，comp_len 应等于原始大小");
        assert!(p.cropped.is_none());
        let expect = base64::engine::general_purpose::STANDARD.encode(bytes);
        assert_eq!(p.b64, expect, "GIF 应原样 base64 透传（保留动画）");

        // GIF 带 crop：忽略裁剪，仍整图透传
        let p2 = process_image(&gif_path, Some(CropRect { x: 0.0, y: 0.0, w: 0.5, h: 0.5 })).unwrap();
        assert_eq!(p2.mime, "image/gif");
        assert!(p2.cropped.is_none(), "GIF 应忽略裁剪");
        assert_eq!(p2.b64, expect);
    }

    #[test]
    fn test_build_patch_mime_placeholder() {
        let gif_patch = build_patch(Some(("QUJD", 0.80, "image/gif")), None, 0.55, 0.25).unwrap();
        assert!(
            gif_patch.contains("data:image/gif;base64,QUJD"),
            "GIF 槽应输出 data:image/gif"
        );
        assert!(!gif_patch.contains("data:image/jpeg;base64,QUJD"));

        let jpg_patch = build_patch(Some(("QUJD", 0.80, "image/jpeg")), None, 0.55, 0.25).unwrap();
        assert!(jpg_patch.contains("data:image/jpeg;base64,QUJD"));
        assert!(!jpg_patch.contains("{MIME}"), "{{MIME}} 占位符应被全部替换");
    }

    #[test]
    fn test_templates_define_mask_variable() {
        let patch = build_patch(
            Some(("d", 0.80, "image/jpeg")),
            Some(("l", 0.78, "image/jpeg")),
            0.55,
            0.25,
        )
        .unwrap();
        assert_eq!(patch.matches("--kimi-wallpaper-mask").count(), 4, "dark/light × scheme/system 共 4 条遮罩变量规则");
        assert!(patch.contains("html[data-color-scheme=dark]{--kimi-wallpaper-mask:rgba(7,7,13,0.80)"));
        assert!(patch.contains("html[data-color-scheme=system]{--kimi-wallpaper-mask:rgba(7,7,13,0.80)"));
        assert!(patch.contains("html[data-color-scheme=light]{--kimi-wallpaper-mask:rgba(250,250,252,0.78)"));
        assert!(patch.contains("html[data-color-scheme=system]{--kimi-wallpaper-mask:rgba(250,250,252,0.78)}"));
    }

    #[test]
    fn test_write_hot_files_video_field() {
        let dist = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-fixture")
            .join("hot-files-video");
        let _ = fs::remove_dir_all(&dist);
        fs::create_dir_all(&dist).unwrap();

        // 带视频字段
        write_hot_files(&dist, "cssA", Some(VIDEO_FILE_NAME)).unwrap();
        let j = fs::read_to_string(hot_json_path(&dist)).unwrap();
        assert!(j.starts_with('{') && j.ends_with('}'));
        assert!(j.contains(&format!("\"video\":\"{VIDEO_FILE_NAME}\"")), "应写入 video 文件名: {j}");

        // 不带视频 -> null
        write_hot_files(&dist, "cssB", None).unwrap();
        let j2 = fs::read_to_string(hot_json_path(&dist)).unwrap();
        assert!(j2.contains("\"video\":null"), "无视频时应写 video:null: {j2}");
        assert_eq!(fs::read_to_string(hot_css_path(&dist)).unwrap(), "cssB");

        // 旧格式 json（无 video 字段）版本号解析仍有效（用大版本号避开 now_ms 取大分支）
        fs::write(hot_json_path(&dist), r#"{"v":99999999999998}"#).unwrap();
        write_hot_files(&dist, "cssC", None).unwrap();
        let j3 = fs::read_to_string(hot_json_path(&dist)).unwrap();
        let re = regex::Regex::new(r#""v"\s*:\s*(\d+)"#).unwrap();
        let v: u64 = re.captures(&j3).unwrap()[1].parse().unwrap();
        assert_eq!(v, 99999999999999, "旧格式（无 video 字段）的 v 应继续 +1");
        assert!(j3.contains("\"video\":null"));
    }

    #[test]
    fn test_install_hook_v2_upgrade() {
        // 手工拼一个 v1 块（= v2 块去掉版本注释行）
        let v1_block = HOT_HOOK_BLOCK
            .replace("    <!-- version: v2 -->\n", "")
            .to_string();
        assert!(!v1_block.contains(HOOK_VERSION_MARK));
        let original = "<html><body>\n</body></html>";
        let with_v1 = format!("<html><body>\n{v1_block}\n</body></html>");

        assert!(hook_installed(&with_v1));
        assert!(hook_needs_upgrade(&with_v1), "v1 块应被判定需升级");

        // 升级：v1 -> v2，只保留一段探针
        let upgraded = install_hook(&with_v1).unwrap();
        assert!(upgraded.contains(HOOK_VERSION_MARK), "升级后应为 v2");
        assert!(!hook_needs_upgrade(&upgraded));
        assert_eq!(upgraded.matches(HOT_HOOK_START).count(), 1, "升级后只应有一段探针块");
        assert_eq!(upgraded.matches("kimi-wallpaper-hot-hook").count(), 2);
        assert!(upgraded.contains("kimi-wallpaper-hot-video"), "v2 探针应含视频层");

        // v2 再 install 幂等
        let again = install_hook(&upgraded).unwrap();
        assert_eq!(again, upgraded, "已是最新 v2 时应原样返回");

        // remove 对 v2 精确还原
        assert_eq!(remove_hook(&upgraded), original, "卸载 v2 应精确还原");
        assert!(!hook_installed(&remove_hook(&upgraded)));
    }

    /// 造一个完整 box：4 字节大端 size + 4 字节 type + payload。
    fn bx(typ: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(8 + payload.len());
        v.extend_from_slice(&((payload.len() as u32 + 8).to_be_bytes()));
        v.extend_from_slice(typ);
        v.extend_from_slice(payload);
        v
    }

    /// 造 moov>trak>mdia>minf>stbl>stco/co64 嵌套结构，entries 为 chunk 偏移。
    fn toy_moov(wide: bool, entries: &[u64]) -> Vec<u8> {
        let mut stco_payload = vec![0u8; 4]; // version/flags
        stco_payload.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        for &e in entries {
            if wide {
                stco_payload.extend_from_slice(&e.to_be_bytes());
            } else {
                stco_payload.extend_from_slice(&(e as u32).to_be_bytes());
            }
        }
        let leaf_typ: &[u8; 4] = if wide { b"co64" } else { b"stco" };
        let stbl = bx(b"stbl", &bx(leaf_typ, &stco_payload));
        let minf = bx(b"minf", &stbl);
        let mdia = bx(b"mdia", &minf);
        let trak = bx(b"trak", &mdia);
        bx(b"moov", &trak)
    }

    fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    /// 从 mp4 数据里抽 stco/co64 的 entries（假定唯一）。
    /// pos 指向 type 字段，其后为 version/flags(4) + entry_count(4) + entries。
    fn read_chunk_entries(mp4: &[u8], wide: bool) -> Vec<u64> {
        let typ: &[u8; 4] = if wide { b"co64" } else { b"stco" };
        let pos = find_subslice(mp4, typ).expect("应找到 stco/co64 box");
        let count = u32::from_be_bytes(mp4[pos + 8..pos + 12].try_into().unwrap()) as usize;
        let esz = if wide { 8 } else { 4 };
        (0..count)
            .map(|i| {
                let off = pos + 12 + i * esz;
                if wide {
                    u64::from_be_bytes(mp4[off..off + 8].try_into().unwrap())
                } else {
                    u32::from_be_bytes(mp4[off..off + 4].try_into().unwrap()) as u64
                }
            })
            .collect()
    }

    /// 抽 mdat 的 payload 字节。
    fn read_mdat_payload(mp4: &[u8]) -> Vec<u8> {
        let pos = find_subslice(mp4, b"mdat").expect("应找到 mdat box");
        let size = u32::from_be_bytes(mp4[pos - 4..pos].try_into().unwrap()) as usize;
        mp4[pos + 4..pos - 4 + size].to_vec()
    }

    #[test]
    fn test_mp4_moov_before_mdat() {
        let ftyp = bx(b"ftyp", b"isom0000");
        let moov = toy_moov(false, &[1000, 2000]);
        let mdat = bx(b"mdat", &[0xABu8; 64]);

        // faststart 布局：moov 在 mdat 前
        let mut good = ftyp.clone();
        good.extend_from_slice(&moov);
        good.extend_from_slice(&mdat);
        assert!(mp4_moov_before_mdat(&good).unwrap(), "moov 在 mdat 前应判 true");
        assert_eq!(
            faststart_remux(&good).unwrap(),
            good,
            "已 faststart 的输入应原样返回"
        );

        // moov 在文件尾
        let mut bad = ftyp.clone();
        bad.extend_from_slice(&mdat);
        bad.extend_from_slice(&moov);
        assert!(!mp4_moov_before_mdat(&bad).unwrap(), "moov 在 mdat 后应判 false");

        // 缺 moov / 缺 mdat / 垃圾数据 均报错
        let mut no_moov = ftyp.clone();
        no_moov.extend_from_slice(&mdat);
        assert!(mp4_moov_before_mdat(&no_moov).is_err());
        let mut no_mdat = ftyp.clone();
        no_mdat.extend_from_slice(&moov);
        assert!(mp4_moov_before_mdat(&no_mdat).is_err());
        assert!(mp4_moov_before_mdat(b"this is definitely not an mp4 file at all").is_err());
    }

    #[test]
    fn test_faststart_remux_stco() {
        let ftyp = bx(b"ftyp", b"isom0000"); // 16 字节
        let moov = toy_moov(false, &[1000, 2000]);
        let mdat_payload = [0xCDu8; 64];
        let mdat = bx(b"mdat", &mdat_payload); // 72 字节，紧随 ftyp
        let mdat_old_off = ftyp.len() as i64; // 16
        let delta = (ftyp.len() as i64 + moov.len() as i64) - mdat_old_off;

        let mut src = ftyp.clone();
        src.extend_from_slice(&mdat);
        src.extend_from_slice(&moov);

        let out = faststart_remux(&src).unwrap();
        assert!(mp4_moov_before_mdat(&out).unwrap(), "重封装后 moov 应在 mdat 前");
        // 布局 = ftyp + moov + mdat
        assert_eq!(out.len(), src.len());
        assert_eq!(&out[..ftyp.len()], &ftyp[..]);
        assert_eq!(
            read_chunk_entries(&out, false),
            vec![(1000 + delta) as u32 as u64, (2000 + delta) as u32 as u64],
            "stco entry 应加上 mdat 位移 delta={delta}"
        );
        assert_eq!(
            read_mdat_payload(&out),
            mdat_payload.to_vec(),
            "mdat 内容应逐字节不变"
        );
        // 幂等：再跑一次应原样返回
        assert_eq!(faststart_remux(&out).unwrap(), out, "重封装应幂等");
    }

    #[test]
    fn test_faststart_remux_co64() {
        let ftyp = bx(b"ftyp", b"isom0000"); // 16 字节
        let moov = toy_moov(true, &[50_000, 90_000, 130_000]);
        let mdat_payload = [0x77u8; 48];
        let mdat = bx(b"mdat", &mdat_payload);
        let mdat_old_off = ftyp.len() as i64;
        let delta = (ftyp.len() as i64 + moov.len() as i64) - mdat_old_off;

        let mut src = ftyp.clone();
        src.extend_from_slice(&mdat);
        src.extend_from_slice(&moov);

        let out = faststart_remux(&src).unwrap();
        assert!(mp4_moov_before_mdat(&out).unwrap());
        assert_eq!(
            read_chunk_entries(&out, true),
            vec![
                (50_000 + delta) as u64,
                (90_000 + delta) as u64,
                (130_000 + delta) as u64
            ],
            "co64 entry 应加上 mdat 位移 delta={delta}"
        );
        assert_eq!(read_mdat_payload(&out), mdat_payload.to_vec());
        assert_eq!(faststart_remux(&out).unwrap(), out, "重封装应幂等");
    }
}
