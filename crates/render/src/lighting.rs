//! Colour conversion and the key light's fitting maths.

use crate::decor::{Aabb, PIT_LENGTH, PIT_TOP_Y, Prop, bounds};
use crate::scene::SceneGeometry;
use glam::camera::rh::{proj::directx, view};
use glam::{Mat4, Vec3};

/// sRGB transfer function, for values in [0, 1].
#[must_use]
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[must_use]
pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[must_use]
pub fn srgb_to_linear_vec3(c: Vec3) -> Vec3 {
    Vec3::new(
        srgb_to_linear(c.x),
        srgb_to_linear(c.y),
        srgb_to_linear(c.z),
    )
}

#[must_use]
pub fn linear_to_srgb_vec3(c: Vec3) -> Vec3 {
    Vec3::new(
        linear_to_srgb(c.x),
        linear_to_srgb(c.y),
        linear_to_srgb(c.z),
    )
}

/// Top of the shadow box: the masking unit (up to 2.4 m) casts, the ceiling (4.5 m) does not.
pub const SHADOW_TOP_Y: f32 = 2.5;

/// An orthographic light camera. Depth runs 0..1 as in wgpu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightFrustum {
    pub view: Mat4,
    pub projection: Mat4,
}

impl LightFrustum {
    #[must_use]
    pub fn view_projection(&self) -> Mat4 {
        self.projection * self.view
    }
}

/// The orthographic light camera looking along `light_dir` whose volume is the tightest fit
/// around `bounds`. It depends only on the light and the box, never on the player's camera.
/// Its up vector is the lane direction (+z), so the lane runs along the map's y axis.
#[must_use]
pub fn fit_light_frustum(light_dir: Vec3, bounds: &Aabb) -> LightFrustum {
    let dir = light_dir.normalize();
    let centre = (bounds.min + bounds.max) * 0.5;
    // Up = down the lane: the light-space x axis then has no z part, so the long, thin lane fills
    // the map's long (y) axis. A light along the lane can't use that up; no theme does that.
    let up = if dir.z.abs() > 0.99 { Vec3::Y } else { Vec3::Z };
    let view = view::look_at_mat4(centre - dir, centre, up);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for i in 0..8 {
        let corner = Vec3::new(
            if i & 1 != 0 {
                bounds.max.x
            } else {
                bounds.min.x
            },
            if i & 2 != 0 {
                bounds.max.y
            } else {
                bounds.min.y
            },
            if i & 4 != 0 {
                bounds.max.z
            } else {
                bounds.min.z
            },
        );
        let p = view.transform_point3(corner);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    // The view looks down -z, so the corner nearest the light has the largest z.
    LightFrustum {
        view,
        projection: directx::orthographic(lo.x, hi.x, lo.y, hi.y, -hi.z, -lo.z),
    }
}

/// What the key light's shadow map covers: lane, gutters, approach, deck and pit, up to
/// [`SHADOW_TOP_Y`].
#[must_use]
pub fn shadow_box(g: &SceneGeometry) -> Aabb {
    let x = g.lane_half_width + g.gutter_width + 0.1;
    Aabb {
        min: Vec3::new(-x, PIT_TOP_Y - 0.1, -g.approach),
        max: Vec3::new(x, SHADOW_TOP_Y, g.lane_length + PIT_LENGTH),
    }
}

/// What the lane mirrors: above the lane surface, the lane and gutters plus 0.3 m each side (the
/// ball returns), from the approach to the end of the pit.
#[must_use]
pub fn reflection_box(g: &SceneGeometry) -> Aabb {
    let x = g.lane_half_width + g.gutter_width + 0.3;
    Aabb {
        min: Vec3::new(-x, 0.0, -g.approach),
        max: Vec3::new(x, SHADOW_TOP_Y, g.lane_length + PIT_LENGTH),
    }
}

/// Indices of the props whose world box overlaps `region` (to pick shadow and reflection casters).
#[must_use]
pub fn props_in_box(props: &[Prop], g: &SceneGeometry, region: &Aabb) -> Vec<usize> {
    props
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            let b = bounds(p, g);
            b.min.x < region.max.x
                && b.max.x > region.min.x
                && b.min.y < region.max.y
                && b.max.y > region.min.y
                && b.min.z < region.max.z
                && b.max.z > region.min.z
        })
        .map(|(i, _)| i)
        .collect()
}
