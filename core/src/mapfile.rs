use super::*;
use crate::assets::Vfs;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor, Read};

const MAX_POLYS: i32 = 5000;
//const MIN_SECTOR: i32 = -25;
const MAX_SECTOR: i32 = 25;
pub(crate) const MIN_SECTORZ: i32 = -35;
const MAX_SECTORZ: i32 = 35;
//const TILESECTOR: i32 = 3;
//const MIN_TILE: i32 = MIN_SECTOR * TILESECTOR;
//const MAX_TILE: i32 = MAX_SECTOR * TILESECTOR;
const MAX_PROPS: i32 = 500;
//const MAX_SPAWNPOINTS: i32 = 255;
//const MAX_COLLIDERS: i32 = 128;

#[allow(dead_code)]
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum PolyType {
    Normal,
    OnlyBulletsCollide,
    OnlyPlayersCollide,
    NoCollide,
    Ice,
    Deadly,
    BloodyDeadly,
    Hurts,
    Regenerates,
    Lava,
    AlphaBullets,
    AlphaPlayers,
    BravoBullets,
    BravoPlayers,
    CharlieBullets,
    CharliePlayers,
    DeltaBullets,
    DeltaPlayers,
    Bouncy,
    Explosive,
    HurtsFlaggers,
    OnlyFlaggers,
    NotFlaggers,
    NonFlaggersCollide,
    Background,
    BackgroundTransition,
}

/// What a ray cast collides with (`TPolyMap.RayCast` parameters).
#[derive(Debug, Copy, Clone)]
pub struct RayCast {
    pub player: bool,
    pub flag: bool,
    pub bullet: bool,
    pub check_collider: bool,
    pub team: Team,
}

impl Default for RayCast {
    fn default() -> Self {
        RayCast {
            player: false,
            flag: false,
            bullet: true,
            check_collider: false,
            team: Team::None,
        }
    }
}

impl RayCast {
    fn collides(&self, polytype: PolyType) -> bool {
        use PolyType::*;
        let (np, nb) = (!self.player, !self.bullet);
        let team = self.team;

        let excluded = match polytype {
            AlphaBullets => team != Team::Alpha || nb,
            AlphaPlayers => team != Team::Alpha || np,
            BravoBullets => team != Team::Bravo || nb,
            BravoPlayers => team != Team::Bravo || np,
            CharlieBullets => team != Team::Charlie || nb,
            CharliePlayers => team != Team::Charlie || np,
            DeltaBullets => team != Team::Delta || nb,
            DeltaPlayers => team != Team::Delta || np,
            OnlyFlaggers => !self.flag || np,
            NotFlaggers => self.flag || np,
            NonFlaggersCollide => !self.flag || np || nb,
            OnlyBulletsCollide => nb,
            OnlyPlayersCollide => np,
            NoCollide | Background | BackgroundTransition => true,
            _ => false,
        };

        !excluded
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MapColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Copy, Clone)]
pub struct MapVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rhw: f32,
    pub color: MapColor,
    pub u: f32,
    pub v: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct MapPolygon {
    pub vertices: [MapVertex; 3],
    pub normals: [Vec3; 3],
    pub polytype: PolyType,
    pub bounciness: f32,
}

#[derive(Debug, Clone, Default)]
pub struct MapSector {
    pub polys: Vec<u16>,
}

#[derive(Debug, Clone)]
pub struct MapProp {
    pub active: bool,
    pub style: u16,
    pub width: i32,
    pub height: i32,
    pub x: f32,
    pub y: f32,
    pub rotation: Rad,
    pub scale_x: f32,
    pub scale_y: f32,
    pub alpha: u8,
    pub color: MapColor,
    pub level: u8,
}

#[derive(Debug, Clone)]
pub struct MapScenery {
    pub filename: String,
    pub date: i32,
}

#[derive(Debug, Clone)]
pub struct MapCollider {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct MapSpawnpoint {
    pub active: bool,
    pub x: i32,
    pub y: i32,
    pub team: i32,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct MapFile {
    pub filename: String,
    pub version: i32,
    pub mapname: String,
    pub texture_name: String,
    pub bg_color_top: MapColor,
    pub bg_color_bottom: MapColor,
    pub start_jet: i32,
    pub grenade_packs: u8,
    pub medikits: u8,
    pub weather: u8,
    pub steps: u8,
    pub random_id: i32,
    pub polygons: Vec<MapPolygon>,
    pub sectors_division: i32,
    pub sectors_num: i32,
    pub sectors: Vec<MapSector>,
    /// Sector polygon lists as a 51x51 grid indexed by [x + 25][y + 25].
    pub sectors_poly: Vec<Vec<MapSector>>,
    pub props: Vec<MapProp>,
    pub scenery: Vec<MapScenery>,
    pub colliders: Vec<MapCollider>,
    pub spawnpoints: Vec<MapSpawnpoint>,
    /// Bot paths (`BotPath`); connections are 1-based indices into this list.
    pub waypoints: Vec<Waypoint>,
    pub perps: Vec<[Vec2; 3]>,
}

impl MapPolygon {
    pub fn bullet_collides(&self, team: Team) -> bool {
        match self.polytype {
            PolyType::AlphaBullets => team == Team::Alpha,
            PolyType::BravoBullets => team == Team::Bravo,
            PolyType::CharlieBullets => team == Team::Charlie,
            PolyType::DeltaBullets => team == Team::Delta,
            PolyType::AlphaPlayers => false,
            PolyType::BravoPlayers => false,
            PolyType::CharliePlayers => false,
            PolyType::DeltaPlayers => false,
            PolyType::OnlyPlayersCollide => false,
            PolyType::NoCollide => false,
            PolyType::OnlyFlaggers => false,
            PolyType::NotFlaggers => false,
            PolyType::Background => false,
            PolyType::BackgroundTransition => false,
            _ => true,
        }
    }
}

impl MapFile {
    /// Loads `maps/<name>.pms` from the asset filesystem.
    pub fn load(vfs: &Vfs, name: &str) -> Result<MapFile, DataError> {
        let path = format!("maps/{name}.pms");
        let data = vfs.read(&path)?;
        MapFile::parse(&path, &data)
    }

    /// The base of a team's flag (`Map.FlagSpawn`): the first active alpha (5) or
    /// bravo (6) flag spawn point.
    pub fn flag_spawn(&self, bravo: bool) -> Option<Vec2> {
        let team = if bravo { 6 } else { 5 };
        self.spawnpoints
            .iter()
            .find(|s| s.active && s.team == team)
            .map(|s| vec2(s.x as f32, s.y as f32))
    }

    /// Parses a `.pms` map.
    pub fn parse(filename: &str, data: &[u8]) -> Result<MapFile, DataError> {
        Self::read(filename, &mut Cursor::new(data))
            .map_err(|e| DataError::parse(filename, e.to_string()))
    }

    fn read(filename: &str, buf: &mut Cursor<&[u8]>) -> io::Result<MapFile> {
        let filename = filename.to_owned();
        let version = buf.read_i32::<LittleEndian>()?;
        let mapname = read_string(buf, 38)?;
        let texture_name = read_string(buf, 24)?;
        let bg_color_top = read_color(buf)?;
        let bg_color_bottom = read_color(buf)?;
        // TPolyMap.LoadData gives 19% more jet fuel than the map file says ("quickfix")
        // (in 64 bits, like FPC on x86_64)
        let start_jet = (119 * i64::from(buf.read_i32::<LittleEndian>()?) / 100) as i32;
        let grenade_packs = buf.read_u8()?;
        let medikits = buf.read_u8()?;
        let weather = buf.read_u8()?;
        let steps = buf.read_u8()?;
        let random_id = buf.read_i32::<LittleEndian>()?;

        let n = buf.read_i32::<LittleEndian>()?;
        if !(0..=MAX_POLYS).contains(&n) {
            return Err(invalid("Wrong PMS data (number of polygons)"));
        }

        let mut polygons: Vec<MapPolygon> = Vec::new();
        let mut perps = Vec::new();

        for _i in 0..n {
            let vertices: [MapVertex; 3] =
                [read_vertex(buf)?, read_vertex(buf)?, read_vertex(buf)?];

            let normals: [Vec3; 3] = [read_vec3(buf)?, read_vec3(buf)?, read_vec3(buf)?];

            let polytype = buf.read_u8()?;

            fn poly_to_enum(id: u8) -> PolyType {
                match id {
                    1 => PolyType::OnlyBulletsCollide,
                    2 => PolyType::OnlyPlayersCollide,
                    3 => PolyType::NoCollide,
                    4 => PolyType::Ice,
                    5 => PolyType::Deadly,
                    6 => PolyType::BloodyDeadly,
                    7 => PolyType::Hurts,
                    8 => PolyType::Regenerates,
                    9 => PolyType::Lava,
                    10 => PolyType::AlphaBullets,
                    11 => PolyType::AlphaPlayers,
                    12 => PolyType::BravoBullets,
                    13 => PolyType::BravoPlayers,
                    14 => PolyType::CharlieBullets,
                    15 => PolyType::CharliePlayers,
                    16 => PolyType::DeltaBullets,
                    17 => PolyType::DeltaPlayers,
                    18 => PolyType::Bouncy,
                    19 => PolyType::Explosive,
                    20 => PolyType::HurtsFlaggers,
                    21 => PolyType::OnlyFlaggers,
                    22 => PolyType::NotFlaggers,
                    23 => PolyType::NonFlaggersCollide,
                    24 => PolyType::Background,
                    25 => PolyType::BackgroundTransition,
                    _ => PolyType::Normal,
                }
            }

            // Vec2Length of the third normal's x/y ("gg" in TPolyMap.LoadData)
            let bounciness = vec2(normals[2].x, normals[2].y).length();

            polygons.push(MapPolygon {
                vertices,
                normals,
                polytype: poly_to_enum(polytype),
                bounciness,
            });

            let mut perp: [Vec2; 3] = [
                vec2(normals[0].x, normals[0].y),
                vec2(normals[1].x, normals[1].y),
                vec2(normals[2].x, normals[2].y),
            ];

            perp[0] = vec2normalize(perp[0]);
            perp[1] = vec2normalize(perp[1]);
            perp[2] = vec2normalize(perp[2]);

            perps.push(perp);
        }

        let sectors_division = buf.read_i32::<LittleEndian>()?;
        let sectors_num = buf.read_i32::<LittleEndian>()?;

        if !(0..=MAX_SECTOR).contains(&sectors_num) {
            return Err(invalid("Wrong PMS data (number of sectors)"));
        }

        let n = (2 * sectors_num + 1) * (2 * sectors_num + 1);
        let mut sectors: Vec<MapSector> = Vec::new();

        for _i in 0..n {
            let m = buf.read_u16::<LittleEndian>()?;

            if i32::from(m) > MAX_POLYS {
                break;
            }

            let mut polys: Vec<u16> = Vec::new();

            for _j in 0..m {
                polys.push(buf.read_u16::<LittleEndian>()?);
            }

            sectors.push(MapSector { polys });
        }

        if sectors.len() != 51 * 51 {
            return Err(invalid("Wrong PMS data (expected 51x51 sectors)"));
        }

        let sectors_poly: Vec<Vec<MapSector>> =
            sectors.chunks(51).map(<[MapSector]>::to_vec).collect();

        let n = buf.read_i32::<LittleEndian>()?;
        if !(0..=MAX_PROPS).contains(&n) {
            return Err(invalid("Wrong PMS data (number of props)"));
        }

        let mut props: Vec<MapProp> = Vec::new();

        for _i in 0..n {
            let active = buf.read_u16::<LittleEndian>()? != 0;
            let style = buf.read_u16::<LittleEndian>()?;
            let width = buf.read_i32::<LittleEndian>()?;
            let height = buf.read_i32::<LittleEndian>()?;
            let x = buf.read_f32::<LittleEndian>()?;
            let y = buf.read_f32::<LittleEndian>()?;
            let rotation = rad(buf.read_f32::<LittleEndian>()?);
            let scale_x = buf.read_f32::<LittleEndian>()?;
            let scale_y = buf.read_f32::<LittleEndian>()?;
            let alpha = buf.read_i32::<LittleEndian>()? as u8;
            let mut color = read_color(buf)?;
            color.a = alpha;
            let level = buf.read_i32::<LittleEndian>()? as u8;

            props.push(MapProp {
                active,
                style,
                width,
                height,
                x,
                y,
                rotation,
                scale_x,
                scale_y,
                alpha,
                color,
                level,
            });
        }

        let n = buf.read_i32::<LittleEndian>()?;
        let mut scenery: Vec<MapScenery> = Vec::new();

        for _i in 0..n {
            let filename = read_string(buf, 50)?;
            let date = buf.read_i32::<LittleEndian>()?;

            scenery.push(MapScenery { filename, date });
        }

        let n = buf.read_i32::<LittleEndian>()?;
        let mut colliders: Vec<MapCollider> = Vec::new();

        for _i in 0..n {
            let active = buf.read_i32::<LittleEndian>()? != 0;
            let x = buf.read_f32::<LittleEndian>()?;
            let y = buf.read_f32::<LittleEndian>()?;
            let radius = buf.read_f32::<LittleEndian>()?;

            colliders.push(MapCollider {
                active,
                x,
                y,
                radius,
            });
        }

        let n = buf.read_i32::<LittleEndian>()?;
        let mut spawnpoints: Vec<MapSpawnpoint> = Vec::new();

        for _i in 0..n {
            // a byte and 3 bytes of record padding
            let active = buf.read_u8()? != 0;
            buf.set_position(buf.position() + 3);
            let x = buf.read_i32::<LittleEndian>()?;
            let y = buf.read_i32::<LittleEndian>()?;
            let team = buf.read_i32::<LittleEndian>()?;
            // TPolyMap.LoadData disables far away spawn points
            let active = active && x.wrapping_abs() < 2_000_000 && y.wrapping_abs() < 2_000_000;

            spawnpoints.push(MapSpawnpoint { active, x, y, team });
        }

        // bot waypoints (older maps may end before them)
        let mut waypoints = Vec::new();
        if let Ok(n) = buf.read_i32::<LittleEndian>()
            && (0..=MAX_WAYPOINTS).contains(&n)
        {
            for _ in 0..n {
                waypoints.push(read_waypoint(buf)?);
            }
        }

        Ok(MapFile {
            filename,
            version,
            mapname,
            texture_name,
            bg_color_top,
            bg_color_bottom,
            start_jet,
            grenade_packs,
            medikits,
            weather,
            steps,
            random_id,
            polygons,
            sectors_division,
            sectors_num,
            sectors,
            sectors_poly,
            props,
            scenery,
            colliders,
            spawnpoints,
            waypoints,
            perps,
        })
    }

    /// Polygon indices (1-based) of sector (x, y), empty outside the map grid.
    pub fn sector(&self, x: i32, y: i32) -> &[u16] {
        let n = self.sectors_num;

        if (-n..=n).contains(&x) && (-n..=n).contains(&y) {
            &self.sectors_poly[(x + 25) as usize][(y + 25) as usize].polys
        } else {
            &[]
        }
    }

    /// Port of `TPolyMap.RayCast`. Returns the distance to the first polygon hit between
    /// `a` and `b` (0 when `a` is inside one), or `None`. A segment longer than `max_dist`
    /// counts as a hit at distance 9999999, like in Soldat.
    pub fn ray_cast(&self, a: Vec2, b: Vec2, max_dist: f32, filter: RayCast) -> Option<f32> {
        let distance = (a - b).length();
        if distance > max_dist {
            return Some(9_999_999.0);
        }

        let div = self.sectors_division as f32;
        let ax = (a.x.min(b.x) / div).round_ties_even() as i32;
        let ay = (a.y.min(b.y) / div).round_ties_even() as i32;
        let bx = (a.x.max(b.x) / div).round_ties_even() as i32;
        let by = (a.y.max(b.y) / div).round_ties_even() as i32;

        if ax > MAX_SECTORZ || bx < MIN_SECTORZ || ay > MAX_SECTORZ || by < MIN_SECTORZ {
            return None;
        }

        let mut hit = None;

        'sectors: for i in ax.max(MIN_SECTORZ)..=bx.min(MAX_SECTORZ) {
            for j in ay.max(MIN_SECTORZ)..=by.min(MAX_SECTORZ) {
                for &w in self.sector(i, j) {
                    let index = w as usize - 1;

                    if !filter.collides(self.polygons[index].polytype) {
                        continue;
                    }

                    if self.point_in_poly(a, &self.polygons[index]) {
                        hit = Some(0.0);
                        break 'sectors;
                    }

                    if let Some(d) = self.line_in_poly(a, b, index) {
                        hit = Some((d - a).length());
                        break 'sectors;
                    }
                }
            }
        }

        if hit.is_none() && filter.check_collider {
            // the segment passes through a collider: |A*x + B*y + C| / sqrt(A^2 + B^2) < r
            let e = a.y - b.y;
            let f = b.x - a.x;
            let g = a.x * b.y - a.y * b.x;
            let h = (e * e + f * f).sqrt();

            for collider in self.colliders.iter().filter(|c| c.active) {
                let c = vec2(collider.x, collider.y);

                if (e * c.x + f * c.y + g).abs() / h <= collider.radius {
                    let r = a.distance_squared(b) + collider.radius * collider.radius;
                    if a.distance_squared(c) <= r && b.distance_squared(c) <= r {
                        return None;
                    }
                }
            }
        }

        hit
    }

    /// Port of `TPolyMap.CollisionTest`: if `pos` is inside a solid polygon, returns the
    /// push-out vector.
    pub fn collision_test(&self, pos: Vec2, is_flag: bool) -> Option<Vec2> {
        use PolyType::*;

        let kx = (pos.x / self.sectors_division as f32).round_ties_even() as i32;
        let ky = (pos.y / self.sectors_division as f32).round_ties_even() as i32;
        let n = self.sectors_num;

        if !(kx > -n && kx < n && ky > -n && ky < n) {
            return None;
        }

        for &w in self.sector(kx, ky) {
            let index = w as usize - 1;
            let polytype = self.polygons[index].polytype;

            let excluded = matches!(
                polytype,
                OnlyBulletsCollide
                    | OnlyPlayersCollide
                    | NoCollide
                    | AlphaPlayers
                    | BravoPlayers
                    | CharliePlayers
                    | DeltaPlayers
                    | Background
                    | BackgroundTransition
            ) || (!is_flag
                && matches!(polytype, OnlyFlaggers | NotFlaggers | NonFlaggersCollide));

            if !excluded && self.point_in_poly(pos, &self.polygons[index]) {
                let (mut d, mut k) = (0.0, 0);
                let perp = self.closest_perpendicular(index as i32, pos, &mut d, &mut k);
                return Some(perp * (1.5 * d));
            }
        }

        None
    }

    /// Port of `TPolyMap.LineInPoly`: intersection of segment a-b with an edge of the polygon.
    pub fn line_in_poly(&self, a: Vec2, b: Vec2, poly: usize) -> Option<Vec2> {
        let vertices = &self.polygons[poly].vertices;

        for i in 0..3 {
            let p = vec2(vertices[i].x, vertices[i].y);
            let q = vec2(vertices[(i + 1) % 3].x, vertices[(i + 1) % 3].y);

            if b.x == a.x && q.x == p.x {
                continue;
            }

            if b.x == a.x {
                let bk = (q.y - p.y) / (q.x - p.x);
                let bm = p.y - bk * p.x;
                let v = vec2(a.x, bk * a.x + bm);

                if v.x > p.x.min(q.x)
                    && v.x < p.x.max(q.x)
                    && v.y > a.y.min(b.y)
                    && v.y < a.y.max(b.y)
                {
                    return Some(v);
                }
            } else if q.x == p.x {
                let ak = (b.y - a.y) / (b.x - a.x);
                let am = a.y - ak * a.x;
                let v = vec2(p.x, ak * p.x + am);

                if v.y > p.y.min(q.y)
                    && v.y < p.y.max(q.y)
                    && v.x > a.x.min(b.x)
                    && v.x < a.x.max(b.x)
                {
                    return Some(v);
                }
            } else {
                let ak = (b.y - a.y) / (b.x - a.x);
                let bk = (q.y - p.y) / (q.x - p.x);

                if ak != bk {
                    let am = a.y - ak * a.x;
                    let bm = p.y - bk * p.x;
                    let x = (bm - am) / (ak - bk);
                    let v = vec2(x, ak * x + am);

                    if v.x > p.x.min(q.x)
                        && v.x < p.x.max(q.x)
                        && v.x > a.x.min(b.x)
                        && v.x < a.x.max(b.x)
                    {
                        return Some(v);
                    }
                }
            }
        }

        None
    }

    pub fn point_in_poly(&self, p: Vec2, poly: &MapPolygon) -> bool {
        let a = &poly.vertices[0];
        let b = &poly.vertices[1];
        let c = &poly.vertices[2];

        let ap_x = p.x - a.x;
        let ap_y = p.y - a.y;
        let p_ab = (b.x - a.x) * ap_y - (b.y - a.y) * ap_x > 0.0f32;
        let p_ac = (c.x - a.x) * ap_y - (c.y - a.y) * ap_x > 0.0f32;

        if p_ac == p_ab {
            return false;
        }

        if ((c.x - b.x) * (p.y - b.y) - (c.y - b.y) * (p.x - b.x) > 0.0f32) != p_ab {
            return false;
        }

        true
    }

    pub fn point_in_poly_edges(&self, x: f32, y: f32, i: i32) -> bool {
        let u_x = x - self.polygons[i as usize].vertices[0].x;
        let u_y = y - self.polygons[i as usize].vertices[0].y;
        let d = self.perps[i as usize][0].x * u_x + self.perps[i as usize][0].y * u_y;
        if d < 0.0 {
            return false;
        }

        let u_x = x - self.polygons[i as usize].vertices[1].x;
        let u_y = y - self.polygons[i as usize].vertices[1].y;
        let d = self.perps[i as usize][1].x * u_x + self.perps[i as usize][1].y * u_y;
        if d < 0.0 {
            return false;
        }

        let u_x = x - self.polygons[i as usize].vertices[2].x;
        let u_y = y - self.polygons[i as usize].vertices[2].y;
        let d = self.perps[i as usize][2].x * u_x + self.perps[i as usize][2].y * u_y;
        if d < 0.0 {
            return false;
        }

        true
    }

    pub fn closest_perpendicular(&self, j: i32, pos: Vec2, d: &mut f32, n: &mut i32) -> Vec2 {
        let px: [f32; 3] = [
            self.polygons[j as usize].vertices[0].x,
            self.polygons[j as usize].vertices[1].x,
            self.polygons[j as usize].vertices[2].x,
        ];

        let py: [f32; 3] = [
            self.polygons[j as usize].vertices[0].y,
            self.polygons[j as usize].vertices[1].y,
            self.polygons[j as usize].vertices[2].y,
        ];

        let mut p1 = vec2(px[0], py[0]);
        let mut p2 = vec2(px[1], py[1]);

        let d1 = point_line_distance(p1, p2, pos);
        *d = d1;

        let mut edge_v1 = 1;
        let mut edge_v2 = 2;

        p1.x = px[1];
        p1.y = py[1];

        p2.x = px[2];
        p2.y = py[2];

        let d2 = point_line_distance(p1, p2, pos);

        if d2 < d1 {
            edge_v1 = 2;
            edge_v2 = 3;
            *d = d2;
        }

        p1.x = px[2];
        p1.y = py[2];

        p2.x = px[0];
        p2.y = py[0];

        let d3 = point_line_distance(p1, p2, pos);

        if (d3 < d2) && (d3 < d1) {
            edge_v1 = 3;
            edge_v2 = 1;
            *d = d3;
        }

        if edge_v1 == 1 && edge_v2 == 2 {
            *n = 1;
            return self.perps[j as usize][0];
        }

        if edge_v1 == 2 && edge_v2 == 3 {
            *n = 2;
            return self.perps[j as usize][1];
        }

        if edge_v1 == 3 && edge_v2 == 1 {
            *n = 3;
            return self.perps[j as usize][2];
        }

        vec2(0.0f32, 0.0f32)
    }

    pub fn sector_polys(&self, pos: Vec2) -> &[u16] {
        let num = self.sectors_num;
        let kx = (pos.x / self.sectors_division as f32).round_ties_even() as i32;
        let ky = (pos.y / self.sectors_division as f32).round_ties_even() as i32;

        if kx >= -num && kx <= num && ky >= -num && ky <= num {
            let i = (kx + num) * (2 * num + 1) + (ky + num);
            &self.sectors[i as usize].polys
        } else {
            &self.sectors[0].polys[0..0]
        }
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

const MAX_WAYPOINTS: i32 = 5000;
pub(crate) const MAX_CONNECTIONS: usize = 20;

/// What a bot does when it reaches a waypoint (`TWaypointAction`).
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum WaypointAction {
    #[default]
    None,
    StopAndCamp,
    Wait1Second,
    Wait5Seconds,
    Wait10Seconds,
    Wait15Seconds,
    Wait20Seconds,
}

/// A bot path node (`TWaypoint`).
#[derive(Debug, Clone, Default)]
pub struct Waypoint {
    pub active: bool,
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub jets: bool,
    pub path_num: u8,
    pub action: WaypointAction,
    pub connections_num: i32,
    /// 1-based waypoint indices; all 20 slots as stored (`ConnectionsNum` are used).
    pub connections: [i32; MAX_CONNECTIONS],
}

fn read_waypoint(buf: &mut Cursor<&[u8]>) -> io::Result<Waypoint> {
    let active = buf.read_u8()? != 0;
    buf.set_position(buf.position() + 3);
    let id = buf.read_i32::<LittleEndian>()?;
    let x = buf.read_i32::<LittleEndian>()?;
    let y = buf.read_i32::<LittleEndian>()?;
    let mut flag = || -> io::Result<bool> { Ok(buf.read_u8()? != 0) };
    let (left, right, up, down, jets) = (flag()?, flag()?, flag()?, flag()?, flag()?);
    let path_num = buf.read_u8()?;
    let action = match buf.read_u8()? {
        1 => WaypointAction::StopAndCamp,
        2 => WaypointAction::Wait1Second,
        3 => WaypointAction::Wait5Seconds,
        4 => WaypointAction::Wait10Seconds,
        5 => WaypointAction::Wait15Seconds,
        6 => WaypointAction::Wait20Seconds,
        _ => WaypointAction::None,
    };
    buf.set_position(buf.position() + 5);
    let connections_num = buf.read_i32::<LittleEndian>()?;
    let mut connections = [0; MAX_CONNECTIONS];
    for c in &mut connections {
        *c = buf.read_i32::<LittleEndian>()?;
    }

    Ok(Waypoint {
        active: active && x.wrapping_abs() < 2_000_000 && y.wrapping_abs() < 2_000_000,
        id,
        x,
        y,
        left,
        right,
        up,
        down,
        jets,
        path_num,
        action,
        connections_num,
        connections,
    })
}

pub fn read_string<T: Read>(reader: &mut T, length: u32) -> io::Result<String> {
    let mut buffer: Vec<u8>;
    let byte = reader.read_u8()?;
    buffer = vec![0u8; byte as usize];
    reader.read_exact(buffer.as_mut_slice())?;

    let filler = length.saturating_sub(u32::from(byte));
    for _i in 0..filler {
        let _ = reader.read_u8()?;
    }

    let x = String::from_utf8_lossy(&buffer).into_owned();

    Ok(x)
}

pub fn read_color<T: Read>(reader: &mut T) -> io::Result<MapColor> {
    let b = reader.read_u8()?;
    let g = reader.read_u8()?;
    let r = reader.read_u8()?;
    let a = reader.read_u8()?;

    Ok(MapColor { r, g, b, a })
}

pub fn read_vertex<T: Read>(reader: &mut T) -> io::Result<MapVertex> {
    let pos = read_vec3(reader)?;
    let rhw = reader.read_f32::<LittleEndian>()?;
    let color = read_color(reader)?;
    let u = reader.read_f32::<LittleEndian>()?;
    let v = reader.read_f32::<LittleEndian>()?;

    Ok(MapVertex {
        x: pos.x,
        y: pos.y,
        z: pos.z,
        rhw,
        color,
        u,
        v,
    })
}

pub fn read_vec3<T: Read>(reader: &mut T) -> io::Result<Vec3> {
    let x = reader.read_f32::<LittleEndian>()?;
    let y = reader.read_f32::<LittleEndian>()?;
    let z = reader.read_f32::<LittleEndian>()?;

    Ok(vec3(x, y, z))
}
