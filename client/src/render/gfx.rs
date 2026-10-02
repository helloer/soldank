use super::*;
pub use soldank_core::sprites::{Group, Interface, Object, Soldier, Spark, SpriteData, Weapon};

macro_rules! soldier_parts_sprite {
    ( None ) => {
        SoldierSprite::None
    };
    ( $group:ident::$id:ident ) => {
        SoldierSprite::$group($group::$id)
    };
}

macro_rules! soldier_parts {
    (
        $(
            $id:ident =
                Sprite($($sprite:tt)+),
                Point($p1:expr, $p2:expr),
                Center($cx:expr, $cy:expr),
                Show($show:expr),
                Flip($flip:expr),
                Team($team:expr),
                Flex($flex:expr),
                Color($color:ident),
                Alpha($alpha:ident)
        )+
    ) => {
        #[derive(Debug, Copy, Clone)]
        pub enum SoldierPart {
            $($id,)+
        }

        impl SoldierPart {
            pub fn id(&self) -> usize { *self as usize }

            pub fn values() -> &'static [SoldierPart] {
                static VALUES: &[SoldierPart] = &[$(SoldierPart::$id,)+];
                VALUES
            }

            pub fn data() -> &'static [SoldierPartInfo] {
                static DATA: &[SoldierPartInfo] = &[
                    $(
                        SoldierPartInfo {
                            name: stringify!($id),
                            sprite: soldier_parts_sprite!($($sprite)+),
                            point: ($p1, $p2),
                            center: ($cx, $cy),
                            flexibility: $flex,
                            flip: $flip,
                            team: $team,
                            color: SoldierColor::$color,
                            alpha: SoldierAlpha::$alpha,
                            visible: $show,
                        },
                    )+
                ];

                DATA
            }
        }

        impl ::std::convert::From<usize> for SoldierPart {
            fn from(id: usize) -> SoldierPart {
                match SoldierPart::values().get(id as usize) {
                    Some(&v) => v,
                    _ => panic!("Invalid sprite identifier."),
                }
            }
        }

        impl ::std::ops::Add<usize> for SoldierPart {
            type Output = SoldierPart;
            fn add(self, x: usize) -> SoldierPart { SoldierPart::from(self.id() + x) }
        }

        impl ::std::ops::Sub<usize> for SoldierPart {
            type Output = SoldierPart;
            fn sub(self, x: usize) -> SoldierPart { SoldierPart::from(self.id() - x) }
        }
    }
}

#[rustfmt::skip]
soldier_parts! {
    SecondaryDeagles       = Sprite(None),                    Point( 5, 10), Center( 0.300,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryMp5           = Sprite(Weapon::Mp5),             Point( 5, 10), Center( 0.300,  0.300), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryAk74          = Sprite(Weapon::Ak74),            Point( 5, 10), Center( 0.300,  0.250), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondarySteyr         = Sprite(Weapon::Steyr),           Point( 5, 10), Center( 0.300,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondarySpas          = Sprite(Weapon::Spas),            Point( 5, 10), Center( 0.300,  0.300), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryRuger         = Sprite(Weapon::Ruger),           Point( 5, 10), Center( 0.300,  0.300), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryM79           = Sprite(Weapon::M79),             Point( 5, 10), Center( 0.300,  0.350), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryBarrett       = Sprite(Weapon::Barrett),         Point( 5, 10), Center( 0.300,  0.350), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryMinimi        = Sprite(Weapon::Minimi),          Point( 5, 10), Center( 0.300,  0.350), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryMinigun       = Sprite(Weapon::Minigun),         Point( 5, 10), Center( 0.200,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondarySocom         = Sprite(None),                    Point( 5, 10), Center( 0.300,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryKnife         = Sprite(None),                    Point( 5, 10), Center( 0.300,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryChainsaw      = Sprite(Weapon::Chainsaw),        Point( 5, 10), Center( 0.250,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryLaw           = Sprite(Weapon::Law),             Point( 5, 10), Center( 0.300,  0.450), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryFlamebow      = Sprite(None),                    Point( 5, 10), Center( 0.300,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryBow           = Sprite(None),                    Point( 5, 10), Center( 0.300,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    SecondaryFlamer        = Sprite(Weapon::Flamer),          Point( 5, 10), Center( 0.300,  0.300), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    LeftThigh              = Sprite(Soldier::Udo),            Point( 6,  3), Center( 0.200,  0.500), Show(true),  Flip(true),  Team(true),  Flex(5.0), Color(Pants),     Alpha(Base )
    LeftThighDmg           = Sprite(Soldier::RannyUdo),       Point( 6,  3), Center( 0.200,  0.500), Show(false), Flip(true),  Team(true),  Flex(5.0), Color(None),      Alpha(Blood)
    LeftFoot               = Sprite(Soldier::Stopa),          Point( 2, 18), Center( 0.350,  0.350), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    LeftJetfoot            = Sprite(Soldier::Lecistopa),      Point( 2, 18), Center( 0.350,  0.350), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    LeftLowerleg           = Sprite(Soldier::Noga),           Point( 3,  2), Center( 0.150,  0.550), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(Pants),     Alpha(Base )
    LeftLowerlegDmg        = Sprite(Soldier::RannyNoga),      Point( 3,  2), Center( 0.150,  0.550), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Blood)
    LeftArm                = Sprite(Soldier::Ramie),          Point(11, 14), Center( 0.000,  0.500), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(Main),      Alpha(Base )
    LeftArmDmg             = Sprite(Soldier::RannyRamie),     Point(11, 14), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Blood)
    LeftForearm            = Sprite(Soldier::Reka),           Point(14, 15), Center( 0.000,  0.500), Show(true),  Flip(false), Team(true),  Flex(5.0), Color(Main),      Alpha(Base )
    LeftForearmDmg         = Sprite(Soldier::RannyReka),      Point(14, 15), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(5.0), Color(None),      Alpha(Blood)
    LeftHand               = Sprite(Soldier::Dlon),           Point(15, 19), Center( 0.000,  0.400), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(Skin),      Alpha(Base )
    GrabbedHelmet          = Sprite(Soldier::Helm),           Point(15, 19), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Main),      Alpha(Base )
    GrabbedHat             = Sprite(Soldier::Kap),            Point(15, 19), Center( 0.100,  0.400), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Main),      Alpha(Base )
    RightThigh             = Sprite(Soldier::Udo),            Point( 5,  4), Center( 0.200,  0.650), Show(true),  Flip(true),  Team(true),  Flex(5.0), Color(Pants),     Alpha(Base )
    RightThighDmg          = Sprite(Soldier::RannyUdo),       Point( 5,  4), Center( 0.200,  0.650), Show(false), Flip(true),  Team(true),  Flex(5.0), Color(None),      Alpha(Blood)
    RightFoot              = Sprite(Soldier::Stopa),          Point( 1, 17), Center( 0.350,  0.350), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    RightJetfoot           = Sprite(Soldier::Lecistopa),      Point( 1, 17), Center( 0.350,  0.350), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    RightLowerleg          = Sprite(Soldier::Noga),           Point( 4,  1), Center( 0.150,  0.550), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(Pants),     Alpha(Base )
    RightLowerlegDmg       = Sprite(Soldier::RannyNoga),      Point( 4,  1), Center( 0.150,  0.550), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Blood)
    Chest                  = Sprite(Soldier::Klata),          Point(10, 11), Center( 0.100,  0.300), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(Main),      Alpha(Base )
    Vest                   = Sprite(Soldier::Kamizelka),      Point(10, 11), Center( 0.100,  0.300), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    ChestDmg               = Sprite(Soldier::RannyKlata),     Point(10, 11), Center( 0.100,  0.300), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Blood)
    Hip                    = Sprite(Soldier::Biodro),         Point( 5,  6), Center( 0.250,  0.600), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(Main),      Alpha(Base )
    HipDmg                 = Sprite(Soldier::RannyBiodro),    Point( 5,  6), Center( 0.250,  0.600), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Blood)
    Head                   = Sprite(Soldier::Morda),          Point( 9, 12), Center( 0.000,  0.500), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(Skin),      Alpha(Base )
    HeadDmg                = Sprite(Soldier::RannyMorda),     Point( 9, 12), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Headblood), Alpha(Blood)
    HeadDead               = Sprite(Soldier::Morda),          Point( 9, 12), Center( 0.500,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Skin),      Alpha(Base )
    HeadDeadDmg            = Sprite(Soldier::RannyMorda),     Point( 9, 12), Center( 0.500,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Headblood), Alpha(Blood)
    MrT                    = Sprite(Soldier::Hair3),          Point( 9, 12), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Hair),      Alpha(Base )
    Helmet                 = Sprite(Soldier::Helm),           Point( 9, 12), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Main),      Alpha(Base )
    Hat                    = Sprite(Soldier::Kap),            Point( 9, 12), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Main),      Alpha(Base )
    RamboBadge             = Sprite(Soldier::Badge),          Point( 9, 12), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    HairDreadlocks         = Sprite(Soldier::Hair1),          Point( 9, 12), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Hair),      Alpha(Base )
    HairDreadlock1         = Sprite(Soldier::Dred),           Point(23, 24), Center( 0.000,  1.220), Show(false), Flip(false), Team(true),  Flex(0.0), Color(Hair),      Alpha(Base )
    HairDreadlock2         = Sprite(Soldier::Dred),           Point(23, 24), Center( 0.100,  0.500), Show(false), Flip(false), Team(true),  Flex(0.0), Color(Hair),      Alpha(Base )
    HairDreadlock3         = Sprite(Soldier::Dred),           Point(23, 24), Center( 0.040, -0.300), Show(false), Flip(false), Team(true),  Flex(0.0), Color(Hair),      Alpha(Base )
    HairDreadlock4         = Sprite(Soldier::Dred),           Point(23, 24), Center( 0.000, -0.900), Show(false), Flip(false), Team(true),  Flex(0.0), Color(Hair),      Alpha(Base )
    HairDreadlock5         = Sprite(Soldier::Dred),           Point(23, 24), Center(-0.200, -1.350), Show(false), Flip(false), Team(true),  Flex(0.0), Color(Hair),      Alpha(Base )
    HairPunk               = Sprite(Soldier::Hair2),          Point( 9, 12), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Hair),      Alpha(Base )
    HairNormal             = Sprite(Soldier::Hair4),          Point( 9, 12), Center( 0.000,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Hair),      Alpha(Base )
    Cigar                  = Sprite(Soldier::Cygaro),         Point( 9, 12), Center(-0.125,  0.400), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(Cygar),     Alpha(Base )
    SilverLchain           = Sprite(Soldier::Lancuch),        Point(10, 22), Center( 0.100,  0.500), Show(false), Flip(false), Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    SilverRchain           = Sprite(Soldier::Lancuch),        Point(11, 22), Center( 0.100,  0.500), Show(false), Flip(false), Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    SilverPendant          = Sprite(Soldier::Metal),          Point(22, 21), Center( 0.500,  0.700), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    GoldenLchain           = Sprite(Soldier::Zlotylancuch),   Point(10, 22), Center( 0.100,  0.500), Show(false), Flip(false), Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    GoldenRchain           = Sprite(Soldier::Zlotylancuch),   Point(11, 22), Center( 0.100,  0.500), Show(false), Flip(false), Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    GoldenPendant          = Sprite(Soldier::Zloto),          Point(22, 21), Center( 0.500,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Base )
    FragGrenade1           = Sprite(Weapon::FragGrenade),     Point( 5,  6), Center( 0.500,  0.100), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Nades)
    FragGrenade2           = Sprite(Weapon::FragGrenade),     Point( 5,  6), Center( 0.500,  0.100), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Nades)
    FragGrenade3           = Sprite(Weapon::FragGrenade),     Point( 5,  6), Center( 0.500,  0.100), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Nades)
    FragGrenade4           = Sprite(Weapon::FragGrenade),     Point( 5,  6), Center( 0.500,  0.100), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Nades)
    FragGrenade5           = Sprite(Weapon::FragGrenade),     Point( 5,  6), Center( 0.500,  0.100), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Nades)
    ClusterGrenade1        = Sprite(Weapon::ClusterGrenade),  Point( 5,  6), Center( 0.500,  0.300), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Nades)
    ClusterGrenade2        = Sprite(Weapon::ClusterGrenade),  Point( 5,  6), Center( 0.500,  0.300), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Nades)
    ClusterGrenade3        = Sprite(Weapon::ClusterGrenade),  Point( 5,  6), Center( 0.500,  0.300), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Nades)
    ClusterGrenade4        = Sprite(Weapon::ClusterGrenade),  Point( 5,  6), Center( 0.500,  0.300), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Nades)
    ClusterGrenade5        = Sprite(Weapon::ClusterGrenade),  Point( 5,  6), Center( 0.500,  0.300), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Nades)
    PrimaryDeagles         = Sprite(Weapon::Deagles),         Point(16, 15), Center( 0.100,  0.800), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryDeaglesClip     = Sprite(Weapon::DeaglesClip),     Point(16, 15), Center( 0.100,  0.800), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryDeaglesFire     = Sprite(Weapon::DeaglesFire),     Point(16, 15), Center(-0.500,  1.000), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryMp5             = Sprite(Weapon::Mp5),             Point(16, 15), Center( 0.150,  0.600), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryMp5Clip         = Sprite(Weapon::Mp5Clip),         Point(16, 15), Center( 0.150,  0.600), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryMp5Fire         = Sprite(Weapon::Mp5Fire),         Point(16, 15), Center( -0.65,  0.850), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryAk74            = Sprite(Weapon::Ak74),            Point(16, 15), Center( 0.150,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryAk74Clip        = Sprite(Weapon::Ak74Clip),        Point(16, 15), Center( 0.150,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryAk74Fire        = Sprite(Weapon::Ak74Fire),        Point(16, 15), Center( -0.37,  0.800), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimarySteyr           = Sprite(Weapon::Steyr),           Point(16, 15), Center( 0.200,  0.600), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimarySteyrClip       = Sprite(Weapon::SteyrClip),       Point(16, 15), Center( 0.200,  0.600), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimarySteyrFire       = Sprite(Weapon::SteyrFire),       Point(16, 15), Center( -0.24,  0.750), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimarySpas            = Sprite(Weapon::Spas),            Point(16, 15), Center( 0.100,  0.600), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimarySpasClip        = Sprite(None),                    Point(16, 15), Center( 0.000,  0.000), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimarySpasFire        = Sprite(Weapon::SpasFire),        Point(16, 15), Center(-0.200,  0.900), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryRuger           = Sprite(Weapon::Ruger),           Point(16, 15), Center( 0.100,  0.700), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryRugerClip       = Sprite(None),                    Point(16, 15), Center( 0.000,  0.000), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryRugerFire       = Sprite(Weapon::RugerFire),       Point(16, 15), Center( -0.35,  0.850), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryM79             = Sprite(Weapon::M79),             Point(16, 15), Center( 0.100,  0.700), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryM79Clip         = Sprite(Weapon::M79Clip),         Point(16, 15), Center( 0.100,  0.700), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryM79Fire         = Sprite(Weapon::M79Fire),         Point(16, 15), Center(-0.400,  0.800), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryBarrett         = Sprite(Weapon::Barrett),         Point(16, 15), Center( 0.150,  0.700), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryBarrettClip     = Sprite(Weapon::BarrettClip),     Point(16, 15), Center( 0.150,  0.700), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryBarrettFire     = Sprite(Weapon::BarrettFire),     Point(16, 15), Center( -0.15,  0.800), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryMinimi          = Sprite(Weapon::Minimi),          Point(16, 15), Center( 0.150,  0.600), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryMinimiClip      = Sprite(Weapon::MinimiClip),      Point(16, 15), Center( 0.150,  0.600), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryMinimiFire      = Sprite(Weapon::MinimiFire),      Point(16, 15), Center(-0.200,  0.900), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryMinigunClip     = Sprite(Weapon::MinigunClip),     Point( 8,  7), Center( 0.500,  0.100), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryMinigun         = Sprite(Weapon::Minigun),         Point(16, 15), Center( 0.050,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryMinigunFire     = Sprite(Weapon::MinigunFire),     Point(16, 15), Center(-0.200,  0.450), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimarySocom           = Sprite(Weapon::Socom),           Point(16, 15), Center( 0.200,  0.550), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimarySocomClip       = Sprite(Weapon::SocomClip),       Point(16, 15), Center( 0.200,  0.550), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimarySocomFire       = Sprite(Weapon::SocomFire),       Point(16, 15), Center( -0.24,  0.850), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryKnife           = Sprite(Weapon::Knife),           Point(16, 20), Center(-0.100,  0.600), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryKnifeClip       = Sprite(None),                    Point(16, 15), Center( 0.000,  0.000), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryKnifeFire       = Sprite(None),                    Point(16, 15), Center( 0.000,  0.000), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryChainsaw        = Sprite(Weapon::Chainsaw),        Point(16, 15), Center( 0.100,  0.500), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryChainsawClip    = Sprite(None),                    Point(16, 15), Center( 0.000,  0.000), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryChainsawFire    = Sprite(Weapon::ChainsawFire),    Point(16, 15), Center( 0.000,  0.000), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryLaw             = Sprite(Weapon::Law),             Point(16, 15), Center( 0.100,  0.600), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryLawClip         = Sprite(None),                    Point(16, 15), Center( 0.000,  0.000), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryLawFire         = Sprite(Weapon::LawFire),         Point(16, 15), Center(-0.100,  0.550), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryBow             = Sprite(Weapon::Bow),             Point(16, 15), Center(-0.400,  0.550), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryBowArrow        = Sprite(Weapon::BowA),            Point(16, 15), Center( 0.000,  0.550), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryBowString       = Sprite(Weapon::BowS),            Point(16, 15), Center(-0.400,  0.550), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryBowReload       = Sprite(Weapon::Bow),             Point(16, 15), Center(-0.400,  0.550), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryBowArrowReload  = Sprite(Weapon::Arrow),           Point(16, 20), Center( 0.000,  0.550), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryBowStringReload = Sprite(Weapon::BowS),            Point(16, 15), Center(-0.400,  0.550), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryBowFire         = Sprite(Weapon::BowFire),         Point(16, 15), Center( 0.000,  0.000), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryFlamer          = Sprite(Weapon::Flamer),          Point(16, 15), Center( 0.200,  0.700), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryFlamerClip      = Sprite(None),                    Point(16, 15), Center( 0.000,  0.000), Show(false), Flip(true),  Team(false), Flex(0.0), Color(None),      Alpha(Base )
    PrimaryFlamerFire      = Sprite(Weapon::FlamerFire),      Point(16, 15), Center( 0.000,  0.000), Show(false), Flip(false), Team(false), Flex(0.0), Color(None),      Alpha(Base )
    RightArm               = Sprite(Soldier::Ramie),          Point(10, 13), Center( 0.000,  0.600), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(Main),      Alpha(Base )
    RightArmDmg            = Sprite(Soldier::RannyRamie),     Point(10, 13), Center(-0.100,  0.500), Show(false), Flip(true),  Team(true),  Flex(0.0), Color(None),      Alpha(Blood)
    RightForearm           = Sprite(Soldier::Reka),           Point(13, 16), Center( 0.000,  0.600), Show(true),  Flip(false), Team(true),  Flex(5.0), Color(Main),      Alpha(Base )
    RightForearmDmg        = Sprite(Soldier::RannyReka),      Point(13, 16), Center( 0.000,  0.600), Show(false), Flip(true),  Team(true),  Flex(5.0), Color(None),      Alpha(Blood)
    RightHand              = Sprite(Soldier::Dlon),           Point(16, 20), Center( 0.000,  0.500), Show(true),  Flip(true),  Team(true),  Flex(0.0), Color(Skin),      Alpha(Base )
}
