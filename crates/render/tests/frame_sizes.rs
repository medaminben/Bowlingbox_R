//! The full path's target sizes for a view (C++ `frameSizes`): no GPU needed.

use bowlingbox_render::FrameSizes;

#[test]
fn a_full_hd_view_halves_down_the_chain() {
    let s = FrameSizes::new([1920, 1080]);
    assert_eq!(s.scene, [1920, 1080]);
    assert_eq!(s.reflection, [960, 540]);
    assert_eq!(s.bloom, [[960, 540], [480, 270], [240, 135], [120, 67]]);
}

#[test]
fn a_small_view_rounds_each_level_down() {
    let s = FrameSizes::new([320, 180]);
    assert_eq!(s.scene, [320, 180]);
    assert_eq!(s.reflection, [160, 90]);
    assert_eq!(s.bloom, [[160, 90], [80, 45], [40, 22], [20, 11]]);
}

#[test]
fn a_tiny_view_stops_the_chain_at_one_pixel() {
    let s = FrameSizes::new([20, 12]);
    assert_eq!(s.scene, [20, 12]);
    assert_eq!(s.reflection, [10, 6]);
    assert_eq!(s.bloom, [[10, 6], [5, 3], [2, 1], [1, 1]]);
}

#[test]
fn no_size_reaches_zero() {
    for view in [[0, 0], [1, 1], [3, 0], [0, 7]] {
        let s = FrameSizes::new(view);
        for size in [s.scene, s.reflection].iter().chain(&s.bloom) {
            assert!(size.iter().all(|&side| side >= 1), "{view:?}: {s:?}");
        }
    }
}
