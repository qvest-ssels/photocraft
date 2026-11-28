//! #16889: the first ten Window › Arrange entries (Tile All Vertically to Consolidate All to Tabs) show a layout icon;
//! the other nine show none. The menu rows draw the icon their command resolves with, so these tests read what the
//! running app resolves for each entry.

use craft::behavior::IconId;
use craft::keymap::CommandRef;
use craft::ui::primitives::has_icon;
use egui::vec2;
use egui_kittest::Harness;
use serde_json::json;

use crate::PhotocraftApp;
use crate::menu_catalog::{ARRANGE_ICONS, CATALOG};
use crate::theme::AppTheme;

/// The ten entries with a layout icon, in menu order, and the icon of each.
const WITH_ICON: [(&str, &str); 10] = [
    ("window.arrange.tileAllVertically", "arrange-tile-all-vertically"),
    ("window.arrange.tileAllHorizontally", "arrange-tile-all-horizontally"),
    ("window.arrange.twoUpHorizontal", "arrange-two-up-horizontal"),
    ("window.arrange.twoUpVertical", "arrange-two-up-vertical"),
    ("window.arrange.threeUpHorizontal", "arrange-three-up-horizontal"),
    ("window.arrange.threeUpVertical", "arrange-three-up-vertical"),
    ("window.arrange.threeUpStacked", "arrange-three-up-stacked"),
    ("window.arrange.fourUp", "arrange-four-up"),
    ("window.arrange.sixUp", "arrange-six-up"),
    ("window.arrange.consolidateAllToTabs", "arrange-consolidate-all-to-tabs"),
];

/// The nine entries without one.
const WITHOUT_ICON: [&str; 9] = [
    "window.arrange.cascade",
    "window.arrange.tile",
    "window.arrange.floatInWindow",
    "window.arrange.floatAllInWindows",
    "window.arrange.matchZoom",
    "window.arrange.matchLocation",
    "window.arrange.matchRotation",
    "window.arrange.matchAll",
    "window.arrange.newWindowForDocument",
];

/// The running app with `documents` documents open.
fn app(documents: usize) -> Harness<'static, Box<PhotocraftApp>> {
    let mut h = Harness::builder().with_size(vec2(1000.0, 700.0)).with_max_steps(64).build_eframe(move |cc| {
        let theme = AppTheme::pro();
        PhotocraftApp::setup_context(&cc.egui_ctx, theme.clone());
        let mut app = PhotocraftApp::with_context(cc.egui_ctx.clone(), photocraft_engine::Session::new(), crate::Services::default());
        app.save_theme(&theme).unwrap();
        for n in 0..documents {
            app.run("file.new", json!({"width": 200, "height": 100, "name": format!("W{n}")})).unwrap();
        }
        app
    });
    h.run_steps(4);
    h
}

/// The icon the running app resolves command `id` with: the one its menu row draws.
fn icon_of(h: &Harness<'_, Box<PhotocraftApp>>, id: &str) -> Option<String> {
    craft::ui::runtime::commands::resolve(&h.ctx, CommandRef::new(id)).icon.as_ref().map(ToString::to_string)
}

#[test]
fn the_ten_layout_entries_resolve_with_their_layout_icon() {
    for documents in [0, 1, 6] {
        let h = app(documents);
        for (id, icon) in WITH_ICON {
            assert_eq!(icon_of(&h, id).as_deref(), Some(icon), "{documents} documents: {id}");
        }
    }
}

#[test]
fn the_other_nine_entries_have_no_icon() {
    let h = app(2);
    for id in WITHOUT_ICON {
        assert_eq!(icon_of(&h, id), None, "{id} shows no icon");
    }
}

/// The two lists cover every Window › Arrange entry of the catalog, so a new entry has to pick a side here.
#[test]
fn every_arrange_entry_is_listed_once() {
    let catalog: Vec<&str> = CATALOG.iter().filter(|(path, label, _, _)| *path == ["Window", "Arrange"] && *label != "---").map(|row| row.3).collect();
    let mut listed: Vec<&str> = WITH_ICON.iter().map(|(id, _)| *id).chain(WITHOUT_ICON).collect();
    let mut sorted = catalog.clone();
    sorted.sort_unstable();
    listed.sort_unstable();
    assert_eq!(listed, sorted);
    assert_eq!(ARRANGE_ICONS, &WITH_ICON[..], "the icon table holds the ten entries in menu order");
    assert_eq!(&catalog[..10], &WITH_ICON.map(|(id, _)| id)[..], "and they are the menu's first ten entries");
}

/// Every icon is in PhotoCraft's icon set, so the row paints a glyph rather than nothing.
#[test]
fn every_layout_icon_is_registered() {
    crate::icons::register();
    for (_, icon) in WITH_ICON {
        assert!(has_icon(&IconId::new(icon)), "{icon} is registered");
    }
}

/// The icons are files, so each one is listed in ATTRIBUTION.md as original work.
#[test]
fn the_icon_files_are_attributed() {
    let attribution = include_str!("../../../../ATTRIBUTION.md");
    let row = attribution.lines().find(|line| line.contains("assets/icons/arrange-")).expect("ATTRIBUTION.md lists the arrange icons");
    assert!(row.contains("Original work"), "{row}");
    for (_, icon) in WITH_ICON {
        let name = icon.trim_start_matches("arrange-");
        assert!(row.contains(name), "the row names {icon}: {row}");
    }
}
