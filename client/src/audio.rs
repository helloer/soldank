//! Sound (`Sound.pas`): Soldat's samples from `sfx/`, played with kira. A sound is quieter
//! the farther it is from the listener, silent beyond `SOUND_MAXDIST`, and panned by its
//! direction. Soldiers have their own channels for the sounds that replace each other
//! (reload, jets, spinning weapons); everything else plays on its own.

use kira::sound::PlaybackState;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::track::MainTrackBuilder;
use kira::{AudioManager, AudioManagerSettings, Decibels, Tween};

/// The sound device: cpal's, or the page's Web Audio in the browser.
#[cfg(not(target_arch = "wasm32"))]
type SoundBackend = kira::DefaultBackend;
#[cfg(target_arch = "wasm32")]
type SoundBackend = crate::web::sound::WebBackend;
use soldank_core::assets::Vfs;
use soldank_core::*;
use std::collections::HashMap;
use std::io::Cursor;

/// `SOUND_MAXDIST`, `SOUND_PANWIDTH`
const SOUND_MAXDIST: f32 = 750.0;
const SOUND_PANWIDTH: f32 = 1000.0;
/// `GRENADEEFFECT_DIST`
const GRENADE_EFFECT_DIST: f32 = 38.0;

/// Where a sound plays: a soldier's channel, the weather's (`CHANNEL_WEATHER`), or any.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum Slot {
    Soldier(SoldierId, Channel),
    Weather,
}

/// `ScaleVolumeSetting`: the volume percentage, scaled so low settings stay usable.
pub fn scale_volume(percent: i64) -> f32 {
    let v = percent.clamp(0, 100) as f64;
    ((1.0404f64.powf(v) - 1.0) / (1.0404 - 1.0) / 1275.0) as f32
}

fn decibels(amplitude: f32) -> Decibels {
    if amplitude <= 0.0 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * amplitude.log10())
    }
}

/// What the sounds are heard from.
#[derive(Debug, Copy, Clone)]
pub struct Listener {
    pub pos: Vec2,
    /// The local player, for the sounds only some hear.
    pub player: Option<SoldierId>,
    pub team: Team,
    /// The soldier the camera follows: bullets whizz past it.
    pub follow: Option<SoldierId>,
}

pub struct Audio {
    manager: Option<AudioManager<SoundBackend>>,
    samples: HashMap<Sfx, StaticSoundData>,
    channels: HashMap<Slot, StaticSoundHandle>,
    rng: PascalRandom,
    /// `VolumeInternal` (from `snd_volume`).
    pub volume: f32,
    /// `snd_effects_battle`: far away shots and explosions sound distant.
    pub battle_effects: bool,
    /// `snd_effects_explosions`: explosions close by deafen for a moment.
    pub explosion_effects: bool,
    /// `GrenadeEffectTimer`
    grenade_effect: i32,
    /// `Whizzed` of each bullet slot.
    whizzed: Vec<bool>,
}

impl Audio {
    /// Opens the audio device (the game stays silent without one) and loads the samples.
    pub fn new(vfs: &Vfs) -> Audio {
        // Soldat's MAX_SOURCES; when they're all playing, new sounds are dropped
        let settings = AudioManagerSettings {
            main_track_builder: MainTrackBuilder::new().sound_capacity(256),
            ..Default::default()
        };
        let manager = match AudioManager::<SoundBackend>::new(settings) {
            Ok(manager) => Some(manager),
            Err(error) => {
                tracing::warn!(%error, "no sound");
                None
            }
        };

        let seed =
            (crate::platform::now().fract() * 1e9) as u32 ^ crate::platform::unix_time() as u32;
        let mut audio = Audio {
            manager,
            samples: HashMap::new(),
            channels: HashMap::new(),
            rng: PascalRandom::new(seed),
            volume: scale_volume(50),
            battle_effects: false,
            explosion_effects: false,
            grenade_effect: 0,
            whizzed: vec![false; MAX_BULLETS],
        };
        audio.load_samples(vfs);
        audio
    }

    /// The sound effects (again, from a server's mod).
    pub fn load_samples(&mut self, vfs: &Vfs) {
        let mut samples = HashMap::new();
        if self.manager.is_some() {
            for &sfx in Sfx::ALL {
                let path = format!("sfx/{}", sfx.filename());
                let data = vfs
                    .read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        StaticSoundData::from_cursor(Cursor::new(bytes)).map_err(|e| e.to_string())
                    });
                match data {
                    Ok(data) => {
                        samples.insert(sfx, data);
                    }
                    Err(error) => tracing::debug!(path, error, "unable to load sound"),
                }
            }
            tracing::info!(loaded = samples.len(), "sound effects");
        }
        self.samples = samples;
    }

    /// A sound the simulation asked for.
    pub fn event(&mut self, soldier: Option<SoldierId>, sound: &SoundEvent, listener: &Listener) {
        match *sound {
            SoundEvent::Play(sound) => {
                let hears = match sound.audience {
                    Audience::All => true,
                    Audience::Player(id) => listener.player == Some(id),
                    Audience::Team(team) => listener.player.is_some() && listener.team == team,
                    Audience::Own => soldier.is_some() && soldier == listener.player,
                    Audience::Others => soldier != listener.player,
                };
                if !hears || (sound.one_in > 1 && self.rng.below(i32::from(sound.one_in)) != 0) {
                    return;
                }
                let sfx = if sound.variants > 1 {
                    sound
                        .sfx
                        .offset(self.rng.below(i32::from(sound.variants)) as u8)
                } else {
                    sound.sfx
                };
                let slot = soldier
                    .zip(sound.channel)
                    .map(|(id, channel)| Slot::Soldier(id, channel));
                self.play(sfx, sound.pos, slot, listener.pos);
            }
            SoundEvent::Stop(channel) => {
                if let Some(id) = soldier {
                    self.stop(Slot::Soldier(id, channel));
                }
            }
            SoundEvent::Pause(channel, paused) => {
                let Some(handle) =
                    soldier.and_then(|id| self.channels.get_mut(&Slot::Soldier(id, channel)))
                else {
                    return;
                };
                match (handle.state(), paused) {
                    (PlaybackState::Playing, true) => handle.pause(Tween::default()),
                    (PlaybackState::Paused, false) => handle.resume(Tween::default()),
                    _ => {}
                }
            }
        }
    }

    /// A sound somewhere (`PlaySound(sfx, pos)`).
    pub fn play_at(&mut self, sfx: Sfx, pos: Vec2, listener: &Listener) {
        self.play(sfx, Some(pos), None, listener.pos);
    }

    /// A sound at the listener (`PlaySound(sfx)`).
    pub fn play_here(&mut self, sfx: Sfx, listener: Vec2) {
        self.play(sfx, None, None, listener);
    }

    /// `FPlaySound`
    fn play(&mut self, sfx: Sfx, emitter: Option<Vec2>, slot: Option<Slot>, listener: Vec2) {
        if self.manager.is_none() || !self.samples.contains_key(&sfx) {
            return;
        }
        let emitter = emitter.unwrap_or(listener);
        let mut dist = (emitter - listener).length() / SOUND_MAXDIST;

        // far away battle noise
        if dist > 0.5 && self.battle_effects {
            use Sfx::*;
            match sfx {
                M79Explosion => self.play(DistM79, Some(emitter), slot, listener),
                GrenadeExplosion | Clustergrenade | ClusterExplosion => {
                    self.play(DistGrenade, Some(emitter), slot, listener)
                }
                Ak74Fire | M249Fire | Ruger77Fire | Spas12Fire | DeserteagleFire | SteyraugFire
                | Barretm82Fire | MinigunFire | Colt1911Fire => {
                    let far = DistGun1.offset(self.rng.below(4) as u8);
                    self.play(far, Some(emitter), slot, listener);
                }
                DistM79 | DistGrenade | DistGun1 | DistGun2 | DistGun3 | DistGun4 => {
                    dist = if dist > 1.0 {
                        dist - 1.0
                    } else {
                        1.0 - 2.0 * dist
                    };
                }
                _ => {}
            }
        }

        // deafened by an explosion
        if self.grenade_effect > 0 && sfx != Sfx::Hum {
            dist += (1.0 - dist) * (self.grenade_effect as f32 / 280.0).sqrt();
        }
        if dist > 1.0 {
            return;
        }

        tracing::trace!(?sfx, dist, ?slot, "play");
        let volume = decibels(self.volume * (1.0 - dist));
        let d = emitter - listener;
        let pan = d.x / (d.x * d.x + d.y * d.y + SOUND_PANWIDTH * SOUND_PANWIDTH).sqrt();

        if let Some(slot) = slot
            && let Some(handle) = self.channels.get_mut(&slot)
        {
            match handle.state() {
                // the channel's sound goes on, only moved
                PlaybackState::Playing | PlaybackState::Resuming => {
                    handle.set_volume(volume, Tween::default());
                    handle.set_panning(pan, Tween::default());
                    return;
                }
                PlaybackState::Paused | PlaybackState::Pausing => {
                    handle.stop(Tween::default());
                }
                _ => {}
            }
        }

        let mut data = self.samples[&sfx].volume(volume).panning(pan);
        if matches!(sfx, Sfx::Rocketz | Sfx::ChainsawR | Sfx::Flamer) {
            data = data.loop_region(..);
        }
        let Some(manager) = self.manager.as_mut() else {
            return;
        };
        match manager.play(data) {
            Ok(handle) => {
                if let Some(slot) = slot {
                    self.channels.insert(slot, handle);
                }
            }
            Err(error) => tracing::debug!(%error, "cannot play sound"),
        }
    }

    fn stop(&mut self, slot: Slot) {
        if let Some(mut handle) = self.channels.remove(&slot) {
            handle.stop(Tween::default());
        }
    }

    /// Silence all channels (a match ended, a map changes).
    pub fn stop_all(&mut self) {
        for (_, mut handle) in self.channels.drain() {
            handle.stop(Tween::default());
        }
    }

    /// Per tick: the explosion deafness wears off, bullets flying by whizz, the weather
    /// blows, and the channels of soldiers who left go quiet.
    pub fn tick(&mut self, world: &World, listener: &Listener) {
        if self.grenade_effect > -1 {
            self.grenade_effect -= 1;
        }

        // whizz above the head (`Whizzed`)
        let ear = listener
            .follow
            .and_then(|id| world.soldiers.get(id))
            .map(|s| s.particle.pos);
        for (slot, bullet) in world.bullets.iter().enumerate() {
            if !bullet.active {
                self.whizzed[slot] = false;
                continue;
            }
            let Some(ear) = ear else { continue };
            let p = bullet.particle.pos;
            if !self.whizzed[slot]
                && bullet.style != BulletStyle::Fist
                && p.x > ear.x - 200.0
                && p.x < ear.x + 200.0
                && p.y > ear.y - 350.0
                && p.y < ear.y + 100.0
            {
                let sfx = Sfx::Bulletby2.offset(self.rng.below(4) as u8);
                self.play(sfx, Some(p), None, listener.pos);
                self.whizzed[slot] = true;
            }
        }

        if matches!(world.map.weather, 1..=3) {
            self.play(Sfx::Wind, None, Some(Slot::Weather), listener.pos);
        }

        self.channels.retain(|slot, handle| match slot {
            Slot::Soldier(id, _) if !world.soldiers.contains_key(*id) => {
                handle.stop(Tween::default());
                false
            }
            _ => handle.state() != PlaybackState::Stopped,
        });
    }

    /// An explosion: close to the player it deafens for a while (`snd_effects_explosions`).
    pub fn explosion(&mut self, pos: Vec2, world: &World, listener: &Listener) {
        if !self.explosion_effects {
            return;
        }
        let Some(me) = listener.player.and_then(|id| world.soldiers.get(id)) else {
            return;
        };
        if me.health > -50.0 && (pos - me.particle.pos).length() < GRENADE_EFFECT_DIST {
            self.grenade_effect = 320;
            self.play_here(Sfx::Hum, listener.pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_setting_is_exponential() {
        assert_eq!(scale_volume(0), 0.0);
        assert!((scale_volume(100) - 1.0).abs() < 0.02);
        let half = scale_volume(50);
        assert!(half > 0.1 && half < 0.15, "{half}");
    }
}
