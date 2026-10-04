//! Where the game's files come from: the base assets (a directory or `soldat.smod`), the font,
//! the mods, and the config directory.

use super::*;

/// The interface font (`font_1_filename`), a file of its own next to opensoldat's
/// `soldat.smod`.
pub(crate) const FONT_FILE: &str = "play-regular.ttf";

/// Where the game's files come from, to mount them again with a server's mod.
pub(crate) struct Assets {
    /// The game's own files.
    pub(crate) base: PathBuf,
    /// In the browser: the game's archive and the font, fetched (mounted again from memory).
    pub(crate) archive: Option<Arc<[u8]>>,
    pub(crate) font: Option<Arc<[u8]>>,
    /// The player's mods (`--mod`).
    pub(crate) mods: Vec<PathBuf>,
    pub(crate) config_dir: PathBuf,
}

impl Assets {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn new(cli: &Cli) -> anyhow::Result<Assets> {
        let base = match cli
            .assets
            .clone()
            .or_else(soldank_core::assets::find_game_files)
        {
            Some(path) => path,
            None => anyhow::bail!(
                "no game assets found: put Soldat's assets in ./assets or ./soldat.smod (or next \
                 to the program), or point --assets / SOLDANK_ASSETS at them (see README)"
            ),
        };
        // (the game's files' folder: Soldat's portable mode, its default)
        let config_dir = cli.config_dir.clone().unwrap_or_else(|| {
            base.parent()
                .filter(|dir| !dir.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
        });
        Ok(Assets {
            base,
            archive: None,
            font: None,
            mods: cli.mods.clone(),
            config_dir,
        })
    }

    /// The game's files: maps and images from servers below them (never replacing them),
    /// then a server's mod over them, or else the player's own mods (Soldat's `ModDir` is
    /// one or the other).
    pub(crate) fn mount(&self, game_mod: Option<&GameMod>) -> anyhow::Result<Vfs> {
        let mut vfs = Vfs::new();
        let downloads = self.config_dir.join("downloads");
        if downloads.is_dir() {
            vfs.mount(&downloads)
                .with_context(|| format!("cannot mount {}", downloads.display()))?;
        }
        match &self.archive {
            Some(archive) => vfs.mount_archive("soldat.smod", archive.clone())?,
            None => vfs
                .mount_game_files(&self.base)
                .with_context(|| format!("cannot mount assets from {}", self.base.display()))?,
        }
        if let Some(font) = &self.font {
            vfs.add(FONT_FILE, font.to_vec());
        }
        // soldat.smod has no font: opensoldat's comes as a file of its own, next to it
        if !vfs.exists(FONT_FILE)
            && let Some(dir) = self.base.parent()
            && let Ok(font) = std::fs::read(dir.join(FONT_FILE))
        {
            vfs.add(FONT_FILE, font);
        }
        let mods = match game_mod {
            Some(game_mod) => vec![self.mod_path(game_mod)],
            None => self.mods.clone(),
        };
        for path in &mods {
            vfs.mount(path)
                .with_context(|| format!("cannot mount mod {}", path.display()))?;
        }
        Ok(vfs)
    }

    /// Where a server's mod is kept: `mods/<name>.smod` in the config directory.
    pub(crate) fn mod_path(&self, game_mod: &GameMod) -> PathBuf {
        self.config_dir.join(game_mod.path())
    }

    /// The mod's archive is here, and it's the server's.
    pub(crate) fn has_mod(&self, game_mod: &GameMod) -> bool {
        std::fs::read(self.mod_path(game_mod)).is_ok_and(|bytes| file_hash(&bytes) == game_mod.hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_font_next_to_the_game_files_is_found() {
        let dir = std::env::temp_dir().join(format!("soldank-font-{}", std::process::id()));
        let base = dir.join("base");
        std::fs::create_dir_all(base.join("maps")).unwrap();
        std::fs::write(base.join("maps/x.pms"), b"map").unwrap();
        std::fs::write(dir.join(FONT_FILE), b"font").unwrap();
        let assets = Assets {
            base: base.clone(),
            archive: None,
            font: None,
            mods: Vec::new(),
            config_dir: dir.clone(),
        };
        let vfs = assets.mount(None).unwrap();
        assert_eq!(vfs.read(FONT_FILE).unwrap(), b"font");
        assert!(vfs.exists("maps/x.pms"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
