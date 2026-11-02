/// Hand a verified local package to the operating system without a shell or
/// command helper. Installation remains visible and controlled by the user.
pub fn open_installer(path: &std::path::Path) -> Result<(), String> {
    // ShellExecute expects a shell path, rather than Windows canonicalize's
    // verbatim \?\ path prefix. Other platforms can resolve symlinks normally.
    let path = if cfg!(windows) {
        std::path::absolute(path)
    } else {
        path.canonicalize()
    }
    .map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    {
        use std::{collections::HashMap, os::fd::AsFd};
        let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let connection = zbus::blocking::Connection::session().map_err(|e| e.to_string())?;
        let portal = zbus::blocking::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.OpenURI",
        )
        .map_err(|e| e.to_string())?;
        // Subscribe before requesting the launch, and keep the connection alive
        // until the portal responds (disconnecting early can cancel its dialog).
        let sender = connection
            .unique_name()
            .ok_or("Desktop session has no connection name")?
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = format!(
            "reclaim_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        );
        let request_path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
        let request = zbus::blocking::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            request_path.as_str(),
            "org.freedesktop.portal.Request",
        )
        .map_err(|e| e.to_string())?;
        let mut responses = request
            .receive_signal("Response")
            .map_err(|e| e.to_string())?;
        let mut options = HashMap::<&str, zbus::zvariant::Value<'_>>::new();
        options.insert("handle_token", zbus::zvariant::Value::from(token.as_str()));
        let handle: zbus::zvariant::OwnedObjectPath = portal.call("OpenFile", &("", zbus::zvariant::Fd::from(file.as_fd()), options)).map_err(|e| format!("Could not open installer: {e}. Open the downloaded file from your file manager."))?;
        if handle.as_str() != request_path {
            return Err("The desktop returned an unexpected installer request. Open the saved installer from your file manager.".into());
        }
        let response = responses
            .next()
            .ok_or("The desktop installer request ended without a response")?;
        let (status, _): (u32, HashMap<String, zbus::zvariant::OwnedValue>) =
            response.body().deserialize().map_err(|e| e.to_string())?;
        if status != 0 {
            return Err(
                "The installer was not opened. You can open the saved file from your file manager."
                    .into(),
            );
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let file: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        // SAFETY: the terminated strings remain alive until ShellExecuteW returns.
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        } as isize;
        if result <= 32 {
            return Err(format!(
                "Could not open installer (Windows error {result}). Open the downloaded file from Explorer."
            ));
        }
    }
    #[cfg(target_os = "macos")]
    {
        use core_foundation_sys::{
            base::{CFRelease, kCFAllocatorDefault},
            url::{CFURLCreateFromFileSystemRepresentation, CFURLRef},
        };
        use std::os::unix::ffi::OsStrExt;
        #[link(name = "CoreServices", kind = "framework")]
        unsafe extern "C" {
            fn LSOpenCFURLRef(url: CFURLRef, launched: *mut CFURLRef) -> i32;
        }
        let bytes = path.as_os_str().as_bytes();
        // SAFETY: URL is created from a live byte slice; the returned +1 reference
        // is released after Launch Services has consumed it.
        unsafe {
            let url = CFURLCreateFromFileSystemRepresentation(
                kCFAllocatorDefault,
                bytes.as_ptr(),
                bytes.len() as isize,
                0,
            );
            if url.is_null() {
                return Err("Could not create installer file URL".into());
            }
            let status = LSOpenCFURLRef(url, std::ptr::null_mut());
            CFRelease(url.cast());
            if status != 0 {
                return Err(format!("Could not open installer: {status}"));
            }
        }
    }
    Ok(())
}
