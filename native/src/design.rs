//! Shared presentation helpers. No game or input ownership logic lives here.
use hudhook::imgui::{StyleColor, Ui};
pub const ACCENT: [f32; 4] = [0.64, 0.71, 1., 1.];
pub const MUTED: [f32; 4] = [0.56, 0.62, 0.73, 1.];
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
    ui.text_colored(MUTED, title);
    ui.spacing();
}
pub fn heading(ui: &Ui, title: &str, subtitle: &str) {
    ui.text_colored(ACCENT, title);
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
    d.add_rect(
        p,
        [p[0] + width, p[1] + height],
        if hover {
            [0.12, 0.15, 0.21, 1.]
        } else {
            [0.085, 0.10, 0.14, 1.]
        },
    )
    .rounding(8.)
    .filled(true)
    .build();
    d.add_rect(
        p,
        [p[0] + width, p[1] + height],
        if focus {
            ACCENT
        } else {
            [0.18, 0.21, 0.29, 1.]
        },
    )
    .rounding(8.)
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
    d.add_text([p[0] + 14., p[1] + 12.], [0.93, 0.95, 1., 1.], label);
    let sx = p[0] + width - 62.;
    let sy = p[1] + 11.;
    d.add_rect(
        [sx, sy],
        [sx + 46., sy + 23.],
        if *value {
            [0.45, 0.54, 0.87, 1.]
        } else {
            [0.22, 0.25, 0.33, 1.]
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
            MUTED,
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
    let _id = ui.push_id(label);
    ui.text_colored(MUTED, label);
    let width = (ui.content_region_avail()[0] - 8.) * 0.5;
    for (i, text) in [left, right].iter().enumerate() {
        if i > 0 {
            ui.same_line_with_spacing(0., 8.);
        }
        let active = *value == (i == 1);
        let _color = ui.push_style_color(
            StyleColor::Button,
            if active {
                [0.28, 0.35, 0.56, 1.]
            } else {
                [0.10, 0.12, 0.17, 1.]
            },
        );
        let _text = ui.push_style_color(
            StyleColor::Text,
            if active { [0.95, 0.96, 1., 1.] } else { MUTED },
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
    let style = ctx.style_mut();
    style.window_rounding = 10.;
    style.child_rounding = 8.;
    style.frame_rounding = 5.;
    style.grab_rounding = 5.;
    style.window_padding = [20., 18.];
    style.frame_padding = [10., 7.];
    style.item_spacing = [10., 9.];
    style.scrollbar_size = 12.;
    style.window_border_size = 1.;
    style.colors[StyleColor::WindowBg as usize] = [0.045, 0.052, 0.073, 0.99];
    style.colors[StyleColor::ChildBg as usize] = [0.055, 0.066, 0.09, 1.];
    style.colors[StyleColor::Text as usize] = [0.91, 0.93, 0.97, 1.];
    style.colors[StyleColor::TextDisabled as usize] = [0.52, 0.58, 0.68, 1.];
    style.colors[StyleColor::Border as usize] = [0.18, 0.21, 0.27, 1.];
    for key in [StyleColor::FrameBg, StyleColor::Button, StyleColor::Header] {
        style.colors[key as usize] = [0.12, 0.15, 0.21, 1.];
    }
    for key in [
        StyleColor::FrameBgHovered,
        StyleColor::ButtonHovered,
        StyleColor::HeaderHovered,
    ] {
        style.colors[key as usize] = [0.22, 0.27, 0.38, 1.];
    }
    for key in [
        StyleColor::CheckMark,
        StyleColor::SliderGrab,
        StyleColor::SliderGrabActive,
    ] {
        style.colors[key as usize] = [0.64, 0.70, 1., 1.];
    }
    style.colors[StyleColor::ButtonActive as usize] = [0.35, 0.42, 0.64, 1.];
    style.colors[StyleColor::HeaderActive as usize] = [0.35, 0.42, 0.64, 1.];
    style.colors[StyleColor::TitleBgActive as usize] = [0.09, 0.11, 0.16, 1.];
}

#[cfg(test)]
mod preview {
    use super::*;
    #[test]
    #[ignore = "manual visual preview export"]
    fn export_ui_preview() {
        use hudhook::imgui::{Condition, DrawCmd};
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../diagnostics");
        let mut ctx = hudhook::imgui::Context::create();
        ctx.set_ini_filename(None);
        theme(&mut ctx);
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
            crate::timing::draw(ui, &rows, &crate::timing::TowerSettings::default(), 3);
            ui.window("RaceDirector Studio").position([520.,38.],Condition::Always).size([940.,810.],Condition::Always).build(||{
                ui.text_colored(ACCENT,"RACE DIRECTOR  /  STUDIO");ui.same_line();ui.text_disabled("v0.1.14  /  60 FPS");
                ui.text_colored([0.46,0.84,0.70,1.],"CONNECTED   /   Race camera & broadcast controls");ui.separator();
                ui.child_window("nav").size([166.,-60.]).build(||{section(ui,"WORKSPACE");for (i,n) in ["Camera","Visuals","Saved shots","Performance","Timing tower","Help"].iter().enumerate(){ui.selectable_config(format!("{:02}   {}",i+1,n)).selected(i==1).size([0.,42.]).build();}section(ui,"SHORTCUTS");ui.text("Insert  Panel");ui.text("F8  Clean lock");ui.text("F10 Restore");});ui.same_line();
                ui.child_window("content").size([0.,-60.]).build(||{heading(ui,"Visuals","Keep the shot clear. Each option works independently.");section(ui,"RACE HUD");toggle(ui,"Hide bottom HUD","Commentary, rank portrait, bottom controls and photo button.",&mut true);toggle(ui,"Hide skill popups + sidebar","Hide runner skill-name plates and the side notification feed.",&mut true);toggle(ui,"Hide runner arrows","Remove the markers above runners; keep minimap icons.",&mut false);section(ui,"CINEMATICS & EFFECTS");toggle(ui,"Skip skill cut-ins","Skip new unique animations. An animation already playing finishes normally.",&mut true);toggle(ui,"Hide speed lines","Remove radial speed blur and skill-line particles.",&mut false);});
                ui.separator();ui.button_with_size("Save settings",[150.,36.]);ui.same_line();ui.button_with_size("Restore visuals  [F10]",[220.,36.]);
            });
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
