//! Standalone Settings popup window (`620×420`, Option A macOS System Settings sidebar).
//! Pure geometry, hit-testing, position clamping, and switch springs live alongside the Win32
//! layered-window implementation so every layout boundary is unit-tested without an HWND.

pub const SET_WIN_W: f64 = 620.;
pub const SET_WIN_H: f64 = 420.;
pub const TITLE_H: f64 = 40.;
pub const SIDEBAR_W: f64 = 168.;
pub const CONTENT_X: f64 = 188.;
pub const CONTENT_W: f64 = 412.;

pub const CLOSE_X: f64 = 584.;
pub const CLOSE_Y: f64 = 8.;
pub const CLOSE_SIZE: f64 = 24.;

pub const NAV_X: f64 = 10.;
pub const NAV_W: f64 = 148.;
pub const NAV_Y0: f64 = 50.;
pub const NAV_H: f64 = 34.;
pub const NAV_STEP: f64 = 38.;

pub const QUIT_X: f64 = 12.;
pub const QUIT_Y: f64 = 376.;
pub const QUIT_W: f64 = 144.;
pub const QUIT_H: f64 = 30.;

pub const SWITCH_CARD_Y: f64 = 120.;
pub const SWITCH_ROW_H: f64 = 54.;

pub const STYLE_CARD_Y: f64 = 114.;
pub const STYLE_CARD_H: f64 = 68.;
pub const STYLE_TILE_PAD: f64 = 8.;
pub const STYLE_TILE_GAP: f64 = 6.;

pub const OPT_CARD_Y: f64 = 194.;
pub const OPT_ROW_H: f64 = 44.;
pub const BLUR_ROW_H: f64 = 54.;
pub const BLUR_TRACK_X0: f64 = CONTENT_X + 14.;
pub const BLUR_TRACK_X1: f64 = CONTENT_X + CONTENT_W - 14.;

pub const UPD_CARD_Y: f64 = 114.;
pub const UPD_AUTO_H: f64 = 52.;
pub const UPD_BODY_H: f64 = 148.;
pub const UPD_ACTION_X: f64 = CONTENT_X + 14.;
pub const UPD_ACTION_Y: f64 = 272.;
pub const UPD_ACTION_W: f64 = 160.;
pub const UPD_ACTION_H: f64 = 30.;
pub const UPD_CANCEL_X: f64 = 372.;
pub const UPD_CANCEL_W: f64 = 88.;

pub const HIT_NONE: i32 = -1;
pub const HIT_TITLEBAR: i32 = 1;
pub const HIT_CLOSE: i32 = 2;
pub const HIT_SWITCH_BASE: i32 = 5000;
pub const HIT_NAV_BASE: i32 = 5100;
pub const HIT_QUIT: i32 = 5200;
pub const HIT_BG_STYLE_BASE: i32 = 5300;
pub const HIT_BG_OPACITY_BASE: i32 = 5310;
pub const HIT_BG_CHOOSE: i32 = 5320;
pub const HIT_BG_RESET: i32 = 5321;
pub const HIT_BG_POS_BASE: i32 = 5330;
pub const HIT_BG_BLUR: i32 = 5340;
pub const HIT_UPD_ACTION: i32 = 5400;
pub const HIT_UPD_AUTO: i32 = 5401;
pub const HIT_UPD_CANCEL: i32 = 5402;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsSection {
    Notch = 0,
    General = 1,
    Appearance = 2,
    Updates = 3,
}

impl SettingsSection {
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Notch,
            1 => Self::General,
            2 => Self::Appearance,
            _ => Self::Updates,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HitContext {
    pub section: SettingsSection,
    pub has_custom_image: bool,
    pub bg_style: u32,
    pub update_action_enabled: bool,
    pub update_downloading: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct AppearanceLayout {
    pub image_row_y: Option<f64>,
    pub framing_row_y: Option<f64>,
    pub blur_row_y: Option<f64>,
    pub opacity_row_y: f64,
    pub total_h: f64,
}

pub fn appearance_layout(has_custom_image: bool, bg_style: u32) -> AppearanceLayout {
    let mut y = OPT_CARD_Y;
    let image_row_y = if bg_style == 4 || has_custom_image {
        let cur = y;
        y += OPT_ROW_H;
        Some(cur)
    } else {
        None
    };
    let framing_row_y = if bg_style == 4 {
        let cur = y;
        y += OPT_ROW_H;
        Some(cur)
    } else {
        None
    };
    let blur_row_y = if bg_style != 3 {
        let cur = y;
        y += BLUR_ROW_H;
        Some(cur)
    } else {
        None
    };
    let opacity_row_y = y;
    y += OPT_ROW_H;
    AppearanceLayout {
        image_row_y,
        framing_row_y,
        blur_row_y,
        opacity_row_y,
        total_h: y - OPT_CARD_Y,
    }
}

pub fn style_tile_rect(i: usize) -> (f64, f64, f64, f64) {
    let tile_w = (CONTENT_W - 2. * STYLE_TILE_PAD - 4. * STYLE_TILE_GAP) / 5.;
    let x = CONTENT_X + STYLE_TILE_PAD + i as f64 * (tile_w + STYLE_TILE_GAP);
    (x, STYLE_CARD_Y + 8., tile_w, STYLE_CARD_H - 16.)
}

pub fn opacity_pill_rect(row_y: f64, i: usize) -> (f64, f64, f64, f64) {
    let start_x = CONTENT_X + CONTENT_W - 14. - 4. * 52. - 3. * 4.;
    (start_x + i as f64 * 56., row_y + 10., 52., 24.)
}

pub fn framing_pill_rect(row_y: f64, i: usize) -> (f64, f64, f64, f64) {
    let start_x = CONTENT_X + CONTENT_W - 14. - 3. * 58. - 2. * 4.;
    (start_x + i as f64 * 62., row_y + 10., 58., 24.)
}

pub fn image_button_rects(
    row_y: f64,
    has_custom_image: bool,
) -> ((f64, f64, f64, f64), Option<(f64, f64, f64, f64)>) {
    let right = CONTENT_X + CONTENT_W - 14.;
    if has_custom_image {
        let reset = (right - 56., row_y + 10., 56., 24.);
        let choose = (reset.0 - 6. - 74., row_y + 10., 74., 24.);
        (choose, Some(reset))
    } else {
        let choose = (right - 74., row_y + 10., 74., 24.);
        (choose, None)
    }
}

fn in_rect(x: f64, y: f64, r: (f64, f64, f64, f64)) -> bool {
    (r.0..r.0 + r.2).contains(&x) && (r.1..r.1 + r.3).contains(&y)
}

pub fn hit_test(ctx: HitContext, x: f64, y: f64) -> i32 {
    if !(0. ..SET_WIN_W).contains(&x) || !(0. ..SET_WIN_H).contains(&y) {
        return HIT_NONE;
    }
    // Titlebar
    if y < TITLE_H {
        if in_rect(x, y, (CLOSE_X, CLOSE_Y, CLOSE_SIZE, CLOSE_SIZE)) {
            return HIT_CLOSE;
        }
        return HIT_TITLEBAR;
    }
    // Left sidebar
    if x < SIDEBAR_W {
        for i in 0..4 {
            let ny = NAV_Y0 + i as f64 * NAV_STEP;
            if in_rect(x, y, (NAV_X, ny, NAV_W, NAV_H)) {
                return HIT_NAV_BASE + i as i32;
            }
        }
        if in_rect(x, y, (QUIT_X, QUIT_Y, QUIT_W, QUIT_H)) {
            return HIT_QUIT;
        }
        return HIT_NONE;
    }
    // Right content pane
    match ctx.section {
        SettingsSection::Notch => {
            for k in 0..4 {
                let ry = SWITCH_CARD_Y + k as f64 * SWITCH_ROW_H;
                if in_rect(x, y, (CONTENT_X, ry, CONTENT_W, SWITCH_ROW_H)) {
                    return HIT_SWITCH_BASE + k as i32;
                }
            }
            HIT_NONE
        }
        SettingsSection::General => {
            for k in 0..3 {
                let ry = SWITCH_CARD_Y + k as f64 * SWITCH_ROW_H;
                if in_rect(x, y, (CONTENT_X, ry, CONTENT_W, SWITCH_ROW_H)) {
                    return HIT_SWITCH_BASE + 4 + k as i32;
                }
            }
            HIT_NONE
        }
        SettingsSection::Appearance => {
            for i in 0..5 {
                if in_rect(x, y, style_tile_rect(i)) {
                    return HIT_BG_STYLE_BASE + i as i32;
                }
            }
            let layout = appearance_layout(ctx.has_custom_image, ctx.bg_style);
            if let Some(ry) = layout.image_row_y {
                let (choose_r, reset_r) = image_button_rects(ry, ctx.has_custom_image);
                if in_rect(x, y, choose_r) {
                    return HIT_BG_CHOOSE;
                }
                if let Some(rr) = reset_r {
                    if in_rect(x, y, rr) {
                        return HIT_BG_RESET;
                    }
                }
            }
            if let Some(ry) = layout.framing_row_y {
                for i in 0..3 {
                    if in_rect(x, y, framing_pill_rect(ry, i)) {
                        return HIT_BG_POS_BASE + i as i32;
                    }
                }
            }
            if let Some(ry) = layout.blur_row_y {
                if (BLUR_TRACK_X0 - 8. ..BLUR_TRACK_X1 + 8.).contains(&x)
                    && (ry + 20. ..ry + BLUR_ROW_H).contains(&y)
                {
                    return HIT_BG_BLUR;
                }
            }
            for i in 0..4 {
                if in_rect(x, y, opacity_pill_rect(layout.opacity_row_y, i)) {
                    return HIT_BG_OPACITY_BASE + i as i32;
                }
            }
            HIT_NONE
        }
        SettingsSection::Updates => {
            if in_rect(x, y, (CONTENT_X, UPD_CARD_Y, CONTENT_W, UPD_AUTO_H)) {
                return HIT_UPD_AUTO;
            }
            if in_rect(
                x,
                y,
                (UPD_ACTION_X, UPD_ACTION_Y, UPD_ACTION_W, UPD_ACTION_H),
            ) {
                return if ctx.update_action_enabled {
                    HIT_UPD_ACTION
                } else {
                    HIT_NONE
                };
            }
            if ctx.update_downloading
                && in_rect(
                    x,
                    y,
                    (UPD_CANCEL_X, UPD_ACTION_Y, UPD_CANCEL_W, UPD_ACTION_H),
                )
            {
                return HIT_UPD_CANCEL;
            }
            HIT_NONE
        }
    }
}

pub fn blur_from_x(x: f64) -> u32 {
    let frac = ((x - BLUR_TRACK_X0) / (BLUR_TRACK_X1 - BLUR_TRACK_X0)).clamp(0., 1.);
    (frac * 30.).round() as u32
}

pub fn clamp_window_pos(
    x: i32,
    y: i32,
    win_w: i32,
    win_h: i32,
    work: (i32, i32, i32, i32),
) -> (i32, i32) {
    let (left, top, right, bottom) = work;
    let max_x = (right - win_w).max(left);
    let max_y = (bottom - win_h).max(top);
    (x.clamp(left, max_x), y.clamp(top, max_y))
}

pub fn advance_switch(
    pos: &mut f64,
    vel: &mut f64,
    target: bool,
    reduced_motion: bool,
    dt: f64,
) -> bool {
    let goal = if target { 1. } else { 0. };
    if reduced_motion {
        let moving = *pos != goal || *vel != 0.;
        *pos = goal;
        *vel = 0.;
        moving
    } else {
        crate::spring(pos, vel, goal, 26., 0.9, dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(section: SettingsSection) -> HitContext {
        HitContext {
            section,
            has_custom_image: false,
            bg_style: 3,
            update_action_enabled: true,
            update_downloading: false,
        }
    }

    #[test]
    fn titlebar_and_sidebar_hit_targets() {
        let c = ctx(SettingsSection::Notch);
        assert_eq!(hit_test(c, 596., 20.), HIT_CLOSE);
        assert_eq!(hit_test(c, 200., 20.), HIT_TITLEBAR);
        for i in 0..4 {
            assert_eq!(
                hit_test(c, 80., NAV_Y0 + i as f64 * NAV_STEP + 16.),
                HIT_NAV_BASE + i as i32
            );
        }
        assert_eq!(hit_test(c, 84., 390.), HIT_QUIT);
        assert_eq!(hit_test(c, -1., 20.), HIT_NONE);
        assert_eq!(hit_test(c, SET_WIN_W + 1., 20.), HIT_NONE);
        assert_eq!(hit_test(c, 84., SET_WIN_H + 1.), HIT_NONE);
    }

    #[test]
    fn notch_and_general_switch_hit_targets() {
        let notch = ctx(SettingsSection::Notch);
        for k in 0..4 {
            assert_eq!(
                hit_test(notch, 350., SWITCH_CARD_Y + k as f64 * SWITCH_ROW_H + 20.),
                HIT_SWITCH_BASE + k as i32
            );
        }
        assert_eq!(
            hit_test(notch, 350., SWITCH_CARD_Y + 4. * SWITCH_ROW_H + 20.),
            HIT_NONE
        );

        let general = ctx(SettingsSection::General);
        for k in 0..3 {
            assert_eq!(
                hit_test(general, 350., SWITCH_CARD_Y + k as f64 * SWITCH_ROW_H + 20.),
                HIT_SWITCH_BASE + 4 + k as i32
            );
        }
        assert_eq!(
            hit_test(general, 350., SWITCH_CARD_Y + 3. * SWITCH_ROW_H + 20.),
            HIT_NONE
        );
    }

    #[test]
    fn appearance_conditional_hit_targets_and_blur_slider() {
        // OLED hides Blur and Image/Framing rows; only Opacity row appears in options card
        let oled = HitContext {
            section: SettingsSection::Appearance,
            has_custom_image: false,
            bg_style: 3,
            update_action_enabled: true,
            update_downloading: false,
        };
        let oled_layout = appearance_layout(oled.has_custom_image, oled.bg_style);
        assert!(oled_layout.blur_row_y.is_none());
        assert!(oled_layout.image_row_y.is_none());
        assert!(oled_layout.framing_row_y.is_none());
        for i in 0..5 {
            let tile_w = (CONTENT_W - 2. * STYLE_TILE_PAD - 4. * STYLE_TILE_GAP) / 5.;
            let tx =
                CONTENT_X + STYLE_TILE_PAD + i as f64 * (tile_w + STYLE_TILE_GAP) + tile_w * 0.5;
            assert_eq!(
                hit_test(oled, tx, STYLE_CARD_Y + 34.),
                HIT_BG_STYLE_BASE + i as i32
            );
        }
        // Opacity pills (70, 80, 90, 100) right-aligned in opacity row
        for i in 0..4 {
            let ox = CONTENT_X + CONTENT_W - 14. - 4. * 52. - 3. * 4. + i as f64 * 56. + 26.;
            assert_eq!(
                hit_test(oled, ox, oled_layout.opacity_row_y + 22.),
                HIT_BG_OPACITY_BASE + i as i32
            );
        }
        // No blur slider hit on OLED
        assert_eq!(
            hit_test(oled, 300., oled_layout.opacity_row_y + 50.),
            HIT_NONE
        );

        // Frost shows Blur slider row
        let frost = HitContext {
            bg_style: 0,
            ..oled
        };
        let frost_layout = appearance_layout(frost.has_custom_image, frost.bg_style);
        let blur_y = frost_layout.blur_row_y.expect("Frost shows blur slider");
        assert_eq!(hit_test(frost, 300., blur_y + 34.), HIT_BG_BLUR);

        // Image without custom image: Choose visible, Reset hidden, Framing visible, Blur visible
        let img_empty = HitContext {
            bg_style: 4,
            has_custom_image: false,
            ..oled
        };
        let l_empty = appearance_layout(false, 4);
        let img_y = l_empty.image_row_y.expect("Image row shown");
        let frame_y = l_empty.framing_row_y.expect("Framing row shown");
        assert_eq!(
            hit_test(img_empty, CONTENT_X + CONTENT_W - 44., img_y + 22.),
            HIT_BG_CHOOSE
        );
        assert_eq!(
            hit_test(img_empty, CONTENT_X + CONTENT_W - 110., img_y + 22.),
            HIT_NONE
        );
        for i in 0..3 {
            let fx = CONTENT_X + CONTENT_W - 14. - 3. * 58. - 2. * 4. + i as f64 * 62. + 29.;
            assert_eq!(
                hit_test(img_empty, fx, frame_y + 22.),
                HIT_BG_POS_BASE + i as i32
            );
        }

        // Image with custom image: both Choose and Reset visible
        let img_loaded = HitContext {
            bg_style: 4,
            has_custom_image: true,
            ..oled
        };
        assert_eq!(
            hit_test(img_loaded, CONTENT_X + CONTENT_W - 108., img_y + 22.),
            HIT_BG_CHOOSE
        );
        assert_eq!(
            hit_test(img_loaded, CONTENT_X + CONTENT_W - 38., img_y + 22.),
            HIT_BG_RESET
        );

        // Blur mapping bounds
        assert_eq!(blur_from_x(BLUR_TRACK_X0 - 20.), 0);
        assert_eq!(blur_from_x(BLUR_TRACK_X0), 0);
        assert_eq!(blur_from_x((BLUR_TRACK_X0 + BLUR_TRACK_X1) * 0.5), 15);
        assert_eq!(blur_from_x(BLUR_TRACK_X1), 30);
        assert_eq!(blur_from_x(BLUR_TRACK_X1 + 20.), 30);
    }

    #[test]
    fn updates_conditional_hit_targets() {
        let idle = ctx(SettingsSection::Updates);
        assert_eq!(hit_test(idle, 350., UPD_CARD_Y + 26.), HIT_UPD_AUTO);
        assert_eq!(
            hit_test(idle, UPD_ACTION_X + 60., UPD_ACTION_Y + 15.),
            HIT_UPD_ACTION
        );
        assert_eq!(
            hit_test(idle, UPD_CANCEL_X + 40., UPD_ACTION_Y + 15.),
            HIT_NONE
        );

        let downloading = HitContext {
            update_action_enabled: false,
            update_downloading: true,
            ..idle
        };
        assert_eq!(
            hit_test(downloading, UPD_ACTION_X + 60., UPD_ACTION_Y + 15.),
            HIT_NONE
        );
        assert_eq!(
            hit_test(downloading, UPD_CANCEL_X + 40., UPD_ACTION_Y + 15.),
            HIT_UPD_CANCEL
        );
    }

    #[test]
    fn clamp_window_pos_handles_primary_and_negative_secondary_monitors() {
        // Primary monitor (0, 0, 1920, 1040)
        assert_eq!(
            clamp_window_pos(650, 310, 620, 420, (0, 0, 1920, 1040)),
            (650, 310)
        );
        assert_eq!(
            clamp_window_pos(-100, -50, 620, 420, (0, 0, 1920, 1040)),
            (0, 0)
        );
        assert_eq!(
            clamp_window_pos(1800, 900, 620, 420, (0, 0, 1920, 1040)),
            (1300, 620)
        );

        // Secondary monitor to the left (-1920, 0, 0, 1080)
        assert_eq!(
            clamp_window_pos(-1500, 200, 620, 420, (-1920, 0, 0, 1080)),
            (-1500, 200)
        );
        assert_eq!(
            clamp_window_pos(-2100, 900, 620, 420, (-1920, 0, 0, 1080)),
            (-1920, 660)
        );
        assert_eq!(
            clamp_window_pos(-200, -10, 620, 420, (-1920, 0, 0, 1080)),
            (-620, 0)
        );

        // Oversized window falls back cleanly to work area origin
        assert_eq!(
            clamp_window_pos(100, 100, 800, 600, (10, 20, 500, 400)),
            (10, 20)
        );
    }

    #[test]
    fn auto_switch_spring_animates_and_snaps_on_reduced_motion() {
        let (mut pos, mut vel) = (0., 0.);
        assert!(advance_switch(&mut pos, &mut vel, true, false, 0.016));
        assert!(pos > 0. && pos < 1.);

        assert!(advance_switch(&mut pos, &mut vel, true, true, 0.016));
        assert_eq!(pos, 1.);
        assert_eq!(vel, 0.);
        assert!(!advance_switch(&mut pos, &mut vel, true, true, 0.016));
    }
}
