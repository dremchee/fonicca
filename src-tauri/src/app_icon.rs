use tauri::Manager;

/// Follow the saved appearance; in system mode the main window resolves it for us.
pub fn refresh(app: &tauri::AppHandle) {
    let theme = match app.state::<crate::AppState>().settings.get().theme {
        crate::settings::Theme::Dark => tauri::Theme::Dark,
        crate::settings::Theme::Light => tauri::Theme::Light,
        crate::settings::Theme::System => app
            .get_webview_window("main")
            .and_then(|window| window.theme().ok())
            .unwrap_or(tauri::Theme::Light),
    };
    apply(app, theme);
}

fn apply(app: &tauri::AppHandle, theme: tauri::Theme) {
    let bytes: &'static [u8] = if theme == tauri::Theme::Dark {
        include_bytes!("../icons/icon-dark.png")
    } else {
        include_bytes!("../icons/icon-light.png")
    };

    #[cfg(target_os = "macos")]
    if let Err(error) = app.run_on_main_thread(move || {
        use objc2::{AllocAnyThread, MainThreadMarker};
        use objc2_app_kit::{NSApplication, NSImage};
        use objc2_foundation::NSData;

        let Some(main_thread) = MainThreadMarker::new() else { return };
        let data = NSData::with_bytes(bytes);
        if let Some(icon) = NSImage::initWithData(NSImage::alloc(), &data) {
            // AppKit retains the image; all calls stay on the main thread.
            unsafe { NSApplication::sharedApplication(main_thread).setApplicationIconImage(Some(&icon)) };
        }
    }) {
        eprintln!("cannot update application icon: {error}");
    }

    #[cfg(not(target_os = "macos"))]
    match tauri::image::Image::from_bytes(bytes) {
        Ok(icon) => {
            for window in app.webview_windows().values() {
                if let Err(error) = window.set_icon(icon.clone()) {
                    eprintln!("cannot update window icon: {error}");
                }
            }
        }
        Err(error) => eprintln!("cannot decode application icon: {error}"),
    }
}
