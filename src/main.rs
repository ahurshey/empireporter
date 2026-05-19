// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 Alex Hurshman
//
// This file is part of CivShare.
//
// CivShare is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 only.
//
// CivShare is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
// A PARTICULAR PURPOSE. See the GNU General Public License for more details.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod io_ops;
mod parser;

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use bevy::input::InputSystem;
use bevy::log::warn;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bevy::winit::WinitWindows;
use bevy_egui::{EguiContexts, EguiPlugin, EguiSet, EguiSettings, egui};
use io_ops::{
    ConflictPolicy, export_selected_bundle, format_empire_bundle, import_selected_to_file,
    load_empire_file,
};
use parser::{EmpireDesign, has_same_identity};
use winit::window::Icon;

const APP_ICON_BYTES: &[u8] = include_bytes!("../assets/app_icon.png");
const STELLARIS_DESIGNS_RELATIVE_PATH: &[&str] = &[
    "Documents",
    "Paradox Interactive",
    "Stellaris",
    "user_empire_designs_v3.4.txt",
];
const CIVSHARE_EXPORT_FILE_NAME: &str = "civshare_export.txt";
const BASE_EGUI_SCALE: f32 = 0.7;
const DEFAULT_UI_ZOOM: f32 = 1.0;
const MIN_UI_ZOOM: f32 = 0.6;
const MAX_UI_ZOOM: f32 = 2.0;
const UI_ZOOM_STEP: f32 = 0.1;
const DEFAULT_SPLIT_FRACTION: f32 = 0.65;
const MIN_SPLIT_FRACTION: f32 = 0.35;
const MAX_SPLIT_FRACTION: f32 = 0.82;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "CivShare - Stellaris Empire Import/Export".to_owned(),
                resolution: WindowResolution::new(1200.0, 800.0),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .init_resource::<CivShareState>()
        .add_systems(
            PreUpdate,
            sync_egui_scale_system
                .after(InputSystem)
                .before(EguiSet::InitContexts),
        )
        .add_systems(Update, set_window_icon_system)
        .add_systems(Update, ui_system)
        .run();
}

fn set_window_icon_system(
    primary_window: Query<Entity, With<PrimaryWindow>>,
    winit_windows: NonSend<WinitWindows>,
    mut icon_applied: Local<bool>,
) {
    if *icon_applied {
        return;
    }

    let Ok(window_entity) = primary_window.get_single() else {
        return;
    };
    let Some(winit_window) = winit_windows.get_window(window_entity) else {
        return;
    };

    match load_window_icon() {
        Ok(icon) => winit_window.set_window_icon(Some(icon)),
        Err(err) => warn!("Could not load embedded app icon: {err}"),
    }

    *icon_applied = true;
}

fn load_window_icon() -> Result<Icon, String> {
    let image = image::load_from_memory(APP_ICON_BYTES)
        .map_err(|err| err.to_string())?
        .into_rgba8();
    let (width, height) = image.dimensions();

    Icon::from_rgba(image.into_raw(), width, height).map_err(|err| err.to_string())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ActiveTab {
    #[default]
    Export,
    Import,
}

#[derive(Resource)]
struct CivShareState {
    active_tab: ActiveTab,
    status: String,
    ui_zoom: f32,
    split_fraction: f32,
    style_configured: bool,
    export_path: Option<PathBuf>,
    export_empires: Vec<EmpireDesign>,
    export_selected: BTreeSet<usize>,
    import_source_path: Option<PathBuf>,
    import_source_empires: Vec<EmpireDesign>,
    import_source_selected: BTreeSet<usize>,
    import_target_path: Option<PathBuf>,
    import_target_empires: Vec<EmpireDesign>,
    conflict_policy: ConflictPolicy,
}

impl Default for CivShareState {
    fn default() -> Self {
        Self {
            active_tab: ActiveTab::Export,
            status: String::new(),
            ui_zoom: DEFAULT_UI_ZOOM,
            split_fraction: DEFAULT_SPLIT_FRACTION,
            style_configured: false,
            export_path: None,
            export_empires: Vec::new(),
            export_selected: BTreeSet::new(),
            import_source_path: None,
            import_source_empires: Vec::new(),
            import_source_selected: BTreeSet::new(),
            import_target_path: None,
            import_target_empires: Vec::new(),
            conflict_policy: ConflictPolicy::Skip,
        }
    }
}

fn ui_system(
    mut contexts: EguiContexts,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CivShareState>,
) {
    let ctx = contexts.ctx_mut();
    configure_readable_ui(ctx, &mut state);
    handle_zoom_shortcuts(&keyboard, &mut state);

    egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("CivShare").size(24.0));
            ui.add_space(16.0);
            tab_button(ui, &mut state.active_tab, ActiveTab::Export, "Export");
            ui.add_space(2.0);
            tab_button(ui, &mut state.active_tab, ActiveTab::Import, "Import");
            ui.add_space(14.0);
            zoom_controls(ui, &mut state);
        });
    });

    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(if state.status.is_empty() {
                "Open a Stellaris user_empire_designs file to begin."
            } else {
                &state.status
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("CivShare v{}", env!("CARGO_PKG_VERSION")));
            });
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| match state.active_tab {
        ActiveTab::Export => export_tab(ui, &mut state),
        ActiveTab::Import => import_tab(ui, &mut state),
    });
}

fn sync_egui_scale_system(state: Res<CivShareState>, mut egui_settings: ResMut<EguiSettings>) {
    let scale_factor = BASE_EGUI_SCALE * state.ui_zoom.clamp(MIN_UI_ZOOM, MAX_UI_ZOOM);
    if (egui_settings.scale_factor - scale_factor).abs() > f32::EPSILON {
        egui_settings.scale_factor = scale_factor;
    }
}

fn configure_readable_ui(ctx: &egui::Context, state: &mut CivShareState) {
    if state.style_configured {
        return;
    }

    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(25.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(16.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(16.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(14.5, egui::FontFamily::Monospace),
    );
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    ctx.set_style(style);

    state.style_configured = true;
}

fn handle_zoom_shortcuts(keyboard: &ButtonInput<KeyCode>, state: &mut CivShareState) {
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        adjust_ui_zoom(state, -UI_ZOOM_STEP);
    }
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        adjust_ui_zoom(state, UI_ZOOM_STEP);
    }
}

fn adjust_ui_zoom(state: &mut CivShareState, delta: f32) {
    state.ui_zoom = (state.ui_zoom + delta).clamp(MIN_UI_ZOOM, MAX_UI_ZOOM);
    state.status = format!("UI zoom set to {}%", zoom_percent(state.ui_zoom));
}

fn zoom_percent(zoom: f32) -> u32 {
    (zoom * 100.0).round() as u32
}

fn zoom_controls(ui: &mut egui::Ui, state: &mut CivShareState) {
    ui.label("Zoom: + / -");
    if ui
        .add(
            egui::Button::new(egui::RichText::new("-").size(18.0).strong())
                .min_size(egui::vec2(38.0, 34.0)),
        )
        .clicked()
    {
        adjust_ui_zoom(state, -UI_ZOOM_STEP);
    }
    ui.label(format!("{}%", zoom_percent(state.ui_zoom)));
    if ui
        .add(
            egui::Button::new(egui::RichText::new("+").size(18.0).strong())
                .min_size(egui::vec2(38.0, 34.0)),
        )
        .clicked()
    {
        adjust_ui_zoom(state, UI_ZOOM_STEP);
    }
}

fn tab_button(ui: &mut egui::Ui, active: &mut ActiveTab, tab: ActiveTab, label: &str) {
    let is_active = *active == tab;
    let fill = if is_active {
        egui::Color32::from_rgb(66, 72, 92)
    } else {
        egui::Color32::from_rgb(37, 41, 53)
    };
    let stroke = if is_active {
        egui::Stroke::new(1.5, egui::Color32::from_rgb(137, 167, 255))
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(75, 80, 96))
    };
    let text = egui::RichText::new(label).size(18.0).strong();

    if ui
        .add(
            egui::Button::new(text)
                .fill(fill)
                .stroke(stroke)
                .rounding(egui::Rounding::same(10.0))
                .min_size(egui::vec2(150.0, 42.0)),
        )
        .clicked()
    {
        *active = tab;
    }
}

fn export_tab(ui: &mut egui::Ui, state: &mut CivShareState) {
    ui.heading("Export full empire designs from Stellaris");
    ui.add(egui::Label::new("Choose the empires you want to share; CivShare preserves the full Stellaris design exactly as the game saved it.").wrap());
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Open Stellaris Empire File").clicked() {
            if let Some(path) = pick_stellaris_designs_file("Open user_empire_designs file") {
                match load_empire_file(&path) {
                    Ok((_content, empires)) => {
                        let count = empires.len();
                        state.export_path = Some(path.clone());
                        state.export_empires = empires;
                        state.export_selected.clear();
                        state.status =
                            format!("Loaded {count} empire designs from {}", path.display());
                    }
                    Err(err) => state.status = err.to_string(),
                }
            }
        }

        if let Some(path) = &state.export_path {
            ui.label(path.display().to_string());
        }
    });

    selection_toolbar(ui, state.export_empires.len(), &mut state.export_selected);
    ui.add_space(6.0);

    let preview_snapshot = selected_preview_snapshot(&state.export_empires, &state.export_selected);
    let mut save_export_clicked = false;
    let mut save_preview_clicked = false;
    let mut copy_preview_clicked = false;

    ui.horizontal_wrapped(|ui| {
        save_export_clicked = ui
            .add_enabled(
                !state.export_selected.is_empty(),
                egui::Button::new("Save Selected Export File"),
            )
            .clicked();
        save_preview_clicked = ui
            .add_enabled(
                preview_snapshot.is_some(),
                egui::Button::new("Save Preview Text As File"),
            )
            .clicked();
        copy_preview_clicked = ui
            .add_enabled(
                preview_snapshot.is_some(),
                egui::Button::new("Copy Preview Text"),
            )
            .clicked();
    });

    if save_export_clicked {
        if let Some(path) = save_bundle_file() {
            match export_selected_bundle(&state.export_empires, &state.export_selected, &path) {
                Ok(count) => {
                    state.status =
                        format!("Exported {count} empire design(s) to {}", path.display());
                }
                Err(err) => state.status = err.to_string(),
            }
        }
    }

    if save_preview_clicked {
        if let Some((name, raw_text)) = &preview_snapshot {
            save_preview_text(name, raw_text, &mut state.status);
        }
    }

    if copy_preview_clicked {
        if let Some((name, raw_text)) = &preview_snapshot {
            ui.ctx().copy_text(raw_text.clone());
            state.status = format!("Copied preview text for {name}");
        }
    }

    ui.add_space(6.0);

    empire_browser(
        ui,
        &state.export_empires,
        &mut state.export_selected,
        None,
        &mut state.split_fraction,
    );
}

fn import_tab(ui: &mut egui::Ui, state: &mut CivShareState) {
    ui.heading("Import selected empire/civ designs");
    ui.add(egui::Label::new("Open Stellaris civs to import, choose the designs you want, then choose the existing Stellaris file to import into.").wrap());
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Open Stellaris Civs To Import").clicked() {
            if let Some(path) = pick_civshare_export_file("Open Stellaris civs to import") {
                match load_empire_file(&path) {
                    Ok((_content, empires)) => {
                        let count = empires.len();
                        state.import_source_path = Some(path.clone());
                        state.import_source_empires = empires;
                        state.import_source_selected.clear();
                        state.status = format!(
                            "Loaded {count} importable empire design(s) from {}",
                            path.display()
                        );
                    }
                    Err(err) => state.status = err.to_string(),
                }
            }
        }

        if let Some(path) = &state.import_source_path {
            ui.label(path.display().to_string());
        }
    });

    ui.horizontal(|ui| {
        if ui.button("Open Target Stellaris File").clicked() {
            if let Some(path) = pick_stellaris_designs_file("Open target user_empire_designs file")
            {
                match load_empire_file(&path) {
                    Ok((_content, empires)) => {
                        let count = empires.len();
                        state.import_target_path = Some(path.clone());
                        state.import_target_empires = empires;
                        state.status = format!(
                            "Loaded target with {count} existing empire design(s): {}",
                            path.display()
                        );
                    }
                    Err(err) => state.status = err.to_string(),
                }
            }
        }

        if let Some(path) = &state.import_target_path {
            ui.label(path.display().to_string());
        }
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Duplicate handling:");
        ui.radio_value(
            &mut state.conflict_policy,
            ConflictPolicy::Skip,
            ConflictPolicy::Skip.label(),
        );
        ui.radio_value(
            &mut state.conflict_policy,
            ConflictPolicy::Replace,
            ConflictPolicy::Replace.label(),
        );
        ui.radio_value(
            &mut state.conflict_policy,
            ConflictPolicy::Append,
            ConflictPolicy::Append.label(),
        );
    });

    selection_toolbar(
        ui,
        state.import_source_empires.len(),
        &mut state.import_source_selected,
    );
    ui.add_space(6.0);

    let preview_snapshot =
        selected_preview_snapshot(&state.import_source_empires, &state.import_source_selected);
    let can_import = !state.import_source_selected.is_empty() && state.import_target_path.is_some();
    let mut import_clicked = false;
    let mut save_preview_clicked = false;
    let mut copy_preview_clicked = false;

    ui.horizontal_wrapped(|ui| {
        import_clicked = ui
            .add_enabled(
                can_import,
                egui::Button::new("Save Import Into Target File"),
            )
            .clicked();
        save_preview_clicked = ui
            .add_enabled(
                preview_snapshot.is_some(),
                egui::Button::new("Save Preview Text As File"),
            )
            .clicked();
        copy_preview_clicked = ui
            .add_enabled(
                preview_snapshot.is_some(),
                egui::Button::new("Copy Preview Text"),
            )
            .clicked();
    });

    if import_clicked {
        if let Some(target_path) = state.import_target_path.clone() {
            match import_selected_to_file(
                &target_path,
                &state.import_source_empires,
                &state.import_source_selected,
                state.conflict_policy,
            ) {
                Ok(report) => {
                    state.status = format_import_report(&report);
                    if let Ok((_content, empires)) = load_empire_file(&target_path) {
                        state.import_target_empires = empires;
                    }
                }
                Err(err) => state.status = err.to_string(),
            }
        }
    }

    if save_preview_clicked {
        if let Some((name, raw_text)) = &preview_snapshot {
            save_preview_text(name, raw_text, &mut state.status);
        }
    }

    if copy_preview_clicked {
        if let Some((name, raw_text)) = &preview_snapshot {
            ui.ctx().copy_text(raw_text.clone());
            state.status = format!("Copied preview text for {name}");
        }
    }

    ui.add_space(6.0);

    empire_browser(
        ui,
        &state.import_source_empires,
        &mut state.import_source_selected,
        Some(&state.import_target_empires),
        &mut state.split_fraction,
    );
}

fn selection_toolbar(ui: &mut egui::Ui, count: usize, selected: &mut BTreeSet<usize>) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(count > 0, egui::Button::new("Select All"))
            .clicked()
        {
            selected.clear();
            selected.extend(0..count);
        }
        if ui
            .add_enabled(!selected.is_empty(), egui::Button::new("Clear"))
            .clicked()
        {
            selected.clear();
        }
        ui.label(format!("{} selected", selected.len()));
    });
}

fn empire_browser(
    ui: &mut egui::Ui,
    empires: &[EmpireDesign],
    selected: &mut BTreeSet<usize>,
    target_empires: Option<&[EmpireDesign]>,
    split_fraction: &mut f32,
) {
    let wide_layout = ui.available_width() >= 980.0;
    let available_height = ui.available_height().max(240.0);

    if wide_layout {
        let browser_height = (available_height - 12.0).min(760.0).max(220.0);
        let total_width = ui.available_width();
        let splitter_width = 8.0;
        let splitter_gutter = 18.0;
        let available_width = (total_width - splitter_width - splitter_gutter * 2.0).max(0.0);
        let (browser_rect, _response) = ui.allocate_exact_size(
            egui::vec2(total_width, browser_height),
            egui::Sense::hover(),
        );

        *split_fraction = (*split_fraction).clamp(MIN_SPLIT_FRACTION, MAX_SPLIT_FRACTION);
        let splitter_x = browser_rect.min.x + available_width * *split_fraction + splitter_gutter;
        let initial_splitter_rect = egui::Rect::from_min_size(
            egui::pos2(splitter_x, browser_rect.min.y),
            egui::vec2(splitter_width, browser_height),
        );
        let splitter_hit_rect = initial_splitter_rect.expand2(egui::vec2(8.0, 0.0));
        let splitter_response = ui.interact(
            splitter_hit_rect,
            ui.id().with("empire_preview_splitter"),
            egui::Sense::drag(),
        );

        if splitter_response.dragged() {
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let pointer_x =
                    pointer_pos.x - browser_rect.min.x - splitter_gutter - splitter_width * 0.5;
                *split_fraction =
                    (pointer_x / available_width).clamp(MIN_SPLIT_FRACTION, MAX_SPLIT_FRACTION);
            }
        }

        if splitter_response.hovered() || splitter_response.dragged() {
            ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::ResizeHorizontal);
        }

        let empire_width = available_width * *split_fraction;
        let preview_width = available_width - empire_width;
        let empire_rect =
            egui::Rect::from_min_size(browser_rect.min, egui::vec2(empire_width, browser_height));
        let splitter_rect = egui::Rect::from_min_size(
            egui::pos2(empire_rect.max.x + splitter_gutter, browser_rect.min.y),
            egui::vec2(splitter_width, browser_height),
        );
        let preview_rect = egui::Rect::from_min_size(
            egui::pos2(splitter_rect.max.x + splitter_gutter, browser_rect.min.y),
            egui::vec2(preview_width, browser_height),
        );

        fixed_data_panel(ui, empire_rect, "Empires", |ui| {
            empire_list(ui, empires, selected, target_empires, browser_height - 54.0);
        });
        fixed_data_panel(ui, preview_rect, "Preview", |ui| {
            preview_selected(ui, empires, selected, browser_height - 54.0);
        });
        paint_splitter(
            ui,
            splitter_rect,
            splitter_response.hovered() || splitter_response.dragged(),
        );
    } else {
        let list_height = (available_height * 0.45).min(430.0).max(150.0);
        data_panel(ui, "Empires", |ui| {
            empire_list(ui, empires, selected, target_empires, list_height);
        });
        ui.add_space(8.0);
        let preview_height = (available_height - list_height - 24.0)
            .min(380.0)
            .max(120.0);
        data_panel(ui, "Preview", |ui| {
            preview_selected(ui, empires, selected, preview_height);
        });
    }
}

fn paint_splitter(ui: &mut egui::Ui, rect: egui::Rect, active: bool) {
    let color = if active {
        egui::Color32::from_rgb(109, 126, 170)
    } else {
        egui::Color32::from_rgb(57, 65, 88)
    };
    let rail = egui::Rect::from_center_size(rect.center(), egui::vec2(2.0, rect.height() - 26.0));
    let handle = egui::Rect::from_center_size(rect.center(), egui::vec2(7.0, 78.0));

    ui.painter().rect_filled(
        rail,
        2.0,
        egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 70),
    );
    ui.painter().rect_filled(handle, 4.0, color);

    for offset in [-18.0, 0.0, 18.0] {
        ui.painter().circle_filled(
            egui::pos2(handle.center().x, handle.center().y + offset),
            1.5,
            egui::Color32::from_rgb(210, 216, 235),
        );
    }
}

fn fixed_data_panel(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let panel_rect = round_rect_to_pixels(rect.shrink(3.0), ui.ctx().pixels_per_point());

    ui.painter()
        .rect_filled(panel_rect, 12.0, egui::Color32::from_rgb(22, 25, 34));

    let inner_rect = round_rect_to_pixels(
        panel_rect.shrink2(egui::vec2(14.0, 12.0)),
        ui.ctx().pixels_per_point(),
    );
    let mut child_ui = ui.child_ui(inner_rect, egui::Layout::top_down(egui::Align::Min), None);
    child_ui.set_clip_rect(inner_rect);
    child_ui.set_width(inner_rect.width());
    child_ui.set_max_width(inner_rect.width());

    child_ui.label(
        egui::RichText::new(title)
            .size(18.0)
            .strong()
            .color(egui::Color32::from_rgb(225, 230, 245)),
    );
    child_ui.separator();
    add_contents(&mut child_ui);

    paint_panel_outline(ui, panel_rect);
}

fn data_panel(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    let panel_width = ui.available_width();
    ui.set_min_width(panel_width);
    ui.set_max_width(panel_width);

    egui::Frame::none()
        .fill(egui::Color32::from_rgb(22, 25, 34))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 66, 88)))
        .rounding(egui::Rounding::same(12.0))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            let inner_width = (panel_width - 24.0).max(80.0);
            ui.set_width(inner_width);
            ui.set_max_width(inner_width);

            ui.label(
                egui::RichText::new(title)
                    .size(18.0)
                    .strong()
                    .color(egui::Color32::from_rgb(225, 230, 245)),
            );
            ui.separator();
            add_contents(ui);
        });
}

fn paint_panel_outline(ui: &mut egui::Ui, rect: egui::Rect) {
    ui.painter().rect_stroke(
        rect.shrink(0.75),
        12.0,
        egui::Stroke::new(1.5, egui::Color32::from_rgb(103, 116, 152)),
    );
}

fn round_rect_to_pixels(rect: egui::Rect, pixels_per_point: f32) -> egui::Rect {
    let round = |value: f32| (value * pixels_per_point).round() / pixels_per_point;
    egui::Rect::from_min_max(
        egui::pos2(round(rect.min.x), round(rect.min.y)),
        egui::pos2(round(rect.max.x), round(rect.max.y)),
    )
}

fn empire_list(
    ui: &mut egui::Ui,
    empires: &[EmpireDesign],
    selected: &mut BTreeSet<usize>,
    target_empires: Option<&[EmpireDesign]>,
    max_height: f32,
) {
    if empires.is_empty() {
        ui.label("No empire designs loaded.");
        return;
    }

    let list_width = ui.available_width().max(80.0);
    egui::ScrollArea::vertical()
        .id_source("empire_list_scroll")
        .auto_shrink([false, false])
        .max_width(list_width)
        .max_height(max_height)
        .show(ui, |ui| {
            ui.set_width(list_width);
            ui.set_max_width(list_width);

            for (index, empire) in empires.iter().enumerate() {
                let mut is_selected = selected.contains(&index);
                let duplicate = target_empires.is_some_and(|targets| {
                    targets
                        .iter()
                        .any(|target| has_same_identity(target, empire))
                });
                let fill = if is_selected {
                    egui::Color32::from_rgb(52, 65, 92)
                } else if index % 2 == 0 {
                    egui::Color32::from_rgb(31, 34, 44)
                } else {
                    egui::Color32::from_rgb(42, 38, 51)
                };
                let stroke = if duplicate {
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(190, 158, 67))
                } else {
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(63, 68, 84))
                };

                let row_width = ui.available_width().min(list_width).max(80.0);

                egui::Frame::none()
                    .fill(fill)
                    .stroke(stroke)
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_width((row_width - 16.0).max(80.0));
                        ui.set_max_width((row_width - 16.0).max(80.0));

                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut is_selected, "").changed() {
                                if is_selected {
                                    selected.insert(index);
                                } else {
                                    selected.remove(&index);
                                }
                            }

                            ui.vertical(|ui| {
                                ui.set_max_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&empire.name).strong());
                                    if duplicate {
                                        ui.colored_label(egui::Color32::YELLOW, "duplicate");
                                    }
                                });
                                summary_chips(ui, empire);
                            });
                        });
                    });
                ui.add_space(3.0);
            }
        });
}

fn selected_preview_snapshot(
    empires: &[EmpireDesign],
    selected: &BTreeSet<usize>,
) -> Option<(String, String)> {
    if selected.is_empty() {
        return None;
    }

    let selected_empires = selected
        .iter()
        .filter_map(|index| empires.get(*index).cloned())
        .collect::<Vec<_>>();

    if selected_empires.is_empty() {
        return None;
    }

    let label = match selected_empires.as_slice() {
        [empire] => empire.name.clone(),
        empires => format!("{} selected empire designs", empires.len()),
    };
    let raw_text = format_empire_bundle(&selected_empires, "\r\n");

    Some((label, raw_text))
}

fn preview_selected(
    ui: &mut egui::Ui,
    empires: &[EmpireDesign],
    selected: &BTreeSet<usize>,
    max_height: f32,
) {
    let Some((label, preview_text)) = selected_preview_snapshot(empires, selected) else {
        ui.label("Select an empire design to preview its raw Stellaris config.");
        return;
    };

    if selected.len() > 1 {
        ui.label(format!("Showing all selected empires: {label}."));
    }

    let preview_width = ui.available_width().max(80.0);
    egui::ScrollArea::vertical()
        .id_source("preview_scroll")
        .auto_shrink([false, false])
        .max_width(preview_width)
        .max_height(max_height)
        .show(ui, |ui| {
            ui.set_width(preview_width);
            ui.set_max_width(preview_width);

            let mut preview = preview_text.as_str();
            ui.add(
                egui::TextEdit::multiline(&mut preview)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(preview_width)
                    .interactive(true),
            );
        });
}

#[derive(Clone)]
struct SummaryChip {
    label: &'static str,
    value: String,
    fill: egui::Color32,
}

fn summary_chips(ui: &mut egui::Ui, empire: &EmpireDesign) {
    let summary = &empire.summary;
    let mut chips = vec![
        SummaryChip {
            label: "Authority",
            value: summary.authority.as_deref().unwrap_or("unknown").to_owned(),
            fill: egui::Color32::from_rgb(63, 78, 122),
        },
        SummaryChip {
            label: "Government",
            value: summary
                .government
                .as_deref()
                .unwrap_or("unknown")
                .to_owned(),
            fill: egui::Color32::from_rgb(74, 66, 116),
        },
        SummaryChip {
            label: "Origin",
            value: summary.origin.as_deref().unwrap_or("unknown").to_owned(),
            fill: egui::Color32::from_rgb(83, 76, 48),
        },
    ];

    let species = match (&summary.species_class, &summary.portrait) {
        (Some(class), Some(portrait)) => format!("{class} / {portrait}"),
        (Some(class), None) => class.clone(),
        (None, Some(portrait)) => portrait.clone(),
        (None, None) => "unknown".to_owned(),
    };
    chips.push(SummaryChip {
        label: "Species",
        value: species,
        fill: egui::Color32::from_rgb(47, 91, 86),
    });

    if summary.ethics.is_empty() {
        chips.push(SummaryChip {
            label: "Ethic",
            value: "none found".to_owned(),
            fill: egui::Color32::from_rgb(79, 54, 90),
        });
    } else {
        for ethic in &summary.ethics {
            chips.push(SummaryChip {
                label: "Ethic",
                value: ethic.clone(),
                fill: egui::Color32::from_rgb(79, 54, 90),
            });
        }
    }

    if summary.civics.is_empty() {
        chips.push(SummaryChip {
            label: "Civic",
            value: "none found".to_owned(),
            fill: egui::Color32::from_rgb(94, 61, 57),
        });
    } else {
        for civic in &summary.civics {
            chips.push(SummaryChip {
                label: "Civic",
                value: civic.clone(),
                fill: egui::Color32::from_rgb(94, 61, 57),
            });
        }
    }

    draw_chip_rows(ui, &chips);
}

fn draw_chip_rows(ui: &mut egui::Ui, chips: &[SummaryChip]) {
    let max_width = ui.available_width().max(100.0);
    let spacing = ui.spacing().item_spacing.x;
    let mut row = Vec::<(SummaryChip, f32)>::new();
    let mut row_width = 0.0;

    for chip in chips {
        let width = chip_width(chip, max_width);
        let next_width = if row.is_empty() {
            width
        } else {
            row_width + spacing + width
        };

        if !row.is_empty() && next_width > max_width {
            chip_row(ui, &row);
            row.clear();
            row_width = 0.0;
        }

        row_width = if row.is_empty() {
            width
        } else {
            row_width + spacing + width
        };
        row.push((chip.clone(), width));
    }

    if !row.is_empty() {
        chip_row(ui, &row);
    }
}

fn chip_width(chip: &SummaryChip, max_width: f32) -> f32 {
    let text_len = chip.label.chars().count() + chip.value.chars().count() + 2;
    let desired = text_len as f32 * 6.8 + 28.0;
    desired.clamp(110.0_f32.min(max_width), 220.0_f32.min(max_width))
}

fn chip_row(ui: &mut egui::Ui, chips: &[(SummaryChip, f32)]) {
    ui.horizontal(|ui| {
        for (chip, width) in chips {
            chip_ui(ui, chip, *width);
        }
    });
}

fn chip_ui(ui: &mut egui::Ui, chip: &SummaryChip, width: f32) {
    egui::Frame::none()
        .fill(chip.fill)
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 32),
        ))
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
        .show(ui, |ui| {
            ui.set_width(width);
            ui.set_min_width(width);
            ui.set_max_width(width);

            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("{}: {}", chip.label, chip.value))
                        .small()
                        .color(egui::Color32::from_rgb(235, 238, 246)),
                )
                .wrap(),
            );
        });
}

fn pick_civshare_export_file(title: &str) -> Option<PathBuf> {
    let mut dialog = text_file_dialog(title);
    if let Some(default_path) = default_civshare_export_path() {
        dialog = apply_dialog_default_path(dialog, &default_path);
    }

    dialog.pick_file()
}

fn pick_stellaris_designs_file(title: &str) -> Option<PathBuf> {
    let mut dialog = text_file_dialog(title);
    if let Some(default_path) = default_stellaris_designs_path() {
        dialog = apply_dialog_default_path(dialog, &default_path);
    }

    dialog.pick_file()
}

fn save_bundle_file() -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new()
        .set_title("Save CivShare export bundle")
        .set_file_name(CIVSHARE_EXPORT_FILE_NAME)
        .add_filter("Text files", &["txt"]);

    if let Some(default_path) = default_civshare_export_path() {
        dialog = apply_dialog_default_path(dialog, &default_path);
    }

    dialog.save_file()
}

fn save_preview_file(_empire_name: &str) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new()
        .set_title("Save preview text")
        .set_file_name(CIVSHARE_EXPORT_FILE_NAME)
        .add_filter("Text files", &["txt"]);

    if let Some(default_path) = default_civshare_export_path() {
        dialog = apply_dialog_default_path(dialog, &default_path);
    }

    dialog.save_file()
}

fn save_preview_text(empire_name: &str, raw_text: &str, status: &mut String) {
    if let Some(path) = save_preview_file(empire_name) {
        match fs::write(&path, raw_text) {
            Ok(()) => {
                *status = format!("Saved preview text to {}", path.display());
            }
            Err(err) => {
                *status = format!("Failed to save preview text: {err}");
            }
        }
    }
}

fn text_file_dialog(title: &str) -> rfd::FileDialog {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("Stellaris text files", &["txt"])
}

fn apply_dialog_default_path(dialog: rfd::FileDialog, path: &Path) -> rfd::FileDialog {
    let dialog = if let Some(parent) = path.parent() {
        dialog.set_directory(parent)
    } else {
        dialog
    };

    if let Some(file_name) = path.file_name() {
        dialog.set_file_name(file_name.to_string_lossy())
    } else {
        dialog
    }
}

fn default_stellaris_designs_path() -> Option<PathBuf> {
    let mut path = PathBuf::from(env::var_os("USERPROFILE")?);
    for component in STELLARIS_DESIGNS_RELATIVE_PATH {
        path.push(component);
    }

    Some(path)
}

fn default_civshare_export_path() -> Option<PathBuf> {
    let mut path = PathBuf::from(env::var_os("USERPROFILE")?);
    path.push("Downloads");
    path.push(CIVSHARE_EXPORT_FILE_NAME);

    Some(path)
}

fn format_import_report(report: &io_ops::ImportReport) -> String {
    let backup = report
        .backup_path
        .as_ref()
        .map(|path| format!(" Backup: {}", path.display()))
        .unwrap_or_default();

    format!(
        "Import complete: {} appended, {} replaced, {} skipped.{backup}",
        report.imported, report.replaced, report.skipped
    )
}
