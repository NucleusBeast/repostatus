#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use iced::{
    Alignment, Border, Color, Element, Fill, Font, Size, Subscription, Task, Theme,
    widget::{
        self, button, checkbox, column, container, row, scrollable, slider, stack, text,
        text_input, tooltip,
    },
};
use repostatus::{
    git::{self, Action, Status},
    storage,
};
use std::{path::PathBuf, time::Duration};

mod appearance;
mod icon;
mod lifecycle;
mod reopen;
mod stats;
mod theme;
mod tray;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("RepoStatus · Git Projects")
        .theme(App::theme)
        .style(|_, theme| iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: theme::Colors::for_theme(theme).ink,
        })
        .default_font(Font::MONOSPACE)
        .subscription(App::subscription)
        .window(iced::window::Settings {
            size: Size::new(1080.0, 520.0),
            min_size: Some(Size::new(980.0, 340.0)),
            exit_on_close_request: false,
            transparent: appearance::SUPPORTED,
            icon: if cfg!(target_os = "windows") {
                Some(
                    iced::window::icon::from_rgba(icon::rgba(32, false), 32, 32)
                        .expect("valid app icon"),
                )
            } else {
                None
            },
            ..Default::default()
        })
        .run()
}

struct Project {
    path: PathBuf,
    status: Option<Status>,
    busy: Option<&'static str>,
    error: Option<String>,
}
impl Project {
    fn name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_string_lossy()
            .into_owned()
    }
}

#[derive(Default)]
enum Dialog {
    #[default]
    None,
    Add,
    Settings,
    Commit(PathBuf),
    Remove(PathBuf),
}
struct App {
    projects: Vec<Project>,
    config: Option<PathBuf>,
    notice: Option<String>,
    dialog: Dialog,
    input: String,
    dialog_error: Option<String>,
    adding: bool,
    tray_ready: bool,
    transparency: u8,
    keep_text_opaque: bool,
    show_stats: bool,
    stats: Option<stats::Snapshot>,
    theme_preference: storage::ThemePreference,
    system_theme: iced::theme::Mode,
}
#[derive(Debug, Clone)]
enum Message {
    WindowOpened(iced::window::Id),
    CloseWindow(iced::window::Id),
    TrayReady(Result<(), String>),
    Tray(tray::Command),
    ToggleWindow(Option<iced::window::Id>),
    Reopen,
    ShowWindow(Option<iced::window::Id>),
    ReopenReady(Result<(), String>),
    DockVisibilityChanged(Result<(), String>),
    Settings,
    ThemeSelected(storage::ThemePreference),
    SystemThemeChanged(iced::theme::Mode),
    TransparencyChanged(u8),
    KeepTextOpaque(bool),
    ShowStats(bool),
    StatsUpdated(Option<stats::Snapshot>),
    ApplyTransparency(Option<iced::window::Id>),
    TransparencyApplied(Result<(), String>),
    SaveSettings,
    ResetTransparency,
    Refresh,
    Add,
    Browse,
    Picked(Option<PathBuf>),
    Input(String),
    SubmitAdd,
    Added(Result<PathBuf, String>),
    CloseDialog,
    RequestRemove(PathBuf),
    Remove(PathBuf),
    RequestCommit(PathBuf),
    Commit(PathBuf),
    Run(PathBuf, Action),
    Finished(PathBuf, Result<Status, String>),
    Dismiss,
    Key(iced::keyboard::Event),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let loaded = std::env::var_os("REPOSTATUS_CONFIG")
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(storage::config_path)
            .and_then(|path| storage::load_config(&path).map(|config| (path, config)));
        let (config, saved, notice) = match loaded {
            Ok((path, saved)) => (Some(path), saved, None),
            Err(e) => (None, storage::Config::default(), Some(e)),
        };
        let mut app = Self {
            projects: saved
                .paths
                .into_iter()
                .map(|path| Project {
                    path,
                    status: None,
                    busy: None,
                    error: None,
                })
                .collect(),
            config,
            notice,
            dialog: Dialog::None,
            input: String::new(),
            dialog_error: None,
            adding: false,
            tray_ready: false,
            transparency: saved.transparency,
            keep_text_opaque: saved.keep_text_opaque,
            show_stats: saved.show_stats,
            stats: None,
            theme_preference: saved.theme,
            system_theme: iced::theme::Mode::None,
        };
        let task = Task::batch([
            app.refresh(),
            iced::system::theme().map(Message::SystemThemeChanged),
        ]);
        (app, task)
    }

    fn theme(&self) -> Theme {
        theme::resolve(self.theme_preference, self.system_theme)
    }

    fn colors(&self) -> theme::Colors {
        theme::Colors::for_theme(&self.theme())
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::system::theme_changes().map(Message::SystemThemeChanged),
            iced::window::open_events().map(Message::WindowOpened),
            iced::window::close_requests().map(Message::CloseWindow),
            tray::subscription().map(Message::Tray),
            reopen::subscription().map(|()| Message::Reopen),
            stats::subscription(self.show_stats).map(Message::StatsUpdated),
            iced::time::every(Duration::from_secs(60)).map(|_| Message::Refresh),
            iced::keyboard::listen().map(Message::Key),
            iced::event::listen_with(|event, status, _| {
                if status == iced::event::Status::Captured
                    && matches!(
                        event,
                        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                            ..
                        })
                    )
                {
                    Some(Message::CloseDialog)
                } else {
                    None
                }
            }),
        ])
    }

    fn refresh(&mut self) -> Task<Message> {
        let tasks = self
            .projects
            .iter_mut()
            .filter(|p| p.busy.is_none())
            .map(|p| {
                p.busy = Some("Checking…");
                let path = p.path.clone();
                Task::perform(git::perform(path.clone(), Action::Refresh), move |result| {
                    Message::Finished(path.clone(), result)
                })
            })
            .collect::<Vec<_>>();
        Task::batch(tasks)
    }

    fn persist(&mut self) {
        if let Some(config) = &self.config {
            if let Err(e) = storage::save_config(
                config,
                &storage::Config {
                    paths: self.projects.iter().map(|p| p.path.clone()).collect(),
                    transparency: self.transparency,
                    keep_text_opaque: self.keep_text_opaque,
                    show_stats: self.show_stats,
                    theme: self.theme_preference,
                },
            ) {
                self.notice = Some(e);
            }
        } else {
            self.notice = Some(
                "Projects and settings cannot be saved. Repair the configuration file and restart the app."
                    .into(),
            );
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowOpened(id) => {
                let setup = Task::batch([
                    appearance::apply(id, self.transparency, self.keep_text_opaque)
                        .map(Message::TransparencyApplied),
                    iced::window::run(id, |_| reopen::initialize()).map(Message::ReopenReady),
                ]);
                return if tray::SUPPORTED {
                    Task::batch([
                        setup,
                        iced::window::run(id, |_| tray::initialize()).map(Message::TrayReady),
                    ])
                } else {
                    setup
                };
            }
            Message::Settings => {
                self.dialog = Dialog::Settings;
                self.dialog_error = None;
            }
            Message::ThemeSelected(preference) => {
                self.theme_preference = preference;
                self.persist();
            }
            Message::SystemThemeChanged(mode) => self.system_theme = mode,
            Message::ShowStats(enabled) => {
                self.show_stats = enabled;
                self.stats = None;
                self.persist();
            }
            Message::StatsUpdated(snapshot) if self.show_stats => self.stats = snapshot,
            Message::TransparencyChanged(value) if appearance::SUPPORTED => {
                self.transparency = value.min(50);
                return iced::window::oldest().map(Message::ApplyTransparency);
            }
            Message::KeepTextOpaque(value) if appearance::SUPPORTED => {
                self.keep_text_opaque = value;
                self.persist();
                return iced::window::oldest().map(Message::ApplyTransparency);
            }
            Message::ApplyTransparency(Some(id)) => {
                return appearance::apply(id, self.transparency, self.keep_text_opaque)
                    .map(Message::TransparencyApplied);
            }
            Message::TransparencyApplied(Err(error)) => {
                self.notice = Some(format!("Could not change window transparency: {error}"));
            }
            Message::SaveSettings => self.persist(),
            Message::ResetTransparency => {
                let task = self.update(Message::TransparencyChanged(0));
                self.persist();
                return task;
            }
            Message::TrayReady(result) => match result {
                Ok(()) => self.tray_ready = true,
                Err(error) => self.notice = Some(error),
            },
            Message::CloseWindow(id) => {
                if self.tray_ready {
                    return lifecycle::hide(id).map(Message::DockVisibilityChanged);
                }
                return iced::exit();
            }
            Message::Tray(tray::Command::Toggle) => {
                return iced::window::oldest().map(Message::ToggleWindow);
            }
            Message::ToggleWindow(Some(id)) => {
                return lifecycle::toggle(id).map(Message::DockVisibilityChanged);
            }
            Message::Reopen => return iced::window::oldest().map(Message::ShowWindow),
            Message::ShowWindow(Some(id)) => {
                return lifecycle::show(id).map(Message::DockVisibilityChanged);
            }
            Message::ReopenReady(Err(error)) => self.notice = Some(error),
            Message::DockVisibilityChanged(Err(error)) => self.notice = Some(error),
            Message::DockVisibilityChanged(Ok(()))
                if self.notice.as_deref() == Some(lifecycle::DOCK_ERROR) =>
            {
                self.notice = None;
            }
            Message::Tray(tray::Command::Refresh) => return self.refresh(),
            Message::Tray(tray::Command::Quit) => return iced::exit(),
            Message::Key(iced::keyboard::Event::KeyPressed { key, modifiers, .. })
                if modifiers.is_empty() =>
            {
                match key.as_ref() {
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                        return self.update(Message::CloseDialog);
                    }
                    iced::keyboard::Key::Character("a") if matches!(self.dialog, Dialog::None) => {
                        return self.update(Message::Add);
                    }
                    iced::keyboard::Key::Character("r") if matches!(self.dialog, Dialog::None) => {
                        return self.update(Message::Refresh);
                    }
                    _ => {}
                }
            }
            Message::Refresh => return self.refresh(),
            Message::Add => {
                self.dialog = Dialog::Add;
                self.input.clear();
                self.dialog_error = None;
                return widget::operation::focus("dialog-input");
            }
            Message::Browse => {
                self.adding = true;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Choose a Git project")
                            .pick_folder()
                            .await
                            .map(|f| f.path().to_owned())
                    },
                    Message::Picked,
                );
            }
            Message::Picked(path) => {
                self.adding = false;
                if let Some(path) = path {
                    self.input = path.to_string_lossy().into_owned();
                    return self.update(Message::SubmitAdd);
                }
            }
            Message::Input(input) => self.input = input,
            Message::SubmitAdd if !self.adding => {
                if self.input.trim().is_empty() {
                    self.dialog_error = Some("Choose or enter a project folder.".into());
                    return Task::none();
                }
                let input = self.input.trim();
                let path = if let Some(suffix) = input.strip_prefix("~/") {
                    dirs::home_dir().unwrap_or_default().join(suffix)
                } else {
                    PathBuf::from(input)
                };
                self.adding = true;
                return Task::perform(git::repository_root(path), Message::Added);
            }
            Message::Added(result) => {
                self.adding = false;
                match result {
                    Ok(path) => {
                        if self.projects.iter().any(|p| p.path == path) {
                            self.dialog_error =
                                Some("This project is already on your dashboard.".into());
                        } else {
                            self.projects.push(Project {
                                path: path.clone(),
                                status: None,
                                busy: None,
                                error: None,
                            });
                            self.persist();
                            self.dialog = Dialog::None;
                            return self.update(Message::Run(path, Action::Refresh));
                        }
                    }
                    Err(e) => self.dialog_error = Some(e),
                }
            }
            Message::CloseDialog if !self.adding => {
                self.dialog = Dialog::None;
                self.dialog_error = None;
            }
            Message::RequestRemove(path) => self.dialog = Dialog::Remove(path),
            Message::Remove(path) => {
                self.projects.retain(|p| p.path != path || p.busy.is_some());
                self.persist();
                self.dialog = Dialog::None;
            }
            Message::RequestCommit(path) => {
                self.dialog = Dialog::Commit(path);
                self.input.clear();
                self.dialog_error = None;
                return widget::operation::focus("dialog-input");
            }
            Message::Commit(path) => {
                if self.input.trim().is_empty() {
                    self.dialog_error = Some("Enter a commit message.".into());
                } else {
                    self.dialog = Dialog::None;
                    return self.update(Message::Run(path, Action::Commit(self.input.clone())));
                }
            }
            Message::Run(path, action) => {
                if let Some(project) = self
                    .projects
                    .iter_mut()
                    .find(|p| p.path == path && p.busy.is_none())
                {
                    project.busy = Some(match &action {
                        Action::Refresh => "Checking…",
                        Action::Pull => "Pulling…",
                        Action::Push => "Pushing…",
                        Action::Commit(_) => "Committing…",
                    });
                    project.error = None;
                    return Task::perform(git::perform(path.clone(), action), move |result| {
                        Message::Finished(path.clone(), result)
                    });
                }
            }
            Message::Finished(path, result) => {
                if let Some(project) = self.projects.iter_mut().find(|p| p.path == path) {
                    project.busy = None;
                    match result {
                        Ok(status) => {
                            project.status = Some(status);
                            project.error = None;
                        }
                        Err(error) => {
                            project.error = Some(error);
                            project.status = None;
                        }
                    }
                }
            }
            Message::Dismiss => self.notice = None,
            _ => {}
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let colors = self.colors();
        let background_only = appearance::SUPPORTED && self.keep_text_opaque;
        let incoming = self
            .projects
            .iter()
            .filter(|p| p.status.as_ref().is_some_and(Status::incoming))
            .count();
        let outbound = self
            .projects
            .iter()
            .filter(|p| p.status.as_ref().is_some_and(Status::outbound))
            .count();
        let busy = self.projects.iter().filter(|p| p.busy.is_some()).count();
        let header = row![
            text("$ repostatus").size(18).color(colors.green),
            text("/ local git projects").size(12).color(colors.muted),
            widget::Space::new().width(Fill),
            action_button("[r] refresh", colors.ink).on_press(Message::Refresh),
            action_button("[a] add", colors.green).on_press(Message::Add),
            action_button("[settings]", colors.muted).on_press(Message::Settings),
        ]
        .spacing(14)
        .align_y(Alignment::Center);
        let headings = row![
            text("#").width(24),
            text("PROJECT").width(iced::Length::FillPortion(2)),
            text("PATH").width(iced::Length::FillPortion(3)),
            text("BRANCH").width(110),
            text("STATUS").width(155),
            text("ACTIONS").width(173),
        ]
        .spacing(12);
        let mut table = column![container(headings).padding([8, 10]).style(move |_| panel(
            if background_only {
                Color::TRANSPARENT
            } else {
                colors.header
            },
            colors
        ))]
        .spacing(0);
        if self.projects.is_empty() {
            table = table.push(
                container(
                    column![
                        text("No repositories added.").size(13).color(colors.ink),
                        text("Press a or click [a] add to choose a local Git project.")
                            .size(12)
                            .color(colors.muted),
                    ]
                    .spacing(8),
                )
                .padding([22, 10]),
            );
        } else {
            table = table.push(
                scrollable(
                    column(
                        self.projects
                            .iter()
                            .enumerate()
                            .map(|(i, p)| self.project_view(p, i)),
                    )
                    .spacing(0),
                )
                .height(Fill),
            );
        }
        let table = container(table).width(Fill).height(Fill).style(move |_| {
            panel(
                if background_only {
                    Color::TRANSPARENT
                } else {
                    colors.bg
                },
                colors,
            )
        });
        let mut content = column![header].spacing(10);
        if let Some(notice) = &self.notice {
            content = content.push(
                container(
                    row![
                        text(notice).size(12).color(colors.red).width(Fill),
                        action_button("[dismiss]", colors.muted).on_press(Message::Dismiss)
                    ]
                    .spacing(12),
                )
                .padding(8)
                .style(move |_| {
                    panel(
                        if background_only {
                            Color::TRANSPARENT
                        } else {
                            colors.header
                        },
                        colors,
                    )
                }),
            );
        }
        let mut footer = row![
            text(format!("{} projects", self.projects.len()))
                .size(12)
                .color(colors.ink),
            text(format!("{incoming} incoming"))
                .size(12)
                .color(colors.blue),
            text(format!("{outbound} outbound"))
                .size(12)
                .color(colors.amber),
        ]
        .spacing(16)
        .align_y(Alignment::Center);
        if self.show_stats {
            footer = footer.push(tooltip(
                text(self.stats.map_or_else(|| "CPU — · RAM —".into(), |snapshot| snapshot.label()))
                    .size(11)
                    .color(colors.muted),
                mono("RepoStatus process · updates every 2 seconds\nCPU: 100% = one fully used core; can exceed 100%\nRAM: resident memory; excludes Git subprocesses"),
                tooltip::Position::Top,
            ));
        }
        content = content.push(table).push(
            footer.push(widget::Space::new().width(Fill)).push(
                text(if busy > 0 {
                    format!("checking {busy} · auto 60s")
                } else if self.tray_ready {
                    "auto 60s · close window to keep running".into()
                } else {
                    "auto 60s · esc closes dialog".into()
                })
                .size(11)
                .color(colors.muted),
            ),
        );
        let base = container(content)
            .padding(14)
            .width(Fill)
            .height(Fill)
            .style(move |_| container::Style {
                background: Some(
                    appearance::background(colors.bg, self.transparency, self.keep_text_opaque)
                        .into(),
                ),
                text_color: Some(colors.ink),
                ..Default::default()
            });
        if matches!(self.dialog, Dialog::None) {
            base.into()
        } else {
            let modal = container(scrollable(self.dialog_view()).height(iced::Length::Shrink))
                .padding(18)
                .width(520)
                .style(move |_| panel(colors.header, colors));
            let overlay =
                container(modal)
                    .center_x(Fill)
                    .center_y(Fill)
                    .style(|_| container::Style {
                        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.65).into()),
                        ..Default::default()
                    });
            stack![base, widget::opaque(overlay)].into()
        }
    }

    fn project_view<'a>(&'a self, p: &'a Project, index: usize) -> Element<'a, Message> {
        let colors = self.colors();
        let s = p.status.as_ref();
        let busy = p.busy.is_some();
        let (label, color) = if let Some(label) = p.busy {
            (label.into(), colors.muted)
        } else if let Some(s) = s {
            if s.conflicts {
                ("Conflicts".into(), colors.red)
            } else if s.detached {
                ("Detached".into(), colors.muted)
            } else if s.incoming() && s.outbound() {
                ("Incoming+Outbound".into(), colors.amber)
            } else if s.incoming() {
                (format!("Incoming ↓{}", s.behind), colors.blue)
            } else if s.outbound() {
                (
                    format!("Outbound ↑{} *{}", s.ahead, s.changed),
                    colors.amber,
                )
            } else if s.upstream.is_none() {
                ("No upstream".into(), colors.muted)
            } else {
                ("Default".into(), colors.green)
            }
        } else {
            ("Unavailable".into(), colors.red)
        };
        let mut pull = action_button("[pull]", colors.blue).width(52);
        if !busy && s.is_some_and(Status::can_pull) {
            pull = pull.on_press(Message::Run(p.path.clone(), Action::Pull));
        }
        let dirty = s.is_some_and(|s| s.changed > 0);
        let mut push =
            action_button(if dirty { "[commit]" } else { "[push]" }, colors.amber).width(69);
        if !busy {
            if s.is_some_and(|s| s.changed > 0 && !s.conflicts && !s.detached) {
                push = push.on_press(Message::RequestCommit(p.path.clone()));
            } else if s.is_some_and(Status::can_push) {
                push = push.on_press(Message::Run(p.path.clone(), Action::Push));
            }
        }
        let mut remove = action_button("[x]", colors.red).width(28);
        if !busy {
            remove = remove.on_press(Message::RequestRemove(p.path.clone()));
        }
        let main = row![
            text(format!("{:02}", index + 1)).size(12).color(colors.muted).width(24),
            container(tooltip(text(compact(&p.name(), 20)).size(13).color(colors.ink), mono(p.name()), tooltip::Position::Top)).width(iced::Length::FillPortion(2)),
            container(tooltip(text(compact(&display_path(&p.path), 34)).size(12).color(colors.muted), mono(p.path.to_string_lossy().into_owned()), tooltip::Position::Top)).width(iced::Length::FillPortion(3)),
            container(tooltip(text(compact(s.map_or("—", |s| s.branch.as_str()), 13)).size(12).color(colors.muted), mono(s.map_or("No branch", |s| s.branch.as_str())), tooltip::Position::Top)).width(110),
            container(tooltip(text(label).size(12).color(color), mono(s.map_or_else(|| "Status unavailable".into(), |s| format!("{} incoming commits / {} outgoing commits / {} changed files\nupstream: {}", s.behind, s.ahead, s.changed, s.upstream.as_deref().unwrap_or("none")))), tooltip::Position::Top)).width(155),
            row![pull, push, tooltip(remove, mono("Remove from list; keep files"), tooltip::Position::Top)].spacing(6).width(173),
        ].spacing(12).align_y(Alignment::Center);
        let mut lines = column![main].spacing(4);
        if let Some(s) = s {
            if s.ahead > 0 && s.behind > 0 {
                lines = lines.push(
                    mono("  ! diverged: merge or rebase in Git before syncing").color(colors.amber),
                );
            } else if s.behind > 0 && s.changed > 0 {
                lines = lines.push(
                    mono("  ! commit or stash local changes before pulling").color(colors.amber),
                );
            }
            if s.upstream.is_none() && !s.detached {
                lines = lines.push(
                    mono("  ! set a tracking branch in Git to enable sync").color(colors.muted),
                );
            }
            if let Some(warning) = &s.warning {
                lines = lines.push(
                    mono(format!(
                        "  ! remote check failed; local snapshot: {warning}"
                    ))
                    .color(colors.red),
                );
            }
        }
        if let Some(error) = &p.error {
            lines = lines.push(mono(format!("  ! {error}")).color(colors.red));
        }
        container(lines)
            .padding([5, 10])
            .width(Fill)
            .style(move |_| container::Style {
                background: Some(
                    if appearance::SUPPORTED && self.keep_text_opaque {
                        if index.is_multiple_of(2) {
                            Color::TRANSPARENT
                        } else {
                            Color {
                                a: 0.05,
                                ..colors.ink
                            }
                        }
                    } else if index.is_multiple_of(2) {
                        colors.bg
                    } else {
                        colors.stripe
                    }
                    .into(),
                ),
                ..Default::default()
            })
            .into()
    }

    fn dialog_view(&self) -> Element<'_, Message> {
        let colors = self.colors();
        let mut body = column![].spacing(12);
        let heading = match &self.dialog {
            Dialog::Settings => Some(("settings", colors.green)),
            Dialog::Add => Some(("+ add project", colors.green)),
            Dialog::Commit(_) => Some(("+ commit changes", colors.amber)),
            Dialog::Remove(_) => Some(("- remove project?", colors.red)),
            Dialog::None => None,
        };
        if let Some((title, color)) = heading {
            body = body.push(mono(title).size(16).color(color));
        }
        match &self.dialog {
            Dialog::Settings => {
                let mut choices = row![].spacing(16);
                for (label, preference) in [
                    ("Light", storage::ThemePreference::Light),
                    ("Dark", storage::ThemePreference::Dark),
                    ("System", storage::ThemePreference::System),
                ] {
                    choices = choices.push(
                        widget::radio(
                            label,
                            preference,
                            Some(self.theme_preference),
                            Message::ThemeSelected,
                        )
                        .text_size(12),
                    );
                }
                body = body
                    .push(mono("theme").color(colors.ink))
                    .push(choices)
                    .push(
                        mono("System follows your desktop appearance. Saved automatically.")
                            .color(colors.muted),
                    );
                if appearance::SUPPORTED {
                    body = body
                        .push(
                            mono(format!("Transparency: {}%", self.transparency)).color(colors.ink),
                        )
                        .push(
                            slider(0..=50, self.transparency, Message::TransparencyChanged)
                                .step(1)
                                .on_release(Message::SaveSettings),
                        )
                        .push(row![
                            mono("Opaque (0%)"),
                            widget::Space::new().width(Fill),
                            mono("50%")
                        ])
                        .push(
                            checkbox(self.keep_text_opaque)
                                .label("Keep text opaque")
                                .text_size(12)
                                .on_toggle(Message::KeepTextOpaque),
                        )
                        .push(
                            mono(if self.keep_text_opaque {
                                "Only the background fades; text and controls stay opaque."
                            } else {
                                "The whole window fades, including text and title bar."
                            })
                            .color(colors.muted),
                        )
                        .push(
                            mono("Changes apply immediately and are saved automatically.")
                                .color(colors.muted),
                        )
                        .push(
                            action_button("[reset to opaque]", colors.ink)
                                .on_press(Message::ResetTransparency),
                        );
                } else {
                    body = body.push(
                        mono("Window transparency is available on macOS and Windows.")
                            .color(colors.muted),
                    );
                }
                body = body
                    .push(mono("footer").color(colors.ink))
                    .push(
                        checkbox(self.show_stats)
                            .label("Show CPU and memory usage")
                            .text_size(12)
                            .on_toggle(Message::ShowStats),
                    )
                    .push(
                        mono("App usage updates every 2 seconds. Saved automatically.")
                            .color(colors.muted),
                    );
            }
            Dialog::Add => {
                body = body
                    .push(mono("Choose an existing local Git repository.").color(colors.muted))
                    .push(
                        text_input("~/projects/repository", &self.input)
                            .id("dialog-input")
                            .size(13)
                            .on_input(Message::Input)
                            .on_submit(Message::SubmitAdd)
                            .padding(8)
                            .style(input_style),
                    )
                    .push(
                        row![
                            action_button(
                                if self.adding {
                                    "[opening…]"
                                } else {
                                    "[browse]"
                                },
                                colors.ink
                            )
                            .on_press_maybe((!self.adding).then_some(Message::Browse)),
                            widget::Space::new().width(Fill),
                            action_button("[add project]", colors.green)
                                .on_press_maybe((!self.adding).then_some(Message::SubmitAdd))
                        ]
                        .spacing(10),
                    );
            }
            Dialog::Commit(path) => {
                body = body
                    .push(
                        mono(format!(
                            "repo: {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ))
                        .color(colors.ink),
                    )
                    .push(
                        mono("Stages all changed/untracked files. Push separately.")
                            .color(colors.muted),
                    )
                    .push(
                        text_input("Commit message", &self.input)
                            .id("dialog-input")
                            .size(13)
                            .on_input(Message::Input)
                            .on_submit(Message::Commit(path.clone()))
                            .padding(8)
                            .style(input_style),
                    )
                    .push(
                        action_button("[stage all & commit]", colors.amber)
                            .on_press(Message::Commit(path.clone())),
                    );
            }
            Dialog::Remove(path) => {
                body = body
                    .push(mono(path.to_string_lossy().into_owned()).color(colors.ink))
                    .push(
                        mono("Removes the list entry. Repository files stay on disk.")
                            .color(colors.muted),
                    )
                    .push(
                        action_button("[remove]", colors.red)
                            .on_press(Message::Remove(path.clone())),
                    );
            }
            Dialog::None => {}
        }
        if let Some(error) = &self.dialog_error {
            body = body.push(mono(error).color(colors.red));
        }
        body.push(
            action_button(
                if matches!(self.dialog, Dialog::Settings) {
                    "[esc] close"
                } else {
                    "[esc] cancel"
                },
                colors.muted,
            )
            .on_press_maybe((!self.adding).then_some(Message::CloseDialog)),
        )
        .into()
    }
}

fn mono<'a>(value: impl text::IntoFragment<'a>) -> widget::Text<'a> {
    text(value).font(Font::MONOSPACE).size(12)
}
fn panel(background: Color, colors: theme::Colors) -> container::Style {
    container::Style {
        background: Some(background.into()),
        text_color: Some(colors.ink),
        border: Border {
            color: colors.line,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}
fn action_button<'a>(label: &'a str, color: Color) -> widget::Button<'a, Message> {
    button(mono(label))
        .padding([3, 2])
        .style(move |theme, status| {
            let colors = theme::Colors::for_theme(theme);
            button::Style {
                background: (status == button::Status::Hovered
                    || status == button::Status::Pressed)
                    .then_some(colors.line.into()),
                text_color: if status == button::Status::Disabled {
                    colors.dim
                } else {
                    color
                },
                border: Border::default(),
                ..Default::default()
            }
        })
}
fn input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let colors = theme::Colors::for_theme(theme);
    text_input::Style {
        background: colors.bg.into(),
        border: Border {
            color: if matches!(status, text_input::Status::Focused { .. }) {
                colors.green
            } else {
                colors.line
            },
            width: 1.0,
            radius: 0.0.into(),
        },
        icon: colors.muted,
        placeholder: colors.muted,
        value: colors.ink,
        selection: colors.line,
    }
}
fn display_path(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(suffix) = path.strip_prefix(home)
    {
        return format!("~/{}", suffix.display());
    }
    path.display().to_string()
}
fn compact(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    let first: String = value.chars().take(max / 2 - 1).collect();
    let last: String = value
        .chars()
        .rev()
        .take(max / 2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{first}…{last}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_selection_persists_and_system_changes_preserve_explicit_choices() {
        let temp = tempfile::TempDir::new().unwrap();
        let config_path = temp.path().join("projects.json");
        let mut app = App {
            projects: vec![],
            config: Some(config_path.clone()),
            notice: None,
            dialog: Dialog::Settings,
            input: String::new(),
            dialog_error: None,
            adding: false,
            tray_ready: false,
            transparency: 25,
            keep_text_opaque: true,
            show_stats: false,
            stats: None,
            theme_preference: storage::ThemePreference::System,
            system_theme: iced::theme::Mode::Light,
        };
        for (preference, expected) in [
            (storage::ThemePreference::Dark, Theme::Dark),
            (storage::ThemePreference::Light, Theme::Light),
            (storage::ThemePreference::System, Theme::Dark),
        ] {
            let _ = app.update(Message::ThemeSelected(preference));
            let _ = app.update(Message::SystemThemeChanged(iced::theme::Mode::Dark));
            assert_eq!(app.theme(), expected);
            assert!(matches!(app.dialog, Dialog::Settings));
            let saved = storage::load_config(&config_path).unwrap();
            assert_eq!(saved.theme, preference);
            assert_eq!(saved.transparency, 25);
            assert!(saved.keep_text_opaque);
            assert!(!saved.show_stats);
        }
        let _ = app.update(Message::SystemThemeChanged(iced::theme::Mode::Light));
        assert_eq!(app.theme(), Theme::Light);
        let _ = app.update(Message::CloseDialog);
        assert!(matches!(app.dialog, Dialog::None));
    }
}
