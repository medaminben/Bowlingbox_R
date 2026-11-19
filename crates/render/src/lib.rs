//! GPU-free render data and maths for Bowlingbox: themes, decor layout, meshes, lighting maths,
//! the camera and the scene description.
//!
//! Axes follow the lane crate: +z runs down the lane, +y is up and +x is the bowler's right. The
//! crate knows nothing of the lane crate; the app converts its types.

// Meshes and layouts index small counts and compare float maths in short formulas, so integer to
// float casts are exact and one-letter names are the maths' own.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)]

mod camera;
mod decor;
mod geometry;
mod lighting;
mod scene;
mod theme;

pub use camera::Camera;
pub use decor::{
    Aabb, FLOOR_TOP_Y, HALL_BACK_Z, HALL_FRONT_Z, HALL_HEIGHT, PIT_LENGTH, PIT_TOP_Y, Prop,
    RETURN_GAP, RETURN_HEIGHT, RETURN_WIDTH, Shape, WALL_THICKNESS, bounds, build_props,
    hall_half_width, lane_pitch, local_bounds, shape_mesh,
};
pub use geometry::{
    MeshData, box_data, cone_data, cylinder_data, lathe_data, pin_data, sphere_data,
};
pub use lighting::{
    LightFrustum, SHADOW_TOP_Y, fit_light_frustum, linear_to_srgb, linear_to_srgb_vec3,
    props_in_box, reflection_box, shadow_box, srgb_to_linear, srgb_to_linear_vec3,
};
pub use scene::{SceneDescription, SceneGeometry, Transform};
pub use theme::{
    MAX_POINT_LIGHTS, Material, Pattern, PointLight, PropSet, THEMES, Theme, ThemeId, next_theme,
    theme, theme_from_name,
};
