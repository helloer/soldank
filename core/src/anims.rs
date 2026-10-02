use super::*;
use crate::assets::Vfs;
use std::sync::Arc;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Anim {
    Stand,
    Run,
    RunBack,
    Jump,
    JumpSide,
    Fall,
    Crouch,
    CrouchRun,
    Reload,
    Throw,
    Recoil,
    SmallRecoil,
    Shotgun,
    ClipOut,
    ClipIn,
    SlideBack,
    Change,
    ThrowWeapon,
    WeaponNone,
    Punch,
    ReloadBow,
    Barret,
    Roll,
    RollBack,
    CrouchRunBack,
    Cigar,
    Match,
    Smoke,
    Wipe,
    Groin,
    Piss,
    Mercy,
    Mercy2,
    TakeOff,
    Prone,
    Victory,
    Aim,
    HandsUpAim,
    ProneMove,
    GetUp,
    AimRecoil,
    HandsUpRecoil,
    Melee,
    Own,
}

#[derive(Debug)]
pub struct AnimFrame {
    pub positions: Vec<Vec2>,
}

#[derive(Debug)]
pub struct AnimData {
    pub id: Anim,
    pub looped: bool,
    pub speed: i32,
    pub frames: Vec<AnimFrame>,
}

#[derive(Debug, Clone)]
pub struct AnimState {
    pub id: Anim,
    data: Arc<AnimData>,
    pub looped: bool,
    pub speed: i32,
    pub count: i32,
    pub frame: usize,
}

/// All loaded animations, indexed by [`Anim`].
#[derive(Debug)]
pub struct Animations(Vec<Arc<AnimData>>);

impl Animations {
    pub fn get(&self, id: Anim) -> &Arc<AnimData> {
        &self.0[id as usize]
    }

    /// A fresh playback state for `id`, starting at frame 1.
    pub fn state(&self, id: Anim) -> AnimState {
        let data = self.get(id).clone();

        AnimState {
            id,
            looped: data.looped,
            speed: data.speed,
            data,
            count: 0,
            frame: 1,
        }
    }

    pub fn load(vfs: &Vfs) -> Result<Animations, DataError> {
        let mut animations = ANIMATION_FILES
            .iter()
            .map(|&(id, file_name, speed, looped)| {
                let path = format!("anims/{file_name}");
                let text = vfs.read_to_string(&path)?;
                AnimData::parse(id, &path, &text, speed, looped).map(Arc::new)
            })
            .collect::<Result<Vec<_>, DataError>>()?;

        animations.sort_by_key(|a| a.id as usize);
        Ok(Animations(animations))
    }
}

impl AnimData {
    pub fn num_frames(&self) -> usize {
        self.frames.len()
    }
}

impl AnimState {
    pub fn do_animation(&mut self) {
        self.count += 1;

        if self.count == self.speed {
            self.count = 0;
            self.frame += 1;

            if self.frame > self.num_frames() {
                if self.looped {
                    self.frame = 1;
                } else {
                    self.frame = self.num_frames();
                }
            }
        }
    }

    pub fn pos(&self, index: usize) -> Vec2 {
        // frame 0 only happens on corpses (Die resets it), where animations aren't applied
        self.data.frames[self.frame.max(1) - 1].positions[index - 1]
    }

    pub fn num_frames(&self) -> usize {
        self.data.num_frames()
    }

    pub fn is_any(&self, animations: &[Anim]) -> bool {
        animations.contains(&self.id)
    }
}

impl AnimData {
    /// Parses a `.poa` animation: blocks of `point index` + `x y z` lines, frames
    /// separated by `NEXTFRAME`, terminated by `ENDFILE`.
    pub fn parse(
        id: Anim,
        file: &str,
        text: &str,
        speed: i32,
        looped: bool,
    ) -> Result<AnimData, DataError> {
        let mut lines = text.lines().map(str::trim);
        let mut frames: Vec<AnimFrame> = Vec::new();
        let mut positions: Vec<Vec2> = Vec::new();

        let add_frame = |frames: &mut Vec<AnimFrame>, positions: &[Vec2]| {
            let n = frames
                .last()
                .map_or(positions.len(), |frame| frame.positions.len());

            if positions.len() != n {
                return Err(DataError::parse(
                    file,
                    "wrong number of points in animation frame",
                ));
            }

            frames.push(AnimFrame {
                positions: positions.to_vec(),
            });

            Ok(())
        };

        // StrToFloat returns Extended, so the scaling happens in extended precision
        let number = |line: Option<&str>| -> Result<f64, DataError> {
            line.and_then(|l| l.parse().ok())
                .ok_or_else(|| DataError::parse(file, format!("expected a number, got {line:?}")))
        };

        loop {
            match lines.next() {
                None | Some("ENDFILE") => break,
                Some("NEXTFRAME") => {
                    add_frame(&mut frames, &positions)?;
                    positions.clear();
                }
                Some(line) => {
                    let point: usize = line.parse().map_err(|_| {
                        DataError::parse(file, format!("expected point index, got {line:?}"))
                    })?;

                    if point != positions.len() + 1 {
                        return Err(DataError::parse(
                            file,
                            format!("unexpected point index {point}"),
                        ));
                    }

                    let x = number(lines.next())?;
                    let _y = number(lines.next())?;
                    let z = number(lines.next())?;

                    positions.push(vec2(fpc(-3.0 * x / 1.1), fpc(-3.0 * z)));
                }
            }
        }

        add_frame(&mut frames, &positions)?;

        Ok(AnimData {
            id,
            looped,
            speed,
            frames,
        })
    }
}

const ANIMATION_FILES: &[(Anim, &str, i32, bool)] = &[
    (Anim::Stand, "stoi.poa", 3, true),
    (Anim::Run, "biega.poa", 1, true),
    (Anim::RunBack, "biegatyl.poa", 1, true),
    (Anim::Jump, "skok.poa", 1, false),
    (Anim::JumpSide, "skokwbok.poa", 1, false),
    (Anim::Fall, "spada.poa", 1, false),
    (Anim::Crouch, "kuca.poa", 1, false),
    (Anim::CrouchRun, "kucaidzie.poa", 2, true),
    (Anim::Reload, "laduje.poa", 2, false),
    (Anim::Throw, "rzuca.poa", 1, false),
    (Anim::Recoil, "odrzut.poa", 1, false),
    (Anim::SmallRecoil, "odrzut2.poa", 1, false),
    (Anim::Shotgun, "shotgun.poa", 1, false),
    (Anim::ClipOut, "clipout.poa", 3, false),
    (Anim::ClipIn, "clipin.poa", 3, false),
    (Anim::SlideBack, "slideback.poa", 2, true),
    (Anim::Change, "change.poa", 1, false),
    (Anim::ThrowWeapon, "wyrzuca.poa", 1, false),
    (Anim::WeaponNone, "bezbroni.poa", 3, false),
    (Anim::Punch, "bije.poa", 1, false),
    (Anim::ReloadBow, "strzala.poa", 1, false),
    (Anim::Barret, "barret.poa", 9, false),
    (Anim::Roll, "skokdolobrot.poa", 1, false),
    (Anim::RollBack, "skokdolobrottyl.poa", 1, false),
    (Anim::CrouchRunBack, "kucaidzietyl.poa", 2, true),
    (Anim::Cigar, "cigar.poa", 3, false),
    (Anim::Match, "match.poa", 3, false),
    (Anim::Smoke, "smoke.poa", 4, false),
    (Anim::Wipe, "wipe.poa", 4, false),
    (Anim::Groin, "krocze.poa", 2, false),
    (Anim::Piss, "szcza.poa", 8, false),
    (Anim::Mercy, "samo.poa", 3, false),
    (Anim::Mercy2, "samo2.poa", 3, false),
    (Anim::TakeOff, "takeoff.poa", 2, false),
    (Anim::Prone, "lezy.poa", 1, false),
    (Anim::Victory, "cieszy.poa", 3, false),
    (Anim::Aim, "celuje.poa", 2, false),
    (Anim::HandsUpAim, "gora.poa", 2, false),
    (Anim::ProneMove, "lezyidzie.poa", 2, true),
    (Anim::GetUp, "wstaje.poa", 1, false),
    (Anim::AimRecoil, "celujeodrzut.poa", 1, false),
    (Anim::HandsUpRecoil, "goraodrzut.poa", 1, false),
    (Anim::Melee, "kolba.poa", 1, false),
    (Anim::Own, "rucha.poa", 3, false),
];
