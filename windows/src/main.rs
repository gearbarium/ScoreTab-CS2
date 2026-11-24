#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText, Ui};
use scoretab_cs2::{domain::*, storage::Store};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const AMBER: Color32 = Color32::from_rgb(255, 176, 46);
const BG: Color32 = Color32::from_rgb(5, 11, 18);
const SURFACE: Color32 = Color32::from_rgb(10, 21, 32);
const RAISED: Color32 = Color32::from_rgb(17, 33, 46);
const TEXT: Color32 = Color32::from_rgb(241, 247, 251);
const MUTED: Color32 = Color32::from_rgb(156, 174, 187);
const OUTLINE: Color32 = Color32::from_rgb(82, 106, 121);
const ELEVATED: Color32 = Color32::from_rgb(8, 17, 27);
const HOVER: Color32 = Color32::from_rgb(23, 44, 58);
const DIVIDER: Color32 = Color32::from_rgb(36, 55, 68);
const SUCCESS: Color32 = Color32::from_rgb(110, 226, 170);
const CT: Color32 = Color32::from_rgb(121, 199, 255);
const FOCUS: Color32 = Color32::from_rgb(255, 209, 132);

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn reason_label(reason: Reason) -> &'static str {
    match reason {
        Reason::Elimination => "Уничтожение",
        Reason::Explosion => "Взрыв",
        Reason::Defuse => "Дефуз",
        Reason::Timeout => "Время",
        Reason::Technical => "Техническое решение",
    }
}

fn initial_match() -> Match {
    Match {
        version: 1,
        teams: [
            Team {
                id: 1,
                name: String::new(),
            },
            Team {
                id: 2,
                name: String::new(),
            },
        ],
        map: String::new(),
        starting_side: Side::T,
        rules: Rules::default(),
        created_at: now(),
        rounds: Vec::new(),
        revisions: Vec::new(),
        draft: Draft::default(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    Setup,
    History,
    Round,
    Protocol,
    Settings,
}

struct App {
    store: Store,
    page: Page,
    setup: Match,
    current: Option<(i64, Match)>,
    history: Vec<(i64, Match)>,
    edit: Option<(u32, Draft)>,
    confirm: Option<Action>,
    error: String,
    notice: String,
    path: String,
    export_folder: Option<PathBuf>,
    settings_back: Page,
    shown_page: Page,
    page_transition: f64,
    emblem: egui::TextureHandle,
    fullscreen: bool,
}

#[derive(Clone, Copy)]
enum Action {
    Delete(i64),
    Void,
}

impl App {
    fn open_settings(&mut self) {
        self.settings_back = self.page;
        self.page = Page::Settings;
        self.error.clear();
    }

    fn settings(&mut self, ui: &mut Ui) {
        ui.heading("Настройки");
        panel(ui, |ui| {
            ui.strong("Папка для экспорта");
            ui.label(self.export_folder.as_ref().map_or_else(
                || "Папка не выбрана".to_owned(),
                |path| path.display().to_string(),
            ));
            ui.label("CSV, Excel и JSON сохраняются сюда автоматически.");
            if primary(ui, "Выбрать папку", true) {
                let mut dialog =
                    rfd::FileDialog::new().set_title("Папка для экспорта ScoreTab CS2");
                if let Some(folder) = &self.export_folder {
                    dialog = dialog.set_directory(folder);
                }
                if let Some(folder) = dialog.pick_folder() {
                    match self.store.set_export_folder(&folder.to_string_lossy()) {
                        Ok(()) => {
                            self.export_folder = Some(folder);
                            self.error.clear();
                        }
                        Err(error) => self.error = error.to_string(),
                    }
                }
            }
        });
        if ui.button("Назад").clicked() {
            self.page = self.settings_back;
            self.error.clear();
        }
    }

    fn save_export(&mut self, value: &Match, extension: &str, bytes: Result<Vec<u8>, String>) {
        let Some(folder) = &self.export_folder else {
            return;
        };
        match bytes.and_then(|bytes| {
            write_export(folder, value.created_at, extension, &bytes)
                .map_err(|error| error.to_string())
        }) {
            Ok(path) => {
                self.error.clear();
                self.notice = format!("Сохранено: {}", path.display());
            }
            Err(error) => {
                self.notice.clear();
                self.error =
                    format!("Не удалось сохранить файл. Проверьте папку в настройках. {error}");
            }
        }
    }
    fn refresh(&mut self) {
        match self.store.list() {
            Ok(history) => {
                self.history = history.matches;
                self.error = if history.unreadable.is_empty() {
                    String::new()
                } else {
                    format!(
                        "Не удалось прочитать матчи: {:?}. Записи сохранены в базе.",
                        history.unreadable
                    )
                };
            }
            Err(error) => self.error = error.to_string(),
        }
    }

    fn open(&mut self, id: i64, value: Match) {
        self.page = if value.progress().is_ok_and(|p| p.finished) {
            Page::Protocol
        } else {
            Page::Round
        };
        self.current = Some((id, value));
        self.notice.clear();
    }

    fn change(&mut self, apply: impl FnOnce(&mut Match) -> RuleResult<()>) {
        let Some((id, before)) = self.current.as_mut() else {
            return;
        };
        let mut after = before.clone();
        let result = apply(&mut after).map_err(|e| e.to_string()).and_then(|_| {
            self.store
                .update(*id, before, &after)
                .map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => {
                *before = after;
                self.error.clear();
            }
            Err(error) => self.error = error,
        }
    }

    fn home(&mut self, ui: &mut Ui) {
        ui.heading("Судейский протокол");
        ui.label(RichText::new("Фиксация раундов Counter-Strike 2").color(MUTED));
        ui.add_space(16.0);
        if primary(ui, "Новый матч", true) {
            self.setup = initial_match();
            self.page = Page::Setup;
        }
        if ui.button("Настройки").clicked() {
            self.open_settings();
        }
        if let Some((id, value)) = self
            .history
            .iter()
            .find(|(_, value)| value.progress().is_ok_and(|p| !p.finished))
            .cloned()
        {
            if ui
                .button(format!(
                    "Продолжить · {} — {}",
                    value.teams[0].name, value.teams[1].name
                ))
                .clicked()
            {
                self.open(id, value);
            }
        }
        if ui.button("История матчей").clicked() {
            self.refresh();
            self.page = Page::History;
        }
        ui.add_space(12.0);
        let mut import = false;
        ui.collapsing("Импорт JSON", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.path).hint_text("Путь к файлу JSON"));
            import = ui.button("Импортировать").clicked();
        });
        if import {
            let result = (|| -> scoretab_cs2::storage::Result<_> {
                let value: Match = serde_json::from_slice(&std::fs::read(self.path.trim())?)?;
                let id = self.store.insert(&value)?;
                Ok((id, value))
            })();
            match result {
                Ok((id, value)) => {
                    self.error.clear();
                    self.open(id, value);
                }
                Err(error) => self.error = error.to_string(),
            }
        }
    }

    fn setup(&mut self, ui: &mut Ui) {
        ui.heading("Новый матч");
        panel(ui, |ui| {
            ui.strong("Команды и карта");
            for (index, team) in self.setup.teams.iter_mut().enumerate() {
                ui.label(format!("Команда {}", index + 1));
                ui.add(
                    egui::TextEdit::singleline(&mut team.name)
                        .desired_width(ui.available_width().min(500.0)),
                );
            }
            ui.horizontal_wrapped(|ui| {
                ui.label("Начальная сторона команды 1");
                for side in [Side::T, Side::CT] {
                    ui.selectable_value(&mut self.setup.starting_side, side, format!("{side:?}"));
                }
            });
            ui.label(format!(
                "Команда 2 начинает за {:?}",
                self.setup.starting_side.opposite()
            ));
            ui.label("Карта");
            ui.add(
                egui::TextEdit::singleline(&mut self.setup.map)
                    .desired_width(ui.available_width().min(500.0)),
            );
        });
        let rules = &mut self.setup.rules;
        panel(ui, |ui| {
            ui.strong("Основное время");
            number(
                ui,
                "Раундов основного времени",
                &mut rules.regulation,
                2..=200,
            );
            number(
                ui,
                "Смена сторон после раунда",
                &mut rules.switch_after,
                1..=199,
            );
            number(ui, "Побед для завершения", &mut rules.win_target, 2..=200);
        });
        panel(ui, |ui| {
            ui.strong("Овертайм");
            let mut overtime = rules.overtime_half.is_some();
            if ui.checkbox(&mut overtime, "Овертайм при ничьей").changed() {
                rules.overtime_half = overtime.then_some(3);
            }
            if let Some(half) = &mut rules.overtime_half {
                number(ui, "Раундов в половине овертайма", half, 1..=50);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Команда 1 начинает овертайм");
                    ui.selectable_value(&mut rules.overtime_start, None, "Как во второй половине");
                    ui.selectable_value(&mut rules.overtime_start, Some(Side::T), "T");
                    ui.selectable_value(&mut rules.overtime_start, Some(Side::CT), "CT");
                });
            }
        });
        if primary(ui, "Начать матч", true) {
            self.setup.created_at = now();
            match self.store.insert(&self.setup) {
                Ok(id) => {
                    self.error.clear();
                    self.open(id, self.setup.clone());
                }
                Err(error) => self.error = error.to_string(),
            }
        }
    }

    fn history(&mut self, ui: &mut Ui) {
        ui.heading("История матчей");
        if self.history.is_empty() {
            panel(ui, |ui| {
                ui.label("Сохранённых матчей пока нет.");
            });
        }
        let mut selected = None;
        for (id, value) in &self.history {
            let Ok(progress) = value.progress() else {
                continue;
            };
            let [a, b] = progress.score();
            panel(ui, |ui| {
                ui.label(
                    RichText::new(format!(
                        "{}  ·  {}",
                        value.teams[0].name, value.teams[1].name
                    ))
                    .size(23.0)
                    .strong(),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!("{a}:{b}"))
                            .size(28.0)
                            .strong()
                            .color(AMBER),
                    );
                    ui.label(RichText::new(utc_date(value.created_at)).color(MUTED));
                    if !value.map.is_empty() {
                        ui.label(&value.map);
                    }
                    ui.label(
                        RichText::new(if progress.finished {
                            "Завершён"
                        } else {
                            "Продолжается"
                        })
                        .color(if progress.finished {
                            SUCCESS
                        } else {
                            CT
                        }),
                    );
                    if ui.button("Открыть").clicked() {
                        selected = Some((*id, value.clone()));
                    }
                    if ui.small_button("Удалить").clicked() {
                        self.confirm = Some(Action::Delete(*id));
                    }
                });
            });
        }
        if let Some((id, value)) = selected {
            self.open(id, value);
        }
    }

    fn round(&mut self, ui: &mut Ui, value: &Match, progress: &Progress) {
        if progress.finished {
            ui.heading("Матч завершён");
            if ui.button("Протокол матча").clicked() {
                self.page = Page::Protocol;
            }
            return;
        }
        let number = value.next();
        ui.heading(round_title(value, number));
        if !self.notice.is_empty() {
            ui.label(RichText::new(&self.notice).color(AMBER));
        }
        let mut draft = value.draft.clone();
        edit_fields(ui, value, number, &mut draft);
        let valid = value.normalize(number, draft.clone());
        if draft.winner.is_some() && draft.reason.is_some() {
            if let Err(error) = &valid {
                ui.label(RichText::new(*error).color(AMBER));
            }
        }
        let keyboard = !ui.ctx().wants_keyboard_input()
            && self.confirm.is_none()
            && ui.input(|input| input.key_pressed(egui::Key::Enter));
        ui.add_space(10.0);
        let (next_round, protocol) = ui
            .horizontal_wrapped(|ui| {
                (
                    primary(ui, "Следующий раунд", valid.is_ok()),
                    ui.button("Протокол матча").clicked(),
                )
            })
            .inner;
        if next_round || (keyboard && valid.is_ok()) {
            let previous = value.position(number);
            self.change(|game| {
                game.draft = draft;
                game.commit(number, now())
            });
            if let Some((_, game)) = &self.current {
                if game.next() != number {
                    let position = game.position(game.next());
                    self.notice = if game.progress().is_ok_and(|p| !p.finished)
                        && previous.first_side != position.first_side
                    {
                        format!(
                            "Смена сторон · {} {:?} · {} {:?}",
                            game.teams[0].name,
                            position.first_side,
                            game.teams[1].name,
                            position.first_side.opposite()
                        )
                    } else {
                        String::new()
                    };
                }
            }
        } else if draft != value.draft {
            self.change(|game| {
                game.draft = draft;
                Ok(())
            });
        }
        if protocol {
            self.page = Page::Protocol;
        }
    }

    fn protocol(&mut self, ui: &mut Ui, value: &Match, progress: &Progress) {
        ui.heading("Протокол матча");
        ui.horizontal_wrapped(|ui| {
            if !progress.finished && ui.button("К текущему раунду").clicked() {
                self.page = Page::Round;
            }
            if ui.button("Копировать счёт").clicked() {
                ui.ctx().copy_text(score_text(value, progress));
            }
            if ui.button("Копировать протокол").clicked() {
                ui.ctx().copy_text(protocol_text(value, progress));
            }
        });
        if value.rounds.is_empty() {
            panel(ui, |ui| {
                ui.label("Завершённые раунды появятся здесь.");
            });
        }
        for (index, round) in value.rounds.iter().enumerate() {
            let position = value.position(round.number);
            let section = if index == 0 {
                Some("Первая половина".to_owned())
            } else {
                let previous = value.position(round.number - 1);
                if position.period != previous.period {
                    Some(format!("Овертайм {}", position.period))
                } else if position.first_side != previous.first_side {
                    Some(if position.period == 0 {
                        "Вторая половина".into()
                    } else {
                        "Смена сторон".into()
                    })
                } else {
                    None
                }
            };
            if let Some(section) = section {
                ui.label(RichText::new(section).color(AMBER).strong());
            }
            let [number, winner, side, plant, reason, score] =
                row(value, round, progress.scores[index]);
            panel(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!("РАУНД {number}"))
                            .color(MUTED)
                            .strong(),
                    );
                    ui.label(RichText::new(score).size(26.0).strong());
                    ui.label(
                        RichText::new(side.as_str())
                            .color(if side == "CT" { CT } else { AMBER })
                            .strong(),
                    );
                    ui.label(RichText::new(winner).strong());
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(reason);
                    ui.label(RichText::new(format!("Плент: {plant}")).color(MUTED));
                    if let Some(site) = &round.result.site {
                        ui.label(site);
                    }
                    if ui.small_button("Редактировать").clicked() {
                        self.edit = Some((round.number, round.result.clone()));
                    }
                });
                if !round.result.note.is_empty() {
                    ui.label(RichText::new(&round.result.note).color(MUTED));
                }
            });
        }
        ui.collapsing(
            "Экспорт и служебные действия",
            |ui| {
                let configured = self.export_folder.is_some();
                let folder_label = if configured {
                    "Изменить папку экспорта"
                } else {
                    "Настроить папку экспорта"
                };
                if ui.button(folder_label).clicked() {
                    self.open_settings();
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(configured, egui::Button::new("CSV"))
                        .clicked()
                    {
                        self.save_export(value, "csv", Ok(csv(value, progress).into_bytes()));
                    }
                    if ui
                        .add_enabled(configured, egui::Button::new("Excel (.xlsx)"))
                        .clicked()
                    {
                        let mut bytes = std::io::Cursor::new(Vec::new());
                        let result = scoretab_cs2::xlsx::export(&mut bytes, value, progress)
                            .map(|_| bytes.into_inner())
                            .map_err(|error| error.to_string());
                        self.save_export(value, "xlsx", result);
                    }
                    if ui
                        .add_enabled(configured, egui::Button::new("JSON"))
                        .clicked()
                    {
                        self.save_export(
                            value,
                            "json",
                            serde_json::to_vec_pretty(value).map_err(|error| error.to_string()),
                        );
                    }
                    let undo = value
                        .revisions
                        .last()
                        .is_some_and(|revision| revision.after.is_some());
                    if ui
                        .add_enabled(undo, egui::Button::new("Восстановить прошлую редакцию"))
                        .clicked()
                    {
                        self.change(|game| game.undo_edit(now()));
                    }
                    if ui
                        .add_enabled(
                            !value.rounds.is_empty(),
                            egui::Button::new("Аннулировать последний раунд"),
                        )
                        .clicked()
                    {
                        self.confirm = Some(Action::Void);
                    }
                });
                if !value.revisions.is_empty() {
                    ui.collapsing("История правок", |ui| {
                        for revision in value.revisions.iter().rev() {
                            ui.label(format!(
                                "{} · Раунд {} · {}",
                                utc_date(revision.saved_at),
                                revision.before.number,
                                if revision.after.is_some() {
                                    "Исправлен"
                                } else {
                                    "Аннулирован"
                                }
                            ));
                        }
                    });
                }
                ui.label(&self.notice);
            },
        );
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            self.fullscreen = !self.fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.edit = None;
            self.confirm = None;
        }
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::new().fill(SURFACE).inner_margin(16))
            .show(ctx, |ui| {
                let compact = ui.available_width() < 400.0;
                let emblem_size = if compact { 42.0 } else { 52.0 };
                let title_size = if compact { 21.0 } else { 25.0 };
                ui.horizontal_wrapped(|ui| {
                    egui::Frame::new()
                        .fill(RAISED)
                        .corner_radius(12)
                        .stroke(egui::Stroke::new(1.0_f32, OUTLINE))
                        .show(ui, |ui| {
                            ui.image((self.emblem.id(), egui::Vec2::splat(emblem_size)));
                        });
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        ui.label(RichText::new("ScoreTab").size(title_size).strong());
                        ui.label(RichText::new("CS2").size(title_size).color(AMBER).strong());
                    });
                    ui.label(RichText::new("by gearbarium").size(12.0).color(MUTED));
                    if self.page != Page::Home && ui.button("Главная").clicked() {
                        self.refresh();
                        self.page = Page::Home;
                        self.error.clear();
                    }
                });
            });
        let time = ctx.input(|input| input.time);
        if self.shown_page != self.page {
            self.shown_page = self.page;
            self.page_transition = time;
        }
        let transition = ((time - self.page_transition) / 0.15).clamp(0.0, 1.0) as f32;
        if transition < 1.0 {
            ctx.request_repaint();
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(24))
            .show(ctx, |ui| {
                ui.add_enabled_ui(self.edit.is_none() && self.confirm.is_none(), |ui| {
                    ui.add_space(14.0);
                    let limit = match self.page {
                        Page::Home | Page::Setup | Page::Settings => 680.0,
                        Page::History => 980.0,
                        _ => 1180.0,
                    };
                    centered_column(ui, limit, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.multiply_opacity(0.85 + 0.15 * transition);
                                if !self.error.is_empty() {
                                    ui.label(RichText::new(&self.error).color(AMBER));
                                }
                                match self.page {
                                    Page::Home => {
                                        panel(ui, |ui| {
                                            ui.vertical_centered(|ui| self.home(ui));
                                        });
                                    }
                                    Page::Setup => self.setup(ui),
                                    Page::History => self.history(ui),
                                    Page::Settings => self.settings(ui),
                                    Page::Round | Page::Protocol => {
                                        if let Some((_, value)) = self.current.clone() {
                                            match value.progress() {
                                                Ok(progress) => match self.page {
                                                    Page::Round
                                                        if ui.available_width() >= 910.0 =>
                                                    {
                                                        ui.columns(2, |columns| {
                                                            scoreboard(
                                                                &mut columns[0],
                                                                &value,
                                                                &progress,
                                                            );
                                                            panel(&mut columns[1], |ui| {
                                                                self.round(ui, &value, &progress)
                                                            });
                                                        });
                                                    }
                                                    Page::Round => {
                                                        scoreboard(ui, &value, &progress);
                                                        panel(ui, |ui| {
                                                            self.round(ui, &value, &progress)
                                                        });
                                                    }
                                                    _ => {
                                                        scoreboard(ui, &value, &progress);
                                                        self.protocol(ui, &value, &progress);
                                                    }
                                                },
                                                Err(error) => {
                                                    ui.label(error);
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                    });
                });
            });
        if let (Some((number, mut draft)), Some((_, value))) =
            (self.edit.clone(), self.current.clone())
        {
            egui::Window::new(format!("Редактировать раунд {number}"))
                .collapsible(false)
                .fade_in(true)
                .fade_out(true)
                .default_width(ctx.screen_rect().width().min(680.0) * 0.92)
                .max_height(ctx.screen_rect().height() * 0.78)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height((ctx.screen_rect().height() * 0.52).min(620.0))
                        .show(ui, |ui| edit_fields(ui, &value, number, &mut draft));
                    self.edit = Some((number, draft.clone()));
                    if !self.error.is_empty() {
                        ui.label(RichText::new(&self.error).color(AMBER));
                    }
                    if primary(
                        ui,
                        "Сохранить изменения",
                        value.normalize(number, draft.clone()).is_ok(),
                    ) {
                        self.change(|game| game.edit(number, draft, now()));
                        if self.error.is_empty() {
                            self.edit = None;
                        }
                    }
                    if ui.button("Отмена").clicked() {
                        self.edit = None;
                        self.error.clear();
                    }
                });
        }
        if let Some(action) = self.confirm {
            egui::Window::new("Подтверждение").collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(ctx, |ui| {
                ui.label(match action { Action::Delete(_) => "Удалить матч и его протокол?",
                    Action::Void => "Аннулировать последний раунд? Исходная запись останется в истории правок." });
                if ui.button("Подтвердить").clicked() {
                    match action {
                        Action::Delete(id) => match self.store.delete(id) {
                            Ok(()) => self.refresh(),
                            Err(error) => self.error = error.to_string(),
                        },
                        Action::Void => self.change(|game| game.void_last(now())),
                    }
                    self.confirm = None;
                }
                if ui.button("Отмена").clicked() { self.confirm = None; }
            });
        }
    }
}

fn centered_column(ui: &mut Ui, width_limit: f32, content: impl FnOnce(&mut Ui)) {
    let available = ui.available_rect_before_wrap();
    let width = available.width().min(width_limit);
    let left = available.center().x - width * 0.5;
    let rect = egui::Rect::from_min_max(
        egui::pos2(left, available.min.y),
        egui::pos2(left + width, available.max.y),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        content,
    );
}

fn panel(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0_f32, DIVIDER))
        .corner_radius(14)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            content(ui);
        });
}

fn primary(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    ui.scope(|ui| {
        let visuals = ui.visuals_mut();
        visuals.widgets.inactive.weak_bg_fill = if enabled { AMBER } else { RAISED };
        visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(255, 193, 91);
        visuals.widgets.active.weak_bg_fill = Color32::from_rgb(216, 139, 24);
        ui.add_enabled(
            enabled,
            egui::Button::new(
                RichText::new(label)
                    .color(if enabled { BG } else { MUTED })
                    .size(21.0)
                    .strong(),
            )
            .corner_radius(10)
            .min_size(egui::vec2(ui.available_width().min(280.0), 58.0)),
        )
        .clicked()
    })
    .inner
}

fn number(ui: &mut Ui, label: &str, value: &mut u32, range: std::ops::RangeInclusive<u32>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(range));
    });
}

fn choice(ui: &mut Ui, label: impl Into<String>, selected: bool) -> bool {
    let (fill, text, border) = if selected {
        (AMBER, BG, FOCUS)
    } else {
        (RAISED, TEXT, OUTLINE)
    };
    ui.add(
        egui::Button::new(RichText::new(label).color(text).strong())
            .fill(fill)
            .stroke(egui::Stroke::new(1.0_f32, border))
            .corner_radius(10)
            .min_size(egui::vec2(104.0, 48.0)),
    )
    .clicked()
}

fn action_card(ui: &mut Ui, label: &str, selected: bool, accent: Color32) -> bool {
    let id = ui.id().with(label);
    let strength = ui.ctx().animate_bool_with_time(id, selected, 0.14);
    let fill = RAISED.lerp_to_gamma(HOVER, strength);
    ui.add(
        egui::Button::new(RichText::new(label).strong().color(if selected {
            accent
        } else {
            TEXT
        }))
        .fill(fill)
        .stroke(egui::Stroke::new(
            1.0_f32,
            if selected { accent } else { DIVIDER },
        ))
        .min_size(egui::vec2(180.0_f32.min(ui.available_width()), 54.0))
        .corner_radius(12),
    )
    .clicked()
}

fn scoreboard(ui: &mut Ui, value: &Match, progress: &Progress) {
    let active = if progress.finished {
        value.next() - 1
    } else {
        value.next()
    };
    let position = value.position(active);
    let score = progress.score();
    let compact = ui.available_width() < 560.0;
    panel(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(if progress.finished {
                    "Матч завершён"
                } else {
                    "Текущий счёт"
                })
                .strong()
                .color(if progress.finished { SUCCESS } else { MUTED }),
            );
            if !value.map.is_empty() {
                ui.label(RichText::new(&value.map).color(MUTED));
            }
        });
        ui.columns(2, |columns| {
            for (index, cell) in columns.iter_mut().enumerate() {
                let side = if index == 0 {
                    position.first_side
                } else {
                    position.first_side.opposite()
                };
                let ct = cell.ctx().animate_bool_with_time(
                    cell.id().with((index, "side")),
                    side == Side::CT,
                    0.18,
                );
                let accent = AMBER.lerp_to_gamma(CT, ct);
                egui::Frame::new()
                    .fill(ELEVATED)
                    .corner_radius(14)
                    .stroke(egui::Stroke::new(1.0_f32, accent.gamma_multiply(0.60)))
                    .inner_margin(if compact { 10 } else { 16 })
                    .show(cell, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(format!("{side:?}")).strong().color(accent));
                            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.set_min_height(if compact { 42.0 } else { 54.0 });
                                ui.label(
                                    RichText::new(&value.teams[index].name)
                                        .size(if compact { 17.0 } else { 22.0 })
                                        .strong(),
                                );
                            });
                            let animated = ui.ctx().animate_value_with_time(
                                ui.id().with((value.created_at, index, "score")),
                                score[index] as f32,
                                0.18,
                            );
                            let pulse = (score[index] as f32 - animated).abs().min(1.0);
                            ui.label(
                                RichText::new(score[index].to_string())
                                    .size((if compact { 53.0 } else { 72.0 }) + pulse * 5.0)
                                    .strong(),
                            );
                        });
                    });
            }
        });
    });
}

fn round_title(value: &Match, number: u32) -> String {
    let position = value.position(number);
    if position.period == 0 {
        format!("Раунд {number}")
    } else {
        format!(
            "Раунд {number} · Овертайм {}, раунд {}",
            position.period, position.number
        )
    }
}

fn edit_fields(ui: &mut Ui, value: &Match, number: u32, draft: &mut Draft) {
    let first_side = value.position(number).first_side;
    ui.label(RichText::new("Победитель").strong());
    ui.horizontal_wrapped(|ui| {
        for (index, team) in value.teams.iter().enumerate() {
            let side = if index == 0 {
                first_side
            } else {
                first_side.opposite()
            };
            if action_card(
                ui,
                &format!("Победа {side:?} · {}", team.name),
                draft.winner == Some(team.id),
                if side == Side::CT { CT } else { AMBER },
            ) && draft.winner != Some(team.id)
            {
                draft.winner = Some(team.id);
                draft.reason = None;
            }
        }
    });
    let automatic = matches!(draft.reason, Some(Reason::Explosion | Reason::Defuse));
    egui::Frame::new()
        .fill(ELEVATED)
        .corner_radius(11)
        .inner_margin(8)
        .stroke(egui::Stroke::new(
            1.0_f32,
            if draft.planted { AMBER } else { DIVIDER },
        ))
        .show(ui, |ui| {
            ui.add_enabled(
                !automatic,
                egui::Checkbox::new(&mut draft.planted, "Бомба установлена"),
            );
        });
    if !draft.planted {
        draft.site = None;
    }
    if let Some(winner) = draft.winner.and_then(|id| value.team_index(id).ok()) {
        let side = if winner == 0 {
            first_side
        } else {
            first_side.opposite()
        };
        let reasons: &[Reason] = if side == Side::T {
            &[Reason::Elimination, Reason::Explosion]
        } else if draft.planted {
            &[Reason::Defuse]
        } else {
            &[Reason::Elimination, Reason::Defuse, Reason::Timeout]
        };
        ui.label("Причина победы");
        ui.horizontal_wrapped(|ui| {
            for &reason in reasons {
                if choice(ui, reason_label(reason), draft.reason == Some(reason)) {
                    draft.reason = Some(reason);
                    if matches!(reason, Reason::Explosion | Reason::Defuse) {
                        draft.planted = true;
                    }
                }
            }
        });
    }
    let details = egui::CollapsingHeader::new("Дополнительные сведения")
        .default_open(draft.reason == Some(Reason::Technical));
    details.show(ui, |ui| {
        if draft.winner.is_some() {
            ui.selectable_value(
                &mut draft.reason,
                Some(Reason::Technical),
                reason_label(Reason::Technical),
            );
        }
        if draft.planted {
            ui.horizontal(|ui| {
                ui.label("Плент");
                ui.selectable_value(&mut draft.site, None, "—");
                for site in ["A", "B"] {
                    ui.selectable_value(&mut draft.site, Some(site.into()), site);
                }
            });
        }
        ui.label(if draft.reason == Some(Reason::Technical) {
            "Основание решения"
        } else {
            "Комментарий судьи"
        });
        ui.text_edit_multiline(&mut draft.note);
    });
    if automatic && ui.small_button("Сбросить причину").clicked() {
        draft.reason = None;
    }
}

fn row(value: &Match, round: &Round, score: [u32; 2]) -> [String; 6] {
    let winner = value.team_index(round.result.winner.unwrap()).unwrap();
    let side = value.position(round.number).first_side;
    [
        round.number.to_string(),
        value.teams[winner].name.clone(),
        format!("{:?}", if winner == 0 { side } else { side.opposite() }),
        if round.result.planted {
            "Да".into()
        } else {
            "Нет".into()
        },
        reason_label(round.result.reason.unwrap()).into(),
        format!("{}:{}", score[0], score[1]),
    ]
}

fn score_text(value: &Match, progress: &Progress) -> String {
    let [a, b] = progress.score();
    format!("{} {a}:{b} {}", value.teams[0].name, value.teams[1].name)
}

fn protocol_text(value: &Match, progress: &Progress) -> String {
    let mut output = score_text(value, progress);
    for (round, score) in value.rounds.iter().zip(&progress.scores) {
        output.push('\n');
        output.push_str(&row(value, round, *score).join(" · "));
        if let Some(site) = &round.result.site {
            output.push_str(&format!(" · {site}"));
        }
        if !round.result.note.is_empty() {
            output.push_str(&format!(" · {}", round.result.note));
        }
    }
    output
}

fn csv(value: &Match, progress: &Progress) -> String {
    let mut output =
        String::from("\u{feff}Раунд,Победитель,Сторона,Плент,Причина,Счёт,Позиция,Комментарий\r\n");
    for (round, score) in value.rounds.iter().zip(&progress.scores) {
        let fields = row(value, round, *score);
        let extra = [
            round.result.site.as_deref().unwrap_or(""),
            round.result.note.as_str(),
        ];
        for (index, field) in fields.iter().map(String::as_str).chain(extra).enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push('"');
            if field.starts_with(['=', '+', '-', '@', '\t', '\r', '\n']) {
                output.push('\'');
            }
            output.push_str(&field.replace('"', "\"\""));
            output.push('"');
        }
        output.push_str("\r\n");
    }
    output
}

fn write_export(
    folder: &std::path::Path,
    match_time: i64,
    extension: &str,
    bytes: &[u8],
) -> std::io::Result<PathBuf> {
    use std::io::{ErrorKind, Write};
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    for copy in 0..100 {
        let suffix = if copy == 0 {
            String::new()
        } else {
            format!("-{copy}")
        };
        let path = folder.join(format!("ScoreTab-{match_time}-{time}{suffix}.{extension}"));
        let mut file = match std::fs::File::create_new(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        if let Err(error) = file.write_all(bytes) {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        return Ok(path);
    }
    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "Не удалось подобрать имя файла.",
    ))
}

fn utc_date(seconds: i64) -> String {
    let day = seconds.div_euclid(86_400) + 719_468;
    let era = day.div_euclid(146_097);
    let within = day.rem_euclid(146_097);
    let year = (within - within / 1460 + within / 36524 - within / 146096) / 365;
    let doy = within - (365 * year + year / 4 - year / 100);
    let month = (5 * doy + 2) / 153;
    let m = if month < 10 { month + 3 } else { month - 9 };
    let d = doy - (153 * month + 2) / 5 + 1;
    let y = year + 400 * era + i64::from(m <= 2);
    format!("{d:02}.{m:02}.{y:04}")
}

struct StartupError(String);

impl eframe::App for StartupError {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Не удалось открыть базу ScoreTab CS2");
            ui.label(&self.0);
        });
    }
}

fn main() -> eframe::Result {
    let app_icon =
        image::load_from_memory(include_bytes!("../../design/scoretab-cs2-app-icon.png"))
            .expect("embedded PNG")
            .to_rgba8();
    let window_icon =
        image::imageops::resize(&app_icon, 64, 64, image::imageops::FilterType::Lanczos3);
    let icon = egui::IconData {
        width: window_icon.width(),
        height: window_icon.height(),
        rgba: window_icon.into_raw(),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(icon)
            .with_inner_size([840.0, 860.0])
            .with_min_inner_size([360.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ScoreTab CS2",
        options,
        Box::new(move |cc| {
            let data = std::env::var_os("SCORETAB_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
                        .join("ScoreTab CS2")
                });
            let store = match std::fs::create_dir_all(&data)
                .map_err(|error| error.to_string())
                .and_then(|_| {
                    Store::open(&data.join("matches.sqlite3")).map_err(|error| error.to_string())
                }) {
                Ok(store) => store,
                Err(error) => return Ok(Box::new(StartupError(error))),
            };
            let emblem =
                image::imageops::resize(&app_icon, 160, 160, image::imageops::FilterType::Lanczos3);
            let emblem = cc.egui_ctx.load_texture(
                "scoretab-emblem",
                egui::ColorImage::from_rgba_unmultiplied([160, 160], emblem.as_raw()),
                Default::default(),
            );
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = BG;
            visuals.window_fill = SURFACE;
            visuals.override_text_color = Some(TEXT);
            visuals.widgets.inactive.weak_bg_fill = RAISED;
            visuals.widgets.inactive.bg_stroke.color = OUTLINE;
            visuals.widgets.hovered.bg_stroke.color = FOCUS;
            visuals.widgets.hovered.weak_bg_fill = HOVER;
            visuals.widgets.active.weak_bg_fill = Color32::from_rgb(216, 139, 24);
            visuals.widgets.noninteractive.bg_stroke.color = DIVIDER;
            visuals.selection.bg_fill = Color32::from_rgb(109, 72, 22);
            visuals.selection.stroke.color = AMBER;
            cc.egui_ctx.set_visuals(visuals);
            cc.egui_ctx.style_mut(|style| {
                style.spacing.item_spacing = egui::vec2(14.0, 12.0);
                style.spacing.button_padding = egui::vec2(18.0, 12.0);
                style.spacing.interact_size.y = 42.0;
                style.spacing.icon_width = 22.0;
                for (kind, size) in [
                    (egui::TextStyle::Heading, 34.0),
                    (egui::TextStyle::Body, 18.0),
                    (egui::TextStyle::Button, 19.0),
                ] {
                    style
                        .text_styles
                        .insert(kind, egui::FontId::proportional(size));
                }
            });
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "Inter".into(),
                std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                    "../../design/inter.ttf"
                ))),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "Inter".into());
            cc.egui_ctx.set_fonts(fonts);
            let export_folder = store
                .export_folder()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let mut app = App {
                store,
                page: Page::Home,
                setup: initial_match(),
                current: None,
                history: Vec::new(),
                edit: None,
                confirm: None,
                error: String::new(),
                notice: String::new(),
                path: String::new(),
                export_folder: export_folder.map(PathBuf::from),
                settings_back: Page::Home,
                shown_page: Page::Home,
                page_transition: -1.0,
                emblem,
                fullscreen: false,
            };
            app.refresh();
            Ok(Box::new(app))
        }),
    )
}
