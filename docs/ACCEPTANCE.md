# Acceptance checklist for v2.0.0

The twelve items are the bar for v2.0.0, copied from the design spec. Each one says how it is
checked: **automated** (a test that runs in CI), **checked here** (done by hand on one Linux machine,
with the date and the result) or **owner** (needs a person at a real window, a speaker or the
Windows machine). The release needs every item checked on Linux and on Windows.

Automated tests live in `apps/bowlingbox/src/app/acceptance.rs` unless a row names another file. Two of
them (items 3 and 4) play a whole throw and are ignored in a debug build; `cargo test --workspace -- --include-ignored`
or `cargo test --release` runs them.

Machine of the "checked here" runs: Linux (Debian 13), X11, Intel UHD (Comet Lake GT2) with Vulkan,
release build, `--mute`, a throwaway `XDG_DATA_HOME`. Date: 2026-11-19. The manual's screenshots are captured
with `cargo bk manual capture` and committed under `docs/manual/assets/shots/en/`.

## The list

1. Launch: lane, 10 pins, ball and an empty scoreboard.
2. The tuned pocket throw at about 80 % power strikes.
3. Holding → to the edge: gutter, `-` on the scoreboard.
4. A partial hit, then the second ball: standing pins stay where they were.
5. Resizing rescales the scoreboard and the lane.
6. `N` during a roll restarts; `Esc` quits with nothing on stderr.
7. `--theme=neon|classic|outdoor` starts in that theme; `--theme=space` warns and starts Neon Hall.
8. `T` cycles Neon → Classic → Outdoor → Neon, and View ▸ Theme follows.
9. In every theme: a gutter ball and flying pins stay visible; the aim guide is translucent; pin
   necks are red; 60 fps at 1920×1080 on an integrated GPU.
10. Sound on a pocket throw: release, rolling that rises with speed, pin knocks, strike chime;
    spare bell; nothing extra on a gutter ball; the ambience follows the theme.
11. M, + / −, B and the Sound menu stay in sync; settings come back after a restart.
12. `--mute` starts muted; with no audio device one `sound disabled:` line and the game runs.

Plus: CI green on Linux and Windows, installers built for both, README, ARCHITECTURE and manual
complete with captured screenshots.

## Status per item

| # | Status | How and result |
| --- | --- | --- |
| 1 | Automated, checked here | Test `item_1_launch_shows_the_lane_ten_pins_a_ball_and_an_empty_board`: aiming, 10 pins standing and drawn, ball at the start, empty scoreboard, `Total 0`, no result text. Screenshots of the three themes (2026-11-19, this machine) show the lane, pins, ball and the empty board. |
| 2 | Automated | `crates/lane/tests/rapier_world.rs` `pocket_throw_is_a_strike` (lane API) and `apps/bowlingbox/src/lane_commands.rs` `the_tuned_pocket_throw_is_a_strike` (through the app's commands and its 60 Hz clock). Both use `POCKET_OFFSET`, `POCKET_ANGLE_DEG` and `POCKET_POWER` (0.8). Not repeated in `acceptance.rs`. |
| 3 | Automated | Test `item_3_holding_right_to_the_edge_is_a_gutter_with_a_dash`: Right pressed until the aim stops at 0.7 m, throw, result `Gutter`, first box `-`, all 10 pins still standing. |
| 4 | Automated | Test `item_4_after_a_partial_hit_the_standing_pins_stay_put`: an edge hit knocks down 1 to 9 pins; after the next aiming phase the standing count matches and no standing pin moved 1 mm. The lane crate has the same check in `second_ball_keeps_standing_pins`. |
| 5 | Checked here | One window, started at 960×640 and resized to 1280×800 by an X11 resize request (2026-11-19, this machine, screenshots at both sizes). The scoreboard, power bar, hint, status line and the lane all fill the new size in the same proportions. The HUD geometry at six sizes is also unit-tested (`hud.rs`). Owner: drag the window edge once in a real session, also on Windows. |
| 6 | Automated, checked here, owner | Tests `item_6_n_restarts_during_every_phase_of_a_roll` (N while rolling and while charging gives a new game) and `item_6_escape_quits_unless_a_dialog_or_menu_takes_it` (Esc runs Quit; behind a dialog it closes the dialog; behind a menu it only closes the menu). Checked here: five normal runs (the three themes, `--screen=controls` and `--screen=sound-menu`; the Neon run included the resize) printed nothing on stderr, 0 bytes each (2026-11-19, this machine). Those runs were ended with SIGTERM from the script, not with Esc, because SIGINT does not go through the window's close path. Owner: press Esc in the real window and look at the terminal. |
| 7 | Automated, checked here | Test `item_7_theme_names_start_that_theme_and_an_unknown_one_warns` (the three names, no warning; `space` gives the warning text and Neon Hall). Screenshots of the three themes show each look, and the status line names it. The `--theme=space` run printed exactly `unknown theme 'space'; use neon, classic or outdoor` and nothing else. |
| 8 | Automated, owner | Test `item_8_t_cycles_the_themes_and_the_menu_reads_the_same_state`: T three times gives Classic, Outdoor, Neon; a menu pick works the same way; both do nothing behind a dialog. The menu draws its radio items from the same theme value every frame. Owner: look at the View ▸ Theme marks while pressing T (no input tool here). |
| 9 | Checked here, owner | 60 fps: measured in Task 4 on this machine (release, 1920×1080 fullscreen, full path, GPU clocks at maximum): Neon 71 frames/s median (63 minimum), Classic 72 (69), Outdoor 90 (84). The aim guide is translucent in the screenshots. Owner: a gutter ball and flying pins in each theme, red pin necks during play, 60 fps at 1920×1080 on the Windows machine. The guide's cyan is up to about 16 of 255 levels less bright than before the Task 4 render change (see ARCHITECTURE). |
| 10 | Owner | Sound by ear: release, rolling pitch rising with speed, pin knocks, strike chime, spare bell, nothing extra on a gutter ball, the ambience changing with the theme, and the stereo pan direction (left lane edge in the left speaker). The cues are unit-tested in `sound_director.rs` with a recording output; hearing them is not automated. |
| 11 | Automated, owner | Tests `item_11_keys_menu_and_status_stay_in_sync` (M, +, −, B change the output, the Sound menu state and the status text together; Volume up is disabled at 100 %, Volume down at 0 %) and `item_11_settings_come_back_through_storage` (save, load, defaults when nothing is stored); `settings_store.rs` tests the keys and bad values. Owner: change the settings, close the window, start again, and see them come back (also on Windows). |
| 12 | Automated, owner | Test `item_12_mute_starts_muted_and_a_silent_output_runs_the_game` (`--mute` mutes the output and the status reads `sound muted`; the game steps on a silent output). `crates/audio/tests/output.rs` `failing_factory_gives_silent_output_and_a_warning` covers the single `sound disabled:` line. Owner: start the game on a machine with no audio device once. |

## What only the owner can check

- Sound by ear, including the pan direction (item 10).
- 60 fps at 1920×1080 on the Windows machine (item 9).
- The whole list on Windows.
- Game ▸ Quit (Ctrl+Q) and Esc in the real window, with the terminal in view (item 6).
- Settings surviving a real restart (item 11).
- A gutter ball and flying pins in each theme, red pin necks, View ▸ Theme marks while pressing T
  (items 8 and 9).
