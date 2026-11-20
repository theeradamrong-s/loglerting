# LogLerting

A desktop app that shows new lines in a log file as they are written.

**Built with:** Rust, Iced, Tokio, regex

## Features
- Live updates while the log file changes
- Filter and keyword search
- Pause / resume and clear
- Settings are saved between runs

## Running it

```bash
cargo run
```

The app starts by watching `src/test.log`. To see live updates, generate log lines in another terminal:

```bash
cd src && python3 loggen.py
```
