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
        portal_open(Some(&path), None)?;
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

#[cfg(target_os = "linux")]
fn portal_open(path: Option<&std::path::Path>, uri: Option<&str>) -> Result<(), String> {
    use std::{collections::HashMap, os::fd::AsFd};
    let file = path
        .map(std::fs::File::open)
        .transpose()
        .map_err(|e| e.to_string())?;
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
    let handle: zbus::zvariant::OwnedObjectPath = (if let Some(file) = &file {
        portal.call(
            "OpenFile",
            &("", zbus::zvariant::Fd::from(file.as_fd()), options),
        )
    } else {
        portal.call("OpenURI", &("", uri.unwrap_or_default(), options))
    })
    .map_err(|e| {
        format!(
            "Could not open link or file: {e}. Try opening it from your browser or file manager."
        )
    })?;
    if handle.as_str() != request_path {
        return Err("The desktop returned an unexpected open request. Try opening the link or file manually.".into());
    }
    let response = responses
        .next()
        .ok_or("The desktop link or file request ended without a response")?;
    let (status, _): (u32, HashMap<String, zbus::zvariant::OwnedValue>) =
        response.body().deserialize().map_err(|e| e.to_string())?;
    if status != 0 {
        return Err("The desktop did not open the link or file. Try opening it manually.".into());
    }

    Ok(())
}

/// Open an HTTPS link using the desktop's native URL handler, without helpers.
pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") || url.contains(['\0', '\r', '\n']) {
        return Err("Only HTTPS links can be opened.".into());
    }
    #[cfg(target_os = "linux")]
    portal_open(None, Some(url))?;
    #[cfg(windows)]
    {
        let url: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        // SAFETY: both terminated strings remain live through the native call.
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        } as isize;
        if result <= 32 {
            return Err(format!("Could not open browser (Windows error {result})."));
        }
    }
    #[cfg(target_os = "macos")]
    {
        use core_foundation_sys::{
            base::{CFRelease, kCFAllocatorDefault},
            string::kCFStringEncodingUTF8,
            url::{CFURLCreateWithBytes, CFURLRef},
        };
        #[link(name = "CoreServices", kind = "framework")]
        unsafe extern "C" {
            fn LSOpenCFURLRef(url: CFURLRef, launched: *mut CFURLRef) -> i32;
        }
        // SAFETY: live UTF-8 bytes; release the created reference after opening.
        unsafe {
            let native_url = CFURLCreateWithBytes(
                kCFAllocatorDefault,
                url.as_ptr(),
                url.len() as isize,
                kCFStringEncodingUTF8,
                std::ptr::null(),
            );
            if native_url.is_null() {
                return Err("Could not create browser URL.".into());
            }
            let status = LSOpenCFURLRef(native_url, std::ptr::null_mut());
            CFRelease(native_url.cast());
            if status != 0 {
                return Err(format!("Could not open browser: {status}"));
            }
        }
    }
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    return Err("Opening browser links is unavailable on this platform.".into());
    Ok(())
}
