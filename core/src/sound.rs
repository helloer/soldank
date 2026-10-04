//! Sound effects the simulation asks for: Soldat plays them from the client parts of its
//! mechanics (`PlaySound`, `StopSound`, `SetSoundPaused`). The simulation never draws
//! random numbers for sounds, so a sound picked at random (`SFX_STEP + Random(4)`) comes
//! with the number of samples to pick from, and the client rolls the dice.

use super::*;

macro_rules! sfx {
    ($($(#[$meta:meta])* $name:ident = $num:literal, $file:literal,)*) => {
        /// Sound samples, numbered like Soldat's (`SFX_*`, 1-based), with their files in
        /// `sfx/`.
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub enum Sfx {
            $($(#[$meta])* $name = $num,)*
        }

        impl Sfx {
            pub const ALL: &'static [Sfx] = &[$(Sfx::$name,)*];

            pub fn filename(self) -> &'static str {
                match self {
                    $(Sfx::$name => $file,)*
                }
            }

            pub fn from_num(num: u8) -> Option<Sfx> {
                match num {
                    $($num => Some(Sfx::$name),)*
                    _ => None,
                }
            }
        }
    };
}

sfx! {
    Ak74Fire = 1, "ak74-fire.wav",
    Rocketz = 2, "rocketz.wav",
    Ak74Reload = 3, "ak74-reload.wav",
    // 4: unused (empty.wav)
    M249Fire = 5, "m249-fire.wav",
    Ruger77Fire = 6, "ruger77-fire.wav",
    Ruger77Reload = 7, "ruger77-reload.wav",
    M249Reload = 8, "m249-reload.wav",
    Mp5Fire = 9, "mp5-fire.wav",
    Mp5Reload = 10, "mp5-reload.wav",
    Spas12Fire = 11, "spas12-fire.wav",
    Spas12Reload = 12, "spas12-reload.wav",
    Standup = 13, "standup.wav",
    Fall = 14, "fall.wav",
    Spawn = 15, "spawn.wav",
    M79Fire = 16, "m79-fire.wav",
    M79Explosion = 17, "m79-explosion.wav",
    M79Reload = 18, "m79-reload.wav",
    GrenadeThrow = 19, "grenade-throw.wav",
    GrenadeExplosion = 20, "grenade-explosion.wav",
    GrenadeBounce = 21, "grenade-bounce.wav",
    Bryzg = 22, "bryzg.wav",
    Infiltmus = 23, "infiltmus.wav",
    Headchop = 24, "headchop.wav",
    ExplosionErg = 25, "explosion-erg.wav",
    WaterStep = 26, "water-step.wav",
    Bulletby = 27, "bulletby.wav",
    Bodyfall = 28, "bodyfall.wav",
    DeserteagleFire = 29, "deserteagle-fire.wav",
    DeserteagleReload = 30, "deserteagle-reload.wav",
    SteyraugFire = 31, "steyraug-fire.wav",
    SteyraugReload = 32, "steyraug-reload.wav",
    Barretm82Fire = 33, "barretm82-fire.wav",
    Barretm82Reload = 34, "barretm82-reload.wav",
    MinigunFire = 35, "minigun-fire.wav",
    MinigunReload = 36, "minigun-reload.wav",
    MinigunStart = 37, "minigun-start.wav",
    MinigunEnd = 38, "minigun-end.wav",
    Pickupgun = 39, "pickupgun.wav",
    Capture = 40, "capture.wav",
    Colt1911Fire = 41, "colt1911-fire.wav",
    Colt1911Reload = 42, "colt1911-reload.wav",
    Changeweapon = 43, "changeweapon.wav",
    Shell = 44, "shell.wav",
    Shell2 = 45, "shell2.wav",
    DeadHit = 46, "dead-hit.wav",
    Throwgun = 47, "throwgun.wav",
    BowFire = 48, "bow-fire.wav",
    Takebow = 49, "takebow.wav",
    Takemedikit = 50, "takemedikit.wav",
    Wermusic = 51, "wermusic.wav",
    Ts = 52, "ts.wav",
    Ctf = 53, "ctf.wav",
    Berserker = 54, "berserker.wav",
    Godflame = 55, "godflame.wav",
    Flamer = 56, "flamer.wav",
    Predator = 57, "predator.wav",
    Killberserk = 58, "killberserk.wav",
    Vesthit = 59, "vesthit.wav",
    Burn = 60, "burn.wav",
    Vesttake = 61, "vesttake.wav",
    Clustergrenade = 62, "clustergrenade.wav",
    ClusterExplosion = 63, "cluster-explosion.wav",
    GrenadePullout = 64, "grenade-pullout.wav",
    Spit = 65, "spit.wav",
    Stuff = 66, "stuff.wav",
    Smoke = 67, "smoke.wav",
    Match = 68, "match.wav",
    Roar = 69, "roar.wav",
    Step = 70, "step.wav",
    Step2 = 71, "step2.wav",
    Step3 = 72, "step3.wav",
    Step4 = 73, "step4.wav",
    Hum = 74, "hum.wav",
    Ric = 75, "ric.wav",
    Ric2 = 76, "ric2.wav",
    Ric3 = 77, "ric3.wav",
    Ric4 = 78, "ric4.wav",
    DistM79 = 79, "dist-m79.wav",
    DistGrenade = 80, "dist-grenade.wav",
    DistGun1 = 81, "dist-gun1.wav",
    DistGun2 = 82, "dist-gun2.wav",
    DistGun3 = 83, "dist-gun3.wav",
    DistGun4 = 84, "dist-gun4.wav",
    Death = 85, "death.wav",
    Death2 = 86, "death2.wav",
    Death3 = 87, "death3.wav",
    CrouchMove = 88, "crouch-move.wav",
    HitArg = 89, "hit-arg.wav",
    HitArg2 = 90, "hit-arg2.wav",
    HitArg3 = 91, "hit-arg3.wav",
    Goprone = 92, "goprone.wav",
    Roll = 93, "roll.wav",
    FallHard = 94, "fall-hard.wav",
    Onfire = 95, "onfire.wav",
    Firecrack = 96, "firecrack.wav",
    Scope = 97, "scope.wav",
    Scopeback = 98, "scopeback.wav",
    Playerdeath = 99, "playerdeath.wav",
    Changespin = 100, "changespin.wav",
    Arg = 101, "arg.wav",
    Lava = 102, "lava.wav",
    Regenerate = 103, "regenerate.wav",
    ProneMove = 104, "prone-move.wav",
    Jump = 105, "jump.wav",
    Crouch = 106, "crouch.wav",
    CrouchMovel = 107, "crouch-movel.wav",
    Step5 = 108, "step5.wav",
    Step6 = 109, "step6.wav",
    Step7 = 110, "step7.wav",
    Step8 = 111, "step8.wav",
    Stop = 112, "stop.wav",
    Bulletby2 = 113, "bulletby2.wav",
    Bulletby3 = 114, "bulletby3.wav",
    Bulletby4 = 115, "bulletby4.wav",
    Bulletby5 = 116, "bulletby5.wav",
    Weaponhit = 117, "weaponhit.wav",
    Clipfall = 118, "clipfall.wav",
    Bonecrack = 119, "bonecrack.wav",
    Gaugeshell = 120, "gaugeshell.wav",
    Colliderhit = 121, "colliderhit.wav",
    KitFall = 122, "kit-fall.wav",
    KitFall2 = 123, "kit-fall2.wav",
    Flag = 124, "flag.wav",
    Flag2 = 125, "flag2.wav",
    Takegun = 126, "takegun.wav",
    InfiltPoint = 127, "infilt-point.wav",
    Menuclick = 128, "menuclick.wav",
    Knife = 129, "knife.wav",
    Slash = 130, "slash.wav",
    ChainsawD = 131, "chainsaw-d.wav",
    ChainsawM = 132, "chainsaw-m.wav",
    ChainsawR = 133, "chainsaw-r.wav",
    Piss = 134, "piss.wav",
    Law = 135, "law.wav",
    ChainsawO = 136, "chainsaw-o.wav",
    M2fire = 137, "m2fire.wav",
    M2explode = 138, "m2explode.wav",
    M2overheat = 139, "m2overheat.wav",
    Signal = 140, "signal.wav",
    M2use = 141, "m2use.wav",
    Scoperun = 142, "scoperun.wav",
    Mercy = 143, "mercy.wav",
    Ric5 = 144, "ric5.wav",
    Ric6 = 145, "ric6.wav",
    Ric7 = 146, "ric7.wav",
    LawStart = 147, "law-start.wav",
    LawEnd = 148, "law-end.wav",
    Boomheadshot = 149, "boomheadshot.wav",
    Snapshot = 150, "snapshot.wav",
    RadioEfcup = 151, "radio/efcup.wav",
    RadioEfcmid = 152, "radio/efcmid.wav",
    RadioEfcdown = 153, "radio/efcdown.wav",
    RadioFfcup = 154, "radio/ffcup.wav",
    RadioFfcmid = 155, "radio/ffcmid.wav",
    RadioFfcdown = 156, "radio/ffcdown.wav",
    RadioEsup = 157, "radio/esup.wav",
    RadioEsmid = 158, "radio/esmid.wav",
    RadioEsdown = 159, "radio/esdown.wav",
    Bounce = 160, "bounce.wav",
    Rain = 161, "sfx_rain.wav",
    Snow = 162, "sfx_snow.wav",
    Wind = 163, "sfx_wind.wav",
}

impl Sfx {
    pub fn num(self) -> u8 {
        self as u8
    }

    /// The sample `n` places after this one (`SFX_RIC + 2`).
    pub fn offset(self, n: u8) -> Sfx {
        Sfx::from_num(self.num() + n).unwrap_or(self)
    }
}

/// A soldier's own sound channels: a sound played on one replaces what was playing, and
/// playing the same again only moves it (`ReloadSoundChannel`, `JetsSoundChannel`,
/// `GattlingSoundChannel`, `GattlingSoundChannel2`).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Channel {
    Reload,
    Jets,
    Gattling,
    Gattling2,
}

/// Who hears a sound: some are only for the local player.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum Audience {
    #[default]
    All,
    /// Only this player (`if Who = MySprite`).
    Player(SoldierId),
    /// Only players of this team (`IsInSameTeam(Sprite[MySprite])`).
    Team(Team),
    /// Only the player of the soldier the sound comes from (`if Num = MySprite`).
    Own,
    /// Everyone but the player of the soldier the sound comes from.
    Others,
}

/// A sound to play (`PlaySound`).
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Sound {
    pub sfx: Sfx,
    /// Consecutive samples to pick one from at random (`sfx + Random(variants)`).
    pub variants: u8,
    /// Played only one time in this many (`if Random(n) = 0 then PlaySound`).
    pub one_in: u8,
    /// Where it sounds; `None` at the listener.
    pub pos: Option<Vec2>,
    /// The soldier's channel, or any free one.
    pub channel: Option<Channel>,
    pub audience: Audience,
}

impl Sound {
    /// `PlaySound(sfx)`: at the listener.
    pub fn new(sfx: Sfx) -> Sound {
        Sound {
            sfx,
            variants: 1,
            one_in: 1,
            pos: None,
            channel: None,
            audience: Audience::All,
        }
    }

    pub fn at(self, pos: Vec2) -> Sound {
        Sound {
            pos: Some(pos),
            ..self
        }
    }

    pub fn variants(self, variants: u8) -> Sound {
        Sound { variants, ..self }
    }

    pub fn one_in(self, one_in: u8) -> Sound {
        Sound { one_in, ..self }
    }

    pub fn channel(self, channel: Channel) -> Sound {
        Sound {
            channel: Some(channel),
            ..self
        }
    }

    pub fn audience(self, audience: Audience) -> Sound {
        Sound { audience, ..self }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SoundEvent {
    Play(Sound),
    Stop(Channel),
    Pause(Channel, bool),
}

impl From<Sound> for SoundEvent {
    fn from(sound: Sound) -> SoundEvent {
        SoundEvent::Play(sound)
    }
}

impl SoundEvent {
    /// `PlaySound(sfx, pos)`
    pub fn at(sfx: Sfx, pos: Vec2) -> SoundEvent {
        Sound::new(sfx).at(pos).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_keep_soldat_numbers() {
        assert_eq!(Sfx::ALL.len(), 162, "163 samples, one unused");
        assert_eq!(Sfx::Ak74Fire.num(), 1);
        assert_eq!(Sfx::Step.offset(3), Sfx::Step4);
        assert_eq!(Sfx::Wind.num(), 163);
        assert_eq!(Sfx::RadioEfcup.filename(), "radio/efcup.wav");
        for (a, b) in Sfx::ALL.iter().zip(&Sfx::ALL[1..]) {
            assert!(a.num() < b.num());
        }
    }
}
