//! Fenêtre de sortie toujours au premier plan, sans curseur visible.
//!
//! La fenêtre est créée « au-dessus de tout » (`WindowLevel::AlwaysOnTop`), sans voler le
//! focus à la régie et sans bouton dans la barre des tâches. Sous Windows on va plus loin :
//! - aucune fenêtre ne peut se glisser devant (ordre imposé à chaque changement, et rappelé
//!   régulièrement pour passer devant les autres fenêtres « toujours visibles ») ;
//! - le curseur reste invisible au-dessus de la sortie, focus ou pas, sans y être enfermé
//!   (le masquage de winit coince le curseur au centre de la fenêtre dès qu'elle a le focus).
//!
//! La touche F fait passer la sortie dans une fenêtre normale et la remet en plein écran ;
//! Échap la remet seulement dans une fenêtre (voir `window_event`). En fenêtre, le premier plan
//! et le curseur caché sont suspendus (`set_enabled`).

use winit::window::{Window, WindowAttributes, WindowLevel};

/// Réglages de création de la fenêtre de sortie plein écran.
pub fn attributes(attrs: WindowAttributes) -> WindowAttributes {
    let attrs = attrs.with_window_level(WindowLevel::AlwaysOnTop).with_active(false);
    #[cfg(windows)]
    let attrs = {
        use winit::platform::windows::WindowAttributesExtWindows;
        attrs.with_skip_taskbar(true)
    };
    attrs
}

/// À appeler une fois la fenêtre plein écran créée.
pub fn install(window: &Window) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(h) = window.window_handle() {
            if let RawWindowHandle::Win32(h) = h.as_raw() {
                win::install(h.hwnd.get());
                return;
            }
        }
    }
    window.set_cursor_visible(false);
}

/// Premier plan et curseur caché en marche (plein écran) ou suspendus (sortie dans une fenêtre).
pub fn set_enabled(window: &Window, on: bool) {
    #[cfg(windows)]
    {
        use winit::platform::windows::WindowExtWindows;
        win::ENABLED.store(on, std::sync::atomic::Ordering::SeqCst);
        window.set_skip_taskbar(on);
    }
    window.set_window_level(if on { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal });
    window.set_cursor_visible(!on);
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, SetCursor, SetTimer, SetWindowLongPtrW, SetWindowPos, GWLP_WNDPROC, HTCLIENT, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, WINDOWPOS, WM_SETCURSOR, WM_TIMER, WM_WINDOWPOSCHANGING, WNDPROC,
    };

    /// Procédure d'origine de la fenêtre (celle de winit). Une seule fenêtre de sortie par processus.
    static PREVIOUS: AtomicIsize = AtomicIsize::new(0);
    /// Faux pendant que la sortie est dans une fenêtre : la procédure laisse tout passer.
    pub static ENABLED: AtomicBool = AtomicBool::new(true);
    const TIMER_ID: usize = 0xC0E5;
    /// Rappel du premier plan (ms) : passe devant une fenêtre « toujours visible » ouverte après.
    const TIMER_MS: u32 = 500;

    pub fn install(hwnd: isize) {
        let hw = HWND(hwnd as *mut c_void);
        unsafe {
            let old = SetWindowLongPtrW(hw, GWLP_WNDPROC, proc_ as *const () as isize);
            PREVIOUS.store(old, Ordering::SeqCst);
            raise(hw);
            SetTimer(Some(hw), TIMER_ID, TIMER_MS, None);
        }
    }

    unsafe fn raise(hw: HWND) {
        let _ = unsafe { SetWindowPos(hw, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER) };
    }

    unsafe extern "system" fn proc_(hw: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let on = ENABLED.load(Ordering::SeqCst);
        match msg {
            // Curseur invisible au-dessus de l'image, sans l'enfermer.
            WM_SETCURSOR if on && (lparam.0 as u32 & 0xFFFF) == HTCLIENT => {
                unsafe { SetCursor(None) };
                return LRESULT(1);
            }
            WM_TIMER if wparam.0 == TIMER_ID => {
                if on {
                    unsafe { raise(hw) };
                }
                return LRESULT(0);
            }
            // Toute remise en ordre de la fenêtre la garde tout en haut.
            WM_WINDOWPOSCHANGING if on => {
                let wp = lparam.0 as *mut WINDOWPOS;
                if !wp.is_null() {
                    let wp = unsafe { &mut *wp };
                    if (wp.flags & SWP_NOZORDER).0 == 0 {
                        wp.hwndInsertAfter = HWND_TOPMOST;
                    }
                }
            }
            _ => {}
        }
        let prev: WNDPROC = unsafe { std::mem::transmute(PREVIOUS.load(Ordering::SeqCst)) };
        unsafe { CallWindowProcW(prev, hw, msg, wparam, lparam) }
    }
}
