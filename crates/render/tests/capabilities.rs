use bowlingbox_render::{Capabilities, RenderMode, choose_path};

const CAPABLE: Capabilities = Capabilities {
    hdr_msaa_samples: 4,
    hdr_resolve: true,
    max_texture_2d: 8192,
    timestamp_query: false,
};

#[test]
fn a_capable_gpu_runs_the_full_path() {
    assert_eq!(choose_path(RenderMode::Full, &CAPABLE), Ok(()));
    let single_sample = Capabilities {
        hdr_msaa_samples: 1,
        max_texture_2d: 4096,
        ..CAPABLE
    };
    assert_eq!(choose_path(RenderMode::Full, &single_sample), Ok(()));
}

#[test]
fn the_basic_mode_names_the_environment_variable() {
    assert_eq!(
        choose_path(RenderMode::Basic, &CAPABLE),
        Err("basic path (BOWLINGBOX_RENDER=basic)".to_owned())
    );
}

#[test]
fn no_hdr_target_names_the_format() {
    let caps = Capabilities {
        hdr_msaa_samples: 0,
        ..CAPABLE
    };
    let reason = choose_path(RenderMode::Full, &caps).unwrap_err();
    assert!(reason.contains("Rgba16Float"), "{reason}");
    assert!(reason.ends_with("; using the basic path"), "{reason}");
    assert!(!reason.starts_with("render: "), "{reason}");
}

#[test]
fn a_small_texture_limit_names_the_shadow_map_size() {
    let caps = Capabilities {
        max_texture_2d: 2048,
        ..CAPABLE
    };
    assert_eq!(
        choose_path(RenderMode::Full, &caps),
        Err(
            "max texture size 2048 is below the 4096 px shadow map; using the basic path"
                .to_owned()
        )
    );
}
