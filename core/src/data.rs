use super::*;
use crate::assets::{Vfs, VfsError};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DataError {
    #[error(transparent)]
    Asset(#[from] VfsError),
    #[error("{file}: {message}")]
    Parse { file: String, message: String },
}

impl DataError {
    pub(crate) fn parse(file: &str, message: impl Into<String>) -> DataError {
        DataError::Parse {
            file: file.to_owned(),
            message: message.into(),
        }
    }
}

/// Immutable game data shared by every world: animations and the soldier skeleton.
#[derive(Debug)]
pub struct GameData {
    pub anims: Arc<Animations>,
    pub soldier_skeleton: ParticleSystem,
}

impl GameData {
    pub fn load(vfs: &Vfs) -> Result<GameData, DataError> {
        let anims = Arc::new(Animations::load(vfs)?);

        let skeleton_file = "objects/gostek.po";
        let soldier_skeleton = ParticleSystem::parse(
            skeleton_file,
            &vfs.read_to_string(skeleton_file)?,
            4.5,
            1.0,
            1.06 * GRAV,
            0.0,
            0.9945,
        )?;

        Ok(GameData {
            anims,
            soldier_skeleton,
        })
    }
}
