use super::parse::{split_commands, tokenize};
use super::{ConfigError, CvarValue, Cvars, Result};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

const MAX_ALIAS_DEPTH: usize = 16;

const BUILTINS: &[(&str, &str)] = &[
    ("alias", "alias <name> <commands>: define a command alias"),
    (
        "bind",
        "bind <key> <command>: bind a key, e.g. bind space +jump",
    ),
    ("cmdlist", "cmdlist: list commands"),
    ("cvarlist", "cvarlist [prefix]: list cvars"),
    ("echo", "echo <text>: print text"),
    ("exec", "exec <file>: run a config file"),
    (
        "inc",
        "inc <cvar> <step> [min] [max]: add step to a numeric cvar",
    ),
    ("reset", "reset <cvar>: restore default value"),
    ("toggle", "toggle <cvar>: flip a boolean cvar"),
    ("unbind", "unbind <key>: remove a key binding"),
    ("unbindall", "unbindall: remove all key bindings"),
];

/// A command the console doesn't execute itself; the game drains and handles these
/// (e.g. `quit`, `map`, `kill`).
#[derive(Debug, Clone, PartialEq)]
pub struct Deferred {
    pub name: String,
    pub args: Vec<String>,
}

/// Key name (lowercase, e.g. `a`, `space`, `mouse1`) to command line.
#[derive(Debug, Default, Clone)]
pub struct Bindings {
    keys: BTreeMap<String, String>,
}

impl Bindings {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.keys.get(&key.to_lowercase()).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.keys.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

#[derive(Debug, Default)]
pub struct Console {
    pub cvars: Cvars,
    pub bindings: Bindings,
    /// Directory `exec` resolves relative paths against.
    pub config_dir: PathBuf,
    commands: BTreeMap<String, &'static str>,
    aliases: HashMap<String, String>,
    deferred: Vec<Deferred>,
    output: Vec<String>,
    depth: usize,
}

impl Console {
    pub fn new(cvars: Cvars) -> Console {
        Console {
            cvars,
            ..Default::default()
        }
    }

    /// Registers a game command; invocations are queued for [`Console::take_deferred`].
    pub fn register_command(&mut self, name: &str, help: &'static str) {
        self.commands.insert(name.to_owned(), help);
    }

    pub fn print(&mut self, line: impl Into<String>) {
        self.output.push(line.into());
    }

    /// Lines printed since the last call.
    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }

    /// Game commands invoked since the last call.
    pub fn take_deferred(&mut self) -> Vec<Deferred> {
        std::mem::take(&mut self.deferred)
    }

    /// Executes a line; errors are printed to the console output and also returned
    /// (the first one) so callers like startup config loading can react.
    pub fn execute(&mut self, line: &str) -> Result<()> {
        let mut first_error = None;

        for command in split_commands(line) {
            let tokens = tokenize(command);
            if tokens.is_empty() {
                continue;
            }

            if let Err(e) = self.execute_tokens(&tokens) {
                self.print(e.to_string());
                first_error.get_or_insert(e);
            }
        }

        first_error.map_or(Ok(()), Err)
    }

    /// Executes a whole config file's contents line by line.
    pub fn execute_script(&mut self, script: &str) -> Result<()> {
        let mut first_error = None;

        for line in script.lines() {
            if let Err(e) = self.execute(line) {
                first_error.get_or_insert(e);
            }
        }

        first_error.map_or(Ok(()), Err)
    }

    fn execute_tokens(&mut self, tokens: &[String]) -> Result<()> {
        let name = tokens[0].to_lowercase();
        let args = &tokens[1..];

        if BUILTINS.iter().any(|(builtin, _)| *builtin == name) {
            return self.builtin(&name, args);
        }

        if self.commands.contains_key(&name) {
            self.deferred.push(Deferred {
                name,
                args: args.to_vec(),
            });
            return Ok(());
        }

        if let Some(body) = self.aliases.get(&name).cloned() {
            if self.depth >= MAX_ALIAS_DEPTH {
                return Err(ConfigError::Recursion(name));
            }

            self.depth += 1;
            let result = self.execute(&body);
            self.depth -= 1;
            return result;
        }

        match args {
            [] => {
                let cvar = self
                    .cvars
                    .get(&name)
                    .ok_or_else(|| ConfigError::Unknown(name.clone()))?;
                let line = format!("{} = \"{}\" - {}", cvar.name, cvar.value, cvar.description);
                self.print(line);
                Ok(())
            }
            [value, ..] => self.cvars.set(&name, value),
        }
    }

    fn builtin(&mut self, name: &str, args: &[String]) -> Result<()> {
        match (name, args) {
            ("echo", args) => {
                let line = args.join(" ");
                self.print(line);
            }
            ("alias", [alias, body @ ..]) if !body.is_empty() => {
                self.aliases.insert(alias.to_lowercase(), body.join(" "));
            }
            ("bind", [key, command @ ..]) if !command.is_empty() => {
                self.bindings
                    .keys
                    .insert(key.to_lowercase(), command.join(" "));
            }
            ("unbind", [key]) => {
                self.bindings.keys.remove(&key.to_lowercase());
            }
            ("unbindall", []) => self.bindings.keys.clear(),
            ("reset", [cvar]) => self.cvars.reset(cvar)?,
            ("toggle", [cvar]) => {
                let value = !self.cvars.bool(self.known(cvar)?);
                self.cvars.set(cvar, if value { "1" } else { "0" })?;
            }
            ("inc", [cvar, step, rest @ ..]) if rest.len() <= 2 => {
                self.known(cvar)?;
                let number = |s: &String| {
                    s.parse::<f64>()
                        .map_err(|_| ConfigError::Usage("inc <cvar> <step> [min] [max]"))
                };
                let step = number(step)?;
                let min = rest.first().map(number).transpose()?.unwrap_or(f64::MIN);
                let max = rest.get(1).map(number).transpose()?.unwrap_or(f64::MAX);
                let value = match self.cvars.get(cvar).map(|c| &c.value) {
                    Some(CvarValue::Int(v)) => {
                        ((*v as f64 + step).clamp(min, max) as i64).to_string()
                    }
                    Some(CvarValue::Float(v)) => {
                        ((f64::from(*v) + step).clamp(min, max) as f32).to_string()
                    }
                    _ => return Err(ConfigError::Usage("inc works on numeric cvars")),
                };
                self.cvars.set(cvar, &value)?;
            }
            ("exec", [file]) => {
                let path = self.config_dir.join(file);
                let script = std::fs::read_to_string(&path).map_err(|e| ConfigError::Exec {
                    path: path.display().to_string(),
                    reason: e.to_string(),
                })?;
                tracing::info!(path = %path.display(), "exec");
                self.execute_script(&script)?;
            }
            ("cvarlist", args) => {
                let prefix = args.first().map(String::as_str).unwrap_or("");
                let lines: Vec<String> = self
                    .cvars
                    .iter()
                    .filter(|c| c.name.starts_with(prefix))
                    .map(|c| format!("{} = \"{}\" - {}", c.name, c.value, c.description))
                    .collect();
                self.output.extend(lines);
            }
            ("cmdlist", []) => {
                let mut lines: Vec<String> =
                    BUILTINS.iter().map(|(_, help)| help.to_string()).collect();
                lines.extend(self.commands.values().map(|help| help.to_string()));
                lines.sort();
                self.output.extend(lines);
            }
            _ => {
                let help = BUILTINS
                    .iter()
                    .find(|(b, _)| *b == name)
                    .map_or("", |(_, help)| help);
                return Err(ConfigError::Usage(help));
            }
        }

        Ok(())
    }

    fn known<'a>(&self, cvar: &'a str) -> Result<&'a str> {
        match self.cvars.get(cvar) {
            Some(_) => Ok(cvar),
            None => Err(ConfigError::Unknown(cvar.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Cvar;

    fn console() -> Console {
        let mut cvars = Cvars::new();
        cvars.register(Cvar::float("sv_gravity", 0.06, "Gravity"));
        cvars.register(Cvar::bool("cl_debug", false, "Debug overlay"));
        cvars.register(Cvar::int("r_zoom", 0, "Zoom").range(-5.0, 5.0));
        let mut console = Console::new(cvars);
        console.register_command("quit", "quit: exit the game");
        console
    }

    #[test]
    fn sets_and_prints_cvars() {
        let mut c = console();
        c.execute("sv_gravity 0.1; sv_gravity").unwrap();
        assert_eq!(c.cvars.float("sv_gravity"), 0.1);
        assert_eq!(c.take_output(), vec![r#"sv_gravity = "0.1" - Gravity"#]);
    }

    #[test]
    fn reports_errors_but_keeps_going() {
        let mut c = console();
        let result = c.execute("nonsense 1; sv_gravity 0.2");
        assert_eq!(result, Err(ConfigError::Unknown("nonsense".into())));
        assert_eq!(c.cvars.float("sv_gravity"), 0.2);
    }

    #[test]
    fn toggle_inc_reset() {
        let mut c = console();
        c.execute("toggle cl_debug; inc r_zoom 3; inc r_zoom 3 -5 5")
            .unwrap();
        assert!(c.cvars.bool("cl_debug"));
        assert_eq!(c.cvars.int("r_zoom"), 5);
        c.execute("reset r_zoom").unwrap();
        assert_eq!(c.cvars.int("r_zoom"), 0);
    }

    #[test]
    fn aliases_and_recursion_guard() {
        let mut c = console();
        c.execute(r#"alias lowgrav "sv_gravity 0.01; echo low""#)
            .unwrap();
        c.execute("lowgrav").unwrap();
        assert_eq!(c.cvars.float("sv_gravity"), 0.01);
        assert_eq!(c.take_output(), vec!["low"]);

        c.execute("alias loop loop").unwrap();
        assert_eq!(
            c.execute("loop"),
            Err(ConfigError::Recursion("loop".into()))
        );
    }

    #[test]
    fn bindings_and_deferred_commands() {
        let mut c = console();
        c.execute("bind SPACE +jump; bind F10 quit; quit now")
            .unwrap();
        assert_eq!(c.bindings.get("space"), Some("+jump"));
        assert_eq!(
            c.take_deferred(),
            vec![Deferred {
                name: "quit".into(),
                args: vec!["now".into()]
            }]
        );
        c.execute("unbind space").unwrap();
        assert_eq!(c.bindings.get("space"), None);
    }

    #[test]
    fn exec_runs_files_relative_to_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("autoexec.cfg"),
            "// startup\nsv_gravity 0.5\ntoggle cl_debug\n",
        )
        .unwrap();

        let mut c = console();
        c.config_dir = dir.path().to_owned();
        c.execute("exec autoexec.cfg").unwrap();
        assert_eq!(c.cvars.float("sv_gravity"), 0.5);
        assert!(c.cvars.bool("cl_debug"));
    }
}
