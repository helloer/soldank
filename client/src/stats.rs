//! The local player's weapon stats (`WepStats`, the F2 menu): shots, hits, kills and
//! headshots per weapon, counted by the client from the world's events.

use soldank_core::{GameEvent, SoldierId, WeaponKind};

/// `WepStats[0..20]`: the weapons by Soldat number (0 USSOCOM, 1-10 the primaries, 11 knife,
/// 12 chainsaw, 13 LAW, 14 flamer, 15 bow, 16 flame bow), then hands, grenade, clusters and
/// the stationary gun.
pub const SLOTS: usize = 21;

const HANDS: usize = 17;
const GRENADE: usize = 18;
const CLUSTERS: usize = 19;
const STATIONARY_GUN: usize = 20;

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct WeaponStat {
    pub shots: u32,
    pub hits: u32,
    pub kills: u32,
    pub headshots: u32,
    /// Soldat names a slot on its first shot and counts hits and kills by name, so a
    /// slot never shot from takes none. The name stays when the counts are reset.
    named: bool,
}

impl WeaponStat {
    /// The accuracy column: `Round(Hits * 100 / Shots)`.
    pub fn accuracy(&self) -> u32 {
        (f64::from(self.hits) * 100.0 / f64::from(self.shots)).round_ties_even() as u32
    }
}

#[derive(Debug, Default, Clone)]
pub struct WeaponStats {
    pub slots: [WeaponStat; SLOTS],
}

/// The slot whose name a bullet's weapon has (`WeaponNameByNum(OwnerWeapon)`, with the
/// grenade, cluster and stationary gun styles named apart). The cluster grenade itself
/// is called "Frag Grenade", like no slot, so its hits and kills count nowhere.
pub fn slot(weapon: WeaponKind) -> Option<usize> {
    use WeaponKind::*;
    Some(match weapon {
        USSOCOM => 0,
        DesertEagles => 1,
        MP5 => 2,
        Ak74 => 3,
        SteyrAUG => 4,
        Spas12 => 5,
        Ruger77 => 6,
        M79 => 7,
        Barrett => 8,
        Minimi => 9,
        Minigun => 10,
        Knife | ThrownKnife => 11,
        Chainsaw => 12,
        LAW => 13,
        Flamer => 14,
        Bow => 15,
        FlameBow => 16,
        NoWeapon => HANDS,
        FragGrenade => GRENADE,
        Cluster => CLUSTERS,
        M2 => STATIONARY_GUN,
        ClusterGrenade => return None,
    })
}

/// The slot a shot counts in (`CreateBullet`): by bullet style for grenades, clusters,
/// the stationary gun and punches, otherwise the weapon in hand (which fired it).
fn shot_slot(weapon: WeaponKind) -> usize {
    match weapon {
        WeaponKind::ClusterGrenade => CLUSTERS,
        weapon => slot(weapon).unwrap_or(HANDS),
    }
}

impl WeaponStats {
    /// Counts the player's shots, hits and kills of a tick.
    pub fn count(&mut self, player: Option<SoldierId>, events: &[GameEvent]) {
        let Some(player) = player else { return };
        for event in events {
            match *event {
                GameEvent::BulletFired { owner, weapon, .. } if owner == player => {
                    let stat = &mut self.slots[shot_slot(weapon)];
                    stat.shots += 1;
                    stat.named = true;
                }
                GameEvent::Hit {
                    attacker, weapon, ..
                } if attacker == player => {
                    if let Some(stat) = self.named(weapon) {
                        stat.hits += 1;
                    }
                }
                GameEvent::Killed {
                    victim,
                    killer,
                    weapon: Some(weapon),
                    headshot,
                    ..
                } if killer == player && victim != player => {
                    if let Some(stat) = self.named(weapon) {
                        stat.kills += 1;
                        stat.headshots += u32::from(headshot);
                    }
                }
                _ => {}
            }
        }
    }

    fn named(&mut self, weapon: WeaponKind) -> Option<&mut WeaponStat> {
        slot(weapon)
            .map(|i| &mut self.slots[i])
            .filter(|stat| stat.named)
    }

    /// `ResetWeaponStats` (a new map): the counts go, the names stay.
    pub fn reset(&mut self) {
        for stat in &mut self.slots {
            *stat = WeaponStat {
                named: stat.named,
                ..Default::default()
            };
        }
    }

    /// The slots shot from, in order.
    pub fn shown(&self) -> impl Iterator<Item = (usize, &WeaponStat)> {
        self.slots.iter().enumerate().filter(|(_, s)| s.shots > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::SlotMap;

    #[test]
    fn counts_the_players_shots_hits_and_kills() {
        let mut ids = SlotMap::<SoldierId, ()>::with_key();
        let (me, other) = (ids.insert(()), ids.insert(()));
        let fired = |owner, weapon| GameEvent::BulletFired {
            owner,
            weapon,
            pos: Default::default(),
        };
        let hit = |attacker, weapon| GameEvent::Hit {
            attacker,
            victim: other,
            weapon,
        };
        let killed = |killer, weapon, headshot| GameEvent::Killed {
            victim: other,
            killer,
            how: soldank_core::DeathKind::Normal,
            weapon: Some(weapon),
            headshot,
            hit: if headshot { 12 } else { 6 },
            shot: Default::default(),
        };

        let mut stats = WeaponStats::default();
        // a hit before any shot: the slot has no name yet
        stats.count(Some(me), &[hit(me, WeaponKind::Ak74)]);
        assert_eq!(stats.slots[3].hits, 0);

        stats.count(
            Some(me),
            &[
                fired(me, WeaponKind::Ak74),
                fired(me, WeaponKind::Ak74),
                fired(me, WeaponKind::Ak74),
                fired(other, WeaponKind::Ak74),
                hit(me, WeaponKind::Ak74),
                hit(other, WeaponKind::Ak74),
                killed(me, WeaponKind::Ak74, true),
                fired(me, WeaponKind::ClusterGrenade),
                fired(me, WeaponKind::Cluster),
                hit(me, WeaponKind::ClusterGrenade),
                hit(me, WeaponKind::Cluster),
            ],
        );
        let ak = stats.slots[3];
        assert_eq!((ak.shots, ak.hits, ak.kills, ak.headshots), (3, 1, 1, 1));
        assert_eq!(ak.accuracy(), 33);
        let clusters = stats.slots[CLUSTERS];
        assert_eq!((clusters.shots, clusters.hits), (2, 1));
        assert_eq!(
            stats.shown().map(|(i, _)| i).collect::<Vec<_>>(),
            [3, CLUSTERS]
        );

        // a new map: the names stay, so hits count before the first shot
        stats.reset();
        assert_eq!(stats.shown().count(), 0);
        stats.count(Some(me), &[hit(me, WeaponKind::Ak74)]);
        assert_eq!(stats.slots[3].hits, 1);
    }

    #[test]
    fn accuracy_rounds_half_to_even() {
        let stat = |hits, shots| WeaponStat {
            hits,
            shots,
            ..Default::default()
        };
        assert_eq!(stat(1, 8).accuracy(), 12); // 12.5
        assert_eq!(stat(3, 8).accuracy(), 38); // 37.5
        assert_eq!(stat(2, 3).accuracy(), 67);
    }
}
