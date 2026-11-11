# Runtime dependency policy

File detection, carving, image validation, and export run in Rust. No FFmpeg,
SQLite executable, shell, PowerShell, diskutil, or lsblk is required by the app.
Checked-in media/database fixtures need no external decoder during tests.

Drive discovery uses Linux kernel files and UDisks D-Bus APIs, Windows Win32
APIs, or macOS IOKit/Disk Arbitration APIs. Native dialogs and accessibility
use OS libraries and desktop services. Those services are allowed API providers;
this policy does not require the operating system to run in one process.

The updater uses HTTPS through Rust libraries and SHA-256 verification. Unix
executable replacement uses Rust filesystem operations. On Windows, Rust code
starts the verified Inno Setup package directly through the Win32 process API;
the app never launches cmd.exe, PowerShell, or a cleanup executable. Native OS
APIs open browser links and installer files without command helpers.

On Linux a native D-Bus session address is required. Process-backed transports,
autolaunch, and fallback address lists are rejected before starting the GUI.

Clippy rejects first-party process-command creation, and cargo-deny bans
self-replace, webbrowser, and open helper dependencies. Development and release
packaging use the separate Rust tooling crate (`cargo xtask`), Rust security tools, and native platform packaging
tools. They are not invoked by the installed application.
