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
        archive: Mutex<ZipArchive<BufReader<File>>>,
        // normalized path -> entry index
        index: HashMap<String, usize>,
    },
}

impl Layer {
    fn contains(&self, key: &str) -> bool {
        match self {
            Layer::Dir { index, .. } => index.contains_key(key),
            Layer::Zip { index, .. } => index.contains_key(key),
        }
    }

    fn keys(&self) -> Box<dyn Iterator<Item = &String> + '_> {
        match self {
            Layer::Dir { index, .. } => Box::new(index.keys()),
            Layer::Zip { index, .. } => Box::new(index.keys()),
        }
    }

    fn read(&self, key: &str) -> Option<Result<Vec<u8>>> {
        match self {
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

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
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

    let archive = ZipArchive::new(BufReader::new(file)).map_err(|source| VfsError::Zip {
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
