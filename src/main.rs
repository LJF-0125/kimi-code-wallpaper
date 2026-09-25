#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use eframe::egui;

const PATCH_START: &str = "/* === kimi-wallpaper-patch";
const PATCH_HEADER: &str = "/* === kimi-wallpaper-patch === */";
const PATCH_END_MARK: &str = "kimi-wallpaper-patch end === */";
const MAX_TEXTURE_SIDE: u32 = 2560;
const MAX_JPEG_BYTES: usize = 900 * 1024;

const DARK_TEMPLATE: &str = r#":root{--kimi-wallpaper-dark:url("data:image/jpeg;base64,{B64}")}
html[data-color-scheme=dark]{background-color:#0a0a10;background-image:linear-gradient(rgba(7,7,13,{DA}),rgba(7,7,13,{DA})),var(--kimi-wallpaper-dark) !important;background-size:cover,cover;background-position:center,center;background-repeat:no-repeat,no-repeat;background-attachment:fixed,fixed}
html[data-color-scheme=dark] .app,html[data-color-scheme=dark] .con,html[data-color-scheme=dark] body,html[data-color-scheme=dark] #app{background:transparent !important}
html[data-color-scheme=dark] .side,html[data-color-scheme=dark] .windows-titlebar{background:rgba(10,10,17,{SA}) !important}
html[data-color-scheme=dark] .global-preview,html[data-color-scheme=dark] .agent-panel,html[data-color-scheme=dark] .global-preview .file-preview,html[data-color-scheme=dark] .global-preview .fp-body,html[data-color-scheme=dark] .global-preview .ui-panel-header{background:rgba(10,10,17,{PA}) !important}
@media (prefers-color-scheme:dark){
html[data-color-scheme=system]{background-color:#0a0a10;background-image:linear-gradient(rgba(7,7,13,{DA}),rgba(7,7,13,{DA})),var(--kimi-wallpaper-dark) !important;background-size:cover,cover;background-position:center,center;background-repeat:no-repeat,no-repeat;background-attachment:fixed,fixed}
html[data-color-scheme=system] .app,html[data-color-scheme=system] .con,html[data-color-scheme=system] body,html[data-color-scheme=system] #app{background:transparent !important}
html[data-color-scheme=system] .side,html[data-color-scheme=system] .windows-titlebar{background:rgba(10,10,17,{SA}) !important}
html[data-color-scheme=system] .global-preview,html[data-color-scheme=system] .agent-panel,html[data-color-scheme=system] .global-preview .file-preview,html[data-color-scheme=system] .global-preview .fp-body,html[data-color-scheme=system] .global-preview .ui-panel-header{background:rgba(10,10,17,{PA}) !important}
}
"#;

const LIGHT_TEMPLATE: &str = r#":root{--kimi-wallpaper-light:url("data:image/jpeg;base64,{B64}")}
html[data-color-scheme=light]{background-color:#f5f5f7;background-image:linear-gradient(rgba(250,250,252,{LA}),rgba(250,250,252,{LA})),var(--kimi-wallpaper-light) !important;background-size:cover,cover;background-position:center,center;background-repeat:no-repeat,no-repeat;background-attachment:fixed,fixed}
html[data-color-scheme=light] .app,html[data-color-scheme=light] .con,html[data-color-scheme=light] body,html[data-color-scheme=light] #app{background:transparent !important}
html[data-color-scheme=light] .side,html[data-color-scheme=light] .windows-titlebar{background:rgba(255,255,255,{SA}) !important}
html[data-color-scheme=light] .global-preview,html[data-color-scheme=light] .agent-panel,html[data-color-scheme=light] .global-preview .file-preview,html[data-color-scheme=light] .global-preview .fp-body,html[data-color-scheme=light] .global-preview .ui-panel-header{background:rgba(255,255,255,{PA}) !important}
@media (prefers-color-scheme:light){
html[data-color-scheme=system]{background-color:#f5f5f7;background-image:linear-gradient(rgba(250,250,252,{LA}),rgba(250,250,252,{LA})),var(--kimi-wallpaper-light) !important;background-size:cover,cover;background-position:center,center;background-repeat:no-repeat,no-repeat;background-attachment:fixed,fixed}
html[data-color-scheme=system] .app,html[data-color-scheme=system] .con,html[data-color-scheme=system] body,html[data-color-scheme=system] #app{background:transparent !important}
html[data-color-scheme=system] .side,html[data-color-scheme=system] .windows-titlebar{background:rgba(255,255,255,{SA}) !important}
html[data-color-scheme=system] .global-preview,html[data-color-scheme=system] .agent-panel,html[data-color-scheme=system] .global-preview .file-preview,html[data-color-scheme=system] .global-preview .fp-body,html[data-color-scheme=system] .global-preview .ui-panel-header{background:rgba(255,255,255,{PA}) !important}
}
"#;

// ---------- 数据结构 ----------

struct Slot {
    path: Option<PathBuf>,
    texture: Option<egui::TextureHandle>,
    alpha: f32,
}

impl Slot {
    fn new(alpha: f32) -> Self {
        Self {
            path: None,
            texture: None,
            alpha,
        }
    }
}

struct BgToolApp {
    install_path: String,
    root: Option<PathBuf>,
    css_name: Option<String>,
    patched: bool,
    bak_exists: bool,
    status_err: Option<String>,
    dark: Slot,
    light: Slot,
    side_alpha: f32,
    panel_alpha: f32,
    log: String,
}

impl BgToolApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            install_path: String::new(),
            root: None,
            css_name: None,
            patched: false,
            bak_exists: false,
            status_err: None,
            dark: Slot::new(0.80),
            light: Slot::new(0.78),
            side_alpha: 0.55,
            panel_alpha: 0.25,
            log: String::new(),
        };
        app.log_push("Kimi Code 背景更换工具已启动");
        match detect_install_path() {
            Some(p) => {
                app.log_push(&format!("自动检测到安装路径: {}", p.display()));
                app.install_path = p.display().to_string();
            }
            None => app.log_push("未自动检测到 Kimi Code 安装路径，请手动选择"),
        }
        app.refresh();
        let _ = cc;
        app
    }

    fn log_push(&mut self, msg: &str) {
        self.log.push_str(msg);
        if !msg.ends_with('\n') {
            self.log.push('\n');
        }
    }

    fn refresh(&mut self) {
        self.css_name = None;
        self.patched = false;
        self.bak_exists = false;
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

    fn process_slot(path: &Option<PathBuf>) -> Result<Option<(String, usize, usize)>, String> {
        match path {
            None => Ok(None),
            Some(p) => process_image(p).map(Some).map_err(|e| format!("{e:#}")),
        }
    }

    fn do_apply(&mut self) {
        let root = match self.root.clone() {
            Some(r) => r,
            None => {
                self.log_push("错误: 安装路径无效，无法应用补丁");
                return;
            }
        };
        let dark = match Self::process_slot(&self.dark.path) {
            Ok(v) => v,
            Err(e) => {
                self.log_push(&format!("深色图片处理失败: {e}"));
                return;
            }
        };
        let light = match Self::process_slot(&self.light.path) {
            Ok(v) => v,
            Err(e) => {
                self.log_push(&format!("浅色图片处理失败: {e}"));
                return;
            }
        };
        if let Some((_, orig, comp)) = &dark {
            self.log_push(&format!("深色图: 原始 {} 字节 -> JPEG {} 字节", orig, comp));
            if comp > &MAX_JPEG_BYTES {
                self.log_push("警告: 深色图压缩后仍超过 900KB，可能导致样式表过大");
            }
        }
        if let Some((_, orig, comp)) = &light {
            self.log_push(&format!("浅色图: 原始 {} 字节 -> JPEG {} 字节", orig, comp));
            if comp > &MAX_JPEG_BYTES {
                self.log_push("警告: 浅色图压缩后仍超过 900KB，可能导致样式表过大");
            }
        }
        let patch = build_patch(
            dark.as_ref().map(|t| (t.0.as_str(), self.dark.alpha)),
            light.as_ref().map(|t| (t.0.as_str(), self.light.alpha)),
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
                self.log_push("请重启 Kimi Code 生效");
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
                self.log_push("请重启 Kimi Code 生效");
            }
            Err(e) => self.log_push(&format!("还原失败: {e:#}")),
        }
        self.refresh();
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
    dark: Option<(&str, f32)>,
    light: Option<(&str, f32)>,
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
    if let Some((b64, da)) = dark {
        out.push_str(
            &DARK_TEMPLATE
                .replace("{B64}", b64)
                .replace("{DA}", &format!("{da:.2}"))
                .replace("{SA}", &sa)
                .replace("{PA}", &pa),
        );
    }
    if let Some((b64, la)) = light {
        out.push_str(
            &LIGHT_TEMPLATE
                .replace("{B64}", b64)
                .replace("{LA}", &format!("{la:.2}"))
                .replace("{SA}", &sa)
                .replace("{PA}", &pa),
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

// ---------- 图片处理 ----------

fn process_image(path: &Path) -> anyhow::Result<(String, usize, usize)> {
    use base64::Engine as _;
    use image::ImageEncoder;

    let data = fs::read(path).with_context(|| format!("读取图片失败: {}", path.display()))?;
    let orig_len = data.len();
    let img = image::load_from_memory(&data).context("解析图片失败（格式不支持或文件损坏）")?;
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
    Ok((b64, orig_len, comp_len))
}

fn load_preview(ctx: &egui::Context, path: &Path) -> anyhow::Result<egui::TextureHandle> {
    let img = image::open(path).context("预览加载失败")?;
    let (w, h) = (img.width(), img.height());
    let scale = 240.0 / w.max(h) as f32;
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

fn slot_ui(ui: &mut egui::Ui, title: &str, slot: &mut Slot, ctx: &egui::Context, log: &mut String) {
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
        ui.horizontal(|ui| {
            if ui.button("选择图片").clicked() {
                let picked = rfd::FileDialog::new()
                    .add_filter("图片文件", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
                    .pick_file();
                if let Some(f) = picked {
                    match load_preview(ctx, &f) {
                        Ok(tex) => {
                            slot.path = Some(f.clone());
                            slot.texture = Some(tex);
                            log.push_str(&format!("已选择{}图片: {}\n", title, f.display()));
                        }
                        Err(e) => log.push_str(&format!("加载预览失败: {e:#}\n")),
                    }
                }
            }
            if ui.button("清除").clicked() {
                slot.path = None;
                slot.texture = None;
            }
        });
        ui.add(
            egui::Slider::new(&mut slot.alpha, 0.0..=0.95)
                .text("遮罩透明度")
                .fixed_decimals(2),
        );
    });
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
            });

            ui.separator();

            let mut log = std::mem::take(&mut self.log);
            ui.columns(2, |cols| {
                slot_ui(&mut cols[0], "深色背景", &mut self.dark, ctx, &mut log);
                slot_ui(&mut cols[1], "浅色背景", &mut self.light, ctx, &mut log);
            });
            self.log = log;

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

        let (b64, orig_len, comp_len) = process_image(&img_path).unwrap();
        assert!(orig_len > 0 && comp_len > 0);
        assert!(!b64.is_empty());

        let patch = build_patch(Some((&b64, 0.80)), None, 0.55, 0.25).unwrap();
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
        let patch2 = build_patch(Some((&b64, 0.80)), None, 0.55, 0.25).unwrap();
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
        let (d64, _, _) = process_image(&dark_png).unwrap();
        let (l64, _, _) = process_image(&light_png).unwrap();

        let patch = build_patch(Some((&d64, 0.80)), Some((&l64, 0.78)), 0.55, 0.25).unwrap();
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
    }

    #[test]
    fn test_empty_apply_removes_patch_and_restore() {
        let (root, css_path) = make_fixture("restore");
        let img_path = root.join("dark.png");
        make_test_image(&img_path);
        let (b64, _, _) = process_image(&img_path).unwrap();
        let patch = build_patch(Some((&b64, 0.80)), None, 0.55, 0.25).unwrap();
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
}
