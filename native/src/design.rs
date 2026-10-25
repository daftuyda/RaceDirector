//! Shared presentation helpers. No game or input ownership logic lives here.
use hudhook::imgui::{StyleColor, Ui};
pub const ACCENT: [f32; 4] = [0.16, 0.36, 0.83, 1.];
pub fn wrap(ui: &Ui, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.into()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && ui.calc_text_size(&candidate)[0] > width {
            lines.push(line);
            line = word.into();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
pub fn section(ui: &Ui, title: &str) {
    ui.spacing();
    ui.text_colored(ui.style_color(StyleColor::TextDisabled), title);
    ui.spacing();
}
pub fn heading(ui: &Ui, title: &str, subtitle: &str) {
    ui.text_colored(ui.style_color(StyleColor::Text), title);
    ui.text_wrapped(subtitle);
    ui.spacing();
    ui.separator();
    ui.spacing();
}
pub fn toggle(ui: &Ui, title: &str, description: &str, value: &mut bool) -> bool {
    let _id = ui.push_id(title);
    let p = ui.cursor_screen_pos();
    let width = ui.content_region_avail()[0].max(120.);
    let lines = wrap(ui, description, (width - 28.).max(80.));
    let line_h = ui.text_line_height();
    let height = 22. + line_h + lines.len() as f32 * (line_h + 2.) + 12.;
    let clicked = ui.invisible_button("##toggle", [width, height]);
    if clicked {
        *value = !*value;
    }
    let hover = ui.is_item_hovered();
    let focus = ui.is_item_focused();
    let d = ui.get_window_draw_list();
    let accent = ui.style_color(StyleColor::SliderGrab);
    let muted = ui.style_color(StyleColor::TextDisabled);
    if hover {
        d.add_rect(
            p,
            [p[0] + width, p[1] + height],
            ui.style_color(StyleColor::FrameBgHovered),
        )
        .rounding(4.)
        .filled(true)
        .build();
    }
    d.add_line(
        [p[0], p[1] + height],
        [p[0] + width, p[1] + height],
        if focus {
            accent
        } else {
            ui.style_color(StyleColor::Border)
        },
    )
    .build();
    let mut label = title.to_owned();
    while !label.is_empty() && ui.calc_text_size(&label)[0] > width - 94. {
        label.pop();
    }
    if label != title {
        while !label.is_empty() && ui.calc_text_size(&format!("{label}..."))[0] > width - 94. {
            label.pop();
        }
        label.push_str("...");
    }
    d.add_text(
        [p[0] + 8., p[1] + 12.],
        ui.style_color(StyleColor::Text),
        label,
    );
    let sx = p[0] + width - 62.;
    let sy = p[1] + 11.;
    d.add_rect(
        [sx, sy],
        [sx + 46., sy + 23.],
        if *value {
            accent
        } else {
            ui.style_color(StyleColor::FrameBgActive)
        },
    )
    .rounding(12.)
    .filled(true)
    .build();
    d.add_circle(
        [sx + if *value { 34.5 } else { 11.5 }, sy + 11.5],
        8.,
        [0.95, 0.96, 1., 1.],
    )
    .filled(true)
    .build();
    for (i, line) in lines.iter().enumerate() {
        d.add_text(
            [p[0] + 14., p[1] + 22. + line_h + i as f32 * (line_h + 2.)],
            muted,
            line,
        );
    }
    if hover && label_truncated(ui, title, width - 94.) {
        ui.tooltip_text(title);
    }
    clicked
}
fn label_truncated(ui: &Ui, title: &str, width: f32) -> bool {
    ui.calc_text_size(title)[0] > width
}
pub fn choice(ui: &Ui, label: &str, left: &str, right: &str, value: &mut bool) {
    let _id = ui.push_id(if label.is_empty() { "##choice" } else { label });
    if !label.is_empty() && !label.starts_with("##") {
        ui.text_colored(ui.style_color(StyleColor::TextDisabled), label);
    }
    let width = (ui.content_region_avail()[0] - 8.) * 0.5;
    for (i, text) in [left, right].iter().enumerate() {
        if i > 0 {
            ui.same_line_with_spacing(0., 8.);
        }
        let active = *value == (i == 1);
        let _color = ui.push_style_color(
            StyleColor::Button,
            if active {
                ui.style_color(StyleColor::SliderGrab)
            } else {
                ui.style_color(StyleColor::FrameBg)
            },
        );
        let _text = ui.push_style_color(
            StyleColor::Text,
            if active {
                [1., 1., 1., 1.]
            } else {
                ui.style_color(StyleColor::Text)
            },
        );
        if ui.button_with_size(text, [width, 34.]) {
            *value = i == 1;
        }
    }
}

pub fn theme(ctx: &mut hudhook::imgui::Context) {
    use hudhook::imgui::{FontConfig, FontSource};
    let windows = std::env::var_os("WINDIR")
        .or_else(|| std::env::var_os("SystemRoot"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:/Windows"));
    let font = std::fs::read(windows.join("Fonts/segoeui.ttf")).ok();
    ctx.fonts().clear();
    if let Some(data) = font.as_ref() {
        ctx.fonts().add_font(&[FontSource::TtfData {
            data,
            size_pixels: 18.,
            config: Some(FontConfig {
                oversample_h: 2,
                oversample_v: 2,
                ..FontConfig::default()
            }),
        }]);
    } else {
        ctx.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(FontConfig {
                size_pixels: 17.,
                ..FontConfig::default()
            }),
        }]);
    }
    apply_theme(ctx, false);
}
/// Update appearance without rebuilding the font atlas or touching game state.
pub fn apply_theme(ctx: &mut hudhook::imgui::Context, dark: bool) {
    let style = ctx.style_mut();
    if dark {
        style.use_dark_colors();
    } else {
        style.use_light_colors();
    }
    style.window_rounding = 8.;
    style.child_rounding = 0.;
    style.frame_rounding = 4.;
    style.grab_rounding = 8.;
    style.window_padding = [20., 18.];
    style.frame_padding = [10., 6.];
    style.item_spacing = [10., 8.];
    style.scrollbar_size = 9.;
    style.window_border_size = 1.;
    style.child_border_size = 0.;
    let bg = if dark {
        [0.09, 0.105, 0.13, 0.99]
    } else {
        [0.965, 0.96, 0.945, 0.99]
    };
    let text = if dark {
        [0.94, 0.95, 0.975, 1.]
    } else {
        [0.075, 0.09, 0.12, 1.]
    };
    let muted = if dark {
        [0.65, 0.70, 0.77, 1.]
    } else {
        [0.36, 0.39, 0.44, 1.]
    };
    let border = if dark {
        [0.22, 0.25, 0.30, 1.]
    } else {
        [0.80, 0.81, 0.82, 1.]
    };
    // White foreground on cobalt meets contrast in both modes.
    let accent = if dark { [0.20, 0.40, 0.88, 1.] } else { ACCENT };
    let frame = if dark {
        [0.145, 0.165, 0.20, 1.]
    } else {
        [0.89, 0.90, 0.91, 1.]
    };
    let hover = if dark {
        [0.19, 0.23, 0.30, 1.]
    } else {
        [0.85, 0.89, 0.95, 1.]
    };
    style.colors[StyleColor::WindowBg as usize] = bg;
    style.colors[StyleColor::ChildBg as usize] = [0., 0., 0., 0.];
    style.colors[StyleColor::PopupBg as usize] = [bg[0], bg[1], bg[2], 1.];
    style.colors[StyleColor::Text as usize] = text;
    style.colors[StyleColor::TextDisabled as usize] = muted;
    style.colors[StyleColor::Border as usize] = border;
    style.colors[StyleColor::Separator as usize] = border;
    for key in [
        StyleColor::FrameBg,
        StyleColor::Button,
        StyleColor::Header,
        StyleColor::TitleBg,
        StyleColor::TitleBgActive,
        StyleColor::Tab,
    ] {
        style.colors[key as usize] = frame;
    }
    for key in [
        StyleColor::FrameBgHovered,
        StyleColor::ButtonHovered,
        StyleColor::HeaderHovered,
        StyleColor::TabHovered,
    ] {
        style.colors[key as usize] = hover;
    }
    for key in [
        StyleColor::FrameBgActive,
        StyleColor::ButtonActive,
        StyleColor::HeaderActive,
        StyleColor::TabActive,
    ] {
        style.colors[key as usize] = hover;
    }
    for key in [
        StyleColor::CheckMark,
        StyleColor::SliderGrab,
        StyleColor::SliderGrabActive,
        StyleColor::NavHighlight,
        StyleColor::SeparatorActive,
        StyleColor::SeparatorHovered,
    ] {
        style.colors[key as usize] = accent;
    }
    style.colors[StyleColor::TextSelectedBg as usize] = [accent[0], accent[1], accent[2], 0.3];
}

#[cfg(test)]
mod preview {
    use super::*;
    #[test]
    #[ignore = "manual visual preview export"]
    fn export_ui_preview() {
        use hudhook::imgui::DrawCmd;
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../diagnostics");
        let mut ctx = hudhook::imgui::Context::create();
        ctx.set_ini_filename(None);
        theme(&mut ctx);
        let dark = std::env::var("RACEDIRECTOR_PREVIEW_DARK").ok().as_deref() == Some("1");
        apply_theme(&mut ctx, dark);
        let fonts = ctx.fonts();
        let atlas = fonts.build_rgba32_texture();
        let aw = atlas.width;
        let ah = atlas.height;
        std::fs::write(dir.join("ui-preview-atlas.rgba"), atlas.data).unwrap();
        ctx.io_mut().display_size = [1500., 900.];
        ctx.io_mut().delta_time = 1. / 60.;
        let names = [
            "Oguri Cap",
            "Agnes Digital",
            "Mejiro Dober",
            "Air Groove",
            "El Condor Pasa",
            "Taiki Shuttle",
            "Silence Suzuka",
            "Tokai Teio",
            "Mejiro McQueen",
            "Gold Ship",
            "Narita Brian",
            "Rice Shower",
            "Grass Wonder",
            "Special Week",
            "Daiwa Scarlet",
            "Vodka",
            "Haru Urara",
            "Sakura Bakushin O",
        ];
        let rows = names
            .iter()
            .enumerate()
            .map(|(i, n)| crate::timing::Runner {
                gate: (i + 1) as i32,
                name: n.to_string(),
                rank: Some(i as i32 + 1),
                distance: Some(1500. - i as f32 * 1.7),
                speed: Some(21.3 - i as f32 * 0.07),
            })
            .collect::<Vec<_>>();
        // Warm-up frame lets ImGui establish window sizes before capturing geometry.
        for _ in 0..2 {
            let ui = ctx.frame();
            crate::overlay::draw_preview(ui, dark, &rows);
            let data = ctx.render();
            let mut lists = Vec::new();
            for list in data.draw_lists() {
                let vertices = list
                    .vtx_buffer()
                    .iter()
                    .map(|v| serde_json::json!([v.pos[0], v.pos[1], v.uv[0], v.uv[1], v.col]))
                    .collect::<Vec<_>>();
                let commands=list.commands().filter_map(|cmd|if let DrawCmd::Elements{count,cmd_params}=cmd{Some(serde_json::json!({"count":count,"idx":cmd_params.idx_offset,"vtx":cmd_params.vtx_offset,"clip":cmd_params.clip_rect}))}else{None}).collect::<Vec<_>>();
                lists.push(serde_json::json!({"vertices":vertices,"indices":list.idx_buffer(),"commands":commands}));
            }
            std::fs::write(dir.join("ui-preview-geometry.json"),serde_json::to_vec(&serde_json::json!({"width":1500,"height":900,"atlasWidth":aw,"atlasHeight":ah,"lists":lists})).unwrap()).unwrap();
        }
    }
}
