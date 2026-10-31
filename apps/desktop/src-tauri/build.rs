fn main() {
    // Les commandes de l'application passent par le système de permissions :
    // une fenêtre ne peut appeler que ce que sa capability autorise.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "parse_clipboard_item",
            "get_overlay_state",
            "hide_overlay",
            "fit_overlay",
            "open_main_window",
            "check_for_update",
            "install_update",
            "get_settings",
            "set_update_channel",
        ]),
    ))
    .expect("échec du build Tauri");
}
