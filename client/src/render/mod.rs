use super::*;

pub mod bullets;
pub mod game;
pub mod gfx;
pub mod map;
pub mod soldiers;
pub mod things;

pub use self::game::GameGraphics;

use self::bullets::*;
use self::map::*;
use self::soldiers::*;
use self::things::*;
use gfx2d::math::Mat2d;
use gfx2d::*;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "gif", "bmp"];

/// Finds an image the way Soldat does (the file may be shipped with any image extension)
/// and decodes it. Returns the resolved asset path (or the requested one if nothing was
/// found) and the image, if it could be loaded.
fn load_image(vfs: &Vfs, dir: &str, fname: &str) -> (String, Option<image::RgbaImage>) {
    let requested = format!("{}/{}", dir.trim_end_matches('/'), fname);

    let Some(path) = vfs.find_with_extensions(&requested, IMAGE_EXTENSIONS) else {
        tracing::warn!(path = requested, "image not found");
        return (requested, None);
    };

    let image = vfs
        .read(&path)
        .map_err(|e| e.to_string())
        .and_then(|data| gfx2d_extra::decode_image_rgba(&data).map_err(|e| e.to_string()));

    match image {
        Ok(image) => (path, Some(image)),
        Err(error) => {
            tracing::warn!(path, error, "cannot load image");
            (path, None)
        }
    }
}
