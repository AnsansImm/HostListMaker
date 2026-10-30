#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use eframe::egui::{self, Color32, RichText, TextEdit};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Config {
    last_folder: Option<PathBuf>,
    #[serde(default = "default_dark_theme")]
    dark_theme: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { last_folder: None, dark_theme: true }
    }
}

fn default_dark_theme() -> bool {
    true
}

struct ListFile {
    path: PathBuf,
    display_name: String,
}

struct App {
    config_path: PathBuf,
    config: Config,
    user_lists: Vec<ListFile>,
    other_lists: Vec<ListFile>,
    selected: Option<PathBuf>,
    editor_text: String,
    status: String,
    save_pending: bool,
    last_edit: Option<Instant>,
    cursor_to_new_line: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = app_directory().join("config.json");
        let config = load_config(&config_path);
        set_theme(&cc.egui_ctx, config.dark_theme);
        let mut app = Self {
            config_path,
            config,
            user_lists: Vec::new(),
            other_lists: Vec::new(),
            selected: None,
            editor_text: String::new(),
            status: "Выберите папку с листами".into(),
            save_pending: false,
            last_edit: None,
            cursor_to_new_line: false,
        };

        if let Some(folder) = app.config.last_folder.clone().filter(|p| p.is_dir()) {
            app.scan_folder(&folder);
        }
        app
    }

    fn scan_folder(&mut self, folder: &Path) {
        self.user_lists.clear();
        self.other_lists.clear();
        self.selected = None;
        self.editor_text.clear();
        self.save_pending = false;
        self.last_edit = None;
        self.cursor_to_new_line = false;

        match fs::read_dir(folder) {
            Ok(entries) => {
                let mut files: Vec<ListFile> = entries
                    .filter_map(Result::ok)
                    .filter_map(|entry| {
                        let path = entry.path();
                        if !path.is_file() || !has_txt_extension(&path) {
                            return None;
                        }
                        Some(ListFile {
                            display_name: entry.file_name().to_string_lossy().into_owned(),
                            path,
                        })
                    })
                    .collect();
                files.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
                for file in files {
                    if file.display_name.to_lowercase().contains("user") {
                        self.user_lists.push(file);
                    } else {
                        self.other_lists.push(file);
                    }
                }
                self.config.last_folder = Some(folder.to_path_buf());
                save_config(&self.config_path, &self.config);
                self.status = format!("Папка: {}", folder.display());
            }
            Err(error) => self.status = format!("Не удалось прочитать папку: {error}"),
        }
    }

    fn open_file(&mut self, path: PathBuf) {
        match fs::read_to_string(&path) {
            Ok(contents) => {
                self.editor_text = editable_text(&contents);
                self.selected = Some(path.clone());
                self.save_pending = false;
                self.last_edit = None;
                self.cursor_to_new_line = true;
                self.status = format!("Открыт: {}", path.file_name().unwrap_or_default().to_string_lossy());
            }
            Err(error) => self.status = format!("Не удалось открыть файл: {error}"),
        }
    }

    fn save_current(&mut self) {
        if let Some(path) = &self.selected {
            // The editor keeps one empty line at the bottom for convenient input.
            // It is intentionally not written to the list file.
            match fs::write(path, normalize(&self.editor_text)) {
                Ok(()) => self.status = "Изменения сохранены".into(),
                Err(error) => self.status = format!("Ошибка сохранения: {error}"),
            }
        }
    }

    fn queue_save(&mut self) {
        self.save_pending = true;
        self.last_edit = Some(Instant::now());
        self.status = "Изменения будут сохранены…".into();
    }

    fn save_when_idle(&mut self, ctx: &egui::Context) {
        const SAVE_DELAY: Duration = Duration::from_millis(400);
        if !self.save_pending {
            return;
        }
        let elapsed = self.last_edit.map(|time| time.elapsed()).unwrap_or(SAVE_DELAY);
        if elapsed >= SAVE_DELAY {
            self.save_current();
            self.save_pending = false;
            self.last_edit = None;
        } else {
            ctx.request_repaint_after(SAVE_DELAY - elapsed);
        }
    }

    fn list_section(ui: &mut egui::Ui, title: &str, files: &[(PathBuf, String)], selected: Option<&Path>) -> Option<PathBuf> {
        ui.label(RichText::new(title).strong().color(Color32::from_rgb(124, 190, 255)));
        let mut clicked = None;
        if files.is_empty() {
            ui.label(RichText::new("Нет файлов").weak());
        }
        for (path, name) in files {
            if ui.selectable_label(selected == Some(path.as_path()), name).clicked() {
                clicked = Some(path.clone());
            }
        }
        clicked
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Выбрать папку").clicked() {
                    let dialog = if let Some(folder) = &self.config.last_folder {
                        rfd::FileDialog::new().set_directory(folder)
                    } else {
                        rfd::FileDialog::new()
                    };
                    if let Some(folder) = dialog.pick_folder() {
                        self.scan_folder(&folder);
                    }
                }
                ui.separator();
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if self.config.dark_theme { "Светлая тема" } else { "Тёмная тема" };
                    if ui.button(label).clicked() {
                        self.config.dark_theme = !self.config.dark_theme;
                        set_theme(ctx, self.config.dark_theme);
                        save_config(&self.config_path, &self.config);
                    }
                });
            });
        });

        let user_files: Vec<_> = self.user_lists.iter().map(|f| (f.path.clone(), f.display_name.clone())).collect();
        let other_files: Vec<_> = self.other_lists.iter().map(|f| (f.path.clone(), f.display_name.clone())).collect();
        let mut opened = None;
        egui::SidePanel::left("files").resizable(true).default_width(235.0).show(ctx, |ui| {
            ui.heading("Файлы .txt");
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                opened = Self::list_section(ui, "User-листы", &user_files, self.selected.as_deref());
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);
                if opened.is_none() {
                    opened = Self::list_section(ui, "Прочие листы", &other_files, self.selected.as_deref());
                } else {
                    let _ = Self::list_section(ui, "Прочие листы", &other_files, self.selected.as_deref());
                }
            });
        });
        if let Some(path) = opened { self.open_file(path); }

        egui::CentralPanel::default().show(ctx, |ui| {
            let name = self.selected.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy()).unwrap_or_else(|| "Файл не выбран".into());
            ui.heading(name);
            ui.label(RichText::new("Домены: a–z, A–Z, 0–9, -, ., _, *; пробел создаёт новую строку. Строка с # — комментарий.").small().weak());
            ui.separator();
            if self.selected.is_some() {
                let editor_id = ui.make_persistent_id("editor");
                let response = egui::ScrollArea::vertical()
                    .id_salt("editor_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(TextEdit::multiline(&mut self.editor_text)
                            .id(editor_id)
                            .font(egui::TextStyle::Monospace)
                            .hint_text("example.org\n# Мой комментарий\n*.example.net")
                            .desired_width(f32::INFINITY)
                            .desired_rows(24))
                    })
                    .inner;
                if self.cursor_to_new_line {
                    let mut state = egui::widgets::text_edit::TextEditState::load(ctx, editor_id).unwrap_or_default();
                    let cursor = egui::text::CCursor::new(self.editor_text.chars().count());
                    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(cursor)));
                    state.store(ctx, editor_id);
                    response.request_focus();
                    self.cursor_to_new_line = false;
                    ctx.request_repaint();
                }
                if response.changed() {
                    self.editor_text = editable_text(&self.editor_text);
                    self.queue_save();
                }
            } else {
                ui.centered_and_justified(|ui| { ui.label("Выберите .txt файл слева"); });
            }
        });

        self.save_when_idle(ctx);
    }
}

fn has_txt_extension(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
}

fn normalize(input: &str) -> String {
    let mut lines = Vec::new();
    for source_line in input.replace('\r', "\n").lines() {
        let line = source_line.trim_start_matches(char::is_whitespace);
        if line.starts_with('#') {
            // Comment lines deliberately retain every character after '#'.
            lines.push(line.to_owned());
            continue;
        }

        let mut domain = String::new();
        for character in line.chars() {
            if character == ' ' {
                if !domain.is_empty() {
                    lines.push(std::mem::take(&mut domain));
                }
            } else if character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | '_' | '*') {
                domain.push(character);
            }
        }
        if !domain.is_empty() {
            lines.push(domain);
        }
    }
    lines.join("\n")
}

/// Keeps a permanent input line after the last domain or comment.
/// `normalize` is used when writing to disk, so that line never becomes a blank record in the file.
fn editable_text(input: &str) -> String {
    let mut text = normalize(input);
    if !text.is_empty() {
        text.push('\n');
    }
    text
}

fn app_directory() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn load_config(path: &Path) -> Config {
    fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

fn save_config(path: &Path, config: &Config) {
    if let Ok(text) = serde_json::to_string_pretty(config) { let _ = fs::write(path, text); }
}

fn set_theme(ctx: &egui::Context, dark: bool) {
    ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
}

fn application_icon() -> Arc<egui::IconData> {
    let image = image::load_from_memory(include_bytes!("../logo.png"))
        .expect("logo.png must be a valid PNG")
        .into_rgba8();
    let (width, height) = image.dimensions();
    Arc::new(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 650.0])
            .with_min_inner_size([700.0, 450.0])
            .with_icon(application_icon()),
        ..Default::default()
    };
    eframe::run_native("Host List Maker", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

#[cfg(test)]
mod tests {
    use super::{editable_text, normalize};

    #[test]
    fn replaces_spaces_and_removes_invalid_characters() {
        assert_eq!(normalize("example.com bad@host! *.site"), "example.com\nbadhost\n*.site");
    }

    #[test]
    fn removes_empty_lines() {
        assert_eq!(normalize("\n\none.org\r\n   \r\ntwo_net\n"), "one.org\ntwo_net");
    }

    #[test]
    fn preserves_comment_text() {
        assert_eq!(
            normalize("# Любой текст: @! $%^&* ()\nexample.org"),
            "# Любой текст: @! $%^&* ()\nexample.org"
        );
    }

    #[test]
    fn keeps_an_input_line_outside_the_saved_content() {
        assert_eq!(editable_text("example.org"), "example.org\n");
        assert_eq!(normalize(&editable_text("example.org")), "example.org");
    }
}
