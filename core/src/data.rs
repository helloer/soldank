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

/// Immutable game data shared by every world: animations, the soldier skeleton and the
/// weapon tables (with the mod's weapons.ini applied).
#[derive(Debug)]
pub struct GameData {
    pub anims: Arc<Animations>,
    pub soldier_skeleton: ParticleSystem,
    pub weapons: Arc<WeaponTable>,
    pub realistic_weapons: Arc<WeaponTable>,
    pub thing_skeletons: ThingSkeletons,
}

impl GameData {
    pub fn load(vfs: &Vfs) -> Result<GameData, DataError> {
        let anims = Arc::new(Animations::load(vfs)?);

        let skeleton_file = "objects/gostek.po";
        let soldier_skeleton = ParticleSystem::parse(
            skeleton_file,
            &vfs.read_to_string(skeleton_file)?,
            3.0, // SCALE in Anims.pas
            1.0,
            1.06 * GRAV,
            0.0,
            0.9945,
        )?;

        Ok(GameData {
            anims,
            soldier_skeleton,
            weapons: Arc::new(load_weapons(vfs, false)),
            realistic_weapons: Arc::new(load_weapons(vfs, true)),
            thing_skeletons: ThingSkeletons::load(vfs)?,
        })
    }

    /// The weapon table for the given mode (`sv_realisticmode`).
    pub fn weapons(&self, realistic: bool) -> &Arc<WeaponTable> {
        if realistic {
            &self.realistic_weapons
        } else {
            &self.weapons
        }
    }
}

/// `LoadWeapons`: the mod's weapons(_realistic).ini over the built-in stats; a missing or
/// broken file means the defaults, like in Soldat.
fn load_weapons(vfs: &Vfs, realistic: bool) -> WeaponTable {
    let file = if realistic {
        "configs/weapons_realistic.ini"
    } else {
        "configs/weapons.ini"
    };

    if !vfs.exists(file) {
        return WeaponTable::new(realistic, None).unwrap();
    }

    vfs.read_to_string(file)
        .map_err(|e| e.to_string())
        .and_then(|text| WeaponTable::new(realistic, Some(&text)))
        .unwrap_or_else(|error| {
            tracing::warn!(file, %error, "using default weapons");
            WeaponTable::new(realistic, None).unwrap()
        })
}
