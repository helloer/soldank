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
    /// The weapons.ini and weapons_realistic.ini in use (`None`: the defaults), for clients
    /// to play with the server's.
    pub weapons_mods: [Option<String>; 2],
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

        let (weapons, weapons_mod) = load_weapons(vfs, false);
        let (realistic_weapons, realistic_mod) = load_weapons(vfs, true);
        Ok(GameData {
            anims,
            soldier_skeleton,
            weapons: Arc::new(weapons),
            realistic_weapons: Arc::new(realistic_weapons),
            thing_skeletons: ThingSkeletons::load(vfs)?,
            weapons_mods: [weapons_mod, realistic_mod],
        })
    }

    /// The same data with another weapons mod (a server's: `ServerVars`).
    pub fn with_weapons_mods(&self, mods: &[Option<String>; 2]) -> GameData {
        let table = |realistic: bool| {
            let text = mods[usize::from(realistic)].as_deref();
            WeaponTable::new(realistic, text).unwrap_or_else(|error| {
                tracing::warn!(%error, "using default weapons");
                WeaponTable::new(realistic, None).unwrap()
            })
        };
        GameData {
            anims: self.anims.clone(),
            soldier_skeleton: self.soldier_skeleton.clone(),
            weapons: Arc::new(table(false)),
            realistic_weapons: Arc::new(table(true)),
            thing_skeletons: self.thing_skeletons.clone(),
            weapons_mods: mods.clone(),
        }
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
fn load_weapons(vfs: &Vfs, realistic: bool) -> (WeaponTable, Option<String>) {
    let file = if realistic {
        "configs/weapons_realistic.ini"
    } else {
        "configs/weapons.ini"
    };

    if !vfs.exists(file) {
        return (WeaponTable::new(realistic, None).unwrap(), None);
    }

    let loaded = vfs
        .read_to_string(file)
        .map_err(|e| e.to_string())
        .and_then(|text| {
            let table = WeaponTable::new(realistic, Some(&text))?;
            Ok((table, Some(text)))
        });
    loaded.unwrap_or_else(|error| {
        tracing::warn!(file, %error, "using default weapons");
        (WeaponTable::new(realistic, None).unwrap(), None)
    })
}
