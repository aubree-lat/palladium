use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

use tauri::AppHandle;

static HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn register(app: &AppHandle) {
    let _ = HANDLE.set(app.clone());
}

#[cfg(target_os = "linux")]
fn read_on_main() -> Option<Vec<u8>> {
    let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
    let pixbuf = clipboard.wait_for_image()?;
    pixbuf.save_to_bufferv("png", &[]).ok()
}

#[cfg(not(target_os = "linux"))]
fn read_on_main() -> Option<Vec<u8>> {
    None
}

pub fn image_png() -> Option<Vec<u8>> {
    let handle = HANDLE.get()?;
    let (tx, rx) = mpsc::channel();

    handle
        .run_on_main_thread(move || {
            let _ = tx.send(read_on_main());
        })
        .ok()?;

    match rx.recv_timeout(Duration::from_secs(3)) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("clipboard read timed out: {e}");
            None
        }
    }
}

pub fn has_main_thread() -> bool {
    HANDLE.get().is_some()
}

pub fn app_handle() -> Option<AppHandle> {
    HANDLE.get().cloned()
}
