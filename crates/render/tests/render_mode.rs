use bowlingbox_render::{RenderMode, render_mode_from_env, render_mode_warning};

#[test]
fn no_value_is_the_basic_path() {
    assert_eq!(render_mode_from_env(None), RenderMode::Basic);
    assert_eq!(render_mode_warning(None), None);
}

#[test]
fn basic_is_accepted_in_any_ascii_case() {
    for value in ["basic", "BASIC", "Basic"] {
        assert_eq!(render_mode_from_env(Some(value)), RenderMode::Basic);
        assert_eq!(render_mode_warning(Some(value)), None);
    }
}

#[test]
fn an_unknown_value_falls_back_to_basic_with_a_warning() {
    assert_eq!(render_mode_from_env(Some("x")), RenderMode::Basic);
    assert_eq!(
        render_mode_warning(Some("x")).as_deref(),
        Some("render: unknown BOWLINGBOX_RENDER 'x'; using the basic path")
    );
}
