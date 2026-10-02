// Sprite identifiers shared by simulation (weapon tables) and rendering.
// Each identifier maps to an image path relative to the asset root (extension resolved at load time).

macro_rules! sprites {
    (
        $($enm:ident {
            $($id:ident = $file:tt)+
        })+
    ) => {
        pub trait SpriteData where Self: ::std::marker::Sized {
            fn id(&self) -> usize;
            fn group(&self) -> Group;
            fn filename(&self) -> &'static str;
            fn values() -> &'static [Self];
        }

        #[derive(Debug, Copy, Clone)]
        pub enum Group {
            $($enm,)+
        }

        impl Group {
            pub fn id(&self) -> usize { *self as usize }
            pub fn values() -> &'static [Group] {
                static VALUES: &[Group] = &[$(Group::$enm,)+];
                VALUES
            }
        }

        $(
            #[derive(Debug, Copy, Clone)]
            pub enum $enm {
                $($id,)+
            }

            impl SpriteData for $enm {
                fn id(&self) -> usize { *self as usize }
                fn group(&self) -> Group { Group::$enm }
                fn filename(&self) -> &'static str {
                    match *self { $($enm::$id => $file,)+ }
                }
                fn values() -> &'static [$enm] {
                    static VALUES: &[$enm] = &[$($enm::$id,)+];
                    VALUES
                }
            }

            impl ::std::convert::From<usize> for $enm {
                fn from(id: usize) -> $enm {
                    match $enm::values().get(id as usize) {
                        Some(&v) => v,
                        _ => panic!("Invalid sprite identifier."),
                    }
                }
            }

            impl ::std::ops::Add<usize> for $enm {
                type Output = $enm;
                fn add(self, x: usize) -> $enm { $enm::from(self.id() + x) }
            }

            impl ::std::ops::Sub<usize> for $enm {
                type Output = $enm;
                fn sub(self, x: usize) -> $enm { $enm::from(self.id() - x) }
            }
        )+
    }
}

// Note: images that have a "2" version go together (opposite gostek direction).
sprites! {
    // The order of the gostek images must match the order of the Team2 versions
    // listed below. Keep both lists together, the range Soldier::Stopa to
    // Soldier::Team2Lecistopa2 is checked for a size restriction.

    Soldier {
        Stopa              = "gostek-gfx/stopa"
        Stopa2             = "gostek-gfx/stopa2"
        Noga               = "gostek-gfx/noga"
        Noga2              = "gostek-gfx/noga2"
        Udo                = "gostek-gfx/udo"
        Udo2               = "gostek-gfx/udo2"
        Biodro             = "gostek-gfx/biodro"
        Biodro2            = "gostek-gfx/biodro2"
        Klata              = "gostek-gfx/klata"
        Klata2             = "gostek-gfx/klata2"
        Morda              = "gostek-gfx/morda"
        Morda2             = "gostek-gfx/morda2"
        Ramie              = "gostek-gfx/ramie"
        Ramie2             = "gostek-gfx/ramie2"
        Reka               = "gostek-gfx/reka"
        Reka2              = "gostek-gfx/reka2"
        Dlon               = "gostek-gfx/dlon"
        Dlon2              = "gostek-gfx/dlon2"
        Lancuch            = "gostek-gfx/lancuch"
        Helm               = "gostek-gfx/helm"
        Helm2              = "gostek-gfx/helm2"
        Badge              = "gostek-gfx/badge"
        Badge2             = "gostek-gfx/badge2"
        Cygaro             = "gostek-gfx/cygaro"
        Cygaro2            = "gostek-gfx/cygaro2"
        Metal              = "gostek-gfx/metal"
        Metal2             = "gostek-gfx/metal2"
        Zloto              = "gostek-gfx/zloto"
        Zloto2             = "gostek-gfx/zloto2"
        Zlotylancuch       = "gostek-gfx/zlotylancuch"
        Kap                = "gostek-gfx/kap"
        Kap2               = "gostek-gfx/kap2"
        Dred               = "gostek-gfx/dred"
        Hair1              = "gostek-gfx/hair1"
        Hair12             = "gostek-gfx/hair12"
        Hair2              = "gostek-gfx/hair2"
        Hair22             = "gostek-gfx/hair22"
        Hair3              = "gostek-gfx/hair3"
        Hair32             = "gostek-gfx/hair32"
        Hair4              = "gostek-gfx/hair4"
        Hair42             = "gostek-gfx/hair42"
        Kamizelka          = "gostek-gfx/kamizelka"
        Kamizelka2         = "gostek-gfx/kamizelka2"
        RannyKlata         = "gostek-gfx/ranny/klata"
        RannyKlata2        = "gostek-gfx/ranny/klata2"
        RannyMorda         = "gostek-gfx/ranny/morda"
        RannyMorda2        = "gostek-gfx/ranny/morda2"
        RannyNoga          = "gostek-gfx/ranny/noga"
        RannyNoga2         = "gostek-gfx/ranny/noga2"
        RannyRamie         = "gostek-gfx/ranny/ramie"
        RannyRamie2        = "gostek-gfx/ranny/ramie2"
        RannyReka          = "gostek-gfx/ranny/reka"
        RannyReka2         = "gostek-gfx/ranny/reka2"
        RannyUdo           = "gostek-gfx/ranny/udo"
        RannyUdo2          = "gostek-gfx/ranny/udo2"
        RannyBiodro        = "gostek-gfx/ranny/biodro"
        RannyBiodro2       = "gostek-gfx/ranny/biodro2"
        Lecistopa          = "gostek-gfx/lecistopa"
        Lecistopa2         = "gostek-gfx/lecistopa2"

        // Soldier Team2 list. Keep it next to the non-team2 list.

        Team2Stopa         = "gostek-gfx/team2/stopa"
        Team2Stopa2        = "gostek-gfx/team2/stopa2"
        Team2Noga          = "gostek-gfx/team2/noga"
        Team2Noga2         = "gostek-gfx/team2/noga2"
        Team2Udo           = "gostek-gfx/team2/udo"
        Team2Udo2          = "gostek-gfx/team2/udo2"
        Team2Biodro        = "gostek-gfx/team2/biodro"
        Team2Biodro2       = "gostek-gfx/team2/biodro2"
        Team2Klata         = "gostek-gfx/team2/klata"
        Team2Klata2        = "gostek-gfx/team2/klata2"
        Team2Morda         = "gostek-gfx/team2/morda"
        Team2Morda2        = "gostek-gfx/team2/morda2"
        Team2Ramie         = "gostek-gfx/team2/ramie"
        Team2Ramie2        = "gostek-gfx/team2/ramie2"
        Team2Reka          = "gostek-gfx/team2/reka"
        Team2Reka2         = "gostek-gfx/team2/reka2"
        Team2Dlon          = "gostek-gfx/team2/dlon"
        Team2Dlon2         = "gostek-gfx/team2/dlon2"
        Team2Lancuch       = "gostek-gfx/team2/lancuch"
        Team2Helm          = "gostek-gfx/team2/helm"
        Team2Helm2         = "gostek-gfx/team2/helm2"
        Team2Badge         = "gostek-gfx/team2/badge"
        Team2Badge2        = "gostek-gfx/team2/badge2"
        Team2Cygaro        = "gostek-gfx/team2/cygaro"
        Team2Cygaro2       = "gostek-gfx/team2/cygaro2"
        Team2Metal         = "gostek-gfx/team2/metal"
        Team2Metal2        = "gostek-gfx/team2/metal2"
        Team2Zloto         = "gostek-gfx/team2/zloto"
        Team2Zloto2        = "gostek-gfx/team2/zloto2"
        Team2Zlotylancuch  = "gostek-gfx/team2/zlotylancuch"
        Team2Kap           = "gostek-gfx/team2/kap"
        Team2Kap2          = "gostek-gfx/team2/kap2"
        Team2Dred          = "gostek-gfx/team2/dred"
        Team2Hair1         = "gostek-gfx/team2/hair1"
        Team2Hair12        = "gostek-gfx/team2/hair12"
        Team2Hair2         = "gostek-gfx/team2/hair2"
        Team2Hair22        = "gostek-gfx/team2/hair22"
        Team2Hair3         = "gostek-gfx/team2/hair3"
        Team2Hair32        = "gostek-gfx/team2/hair32"
        Team2Hair4         = "gostek-gfx/team2/hair4"
        Team2Hair42        = "gostek-gfx/team2/hair42"
        Team2Kamizelka     = "gostek-gfx/team2/kamizelka"
        Team2Kamizelka2    = "gostek-gfx/team2/kamizelka2"
        Team2RannyKlata    = "gostek-gfx/team2/ranny/klata"
        Team2RannyKlata2   = "gostek-gfx/team2/ranny/klata2"
        Team2RannyMorda    = "gostek-gfx/team2/ranny/morda"
        Team2RannyMorda2   = "gostek-gfx/team2/ranny/morda2"
        Team2RannyNoga     = "gostek-gfx/team2/ranny/noga"
        Team2RannyNoga2    = "gostek-gfx/team2/ranny/noga2"
        Team2RannyRamie    = "gostek-gfx/team2/ranny/ramie"
        Team2RannyRamie2   = "gostek-gfx/team2/ranny/ramie2"
        Team2RannyReka     = "gostek-gfx/team2/ranny/reka"
        Team2RannyReka2    = "gostek-gfx/team2/ranny/reka2"
        Team2RannyUdo      = "gostek-gfx/team2/ranny/udo"
        Team2RannyUdo2     = "gostek-gfx/team2/ranny/udo2"
        Team2RannyBiodro   = "gostek-gfx/team2/ranny/biodro"
        Team2RannyBiodro2  = "gostek-gfx/team2/ranny/biodro2"
        Team2Lecistopa     = "gostek-gfx/team2/lecistopa"
        Team2Lecistopa2    = "gostek-gfx/team2/lecistopa2"

        // Preserve order of these parachute textures.

        ParaRope           = "gostek-gfx/para-rope"
        Para               = "gostek-gfx/para"
        Para2              = "gostek-gfx/para2"
    }

    // The range Shell to M2Stat is checked for size restriction too, so
    // keep that range together.

    Weapon {
        Shell              = "weapons-gfx/shell"
        Bullet             = "weapons-gfx/bullet"
        Smudge             = "weapons-gfx/smudge"
        Missile            = "weapons-gfx/missile"
        Cluster            = "weapons-gfx/cluster"
        ClusterGrenade     = "weapons-gfx/cluster-grenade"
        FragGrenade        = "weapons-gfx/frag-grenade"
        Ak74               = "weapons-gfx/ak74"
        Ak742              = "weapons-gfx/ak74-2"
        Ak74Clip           = "weapons-gfx/ak74-clip"
        Ak74Clip2          = "weapons-gfx/ak74-clip2"
        Ak74Shell          = "weapons-gfx/ak74-shell"
        Ak74Bullet         = "weapons-gfx/ak74-bullet"
        Ak74Fire           = "weapons-gfx/ak74-fire"
        Minimi             = "weapons-gfx/m249"
        Minimi2            = "weapons-gfx/m249-2"
        MinimiClip         = "weapons-gfx/m249-clip"
        MinimiClip2        = "weapons-gfx/m249-clip2"
        MinimiShell        = "weapons-gfx/m249-shell"
        MinimiBullet       = "weapons-gfx/m249-bullet"
        MinimiFire         = "weapons-gfx/m249-fire"
        Ruger              = "weapons-gfx/ruger77"
        Ruger2             = "weapons-gfx/ruger77-2"
        RugerShell         = "weapons-gfx/ruger77-shell"
        RugerBullet        = "weapons-gfx/ruger77-bullet"
        RugerFire          = "weapons-gfx/ruger77-fire"
        Mp5                = "weapons-gfx/mp5"
        Mp52               = "weapons-gfx/mp5-2"
        Mp5Clip            = "weapons-gfx/mp5-clip"
        Mp5Clip2           = "weapons-gfx/mp5-clip2"
        Mp5Shell           = "weapons-gfx/mp5-shell"
        Mp5Bullet          = "weapons-gfx/mp5-bullet"
        Mp5Fire            = "weapons-gfx/mp5-fire"
        Spas               = "weapons-gfx/spas12"
        Spas2              = "weapons-gfx/spas12-2"
        SpasShell          = "weapons-gfx/spas12-shell"
        SpasBullet         = "weapons-gfx/spas12-bullet"
        SpasFire           = "weapons-gfx/spas12-fire"
        M79                = "weapons-gfx/m79"
        M792               = "weapons-gfx/m79-2"
        M79Clip            = "weapons-gfx/m79-clip"
        M79Clip2           = "weapons-gfx/m79-clip2"
        M79Shell           = "weapons-gfx/m79-shell"
        M79Bullet          = "weapons-gfx/m79-bullet"
        M79Fire            = "weapons-gfx/m79-fire"
        Deagles            = "weapons-gfx/deserteagle"
        Deagles2           = "weapons-gfx/deserteagle-2"
        NDeagles           = "weapons-gfx/n-deserteagle"
        NDeagles2          = "weapons-gfx/n-deserteagle-2"
        DeaglesClip        = "weapons-gfx/deserteagle-clip"
        DeaglesClip2       = "weapons-gfx/deserteagle-clip2"
        DeaglesShell       = "weapons-gfx/eagles-shell"
        DeaglesBullet      = "weapons-gfx/eagles-bullet"
        DeaglesFire        = "weapons-gfx/eagles-fire"
        Steyr              = "weapons-gfx/steyraug"
        Steyr2             = "weapons-gfx/steyraug-2"
        SteyrClip          = "weapons-gfx/steyraug-clip"
        SteyrClip2         = "weapons-gfx/steyraug-clip2"
        SteyrShell         = "weapons-gfx/steyraug-shell"
        SteyrBullet        = "weapons-gfx/steyraug-bullet"
        SteyrFire          = "weapons-gfx/steyraug-fire"
        Barrett            = "weapons-gfx/barretm82"
        Barrett2           = "weapons-gfx/barretm82-2"
        BarrettClip        = "weapons-gfx/barretm82-clip"
        BarrettClip2       = "weapons-gfx/barretm82-clip2"
        BarrettShell       = "weapons-gfx/barretm82-shell"
        BarrettBullet      = "weapons-gfx/barretm82-bullet"
        BarrettFire        = "weapons-gfx/barret-fire"
        Minigun            = "weapons-gfx/minigun"
        Minigun2           = "weapons-gfx/minigun-2"
        MinigunClip        = "weapons-gfx/minigun-clip"
        MinigunShell       = "weapons-gfx/minigun-shell"
        MinigunBullet      = "weapons-gfx/minigun-bullet"
        MinigunFire        = "weapons-gfx/minigun-fire"
        Socom              = "weapons-gfx/colt1911"
        Socom2             = "weapons-gfx/colt1911-2"
        NSocom             = "weapons-gfx/n-colt1911"
        NSocom2            = "weapons-gfx/n-colt1911-2"
        SocomClip          = "weapons-gfx/colt1911-clip"
        SocomClip2         = "weapons-gfx/colt1911-clip2"
        ColtShell          = "weapons-gfx/colt-shell"
        ColtBullet         = "weapons-gfx/colt-bullet"
        SocomFire          = "weapons-gfx/colt1911-fire"
        Bow                = "weapons-gfx/bow"
        Bow2               = "weapons-gfx/bow-2"
        BowS               = "weapons-gfx/bow-s"
        BowA               = "weapons-gfx/bow-a"
        NBow               = "weapons-gfx/n-bow"
        NBow2              = "weapons-gfx/n-bow-2"
        Arrow              = "weapons-gfx/arrow"
        BowFire            = "weapons-gfx/bow-fire"
        Flamer             = "weapons-gfx/flamer"
        Flamer2            = "weapons-gfx/flamer-2"
        FlamerFire         = "weapons-gfx/flamer-fire"
        Knife              = "weapons-gfx/knife"
        Knife2             = "weapons-gfx/knife2"
        Chainsaw           = "weapons-gfx/chainsaw"
        Chainsaw2          = "weapons-gfx/chainsaw2"
        ChainsawFire       = "weapons-gfx/chainsaw-fire"
        Law                = "weapons-gfx/law"
        Law2               = "weapons-gfx/law-2"
        LawFire            = "weapons-gfx/law-fire"
        M2                 = "weapons-gfx/m2"
        M22                = "weapons-gfx/m2-2"
        M2Stat             = "weapons-gfx/m2-stat"
    }

    // Preserve order of:
    // - ExplosionExplode1 to ExplosionExplode16
    // - ExplosionSmoke1 to Minismoke
    // - FlamesExplode1 to FlamesExplode16

    Spark {
        Smoke              = "sparks-gfx/smoke"
        Lilfire            = "sparks-gfx/lilfire"
        Odprysk            = "sparks-gfx/odprysk"
        Blood              = "sparks-gfx/blood"
        ExplosionExplode1  = "sparks-gfx/explosion/explode1"
        ExplosionExplode2  = "sparks-gfx/explosion/explode2"
        ExplosionExplode3  = "sparks-gfx/explosion/explode3"
        ExplosionExplode4  = "sparks-gfx/explosion/explode4"
        ExplosionExplode5  = "sparks-gfx/explosion/explode5"
        ExplosionExplode6  = "sparks-gfx/explosion/explode6"
        ExplosionExplode7  = "sparks-gfx/explosion/explode7"
        ExplosionExplode8  = "sparks-gfx/explosion/explode8"
        ExplosionExplode9  = "sparks-gfx/explosion/explode9"
        ExplosionExplode10 = "sparks-gfx/explosion/explode10"
        ExplosionExplode11 = "sparks-gfx/explosion/explode11"
        ExplosionExplode12 = "sparks-gfx/explosion/explode12"
        ExplosionExplode13 = "sparks-gfx/explosion/explode13"
        ExplosionExplode14 = "sparks-gfx/explosion/explode14"
        ExplosionExplode15 = "sparks-gfx/explosion/explode15"
        ExplosionExplode16 = "sparks-gfx/explosion/explode16"
        Bigsmoke           = "sparks-gfx/bigsmoke"
        Spawnspark         = "sparks-gfx/spawnspark"
        Pin                = "sparks-gfx/pin"
        Lilsmoke           = "sparks-gfx/lilsmoke"
        Stuff              = "sparks-gfx/stuff"
        Cygaro             = "sparks-gfx/cygaro"
        Plomyk             = "sparks-gfx/plomyk"
        Blacksmoke         = "sparks-gfx/blacksmoke"
        Rain               = "sparks-gfx/rain"
        Sand               = "sparks-gfx/sand"
        Odlamek1           = "sparks-gfx/odlamek1"
        Odlamek2           = "sparks-gfx/odlamek2"
        Odlamek3           = "sparks-gfx/odlamek3"
        Odlamek4           = "sparks-gfx/odlamek4"
        Skrawek            = "sparks-gfx/skrawek"
        Puff               = "sparks-gfx/puff"
        Snow               = "sparks-gfx/snow"
        ExplosionSmoke1    = "sparks-gfx/explosion/smoke1"
        ExplosionSmoke2    = "sparks-gfx/explosion/smoke2"
        ExplosionSmoke3    = "sparks-gfx/explosion/smoke3"
        ExplosionSmoke4    = "sparks-gfx/explosion/smoke4"
        ExplosionSmoke5    = "sparks-gfx/explosion/smoke5"
        ExplosionSmoke6    = "sparks-gfx/explosion/smoke6"
        ExplosionSmoke7    = "sparks-gfx/explosion/smoke7"
        ExplosionSmoke8    = "sparks-gfx/explosion/smoke8"
        ExplosionSmoke9    = "sparks-gfx/explosion/smoke9"
        Splat              = "sparks-gfx/splat"
        Minismoke          = "sparks-gfx/minismoke"
        Bigsmoke2          = "sparks-gfx/bigsmoke2"
        Jetfire            = "sparks-gfx/jetfire"
        Lilblood           = "sparks-gfx/lilblood"
        FlamesExplode1     = "sparks-gfx/flames/explode1"
        FlamesExplode2     = "sparks-gfx/flames/explode2"
        FlamesExplode3     = "sparks-gfx/flames/explode3"
        FlamesExplode4     = "sparks-gfx/flames/explode4"
        FlamesExplode5     = "sparks-gfx/flames/explode5"
        FlamesExplode6     = "sparks-gfx/flames/explode6"
        FlamesExplode7     = "sparks-gfx/flames/explode7"
        FlamesExplode8     = "sparks-gfx/flames/explode8"
        FlamesExplode9     = "sparks-gfx/flames/explode9"
        FlamesExplode10    = "sparks-gfx/flames/explode10"
        FlamesExplode11    = "sparks-gfx/flames/explode11"
        FlamesExplode12    = "sparks-gfx/flames/explode12"
        FlamesExplode13    = "sparks-gfx/flames/explode13"
        FlamesExplode14    = "sparks-gfx/flames/explode14"
        FlamesExplode15    = "sparks-gfx/flames/explode15"
        FlamesExplode16    = "sparks-gfx/flames/explode16"
    }

    Object {
        Flag               = "textures/objects/flag"
        Infflag            = "textures/objects/infflag"
        Medikit            = "textures/objects/medikit"
        Grenadekit         = "textures/objects/grenadekit"
        Flamerkit          = "textures/objects/flamerkit"
        Predatorkit        = "textures/objects/predatorkit"
        Vestkit            = "textures/objects/vestkit"
        Berserkerkit       = "textures/objects/berserkerkit"
        Clusterkit         = "textures/objects/clusterkit"
        Ilum               = "objects-gfx/ilum"
        FlagHandle         = "objects-gfx/flag"
    }

    // Preserve order of Guns*

    Interface {
        Sight              = "interface-gfx/sight"
        GunsDeagles        = "interface-gfx/guns/1"
        GunsMp5            = "interface-gfx/guns/2"
        GunsAk74           = "interface-gfx/guns/3"
        GunsSteyr          = "interface-gfx/guns/4"
        GunsSpas           = "interface-gfx/guns/5"
        GunsRuger          = "interface-gfx/guns/6"
        GunsM79            = "interface-gfx/guns/7"
        GunsBarrett        = "interface-gfx/guns/8"
        GunsMinimi         = "interface-gfx/guns/9"
        GunsMinigun        = "interface-gfx/guns/0"
        GunsSocom          = "interface-gfx/guns/10"
        GunsKnife          = "interface-gfx/guns/knife"
        GunsChainsaw       = "interface-gfx/guns/chainsaw"
        GunsLaw            = "interface-gfx/guns/law"
        GunsFlamer         = "interface-gfx/guns/flamer"
        GunsBow            = "interface-gfx/guns/bow"
        GunsFist           = "interface-gfx/guns/fist"
        GunsM2             = "interface-gfx/guns/m2"
        Star               = "interface-gfx/star"
        Smalldot           = "interface-gfx/smalldot"
        Deaddot            = "interface-gfx/deaddot"
        Mute               = "interface-gfx/mute"
        Prot               = "interface-gfx/prot"
        Scroll             = "interface-gfx/scroll"
        Bot                = "interface-gfx/bot"
        Cursor             = "interface-gfx/cursor"
        Health             = "interface-gfx/health"
        Ammo               = "interface-gfx/ammo"
        Jet                = "interface-gfx/jet"
        HealthBar          = "interface-gfx/health-bar"
        JetBar             = "interface-gfx/jet-bar"
        ReloadBar          = "interface-gfx/reload-bar"
        Back               = "interface-gfx/back"
        Overlay            = "interface-gfx/overlay"
        Nade               = "interface-gfx/nade"
        Noflag             = "interface-gfx/noflag"
        Flag               = "interface-gfx/flag"
        ClusterNade        = "interface-gfx/cluster-nade"
        Dot                = "interface-gfx/dot"
        FireBar            = "interface-gfx/fire-bar"
        FireBarR           = "interface-gfx/fire-bar-r"
        VestBar            = "interface-gfx/vest-bar"
        Menucursor         = "interface-gfx/menucursor"
        Arrow              = "interface-gfx/arrow"
        TitleL             = "interface-gfx/title-l"
        TitleR             = "interface-gfx/title-r"
    }
}
