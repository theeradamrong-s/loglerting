use iced::{
    executor, theme::Theme,
    widget::{button, column, container, row, scrollable, text, text_input, Column, Scrollable},
    Application, Command, Element, Length, Settings, Subscription,
};
use iced::time;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::PathBuf,
    time::Duration,
};
use directories::ProjectDirs;
use futures_timer::Delay;
use log::{info, warn};
use simplelog::{Config as LogConfig, LevelFilter, SimpleLogger, WriteLogger};
use chrono::Local;

const DEFAULT_LOG_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/test.log");

#[derive(Debug, Clone)]
enum Message {
    NavigateTo(Screen),
    NewLine(String),
    Tick,
    TogglePause,
    Clear,
    UpdateFilter(String),
    SaveSettings,
    LoadSettings,
    UpdateLogPaths(String),
    UpdateSearch(String),
    ClearSearch,
    ToggleTheme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Overview,
    Logs,
    Settings,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppSettings {
    log_paths: String,
    filter_regex: Option<String>,
    highlight_alerts: bool,
    theme: String, // "light" or "dark"
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            log_paths: DEFAULT_LOG_PATH.to_string(),
            filter_regex: None,
            highlight_alerts: true,
            theme: "light".to_string(),
        }
    }
}

struct LogLerting {
    screen: Screen,
    all_logs: Vec<String>,
    logs: Vec<String>,
    paused: bool,
    filter_input: String,
    search_input: String,
    settings: AppSettings,
    config_file: Option<PathBuf>,
    scroll_id: scrollable::Id,
    error_msg: Option<String>,
}

impl LogLerting {
    fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "example", "loglerting")
            .map(|proj| {
                let dir = proj.config_dir();
                fs::create_dir_all(dir).ok();
                dir.join("config.toml")
            })
    }

    fn load_settings() -> AppSettings {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                if let Ok(text) = fs::read_to_string(&path) {
                    if let Ok(cfg) = toml::from_str::<AppSettings>(&text) {
                        return cfg;
                    } else {
                        warn!("Failed to parse config TOML at {:?}", path);
                    }
                }
            }
        }
        AppSettings::default()
    }

    fn save_settings(&self) -> Result<(), String> {
        if let Some(path) = Self::config_path() {
            let t = toml::to_string_pretty(&self.settings)
                .map_err(|e| format!("Failed to serialize settings: {}", e))?;
            fs::write(&path, t).map_err(|e| format!("Failed to write settings: {}", e))?;
            info!("Saved settings to {:?}", path);
            Ok(())
        } else {
            Err("Config path not available".into())
        }
    }

    fn parse_paths(s: &str) -> Vec<PathBuf> {
        s.split(',')
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .collect()
    }

    fn reapply_filter(&mut self) {
        if self.filter_input.trim().is_empty() {
            self.logs = self.all_logs.clone();
            return;
        }

        match Regex::new(&self.filter_input) {
            Ok(re) => {
                self.logs = self
                    .all_logs
                    .iter()
                    .filter(|l| re.is_match(l))
                    .cloned()
                    .collect();
            }
            Err(e) => {
                self.error_msg = Some(format!("Invalid regex: {}", e));
                warn!("Invalid filter regex: {}", e);
            }
        }
    }
}

struct TailState {
    paths: Vec<PathBuf>,
    offsets: Vec<u64>,
}

impl TailState {
    fn new(paths: Vec<PathBuf>) -> Self {
        let offsets = paths
            .iter()
            .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
            .collect();
        Self { paths, offsets }
    }
}

impl Application for LogLerting {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Self::Message>) {
        let _ = SimpleLogger::init(LevelFilter::Info, LogConfig::default());
        if let Ok(file) = OpenOptions::new().create(true).append(true).open(DEFAULT_LOG_PATH) {
            let _ = WriteLogger::init(LevelFilter::Info, LogConfig::default(), file);
        }

        let settings = Self::load_settings();

        (
            LogLerting {
                screen: Screen::Overview,
                all_logs: Vec::new(),
                logs: Vec::new(),
                paused: false,
                filter_input: settings.filter_regex.clone().unwrap_or_default(),
                search_input: String::new(),
                settings,
                config_file: Self::config_path(),
                scroll_id: scrollable::Id::unique(),
                error_msg: None,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "LogLerting".to_string()
    }

    fn theme(&self) -> Self::Theme {
        if self.settings.theme == "dark" {
            Theme::Dark
        } else {
            Theme::Light
        }
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        match message {
            Message::NavigateTo(screen) => self.screen = screen,
            Message::NewLine(line) => {
                if !self.paused {
                    let matches_regex = if self.filter_input.trim().is_empty() {
                        true
                    } else {
                        Regex::new(&self.filter_input)
                            .map(|re| re.is_match(&line))
                            .unwrap_or(true)
                    };

                    if matches_regex {
                        self.all_logs.push(line.clone());
                        self.logs.push(line);
                        if self.logs.len() > 10_000 {
                            self.logs.drain(0..5_000);
                        }
                    }
                }
            }
            Message::TogglePause => self.paused = !self.paused,
            Message::Clear => {
                self.all_logs.clear();
                self.logs.clear();
            }
            Message::UpdateFilter(s) => {
                self.filter_input = s.clone();
                self.settings.filter_regex = if s.trim().is_empty() { None } else { Some(s) };
            }
            Message::UpdateSearch(s) => {
                self.search_input = s.clone();
                if s.trim().is_empty() {
                    self.paused = false;
                    self.logs = self.all_logs.clone();
                } else {
                    self.paused = true;
                    self.logs = self
                        .all_logs
                        .iter()
                        .filter(|l| l.contains(&s))
                        .cloned()
                        .collect();
                }
            }
            Message::ClearSearch => {
                self.search_input.clear();
                self.paused = false;
                self.logs = self.all_logs.clone();
            }
            Message::SaveSettings => {
                if let Err(e) = self.save_settings() {
                    self.error_msg = Some(e);
                } else {
                    self.error_msg = Some("Settings saved.".to_string());
                }
            }
            Message::LoadSettings => {
                self.settings = Self::load_settings();
                self.filter_input = self.settings.filter_regex.clone().unwrap_or_default();
            }
            Message::UpdateLogPaths(s) => self.settings.log_paths = s,
            Message::ToggleTheme => {
                self.settings.theme = if self.settings.theme == "light" {
                    "dark".to_string()
                } else {
                    "light".to_string()
                };
                let _ = self.save_settings();
            }
            _ => {}
        }
        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.screen != Screen::Logs {
            return Subscription::none();
        }

        let paths = Self::parse_paths(&self.settings.log_paths);
        if paths.is_empty() {
            return time::every(Duration::from_secs(1)).map(|_| Message::Tick);
        }

        let initial = TailState::new(paths);
        iced::subscription::unfold("tailer", initial, |mut state| async move {
            let mut emitted = None;

            for i in 0..state.paths.len() {
                let path = &state.paths[i];
                if let Ok(meta) = std::fs::metadata(path) {
                    let len = meta.len();
                    if len < state.offsets[i] {
                        state.offsets[i] = 0;
                    }
                    if let Ok(mut f) = File::open(path) {
                        f.seek(SeekFrom::Start(state.offsets[i])).ok();
                        let mut reader = BufReader::new(&mut f);
                        let mut line = String::new();
                        while let Ok(n) = reader.read_line(&mut line) {
                            if n == 0 { break; }
                            state.offsets[i] += n as u64;
                            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                            let filename = path.file_name()
                                .map(|f| f.to_string_lossy())
                                .unwrap_or_else(|| "unknown".into());

                            emitted = Some(Message::NewLine(format!(
                                "[{}] {}: {}",
                                timestamp,
                                filename,
                                line.trim()
                            )));

                            line.clear();
                            break;
                        }
                    }
                }
            }

            Delay::new(Duration::from_millis(500)).await;
            (emitted.unwrap_or(Message::Tick), state)
        })
    }

    fn view(&self) -> Element<Message> {
        let menu = row![
            button(text("Overview")).on_press(Message::NavigateTo(Screen::Overview)),
            button(text("Logs")).on_press(Message::NavigateTo(Screen::Logs)),
            button(text("Settings")).on_press(Message::NavigateTo(Screen::Settings)),
        ]
        .spacing(10);

        let content: Element<_> = match self.screen {
            Screen::Overview => column![
                text("LogLerting - Overview").size(24),
                text(format!(
                    "Watching: {}\nEntries: {}\nPaused: {}",
                    self.settings.log_paths,
                    self.logs.len(),
                    self.paused
                )),
                row![
                    button(text("Go Logs")).on_press(Message::NavigateTo(Screen::Logs)),
                    button(text("Clear")).on_press(Message::Clear)
                ]
            ]
            .spacing(12)
            .into(),

            Screen::Logs => {
                let controls = column![
                    row![
                        button(text(if self.paused { "Resume" } else { "Pause" }))
                            .on_press(Message::TogglePause),
                        button(text("Clear")).on_press(Message::Clear),
                    ]
                    .spacing(10),
                    row![
                        text_input("Filter (Case Sensitive)", &self.filter_input)
                            .on_input(Message::UpdateFilter)
                            .padding(6)
                            .width(Length::Fixed(320.0)),
                        text_input("Search", &self.search_input)
                            .on_input(Message::UpdateSearch)
                            .padding(6)
                            .width(Length::Fixed(320.0)),
                        button(text("Clear Search")).on_press(Message::ClearSearch),
                        button(text("Save Settings")).on_press(Message::SaveSettings),
                    ]
                    .spacing(10)
                ]
                .spacing(10);

                let log_col = self.logs.iter()
                    .fold(Column::new().spacing(4), |c, l| c.push(text(l)));

                let scroll = Scrollable::new(log_col)
                    .id(self.scroll_id.clone())
                    .width(Length::Fill)
                    .height(Length::Fill);

                column![controls, scroll].spacing(30).into()
            }

            Screen::Settings => column![
                text("Settings").size(24),
                text(format!("Config: {}", self.config_file.as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default())),
                text(format!("Current theme: {}", self.settings.theme)),
                row![
                    text("Log paths:"),
                    text_input("paths", &self.settings.log_paths)
                        .on_input(Message::UpdateLogPaths)
                        .padding(6)
                        .width(Length::Fixed(480.0))
                ],
                row![
                    button(text("Reload")).on_press(Message::LoadSettings),
                    button(text("Save")).on_press(Message::SaveSettings),
                    button(text(if self.settings.theme == "light" {
                        "Switch to Dark"
                    } else {
                        "Switch to Light"
                    }))
                    .on_press(Message::ToggleTheme),
                ],
                if let Some(ref msg) = self.error_msg {
                    row![text(format!("Status: {}", msg))]
                } else {
                    row![text("")]
                }
            ]
            .spacing(10)
            .into(),
        };

        container(column![menu, content].spacing(16).padding(16))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn main() -> iced::Result {
    LogLerting::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(1000.0, 700.0),
            ..Default::default()
        },
        ..Settings::default()
    })
}
