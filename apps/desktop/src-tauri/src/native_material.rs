use serde::Serialize;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
static NATIVE_GLASS: AtomicBool = AtomicBool::new(false);
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    native_glass: bool,
    reduce_transparency: bool,
    increase_contrast: bool,
    reduce_motion: bool,
}
pub fn install(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    let _ = window.with_webview(|webview| {
        use objc2::{msg_send, runtime::AnyClass, MainThreadOnly};
        use objc2_app_kit::{
            NSAutoresizingMaskOptions, NSColor, NSGlassEffectView, NSGlassEffectViewStyle, NSView,
        };
        use objc2_foundation::MainThreadMarker;
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        if AnyClass::get(c"NSGlassEffectView").is_none() {
            return;
        }
        // Public AppKit/WebKit only; no private drawsBackground KVC.
        unsafe {
            let view = &*(webview.inner() as *const NSView);
            let Some(parent) = view.superview() else {
                return;
            };
            let glass =
                NSGlassEffectView::initWithFrame(NSGlassEffectView::alloc(mtm), view.frame());
            glass.setStyle(NSGlassEffectViewStyle::Regular);
            glass.setCornerRadius(0.0);
            glass.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            let _: () = msg_send![view, setUnderPageBackgroundColor: &*NSColor::clearColor()];
            // Preserve the WKWebView. Opaque content masks the backing; navigation
            // reveals it. The existing WebView still owns input and resizing.
            view.removeFromSuperview();
            parent.addSubview(&glass);
            glass.setContentView(Some(view));
            NATIVE_GLASS.store(view.isDescendantOf(&glass), Ordering::Relaxed);
        }
    });
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}
fn current() -> Appearance {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWorkspace;
        let workspace = NSWorkspace::sharedWorkspace();
        Appearance {
            native_glass: NATIVE_GLASS.load(Ordering::Relaxed),
            reduce_transparency: workspace.accessibilityDisplayShouldReduceTransparency(),
            increase_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
            reduce_motion: workspace.accessibilityDisplayShouldReduceMotion(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    Appearance {
        native_glass: false,
        reduce_transparency: false,
        increase_contrast: false,
        reduce_motion: false,
    }
}
#[tauri::command]
pub async fn native_appearance(app: tauri::AppHandle) -> Result<Appearance, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(current());
    })
    .map_err(|_| "外观状态不可用")?;
    receiver.await.map_err(|_| "外观状态不可用".into())
}
#[tauri::command]
pub fn native_theme(window: tauri::WebviewWindow, theme: &str) -> Result<(), String> {
    let value = match theme {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        "system" => None,
        _ => return Err("外观无效".into()),
    };
    window.set_theme(value).map_err(|_| "外观设置不可用".into())
}
