Overview:
    LogLerting is a desktop app written in Rust using Iced GUI. It monitors log files in real-time, allows regex filtering, and saves user settings. There are three screens: Overview, Logs, and Settings.

State Model:
    The app state is stored in the LogLerting struct:
    * screen — current screen.
    * logs — list of log lines displayed.
    * paused — pause/resume tailing.
    * filter_input — regex for filtering logs.
    * settings — user settings including log paths and highlight options.
    * scroll_id — scrollable ID for the Logs screen.

Message Flow:
    App behavior is controlled by the Message enum:
    * NavigateTo(Screen) — switch screens.
    * NewLine(String) — append new log lines.
    * TogglePause — pause/resume log updates.
    * Clear — clear log display.
    * UpdateFilter(String) — update regex filter.
    * SaveSettings / LoadSettings — persist/load configuration.
    * UpdateLogPaths(String) — change watched log files.

Subscriptions:
    * Log tailing runs in a background async task using iced::subscription::unfold.
    * Each tick reads new lines from files and sends NewLine messages.
    * Updates occur without blocking the UI.

Config File:
    * Stored in the user config directory using the directories crate.
    * File format: TOML.