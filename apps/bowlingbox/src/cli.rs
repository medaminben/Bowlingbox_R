//! The command line and what the app does with it at startup.

use bowlingbox_render::{
    RenderMode, ThemeId, render_mode_from_env, render_mode_warning, theme_from_name,
};
use clap::Parser;

use crate::hud_text;

#[derive(Parser, Debug, Default)]
#[command(name = "bowlingbox", version = env!("BK_VERSION_FULL"), about = "A 3D bowling game")]
pub struct Cli {
    /// Look of the hall: neon, classic or outdoor (any letter case)
    #[arg(long, value_name = "NAME")]
    pub theme: Option<String>,
    /// Start without sound (this session only)
    #[arg(long)]
    pub mute: bool,
    /// Show the top-down physics view instead of the 3D view
    #[arg(long)]
    pub debug_view: bool,
}

/// What the command line and the environment decided.
#[derive(Debug, PartialEq)]
pub struct Startup {
    pub theme: ThemeId,
    pub debug_view: bool,
    pub mute: bool,
    pub render_mode: RenderMode,
    /// Lines for stderr, each printed once at startup.
    pub warnings: Vec<String>,
}

impl Cli {
    /// `render_env` is the value of `BOWLINGBOX_RENDER`. An unknown theme name falls back to the
    /// default theme with a warning.
    pub fn startup(&self, render_env: Option<&str>) -> Startup {
        let mut warnings = Vec::new();
        warnings.extend(render_mode_warning(render_env));
        let theme_id = match self.theme.as_deref() {
            None => ThemeId::NeonHall,
            Some(name) => theme_from_name(name).unwrap_or_else(|| {
                warnings.push(hud_text::unknown_theme_message(name));
                ThemeId::NeonHall
            }),
        };
        Startup {
            theme: theme_id,
            debug_view: self.debug_view,
            mute: self.mute,
            render_mode: render_mode_from_env(render_env),
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("bowlingbox").chain(args.iter().copied())).unwrap()
    }

    #[test]
    fn no_arguments_start_with_neon_hall() {
        let startup = parse(&[]).startup(None);
        assert_eq!(startup.theme, ThemeId::NeonHall);
        assert!(!startup.debug_view);
        assert_eq!(startup.warnings, Vec::<String>::new());
    }

    #[test]
    fn theme_names_ignore_case() {
        assert_eq!(
            parse(&["--theme=CLASSIC"]).startup(None).theme,
            ThemeId::ClassicAlley
        );
        assert_eq!(
            parse(&["--theme", "Outdoor"]).startup(None),
            Startup {
                theme: ThemeId::Outdoor,
                debug_view: false,
                mute: false,
                render_mode: RenderMode::Full,
                warnings: vec![],
            }
        );
    }

    #[test]
    fn an_unknown_theme_warns_and_gives_neon_hall() {
        let startup = parse(&["--theme=space"]).startup(None);
        assert_eq!(startup.theme, ThemeId::NeonHall);
        assert_eq!(
            startup.warnings,
            ["unknown theme 'space'; use neon, classic or outdoor"]
        );
    }

    #[test]
    fn an_unknown_render_path_warns_exactly_once() {
        let startup = parse(&["--theme=outdoor"]).startup(Some("fancy"));
        assert_eq!(
            startup.warnings,
            ["render: unknown BOWLINGBOX_RENDER 'fancy'; using the full path where supported"]
        );
        assert_eq!(startup.render_mode, RenderMode::Full);
        let basic = parse(&[]).startup(Some("BASIC"));
        assert_eq!(basic.warnings, Vec::<String>::new());
        assert_eq!(basic.render_mode, RenderMode::Basic);
    }

    #[test]
    fn mute_starts_muted_and_the_default_does_not() {
        assert!(parse(&["--mute"]).startup(None).mute);
        assert!(!parse(&[]).startup(None).mute);
    }

    #[test]
    fn flags_parse() {
        let cli = parse(&["--mute", "--debug-view"]);
        assert!(cli.mute && cli.debug_view);
        assert!(Cli::try_parse_from(["bowlingbox", "--bogus"]).is_err());
    }
}
