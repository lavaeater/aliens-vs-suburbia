//! The math that keeps a low-res render steady: snap the camera to its own texel grid and
//! move the upscaled picture by what the snap took away. Pure, so it is tested here.

use bevy::math::{IVec2, Quat, UVec2, Vec2, Vec3};

/// The camera snapped onto the texel grid, measured along its own right/up axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapped {
    /// Where to put the camera.
    pub translation: Vec3,
    /// How far the wanted position lies from the snapped one, in texels (x right, y up),
    /// each within -0.5..=0.5.
    pub remainder: Vec2,
    /// Which texel of the world grid the camera sits on (x right, y up). The dither pattern
    /// is offset by it so it stays put on the world while the camera moves.
    pub texel: IVec2,
}

/// Snap `translation` so that, seen through a camera turned by `rotation`, it lands on a
/// multiple of `texel` world units across and up the screen. Depth is left alone.
pub fn snap_to_texels(translation: Vec3, rotation: Quat, texel: f32) -> Snapped {
    if texel <= 0.0 || !texel.is_finite() {
        return Snapped {
            translation,
            remainder: Vec2::ZERO,
            texel: IVec2::ZERO,
        };
    }
    let local = rotation.inverse() * translation;
    let index = (local.truncate() / texel).round();
    let snapped = Vec3::new(index.x * texel, index.y * texel, local.z);
    Snapped {
        translation: rotation * snapped,
        remainder: (local.truncate() - snapped.truncate()) / texel,
        texel: index.as_ivec2(),
    }
}

/// The low-res picture: its size, how big a texel is on screen and where the picture sits.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CanvasLayout {
    /// Size of the render target in texels (window / pixel size, plus a texel of overscan
    /// on every side so the shifted picture never shows an edge).
    pub image: UVec2,
    /// Size of one texel on screen, in logical pixels.
    pub texel_on_screen: f32,
    /// Offset of the picture's centre from the window's, in logical pixels, y up.
    pub shift: Vec2,
}

impl CanvasLayout {
    /// The render target for a window of `physical` pixels drawn at `pixel_size` screen
    /// pixels per texel.
    pub fn image_size(physical: UVec2, pixel_size: u32) -> UVec2 {
        let px = pixel_size.max(1);
        UVec2::new(physical.x.div_ceil(px), physical.y.div_ceil(px)) + UVec2::splat(2)
    }

    /// Move the picture against the snap: if the camera stands `remainder` texels short of
    /// where it wants to be, the world shows up that far the other way.
    pub fn shift_for(remainder: Vec2, texel_on_screen: f32) -> Vec2 {
        -remainder * texel_on_screen
    }

    /// A cursor position in the window (logical pixels, origin top left, y down) as a
    /// position in the low-res render target, which is what the game camera's
    /// `viewport_to_world` wants.
    pub fn window_to_image(&self, cursor: Vec2, window: Vec2) -> Vec2 {
        let centre = window / 2.0 + Vec2::new(self.shift.x, -self.shift.y);
        (cursor - centre) / self.texel_on_screen.max(1e-6) + self.image.as_vec2() / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_4;

    fn camera_rotation() -> Quat {
        Quat::from_rotation_y(0.7) * Quat::from_rotation_x(-FRAC_PI_4)
    }

    #[test]
    fn snapped_position_lies_on_the_grid_and_keeps_depth() {
        let rot = camera_rotation();
        let s = snap_to_texels(Vec3::new(3.37, 5.1, -2.22), rot, 0.25);
        let local = rot.inverse() * s.translation;
        let want_local = rot.inverse() * Vec3::new(3.37, 5.1, -2.22);
        for v in [local.x, local.y] {
            assert!(
                ((v / 0.25) - (v / 0.25).round()).abs() < 1e-3,
                "{v} on the grid"
            );
        }
        assert!((local.z - want_local.z).abs() < 1e-4);
    }

    #[test]
    fn remainder_adds_back_to_the_wanted_position() {
        let rot = camera_rotation();
        let want = Vec3::new(-7.9, 4.0, 11.3);
        let s = snap_to_texels(want, rot, 0.1);
        assert!(s.remainder.x.abs() <= 0.5 + 1e-4 && s.remainder.y.abs() <= 0.5 + 1e-4);
        let back = rot.inverse() * s.translation + (s.remainder * 0.1).extend(0.0);
        assert!((back - rot.inverse() * want).length() < 1e-4);
    }

    #[test]
    fn moving_one_texel_moves_the_index_by_one() {
        let rot = camera_rotation();
        let a = snap_to_texels(Vec3::new(1.0, 2.0, 3.0), rot, 0.2);
        let b = snap_to_texels(Vec3::new(1.0, 2.0, 3.0) + rot * Vec3::X * 0.2, rot, 0.2);
        assert_eq!(b.texel - a.texel, IVec2::new(1, 0));
    }

    #[test]
    fn a_bad_texel_size_leaves_the_camera_alone() {
        let p = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(snap_to_texels(p, Quat::IDENTITY, 0.0).translation, p);
    }

    #[test]
    fn image_covers_the_window_with_overscan() {
        assert_eq!(
            CanvasLayout::image_size(UVec2::new(1920, 1080), 4),
            UVec2::new(482, 272)
        );
        assert_eq!(
            CanvasLayout::image_size(UVec2::new(1001, 7), 3),
            UVec2::new(336, 5)
        );
    }

    #[test]
    fn window_centre_maps_to_image_centre_when_unshifted() {
        let layout = CanvasLayout {
            image: UVec2::new(100, 60),
            texel_on_screen: 4.0,
            shift: Vec2::ZERO,
        };
        assert_eq!(
            layout.window_to_image(Vec2::new(200.0, 120.0), Vec2::new(400.0, 240.0)),
            Vec2::new(50.0, 30.0)
        );
        assert_eq!(
            layout.window_to_image(Vec2::new(204.0, 116.0), Vec2::new(400.0, 240.0)),
            Vec2::new(51.0, 29.0)
        );
    }

    #[test]
    fn a_shifted_picture_moves_the_cursor_mapping_with_it() {
        // Picture moved 2 px right and 2 px up: the texel under a fixed cursor is one
        // half-texel to the left and half a texel lower in the image (y down).
        let layout = CanvasLayout {
            image: UVec2::new(100, 60),
            texel_on_screen: 4.0,
            shift: Vec2::new(2.0, 2.0),
        };
        assert_eq!(
            layout.window_to_image(Vec2::new(200.0, 120.0), Vec2::new(400.0, 240.0)),
            Vec2::new(49.5, 30.5)
        );
    }
}
