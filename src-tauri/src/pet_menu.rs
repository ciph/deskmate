use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use crate::window_layout::Rect;

const WINDOW: &str = "pet-menu";
const UPDATE: &str = "deskmate://pet-menu-update";
const RESULT: &str = "deskmate://pet-menu-result";
const INITIAL_DIAMETER: f64 = 416.0;

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Item {
    id: String,
    label: String,
    icon: String,
    enabled: bool,
    #[serde(default)]
    checked: Option<bool>,
    #[serde(default)]
    children: Option<Vec<Item>>,
}
#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Anchor { left: f64, top: f64, width: f64, height: f64 }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Layout { width: f64, height: f64, center_x: f64, center_y: f64, inner_radius: f64, outer_radius: f64 }
#[derive(Debug, Clone)]
struct Placement { bounds: Rect, layout: Layout }
#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Model {
    id: String,
    revision: u32,
    title: String,
    theme: String,
    language: String,
    anchor: Option<Anchor>,
    #[serde(skip_deserializing)]
    layout: Option<Layout>,
    items: Vec<Item>,
}
#[derive(Clone, Serialize)]
struct ResultEvent { id: String, action: Option<String> }
struct Active { model: Model, placement: Placement, shown: bool, focused: bool }
#[derive(Default)]
pub(crate) struct MenuState(Mutex<Option<Active>>);

fn require_window(label: &str, expected: &str) -> Result<(), String> {
    if label == expected { Ok(()) } else { Err("pet_menu_window_required".into()) }
}
fn known_action(id: &str) -> bool {
    matches!(id, "chat" | "ai-usage" | "open-widget-settings" | "open-pet-settings" | "poke-pet"
        | "morph-poke" | "morph-walk" | "morph-sleep" | "morph-auto" | "morph-mochi"
        | "morph-pillow" | "morph-ricecake" | "morph-ghost" | "morph-appearance-milk" | "morph-appearance-classic")
}
fn enabled_action(items: &[Item], id: &str) -> bool {
    items.iter().any(|item| item.enabled && match &item.children {
        Some(children) => enabled_action(children, id),
        None => item.id == id && known_action(id),
    })
}
fn validate(model: &Model) -> Result<(), String> {
    fn items_valid(items: &[Item], depth: usize, ids: &mut std::collections::HashSet<String>) -> bool {
        (2..=6).contains(&items.len()) && items.iter().all(|item| {
            item.id.len() <= 80 && item.label.len() <= 160 && item.icon.len() <= 32 && ids.insert(item.id.clone())
                && match &item.children {
                    Some(children) => depth == 0 && items_valid(children, depth + 1, ids),
                    None => known_action(&item.id),
                }
        })
    }
    if model.id.is_empty() || model.id.len() > 80 || model.title.len() > 160 || model.theme.len() > 32
        || model.language.len() > 16 || model.anchor.as_ref().is_some_and(|a|
            [a.left, a.top, a.width, a.height].iter().any(|v| !v.is_finite() || v.abs() > 5000.0) || a.width <= 0.0 || a.height <= 0.0)
        || !items_valid(&model.items, 0, &mut Default::default()) {
        return Err("invalid_pet_menu".into());
    }
    Ok(())
}

fn finish(app: &tauri::AppHandle, id: &str, action: Option<String>) -> Result<(), String> {
    let result = {
        let state = app.state::<MenuState>();
        let mut active = state.0.lock().map_err(|e| e.to_string())?;
        let Some(session) = active.as_ref().filter(|session| session.model.id == id) else { return Ok(()) };
        if action.as_ref().is_some_and(|action| !enabled_action(&session.model.items, action)) {
            return Err("invalid_pet_menu_action".into());
        }
        let result = ResultEvent { id: id.into(), action };
        *active = None;
        result
    };
    if let Some(window) = app.get_webview_window(WINDOW) { let _ = window.hide(); }
    app.emit_to("pet", RESULT, result).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn open_pet_radial_menu(webview: tauri::Webview, mut menu: Model) -> Result<(), String> {
    require_window(webview.label(), "pet")?;
    validate(&menu)?;
    let app = webview.app_handle();
    let placement = placement_for_pet(app, menu.anchor.as_ref())?;
    menu.layout = Some(placement.layout.clone());
    {
        let state = app.state::<MenuState>();
        let mut active = state.0.lock().map_err(|e| e.to_string())?;
        if active.is_some() { return Err("pet_menu_already_open".into()); }
        *active = Some(Active { model: menu.clone(), placement, shown: false, focused: false });
    }
    let opened = (|| -> Result<(), String> {
        if app.get_webview_window(WINDOW).is_none() {
            let mut builder = tauri::WebviewWindowBuilder::new(app, WINDOW, tauri::WebviewUrl::App("pet-menu.html".into()))
                .title("YUME pet menu").inner_size(INITIAL_DIAMETER, INITIAL_DIAMETER).decorations(false).transparent(true)
                .shadow(false).resizable(false).always_on_top(true).skip_taskbar(true)
                .visible(false).focused(false).disable_drag_drop_handler();
            // WebView2 windows sharing a profile must use the same browser arguments.
            if let Some(args) = app.config().app.windows.iter().find(|window| window.label == "pet")
                .and_then(|window| window.additional_browser_args.as_deref()) {
                builder = builder.additional_browser_args(args);
            }
            builder.build().map_err(|e| e.to_string())?;
        }
        app.emit_to(WINDOW, UPDATE, &menu).map_err(|e| e.to_string())
    })();
    if opened.is_err() { let _ = finish(app, &menu.id, None); }
    opened?;
    // A failed page load must never leave the pet held indefinitely.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let expired = app.state::<MenuState>().0.lock().ok().and_then(|active| active.as_ref()
            .filter(|s| s.model.id == menu.id && !s.shown).map(|s| s.model.id.clone()));
        if let Some(id) = expired { let _ = finish(&app, &id, None); }
    });
    Ok(())
}

#[tauri::command]
pub(crate) fn update_pet_radial_menu(webview: tauri::Webview, mut menu: Model) -> Result<(), String> {
    require_window(webview.label(), "pet")?;
    validate(&menu)?;
    let app = webview.app_handle();
    {
        let state = app.state::<MenuState>();
        let mut active = state.0.lock().map_err(|e| e.to_string())?;
        let Some(session) = active.as_mut().filter(|s| s.model.id == menu.id && s.model.revision < menu.revision) else { return Ok(()) };
        menu.anchor = session.model.anchor.clone();
        menu.layout = session.model.layout.clone();
        session.model = menu.clone();
    }
    app.emit_to(WINDOW, UPDATE, menu).map_err(|e| e.to_string())
}
#[tauri::command]
pub(crate) fn get_pet_radial_menu(webview: tauri::Webview) -> Result<Option<Model>, String> {
    require_window(webview.label(), WINDOW)?;
    Ok(webview.state::<MenuState>().0.lock().map_err(|e| e.to_string())?.as_ref().map(|s| s.model.clone()))
}

fn position(pet: Rect, work: Rect, scale: f64) -> Result<Placement, String> {
    let center_x = pet.x as f64 + pet.width as f64 / 2.0;
    let center_y = pet.y as f64 + pet.height as f64 / 2.0;
    let inner_radius = ((pet.width as f64).hypot(pet.height as f64) / (2.0 * scale) + 4.0).max(84.0);
    let outer_radius = inner_radius + 64.0;
    let extent = (outer_radius + 10.0) * scale;
    let x = (center_x - extent).max(work.x as f64).floor() as i64;
    let y = (center_y - extent).max(work.y as f64).floor() as i64;
    let right = (center_x + extent).min((work.x + work.width) as f64).ceil() as i64;
    let bottom = (center_y + extent).min((work.y + work.height) as f64).ceil() as i64;
    if right <= x || bottom <= y { return Err("pet_menu_outside_work_area".into()); }
    let bounds = Rect { x, y, width: right - x, height: bottom - y };
    Ok(Placement { layout: Layout { width: bounds.width as f64 / scale, height: bounds.height as f64 / scale,
        center_x: (center_x - x as f64) / scale, center_y: (center_y - y as f64) / scale, inner_radius, outer_radius }, bounds })
}
fn placement_for_pet(app: &tauri::AppHandle, anchor: Option<&Anchor>) -> Result<Placement, String> {
    let pet = app.get_webview_window("pet").ok_or("pet_window_missing")?;
    let scale = pet.scale_factor().map_err(|e| e.to_string())?;
    let origin = pet.inner_position().map_err(|e| e.to_string())?;
    let bounds = match anchor {
        Some(a) => Rect { x: (origin.x as f64 + a.left * scale).round() as i64, y: (origin.y as f64 + a.top * scale).round() as i64,
            width: (a.width * scale).ceil() as i64, height: (a.height * scale).ceil() as i64 },
        None => crate::pet_geometry::chat_bounds(&pet)?,
    };
    let monitor = pet.monitor_from_point((bounds.x + bounds.width / 2) as f64, (bounds.y + bounds.height / 2) as f64)
        .map_err(|e| e.to_string())?.or(pet.current_monitor().map_err(|e| e.to_string())?).ok_or("pet_monitor_missing")?;
    let area = monitor.work_area();
    let work = Rect { x: area.position.x.into(), y: area.position.y.into(), width: area.size.width.into(), height: area.size.height.into() };
    position(bounds, work, monitor.scale_factor())
}
#[tauri::command]
pub(crate) async fn pet_radial_menu_ready(webview: tauri::Webview, id: String) -> Result<(), String> {
    require_window(webview.label(), WINDOW)?;
    let app = webview.app_handle();
    let placement = {
        let state = app.state::<MenuState>();
        let mut active = state.0.lock().map_err(|e| e.to_string())?;
        let Some(session) = active.as_mut().filter(|s| s.model.id == id && !s.shown) else { return Ok(()) };
        session.shown = true;
        session.placement.clone()
    };
    let ready = (|| -> Result<(), String> {
        let window = app.get_webview_window(WINDOW).ok_or("pet_menu_window_missing")?;
        let bounds = placement.bounds;
        // Clip the popup window, not its center: the visible fan still surrounds the stationary pet.
        window.set_position(tauri::PhysicalPosition::new(bounds.x as i32, bounds.y as i32)).map_err(|e| e.to_string())?;
        window.set_size(tauri::PhysicalSize::new(bounds.width as u32, bounds.height as u32)).map_err(|e| e.to_string())?;
        let still_open = app.state::<MenuState>().0.lock().map_err(|e| e.to_string())?.as_ref().is_some_and(|s| s.model.id == id);
        if still_open { window.show().map_err(|e| e.to_string())?; window.set_focus().map_err(|e| e.to_string())?; }
        Ok(())
    })();
    if ready.is_err() { let _ = finish(app, &id, None); }
    ready
}
#[tauri::command]
pub(crate) fn finish_pet_radial_menu(webview: tauri::Webview, id: String, action: Option<String>) -> Result<(), String> {
    if webview.label() != WINDOW && !(webview.label() == "pet" && action.is_none()) { return Err("pet_menu_window_required".into()); }
    finish(webview.app_handle(), &id, action)
}
pub(crate) fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() == "pet" && matches!(event, tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. } | tauri::WindowEvent::Destroyed) {
        let id = window.try_state::<MenuState>().and_then(|state| state.0.lock().ok().and_then(|active| active.as_ref().filter(|s| s.shown).map(|s| s.model.id.clone())));
        if let Some(id) = id { let _ = finish(window.app_handle(), &id, None); }
        return;
    }
    if window.label() != WINDOW { return; }
    let Some(state) = window.try_state::<MenuState>() else { return };
    let id = {
        let Ok(mut active) = state.0.lock() else { return };
        let Some(session) = active.as_mut() else { return };
        match event {
            tauri::WindowEvent::Focused(true) => { session.focused = true; None }
            tauri::WindowEvent::Focused(false) if session.focused => Some(session.model.id.clone()),
            tauri::WindowEvent::CloseRequested { api, .. } => { api.prevent_close(); Some(session.model.id.clone()) }
            tauri::WindowEvent::Destroyed => Some(session.model.id.clone()),
            _ => None,
        }
    };
    if let Some(id) = id { let _ = finish(window.app_handle(), &id, None); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    #[ignore = "requires a built frontend and the real Windows WebView2 runtime"]
    fn native_menu_page_loads_and_completes_the_ready_handshake() {
        struct TestLogger;
        impl log::Log for TestLogger {
            fn enabled(&self, _: &log::Metadata<'_>) -> bool { true }
            fn log(&self, record: &log::Record<'_>) { eprintln!("{}: {}", record.target(), record.args()); }
            fn flush(&self) {}
        }
        static LOGGER: TestLogger = TestLogger;
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Warn);
        use std::borrow::Cow;
        use tauri::utils::assets::{AssetKey, AssetsIter, CspHash};
        struct BuiltAssets(std::path::PathBuf);
        impl tauri::Assets<tauri::Wry> for BuiltAssets {
            fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
                let path = self.0.join(key.as_ref().trim_start_matches('/'));
                std::fs::read(&path).ok().map(Cow::Owned)
            }
            fn iter(&self) -> Box<AssetsIter<'_>> { Box::new(std::iter::empty()) }
            fn csp_hashes(&self, _: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> { Box::new(std::iter::empty()) }
        }
        let mut context = tauri::generate_context!();
        let mut pet = context.config().app.windows.iter().find(|window| window.label == "pet").unwrap().clone();
        pet.url = tauri::WebviewUrl::External("about:blank".parse().unwrap());
        pet.visible = false; pet.focus = false;
        context.config_mut().identifier = "com.deskmate.petmenuhandshake20261103".into();
        for window in &mut context.config_mut().app.windows { window.create = false; }
        context.config_mut().build.dev_url = None;
        context.set_assets(Box::new(BuiltAssets(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist"))));
        let app = tauri::Builder::<tauri::Wry>::default().any_thread()
            .manage(MenuState::default()).on_window_event(window_event)
            .invoke_handler(tauri::generate_handler![get_pet_radial_menu, pet_radial_menu_ready, finish_pet_radial_menu])
            .setup(move |app| {
                tauri::WebviewWindowBuilder::from_config(app, &pet)?.build()?;
                Ok(())
            }).build(context).unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let mut pending = Some(send);
        app.run_return(move |app, event| {
            if !matches!(event, tauri::RunEvent::Ready) { return; }
            let Some(send) = pending.take() else { return };
            let app = app.clone();
            std::thread::spawn(move || {
                let pet = app.get_webview_window("pet").unwrap();
                let result = (|| -> Result<(), String> {
                    for attempt in 1..=2 {
                        let id = format!("native-handshake-{attempt}");
                        let model: Model = serde_json::from_value(serde_json::json!({
                            "id":id, "revision":1, "title":"百变小著", "theme":"mint", "language":"zh-CN",
                            "anchor":{"left":80,"top":130,"width":160,"height":140},
                            "items":[{"id":"chat","label":"聊天","icon":"chat","enabled":true},
                                {"id":"ai-usage","label":"AI 用量","icon":"ai","enabled":true}]
                        })).unwrap();
                        tauri::async_runtime::block_on(open_pet_radial_menu(pet.as_ref().clone(), model))?;
                        let window = app.get_webview_window(WINDOW).ok_or("menu window missing")?;
                        // build() can succeed before native creation fails; query the actual window.
                        window.url().map_err(|e| format!("native menu creation failed: {e}"))?;
                        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
                        loop {
                            let ready = app.state::<MenuState>().0.lock().unwrap().as_ref()
                                .is_some_and(|session| session.model.id == id && session.shown);
                            if ready && window.is_visible().map_err(|e| e.to_string())? { break; }
                            if std::time::Instant::now() >= deadline {
                                return Err(format!("menu {attempt} did not complete its ready handshake and show"));
                            }
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                        finish_pet_radial_menu(window.as_ref().clone(), id, Some("chat".into()))?;
                        if app.state::<MenuState>().0.lock().unwrap().is_some()
                            || window.is_visible().map_err(|e| e.to_string())? {
                            return Err("menu did not release its session and hide".into());
                        }
                        eprintln!("native menu {attempt}: real page ready, visible, action accepted, hidden");
                    }
                    Ok(())
                })();
                let _ = send.send(result);
                app.exit(0);
            });
        });
        receive.recv_timeout(std::time::Duration::from_secs(5)).unwrap().unwrap();
    }

    #[test]
    fn clipped_bounds_preserve_the_pet_center_at_edges_and_across_dpi() {
        let work = Rect { x: -1920, y: -100, width: 1920, height: 1040 };
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for (x, y) in [(-1920, -100), (-160, 730), (-1100, 400), (-1900, 700)] {
                let pet = Rect { x, y, width: 160, height: 140 };
                let p = position(pet, work, scale).unwrap();
                assert!(p.bounds.x >= work.x && p.bounds.y >= work.y && p.bounds.x + p.bounds.width <= work.x + work.width && p.bounds.y + p.bounds.height <= work.y + work.height);
                assert!((p.bounds.x as f64 + p.layout.center_x * scale - (x + 80) as f64).abs() < 0.001);
                assert!((p.bounds.y as f64 + p.layout.center_y * scale - (y + 70) as f64).abs() < 0.001);
                assert!(p.layout.inner_radius * scale > (pet.width as f64).hypot(pet.height as f64) / 2.0);
            }
        }
    }
    #[test]
    fn actions_require_enabled_leaf_and_expected_window() {
        let leaf = Item { id: "chat".into(), label: "Chat".into(), icon: "chat".into(), enabled: true, checked: None, children: None };
        assert!(enabled_action(&[leaf.clone()], "chat"));
        assert!(!enabled_action(&[leaf.clone()], "open_settings"));
        let mut group = Item { id: "group".into(), children: Some(vec![leaf]), enabled: false, label: "Group".into(), icon: "pet".into(), checked: None };
        assert!(!enabled_action(&[group.clone()], "chat"));
        group.enabled = true;
        assert!(!enabled_action(&[group.clone()], "group"));
        assert!(enabled_action(&[group], "chat"));
        assert!(require_window("settings", "pet").is_err());
        assert!(require_window("pet-menu", "pet").is_err());
        assert!(require_window("pet-menu", WINDOW).is_ok());
    }
    #[test]
    fn models_reject_duplicate_unknown_and_oversized_actions() {
        let leaf = Item { id: "chat".into(), label: "Chat".into(), icon: "chat".into(), enabled: true, checked: None, children: None };
        let usage = Item { id: "ai-usage".into(), ..leaf.clone() };
        let mut model = Model { id: "session".into(), revision: 1, title: "Pet".into(), theme: "mint".into(), language: "en-US".into(),
            anchor: None, layout: None, items: vec![leaf.clone(), usage.clone()] };
        assert!(validate(&model).is_ok());
        model.items[1] = leaf;
        assert!(validate(&model).is_err());
        model.items[1] = Item { id: "delete_files".into(), ..usage.clone() };
        assert!(validate(&model).is_err());
        model.items[1] = usage;
        model.anchor = Some(Anchor { left: 0.0, top: f64::NAN, width: 100.0, height: 100.0 });
        assert!(validate(&model).is_err());
    }
}
