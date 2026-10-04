//! Virtual filesystem for game assets.
//!
//! Assets are looked up across a stack of mounted layers: plain directories and `.smod`
//! archives (zip files whose root is the asset root, e.g. `anims/`, `maps/`, `textures/`).
//! Later mounts override earlier ones, so a mod can be layered on top of the base game.
//!
//! Lookups are case-insensitive and accept both `/` and `\` separators, matching how
//! Soldat resolves paths on every platform.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use thiserror::Error;
use zip::ZipArchive;

#[derive(Debug, Error)]
pub enum VfsError {
    #[error("asset not found: {0}")]
    NotFound(String),
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid archive {path}: {source}")]
    Zip {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },
}

pub type Result<T> = std::result::Result<T, VfsError>;

/// Normalizes a path into the form used as lookup key: lowercase, `/` separated,
/// without leading `./` or `/`.
pub fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/").to_lowercase();
    let mut path = path.as_str();

    loop {
        if let Some(rest) = path.strip_prefix("./") {
            path = rest;
        } else if let Some(rest) = path.strip_prefix('/') {
            path = rest;
        } else {
            break;
        }
    }

    path.to_owned()
}

enum Layer {
    Dir {
        root: PathBuf,
        // normalized path -> path relative to root, as stored on disk
        index: HashMap<String, PathBuf>,
    },
    Zip {
        path: PathBuf,
        archive: Mutex<ZipArchive<Box<dyn ReadSeek>>>,
        // normalized path -> entry index
        index: HashMap<String, usize>,
    },
    /// Files added while running (downloads).
    Memory { files: HashMap<String, Vec<u8>> },
}

impl Layer {
    fn contains(&self, key: &str) -> bool {
        match self {
            Layer::Dir { index, .. } => index.contains_key(key),
            Layer::Zip { index, .. } => index.contains_key(key),
            Layer::Memory { files } => files.contains_key(key),
        }
    }

    /// A file's name as stored (its case), by key.
    fn original(&self, key: &str) -> Option<String> {
        let name = match self {
            Layer::Dir { index, .. } => index.get(key)?.to_string_lossy().into_owned(),
            Layer::Zip { archive, index, .. } => {
                let archive = archive.lock().unwrap_or_else(|e| e.into_inner());
                archive.name_for_index(*index.get(key)?)?.to_string()
            }
            Layer::Memory { files } => files.get_key_value(key)?.0.clone(),
        };
        Some(name.replace('\\', "/"))
    }

    fn keys(&self) -> Box<dyn Iterator<Item = &String> + '_> {
        match self {
            Layer::Dir { index, .. } => Box::new(index.keys()),
            Layer::Zip { index, .. } => Box::new(index.keys()),
            Layer::Memory { files } => Box::new(files.keys()),
        }
    }

    fn read(&self, key: &str) -> Option<Result<Vec<u8>>> {
        match self {
            Layer::Memory { files } => files.get(key).cloned().map(Ok),
            Layer::Dir { root, index } => {
                let path = root.join(index.get(key)?);
                Some(std::fs::read(&path).map_err(|source| VfsError::Io { path, source }))
            }
            Layer::Zip {
                path,
                archive,
                index,
            } => {
                let entry = *index.get(key)?;
                let mut archive = archive.lock().unwrap_or_else(|e| e.into_inner());

                let result = archive
                    .by_index(entry)
                    .map_err(|source| VfsError::Zip {
                        path: path.clone(),
                        source,
                    })
                    .and_then(|mut file| {
                        let mut buf = Vec::with_capacity(file.size() as usize);
                        file.read_to_end(&mut buf)
                            .map_err(|source| VfsError::Io {
                                path: path.join(key),
                                source,
                            })
                            .map(|_| buf)
                    });

                Some(result)
            }
        }
    }
}

/// The game's own files where they're usually kept: `assets/` or `soldat.smod` in the current
/// directory, else next to the program (a release keeps them there).
pub fn find_game_files() -> Option<PathBuf> {
    let here = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let dirs = std::iter::once(PathBuf::from(".")).chain(here);
    dirs.flat_map(|dir| ["assets", "soldat.smod"].map(|name| dir.join(name)))
        .find(|path| path.exists())
}

/// What a zip archive is read from: a file, or bytes in memory.
trait ReadSeek: Read + std::io::Seek + Send {}

impl<T: Read + std::io::Seek + Send> ReadSeek for T {}

/// A stack of mounted asset sources.
#[derive(Default)]
pub struct Vfs {
    layers: Vec<Layer>,
}

impl Vfs {
    pub fn new() -> Vfs {
        Vfs::default()
    }

    /// Mounts a directory or a `.smod`/`.zip` archive on top of the existing layers.
    pub fn mount<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        let meta = std::fs::metadata(path).map_err(|source| VfsError::Io {
            path: path.to_owned(),
            source,
        })?;

        let layer = if meta.is_dir() {
            mount_dir(path)?
        } else {
            mount_zip(path)?
        };

        tracing::info!(path = %path.display(), files = layer.keys().count(), "mounted assets");
        self.layers.push(layer);
        Ok(())
    }

    /// Mounts a `.smod`/`.zip` archive held in memory (fetched in the browser; shared bytes
    /// mount again without a copy), `name` for errors.
    pub fn mount_archive(
        &mut self,
        name: &str,
        bytes: impl AsRef<[u8]> + Send + 'static,
    ) -> Result<()> {
        let layer = zip_layer(Path::new(name), Box::new(std::io::Cursor::new(bytes)))?;
        tracing::info!(name, files = layer.keys().count(), "mounted assets");
        self.layers.push(layer);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Adds a file over everything mounted so far (a download, say).
    pub fn add(&mut self, path: &str, bytes: Vec<u8>) {
        if !matches!(self.layers.last(), Some(Layer::Memory { .. })) {
            self.layers.push(Layer::Memory {
                files: HashMap::new(),
            });
        }
        if let Some(Layer::Memory { files }) = self.layers.last_mut() {
            files.insert(normalize(path), bytes);
        }
    }

    pub fn exists(&self, path: &str) -> bool {
        let key = normalize(path);
        self.layers.iter().any(|layer| layer.contains(&key))
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let key = normalize(path);

        self.layers
            .iter()
            .rev()
            .find_map(|layer| layer.read(&key))
            .unwrap_or_else(|| Err(VfsError::NotFound(path.to_owned())))
    }

    pub fn read_to_string(&self, path: &str) -> Result<String> {
        let bytes = self.read(path)?;

        Ok(String::from_utf8(bytes).unwrap_or_else(|e| {
            // Soldat text files are often Windows-1252; keep going with a lossy decode.
            tracing::debug!(path, "asset is not valid UTF-8, decoding lossily");
            String::from_utf8_lossy(e.as_bytes()).into_owned()
        }))
    }

    /// Finds `path` with its extension replaced by the first of `extensions` that exists.
    /// Mirrors Soldat's image lookup, where e.g. `stopa.bmp` may be shipped as `stopa.png`.
    pub fn find_with_extensions(&self, path: &str, extensions: &[&str]) -> Option<String> {
        let key = normalize(path);
        let stem = match key.rfind('.') {
            Some(dot) if !key[dot..].contains('/') => &key[..dot],
            _ => key.as_str(),
        };

        extensions
            .iter()
            .map(|ext| format!("{stem}.{ext}"))
            .find(|candidate| self.exists(candidate))
    }

    /// Lists files directly inside `dir` (normalized names, sorted, no duplicates).
    pub fn list(&self, dir: &str) -> Vec<String> {
        let mut prefix = normalize(dir);
        if !prefix.is_empty() && !prefix.ends_with('/') {
            prefix.push('/');
        }

        let mut files: Vec<String> = self
            .layers
            .iter()
            .flat_map(|layer| layer.keys())
            .filter_map(|key| key.strip_prefix(&prefix))
            .filter(|rest| !rest.contains('/'))
            .map(str::to_owned)
            .collect();

        files.sort();
        files.dedup();
        files
    }

    /// The directories in a directory, with their names as stored.
    pub fn list_dirs(&self, dir: &str) -> Vec<String> {
        let mut prefix = normalize(dir);
        if !prefix.is_empty() && !prefix.ends_with('/') {
            prefix.push('/');
        }
        let mut dirs: Vec<String> = Vec::new();
        for layer in self.layers.iter().rev() {
            for key in layer.keys() {
                let Some((name, _)) = key.strip_prefix(&prefix).and_then(|r| r.split_once('/'))
                else {
                    continue;
                };
                if dirs.iter().any(|d| d.eq_ignore_ascii_case(name)) {
                    continue;
                }
                // the stored case, from the file's own name
                let stored = layer
                    .original(key)
                    .and_then(|path| {
                        let rest = &path[prefix.len().min(path.len())..];
                        rest.split('/').next().map(str::to_string)
                    })
                    .filter(|stored| stored.eq_ignore_ascii_case(name))
                    .unwrap_or_else(|| name.to_string());
                dirs.push(stored);
            }
        }
        dirs.sort_by_key(|d| d.to_ascii_lowercase());
        dirs
    }

    /// The files of a directory with their names as stored (the case they were made with; the
    /// top layer's when several have one).
    pub fn list_names(&self, dir: &str) -> Vec<String> {
        let mut names: Vec<String> = self
            .list(dir)
            .into_iter()
            .map(|file| {
                let key = if dir.is_empty() {
                    file.clone()
                } else {
                    format!("{}/{file}", normalize(dir).trim_end_matches('/'))
                };
                self.layers
                    .iter()
                    .rev()
                    .find_map(|layer| layer.original(&key))
                    .and_then(|name| name.rsplit('/').next().map(str::to_string))
                    .unwrap_or(file)
            })
            .collect();
        names.sort_by_key(|n| n.to_ascii_lowercase());
        names
    }
}

fn mount_dir(root: &Path) -> Result<Layer> {
    fn walk(root: &Path, rel: &Path, index: &mut HashMap<String, PathBuf>) -> Result<()> {
        let dir = root.join(rel);
        let entries = std::fs::read_dir(&dir).map_err(|source| VfsError::Io {
            path: dir.clone(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| VfsError::Io {
                path: dir.clone(),
                source,
            })?;
            let rel = rel.join(entry.file_name());
            let file_type = entry.file_type().map_err(|source| VfsError::Io {
                path: entry.path(),
                source,
            })?;

            if file_type.is_dir() {
                walk(root, &rel, index)?;
            } else if let Some(name) = rel.to_str() {
                index.insert(normalize(name), rel);
            } else {
                tracing::warn!(path = %rel.display(), "skipping asset with non UTF-8 name");
            }
        }

        Ok(())
    }

    let mut index = HashMap::new();
    walk(root, Path::new(""), &mut index)?;

    Ok(Layer::Dir {
        root: root.to_owned(),
        index,
    })
}

fn mount_zip(path: &Path) -> Result<Layer> {
    let file = File::open(path).map_err(|source| VfsError::Io {
        path: path.to_owned(),
        source,
    })?;

    zip_layer(path, Box::new(BufReader::new(file)))
}

fn zip_layer(path: &Path, reader: Box<dyn ReadSeek>) -> Result<Layer> {
    let archive = ZipArchive::new(reader).map_err(|source| VfsError::Zip {
        path: path.to_owned(),
        source,
    })?;

    let index = archive
        .file_names()
        .filter(|name| !name.ends_with('/'))
        .filter_map(|name| Some((normalize(name), archive.index_for_name(name)?)))
        .collect();

    Ok(Layer::Zip {
        path: path.to_owned(),
        archive: Mutex::new(archive),
        index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn write_zip(path: &Path, files: &[(&str, &str)]) {
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, content) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn added_files_cover_mounted_ones() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "maps/a.pms", "old");
        let mut vfs = Vfs::new();
        vfs.mount(dir.path()).unwrap();
        vfs.add("Maps/A.pms", b"new".to_vec());
        vfs.add("maps/b.pms", b"b".to_vec());
        assert_eq!(vfs.read("maps/a.pms").unwrap(), b"new");
        assert_eq!(vfs.list("maps"), ["a.pms", "b.pms"]);
    }

    #[test]
    fn normalizes_paths() {
        assert_eq!(normalize("./Maps\\ctf_Ash.PMS"), "maps/ctf_ash.pms");
        assert_eq!(normalize("/anims/stand.poa"), "anims/stand.poa");
    }

    #[test]
    fn directory_lookup_is_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Gostek-gfx/Stopa.png", "png");

        let mut vfs = Vfs::new();
        vfs.mount(dir.path()).unwrap();

        assert!(vfs.exists("gostek-gfx/stopa.png"));
        assert_eq!(vfs.read_to_string("GOSTEK-GFX\\STOPA.PNG").unwrap(), "png");
        assert!(matches!(
            vfs.read("missing.png"),
            Err(VfsError::NotFound(_))
        ));
    }

    #[test]
    fn later_mounts_override_earlier_ones() {
        let base = tempfile::tempdir().unwrap();
        write(base.path(), "mod.ini", "base");
        write(base.path(), "anims/stand.poa", "stand");

        let modfile = base.path().join("mod.smod");
        write_zip(&modfile, &[("mod.ini", "mod"), ("Maps/test.pms", "map")]);

        let mut vfs = Vfs::new();
        vfs.mount(base.path()).unwrap();
        vfs.mount(&modfile).unwrap();

        assert_eq!(vfs.read_to_string("mod.ini").unwrap(), "mod");
        assert_eq!(vfs.read_to_string("anims/stand.poa").unwrap(), "stand");
        assert_eq!(vfs.read_to_string("maps/TEST.pms").unwrap(), "map");
        assert_eq!(vfs.list("maps"), vec!["test.pms"]);
        assert_eq!(vfs.list_names("maps"), vec!["test.pms"]);
        assert_eq!(vfs.list_names("anims"), vec!["stand.poa"]);
    }

    #[test]
    fn directories_are_listed() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "custom-interfaces/Storm/setup.sif", "");
        write(dir.path(), "custom-interfaces/tech/setup.sif", "");
        write(dir.path(), "custom-interfaces/readme.txt", "");
        let mut vfs = Vfs::new();
        vfs.mount(dir.path()).unwrap();
        assert_eq!(vfs.list_dirs("custom-interfaces"), ["Storm", "tech"]);
    }

    #[test]
    fn names_keep_their_case() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Maps/ctf_Ash.pms", "map");
        let zip = dir.path().join("mod.smod");
        write_zip(&zip, &[("maps/inf_Abel.pms", "map")]);
        let mut vfs = Vfs::new();
        vfs.mount(dir.path()).unwrap();
        vfs.mount(&zip).unwrap();
        assert_eq!(vfs.list("maps"), ["ctf_ash.pms", "inf_abel.pms"]);
        assert_eq!(vfs.list_names("maps"), ["ctf_Ash.pms", "inf_Abel.pms"]);
    }

    #[test]
    fn archives_mount_from_memory() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("soldat.smod");
        write_zip(&file, &[("Maps/test.pms", "map"), ("mod.ini", "ini")]);
        let mut vfs = Vfs::new();
        vfs.mount_archive("soldat.smod", std::fs::read(&file).unwrap())
            .unwrap();
        assert_eq!(vfs.read_to_string("maps/test.pms").unwrap(), "map");
        assert_eq!(vfs.list("maps"), vec!["test.pms"]);
        assert!(
            vfs.mount_archive("bad.smod", b"not a zip".to_vec())
                .is_err()
        );
    }

    #[test]
    fn finds_alternative_image_extensions() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "scenery-gfx/tree.png", "");

        let mut vfs = Vfs::new();
        vfs.mount(dir.path()).unwrap();

        let exts = ["png", "jpg", "gif", "bmp"];
        assert_eq!(
            vfs.find_with_extensions("scenery-gfx/Tree.bmp", &exts),
            Some("scenery-gfx/tree.png".to_owned())
        );
        assert_eq!(
            vfs.find_with_extensions("scenery-gfx/rock.bmp", &exts),
            None
        );
    }
}
