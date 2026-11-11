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
    shell_open(path.as_os_str()).map_err(|code| {
        format!(
            "Could not open installer (Windows error {code}). Open the downloaded file from Explorer."
        )
    })?;
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let bytes = path.as_os_str().as_bytes();
        // SAFETY: the URL is created from a live byte slice; the +1 reference is
        // handed to `launch`, which releases it.
        let url = unsafe {
            core_foundation_sys::url::CFURLCreateFromFileSystemRepresentation(
                core_foundation_sys::base::kCFAllocatorDefault,
                bytes.as_ptr(),
                bytes.len() as isize,
                0,
            )
        };
        launch(url)
            .ok_or("Could not create installer file URL")?
            .map_err(|status| format!("Could not open installer: {status}"))?;
    }
    Ok(())
}

/// `ShellExecuteW` "open" for a file or URL; returns the Windows error code on failure.
#[cfg(windows)]
fn shell_open(target: &std::ffi::OsStr) -> Result<(), isize> {
    use std::os::windows::ffi::OsStrExt;
    let target: Vec<u16> = target.encode_wide().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    // SAFETY: both terminated strings remain alive until ShellExecuteW returns.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    } as isize;
    if result <= 32 { Err(result) } else { Ok(()) }
}

/// Open a newly created (+1) URL with Launch Services, then release it.
/// `None` when the URL could not be created; otherwise the Launch Services status.
#[cfg(target_os = "macos")]
fn launch(url: core_foundation_sys::url::CFURLRef) -> Option<Result<(), i32>> {
    use core_foundation_sys::{base::CFRelease, url::CFURLRef};
    #[link(name = "CoreServices", kind = "framework")]
    unsafe extern "C" {
        fn LSOpenCFURLRef(url: CFURLRef, launched: *mut CFURLRef) -> i32;
    }
    if url.is_null() {
        return None;
    }
    // SAFETY: `url` is a live, owned CFURL; it is released exactly once here.
    let status = unsafe {
        let status = LSOpenCFURLRef(url, std::ptr::null_mut());
        CFRelease(url.cast());
        status
    };
    Some(if status == 0 { Ok(()) } else { Err(status) })
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
    shell_open(std::ffi::OsStr::new(url))
        .map_err(|code| format!("Could not open browser (Windows error {code})."))?;
    #[cfg(target_os = "macos")]
    {
        // SAFETY: live UTF-8 bytes; the +1 reference is handed to `launch`.
        let native_url = unsafe {
            core_foundation_sys::url::CFURLCreateWithBytes(
                core_foundation_sys::base::kCFAllocatorDefault,
                url.as_ptr(),
                url.len() as isize,
                core_foundation_sys::string::kCFStringEncodingUTF8,
                std::ptr::null(),
            )
        };
        launch(native_url)
            .ok_or("Could not create browser URL.")?
            .map_err(|status| format!("Could not open browser: {status}"))?;
    }
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    return Err("Opening browser links is unavailable on this platform.".into());
    Ok(())
}
