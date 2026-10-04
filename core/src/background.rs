//! `TBackgroundState`: soldiers and things pass through background polygons once they
//! entered them through a background transition polygon.

use super::*;

const POLY_UNKNOWN: i32 = -2;
const POLY_NONE: i32 = -1;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct BackgroundState {
    /// `BACKGROUND_TRANSITION` (true) or `BACKGROUND_NORMAL`.
    pub transition: bool,
    /// `BackgroundPoly`: unknown, none, or a 1-based number. Soldat stores a polygon
    /// number from `BackgroundTest` but an index into the background polygon list from
    /// `BackgroundFindCurrentPoly`, and reads both as polygon numbers; ported as is.
    pub poly: i32,
    test_result: bool,
}

impl Default for BackgroundState {
    /// What `CreateSprite`, `Respawn` and `CreateThing` set.
    fn default() -> Self {
        BackgroundState {
            transition: true,
            poly: POLY_UNKNOWN,
            test_result: false,
        }
    }
}

impl BackgroundState {
    /// `BackgroundTest`: whether polygon `poly` (0-based) is passed through.
    pub fn test(&mut self, map: &MapFile, poly: usize) -> bool {
        match map.polygons[poly].polytype {
            PolyType::Background if self.transition => {
                self.test_result = true;
                self.poly = poly as i32 + 1;
                true
            }
            PolyType::BackgroundTransition => {
                self.test_result = true;
                self.transition = true;
                true
            }
            _ => false,
        }
    }

    /// `BackgroundTestBigPolyCenter`
    pub fn big_poly_center(&mut self, map: &MapFile, pos: Vec2) {
        if !self.transition {
            return;
        }
        if self.poly == POLY_UNKNOWN {
            self.poly = Self::find_current_poly(map, pos);
            if self.poly != POLY_NONE {
                self.test_result = true;
            }
        } else if self.poly != POLY_NONE
            && map
                .polygons
                .get(self.poly as usize - 1)
                .is_some_and(|p| map.point_in_poly(pos, p))
        {
            self.test_result = true;
        }
    }

    /// `BackgroundFindCurrentPoly`: 1-based index into the background polygon list.
    fn find_current_poly(map: &MapFile, pos: Vec2) -> i32 {
        map.polygons
            .iter()
            .filter(|p| {
                matches!(
                    p.polytype,
                    PolyType::Background | PolyType::BackgroundTransition
                )
            })
            .position(|p| map.point_in_poly(pos, p))
            .map_or(POLY_NONE, |i| i as i32 + 1)
    }

    /// `BackgroundTestPrepare`
    pub fn prepare(&mut self) {
        self.test_result = false;
    }

    /// `BackgroundTestReset`: no background contact this tick, back to normal.
    pub fn reset(&mut self) {
        if !self.test_result {
            self.transition = false;
            self.poly = POLY_NONE;
        }
    }
}
