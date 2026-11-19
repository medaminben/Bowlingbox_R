use bowlingbox_render::{RenderMode, render_mode_from_env, render_mode_warning};

#[test]
fn no_value_is_the_full_path() {
    assert_eq!(render_mode_from_env(None), RenderMode::Full);
    assert_eq!(render_mode_warning(None), None);
}

#[test]
fn basic_and_full_are_accepted_in_any_ascii_case() {
    for value in ["basic", "BASIC", "Basic"] {
        assert_eq!(render_mode_from_env(Some(value)), RenderMode::Basic);
        assert_eq!(render_mode_warning(Some(value)), None);
    }
    for value in ["full", "FULL", "Full"] {
        assert_eq!(render_mode_from_env(Some(value)), RenderMode::Full);
        assert_eq!(render_mode_warning(Some(value)), None);
    }
}

#[test]
fn an_unknown_value_means_full_with_a_warning() {
    assert_eq!(render_mode_from_env(Some("x")), RenderMode::Full);
    assert_eq!(
        render_mode_warning(Some("x")).as_deref(),
        Some("render: unknown BOWLINGBOX_RENDER 'x'; using the full path where supported")
    );
}
