
#[cfg(target_os = "linux")]
mod imp {
    use std::cell::RefCell;
    use std::ffi::CStr;
    use std::rc::Rc;

    use webkit2gtk::glib::prelude::*;
    use webkit2gtk::glib::translate::ToGlibPtr;
    use webkit2gtk::{
        FileChooserRequestExt, PermissionRequestExt, SettingsExt, WebViewExt,
    };

    mod ffi {
        use std::os::raw::{c_char, c_int};

        #[repr(C)]
        pub struct WebKitFeature {
            _private: [u8; 0],
        }
        #[repr(C)]
        pub struct WebKitFeatureList {
            _private: [u8; 0],
        }

        extern "C" {
            pub fn webkit_settings_get_all_features() -> *mut WebKitFeatureList;
            pub fn webkit_feature_list_get_length(list: *mut WebKitFeatureList) -> usize;
            pub fn webkit_feature_list_get(
                list: *mut WebKitFeatureList,
                index: usize,
            ) -> *mut WebKitFeature;
            pub fn webkit_feature_list_unref(list: *mut WebKitFeatureList);
            pub fn webkit_feature_get_identifier(feature: *mut WebKitFeature) -> *const c_char;
            pub fn webkit_settings_set_feature_enabled(
                settings: *mut webkit2gtk::ffi::WebKitSettings,
                feature: *mut WebKitFeature,
                enabled: c_int,
            );
            pub fn webkit_settings_get_feature_enabled(
                settings: *mut webkit2gtk::ffi::WebKitSettings,
                feature: *mut WebKitFeature,
            ) -> c_int;
        }
    }

    fn wants_enabling(id: &str) -> bool {
        id == "PeerConnectionEnabled"
            || id == "MediaStreamEnabled"
            || id == "MediaDevicesEnabled"
            || id.starts_with("WebRTC")
    }

    fn enable_webrtc_features(settings: &webkit2gtk::Settings) -> usize {
        let raw: *mut webkit2gtk::ffi::WebKitSettings = settings.to_glib_none().0;
        let mut changed = 0;

        unsafe {
            let list = ffi::webkit_settings_get_all_features();
            if list.is_null() {
                log::warn!("WebKit exposed no feature list");
                return 0;
            }

            let len = ffi::webkit_feature_list_get_length(list);
            for i in 0..len {
                let feature = ffi::webkit_feature_list_get(list, i);
                if feature.is_null() {
                    continue;
                }
                let id_ptr = ffi::webkit_feature_get_identifier(feature);
                if id_ptr.is_null() {
                    continue;
                }
                let Ok(id) = CStr::from_ptr(id_ptr).to_str() else {
                    continue;
                };

                if !wants_enabling(id) {
                    continue;
                }

                let before = ffi::webkit_settings_get_feature_enabled(raw, feature) != 0;
                if !before {
                    ffi::webkit_settings_set_feature_enabled(raw, feature, 1);
                    let after = ffi::webkit_settings_get_feature_enabled(raw, feature) != 0;
                    log::info!("WebKit feature {id}: {before} -> {after}");
                    if after {
                        changed += 1;
                    }
                } else {
                    log::debug!("WebKit feature {id} already enabled");
                }
            }

            ffi::webkit_feature_list_unref(list);
        }

        changed
    }

    pub fn tune(window: &tauri::WebviewWindow) {
        let result = window.with_webview(|platform| {
            let webview = platform.inner();

            match WebViewExt::settings(&webview) {
                Some(settings) => {
                    settings.set_enable_media_stream(true);
                    settings.set_enable_webrtc(true);
                    settings.set_enable_mediasource(true);
                    settings.set_enable_media_capabilities(true);
                    settings.set_enable_encrypted_media(true);

                    let changed = enable_webrtc_features(&settings);

                    log::info!(
                        "WebKit media enabled (media_stream={}, webrtc={}, mediasource={}, \
                         {changed} feature flags flipped)",
                        settings.enables_media_stream(),
                        settings.enables_webrtc(),
                        settings.enables_mediasource(),
                    );
                }
                None => log::warn!("could not reach WebKit settings; voice will not work"),
            }

            webview.connect_permission_request(|_, request| {
                let kind = request.type_().name();
                let granted = matches!(
                    kind,
                    "WebKitUserMediaPermissionRequest"
                        | "WebKitDeviceInfoPermissionRequest"
                        | "WebKitNotificationPermissionRequest"
                );

                if granted {
                    log::debug!("granting {kind}");
                    request.allow();
                } else {
                    log::debug!("denying {kind}");
                    request.deny();
                }
                true
            });

            webview.connect_run_file_chooser(|webview, request| {
                use gtk::prelude::*;

                let multiple = request.selects_multiple();
                let parent = webview
                    .toplevel()
                    .and_then(|top| top.downcast::<gtk::Window>().ok());

                let dialog = gtk::FileChooserNative::new(
                    Some(if multiple { "Select Files" } else { "Select File" }),
                    parent.as_ref(),
                    gtk::FileChooserAction::Open,
                    Some("_Open"),
                    Some("_Cancel"),
                );
                dialog.set_select_multiple(multiple);

                if let Some(filter) = request.mime_types_filter() {
                    dialog.add_filter(filter);
                }

                let holder: Rc<RefCell<Option<gtk::FileChooserNative>>> =
                    Rc::new(RefCell::new(Some(dialog.clone())));
                let request = request.clone();

                dialog.connect_response(move |dialog, response| {
                    if response == gtk::ResponseType::Accept {
                        let paths: Vec<String> = dialog
                            .filenames()
                            .iter()
                            .filter_map(|p| p.to_str().map(str::to_owned))
                            .collect();
                        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
                        log::debug!("uploading {} file(s)", refs.len());
                        request.select_files(&refs);
                    } else {
                        request.cancel();
                    }
                    dialog.hide();
                    holder.borrow_mut().take();
                });

                dialog.show();
                true
            });
        });

        if let Err(e) = result {
            log::error!("could not tune the webview: {e}");
        }
    }
}

#[cfg(target_os = "linux")]
pub use imp::tune;

#[cfg(not(target_os = "linux"))]
pub fn tune(_window: &tauri::WebviewWindow) {}
