//! The console and kill logs (`LogFile.pas`): `logs/consolelog-<yy-mm-dd>-NN.txt` and
//! `logs/kills/killlog-<yy-mm-dd>-NN.txt` in the config directory, with `log_enable` and
//! `log_level` (above 0: off by default, like Soldat's), written every `log_filesupdate` ticks
//! and started anew past 500 KB.

use std::path::{Path, PathBuf};

/// `MAX_LOGFILESIZE`
const MAX_FILE_SIZE: u64 = 512_000;

/// A log file and the lines not written yet.
struct Log {
    path: PathBuf,
    lines: Vec<String>,
}

impl Log {
    /// `NewLogFile`: the first free `<kind>-<date>-NN.txt` in `dir`, with its first line.
    fn new(dir: &Path, kind: &str, first: &str) -> Log {
        let date = chrono::Local::now().format("%y-%m-%d");
        let path = (1..)
            .map(|n| dir.join(format!("{kind}-{date}-{n:02}.txt")))
            .find(|p| !p.exists())
            .unwrap_or_default();
        let mut log = Log {
            path,
            lines: Vec::new(),
        };
        log.add(first, true);
        log
    }

    /// `AddLineToLogFile`
    fn add(&mut self, line: &str, dated: bool) {
        if line.is_empty() {
            return;
        }
        self.lines.push(if dated {
            format!(
                "{} {line}",
                chrono::Local::now().format("%y/%m/%d %H:%M:%S")
            )
        } else {
            line.to_string()
        });
    }

    /// `WriteLogFile`: the new lines at the end of the file.
    fn write(&mut self) {
        use std::io::Write;
        if self.lines.is_empty() {
            return;
        }
        let text: String = self.lines.drain(..).map(|l| l + "\r\n").collect();
        let written = self
            .path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?
                    .write_all(text.as_bytes())
            });
        if let Err(error) = written {
            tracing::warn!(%error, path = %self.path.display(), "File logging error");
        }
    }

    fn too_big(&self) -> bool {
        std::fs::metadata(&self.path).is_ok_and(|m| m.len() > MAX_FILE_SIZE)
    }
}

/// The server's logs.
pub struct Logs {
    dir: PathBuf,
    console: Log,
    kills: Log,
}

impl Logs {
    /// `NewLogFiles` in `dir/logs`.
    pub fn new(dir: &Path) -> Logs {
        let dir = dir.join("logs");
        Logs {
            console: Log::new(&dir, "consolelog", "   Console Log Started"),
            kills: Log::new(&dir.join("kills"), "killlog", "   Kill Log Started"),
            dir,
        }
    }

    /// A console line.
    pub fn console(&mut self, line: &str) {
        self.console.add(line, true);
    }

    /// A kill: when, who, whom, with what (`TSprite.Die`).
    pub fn kill(&mut self, killer: &str, victim: &str, weapon: &str) {
        let when = chrono::Local::now().format("%y/%m/%d %H:%M:%S");
        for line in [&format!("--- {when}"), killer, victim, weapon] {
            self.kills.add(line, false);
        }
    }

    /// To the files; full ones are started anew.
    pub fn write(&mut self) {
        self.kills.write();
        self.console.write();
        if self.kills.too_big() || self.console.too_big() {
            let dir = self.dir.clone();
            *self = Logs::new(dir.parent().unwrap_or(&dir));
        }
    }
}
