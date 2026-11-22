//! Whether the desktop asks apps to reduce motion. Read once at startup; when
//! the setting cannot be read, animations stay on.
// Hardened module: no unchecked indexing or arithmetic in shipped code (CWE-125, CWE-190).
#![cfg_attr(
    not(test),
    deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)
)]

/// GNOME's "Animations" switch, or KDE's animation speed set to instant, read
/// through the desktop portal so it also works inside a sandbox.
#[cfg(target_os = "linux")]
pub fn reduce_motion() -> bool {
    use zbus::zvariant::OwnedValue;
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(portal) = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    ) else {
        return false;
    };
    let read = |namespace: &str, key: &str| -> Option<OwnedValue> {
        portal
            .call::<_, _, OwnedValue>("ReadOne", &(namespace, key))
            .ok()
    };
    if let Some(enabled) = read("org.gnome.desktop.interface", "enable-animations")
        .and_then(|v| bool::try_from(v).ok())
    {
        return !enabled;
    }
    read("org.kde.kdeglobals.KDE", "AnimationDurationFactor")
        .and_then(|v| animation_factor(&v))
        .is_some_and(|factor| factor <= 0.0)
}

/// KDE's animation speed. Its portal sends the config entry as text ("0"),
/// so both a number and a numeric string are accepted.
#[cfg(target_os = "linux")]
fn animation_factor(value: &zbus::zvariant::Value<'_>) -> Option<f64> {
    use zbus::zvariant::Value;
    if let Value::F64(factor) = value {
        Some(*factor)
    } else if let Value::Str(text) = value {
        text.as_str().trim().parse().ok()
    } else if let Value::Value(inner) = value {
        animation_factor(inner)
    } else {
        None
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::animation_factor;
    use zbus::zvariant::Value;

    #[test]
    fn kde_animation_factor_reads_text_and_numbers() {
        assert_eq!(animation_factor(&Value::from("0")), Some(0.0));
        assert_eq!(animation_factor(&Value::from("1")), Some(1.0));
        assert_eq!(animation_factor(&Value::from(0.5_f64)), Some(0.5));
        assert_eq!(animation_factor(&Value::new(Value::from("0"))), Some(0.0));
        assert_eq!(animation_factor(&Value::from("fast")), None);
        assert_eq!(animation_factor(&Value::from(true)), None);
    }
}

/// Windows "Show animations in Windows" (Settings > Accessibility > Visual effects).
#[cfg(windows)]
pub fn reduce_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    let mut enabled: windows_sys::core::BOOL = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the live local.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut enabled).cast(), 0)
    };
    ok != 0 && enabled == 0
}

/// macOS "Reduce motion" (System Settings > Accessibility > Display).
#[cfg(target_os = "macos")]
pub fn reduce_motion() -> bool {
    use core_foundation_sys::{
        base::{Boolean, CFRelease, kCFAllocatorDefault},
        preferences::CFPreferencesGetAppBooleanValue,
        string::{CFStringCreateWithBytes, CFStringRef, kCFStringEncodingUTF8},
    };
    let string = |text: &str| -> CFStringRef {
        // SAFETY: live UTF-8 bytes; the +1 string is released below.
        unsafe {
            CFStringCreateWithBytes(
                kCFAllocatorDefault,
                text.as_ptr(),
                isize::try_from(text.len()).unwrap_or(isize::MAX),
                kCFStringEncodingUTF8,
                0,
            )
        }
    };
    let key = string("reduceMotion");
    let app = string("com.apple.universalaccess");
    let mut valid: Boolean = 0;
    let reduce = !key.is_null()
        && !app.is_null()
        // SAFETY: both strings are live CFStrings; `valid` is a live local.
        && unsafe { CFPreferencesGetAppBooleanValue(key, app, &raw mut valid) } != 0
        && valid != 0;
    for owned in [key, app] {
        if !owned.is_null() {
            // SAFETY: each string was created above (+1) and is released once.
            unsafe { CFRelease(owned.cast()) };
        }
    }
    reduce
}

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
pub fn reduce_motion() -> bool {
    false
}
