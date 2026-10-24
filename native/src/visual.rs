//! Narrow scene classifiers grounded in the observed Steam Global hierarchy.
use crate::model::path_matches;
const UI: &str = "Gallop.GameSystem/SystemManagerRoot/SystemSingleton/UIManager/GameCanvas/MainCanvas/RaceMainView(Clone)";
const COURSE: &str = "RaceScene/RaceManager(Clone)/RaceView/Course";
const EFFECT: &str = "RaceScene/RaceManager(Clone)/RaceView/Effect";
pub fn bottom_hud(path: &str) -> bool {
    path_matches(
        path,
        &format!("{UI}/RaceUILandscape(Clone)/ContentsRoot/Bottom"),
    ) || path_matches(
        path,
        &format!("{UI}/RaceUILandscape(Clone)/ContentsRoot/Btn_PhotoLibrary"),
    )
}
pub fn skill_popup(path: &str) -> bool {
    path_matches(
        path,
        &format!("{UI}/RaceUILandscape(Clone)/ContentsRoot/SkillInfoRoot"),
    ) || path_matches(
        path,
        &format!("{UI}/RaceUILandscape(Clone)/ContentsRoot/MessagePlate"),
    )
}
pub fn runner_marker(path: &str) -> bool {
    path_matches(path, "RaceScene/RaceManager(Clone)/RaceView/marker_player")
}
pub fn tree(path: &str) -> bool {
    path_matches(path, COURSE)
        && path.split('/').any(|s| {
            s.strip_prefix("trees_pack")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
}
pub fn skill_glow(path: &str) -> bool {
    path.strip_prefix(EFFECT)
        .and_then(|p| p.strip_prefix('/'))
        .and_then(|p| p.split('/').next())
        .is_some_and(|p| p.starts_with("pfb_eff_com_skill_") && p.ends_with("(Clone)"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bottom_includes_photo_but_preserves_header() {
        let base = format!("{UI}/RaceUILandscape(Clone)/ContentsRoot");
        assert!(bottom_hud(&format!(
            "{base}/Bottom/JikkyoUILandscape/Message"
        )));
        assert!(bottom_hud(&format!("{base}/Btn_PhotoLibrary")));
        assert!(bottom_hud(&format!("{base}/Btn_PhotoLibrary/Icon")));
        for suffix in ["Header/MiniMap", "Btn_PhotoLibraryExtra", "BottomExtra"] {
            assert!(!bottom_hud(&format!("{base}/{suffix}")));
        }
    }
    #[test]
    fn trees_only_match_course_tree_packs() {
        assert!(tree(&format!(
            "{COURSE}/pfb_env_race(Clone)/_billboard/trees_pack07/tree118"
        )));
        assert!(!tree(&format!("{COURSE}/pfb_env_race(Clone)/track")));
        assert!(!tree("Menu/trees_pack07/tree118"));
        assert!(!tree(&format!("{COURSE}/trees_package/sign")));
    }
    #[test]
    fn skill_glow_excludes_runners_dust_and_ui() {
        assert!(skill_glow(&format!(
            "{EFFECT}/pfb_eff_com_skill_00_000(Clone)/Chest/camera_chest"
        )));
        assert!(!skill_glow(&format!(
            "{EFFECT}/pfb_eff_com_turf_01_01(Clone)/smoke"
        )));
        assert!(!skill_glow(
            "RaceScene/RaceManager(Clone)/RaceView/Horse/Skill Runner/body"
        ));
        assert!(!skill_glow("UI/pfb_eff_com_skill_00_000(Clone)/particle"));
    }
}

#[cfg(test)]
mod popup_marker_tests {
    use super::*;
    #[test]
    fn popup_and_arrow_masks_leave_minimap_and_other_ui_alone() {
        let base = format!("{UI}/RaceUILandscape(Clone)/ContentsRoot");
        assert!(skill_popup(&format!(
            "{base}/SkillInfoRoot/SkillInfoTemplate(Clone)/PlateTemplate(Clone)/Icon"
        )));
        assert!(skill_popup(&format!(
            "{base}/MessagePlate/MessagePlateRoot/PlateTemplate(Clone)/TextMain"
        )));
        for suffix in [
            "SkillInfoRootOther/Icon",
            "Header/MiniMap/HorseIcons/Img_horse_icon_marker",
            "Bottom/Rank",
            "MessagePlateOther/PlateTemplate(Clone)",
        ] {
            assert!(!skill_popup(&format!("{base}/{suffix}")));
            assert!(!runner_marker(&format!("{base}/{suffix}")));
        }
        assert!(runner_marker(
            "RaceScene/RaceManager(Clone)/RaceView/marker_player"
        ));
        assert!(runner_marker(
            "RaceScene/RaceManager(Clone)/RaceView/marker_player/mesh"
        ));
        assert!(!runner_marker(
            "RaceScene/RaceManager(Clone)/RaceView/marker_player_extra"
        ));
        assert!(!runner_marker(
            "RaceScene/RaceManager(Clone)/RaceView/Horse/body"
        ));
    }
}

/// Observed skill particle sub-tree; do not hide track markings or runner meshes.
pub fn speed_line(path: &str) -> bool {
    skill_glow(path) && path.split('/').any(|segment| segment == "Line")
}

#[cfg(test)]
mod cinematic_tests {
    use super::*;
    #[test]
    fn speed_lines_preserve_aura_track_lines_and_other_scenes() {
        let skill = format!("{EFFECT}/pfb_eff_com_skill_00_000(Clone)/Chest/top_chest");
        assert!(speed_line(&format!("{skill}/Line")));
        assert!(speed_line(&format!("{skill}/Line/mesh")));
        assert!(!speed_line(&format!("{skill}/Aura")));
        assert!(!speed_line(&format!("{skill}/LineOther")));
        assert!(!speed_line(&format!("{COURSE}/Line")));
        assert!(!speed_line("Menu/pfb_eff_com_skill_00_000(Clone)/Line"));
    }
}
