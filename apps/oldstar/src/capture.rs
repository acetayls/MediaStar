//! Fenêtres du PC : recherche par titre, capture (même cachée derrière d'autres) et clics.
//! Windows ci-dessous ; macOS dans `capture_mac.rs` ; ailleurs, pas de capture.
//!
//! La capture passe par `PrintWindow(PW_CLIENTONLY | PW_RENDERFULLCONTENT)` : la fenêtre se
//! dessine elle-même dans une image en mémoire, y compris le contenu DirectX/DWM. Une fenêtre
//! réduite dans la barre des tâches ne se dessine plus : il faut la laisser ouverte (derrière
//! d'autres si besoin, ou sur un autre écran).
//!
//! Les touchers arrivent en coordonnées de la zone cliente de la fenêtre.
//! - `sendinput` : vrais clics souris (la fenêtre passe au premier plan, le curseur revient
//!   ensuite à sa place) ; marche avec toutes les applis.
//! - `postmessage` : messages WM_LBUTTON* envoyés à la fenêtre sous le point, qui peut rester
//!   derrière ; certaines applis (DirectX, WPF) les ignorent.

/// Image BGRA, lignes de haut en bas.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub bgra: Vec<u8>,
}

/// Une fenêtre de premier niveau visible, pour la recherche et le choix libre.
#[derive(Clone, Debug, serde::Serialize)]
pub struct WinInfo {
    pub id: isize,
    pub title: String,
    /// Nom du programme (« Lake Controller.exe ») et chemin complet.
    pub exe: String,
    pub path: String,
    /// Réduite dans la barre des tâches : elle ne se dessine plus, la capture échoue.
    pub minimized: bool,
    /// Surface de la fenêtre en pixels, pour choisir la principale.
    pub area: i64,
}

/// Critères de recherche d'une fenêtre de la config. Tous ceux qui sont remplis doivent
/// correspondre (sans tenir compte des majuscules) :
/// - `title` : un bout du titre, ou plusieurs séparés par « | » ;
/// - `exe` : motifs du nom du programme, avec * et ? (« lake*.exe ») ;
/// - `path` : un bout du chemin du programme (« \Lake\ » dans `C:\Program Files (x86)\Lake\…`).
pub struct Match<'a> {
    pub title: &'a str,
    pub exe: &'a [String],
    pub path: &'a str,
}

impl Match<'_> {
    pub fn is_empty(&self) -> bool {
        self.title.trim().is_empty() && self.exe.iter().all(|e| e.trim().is_empty()) && self.path.trim().is_empty()
    }

    pub fn matches(&self, w: &WinInfo) -> bool {
        if self.is_empty() {
            return false;
        }
        let title = w.title.to_lowercase();
        let title_ok = self.title.trim().is_empty()
            || self.title.split('|').map(|t| t.trim().to_lowercase()).any(|t| !t.is_empty() && title.contains(&t));
        let exes: Vec<&String> = self.exe.iter().filter(|e| !e.trim().is_empty()).collect();
        let exe_ok = exes.is_empty() || exes.iter().any(|e| wildcard(e.trim(), &w.exe));
        let path_ok = self.path.trim().is_empty() || w.path.to_lowercase().contains(&self.path.trim().to_lowercase());
        title_ok && exe_ok && path_ok
    }
}

/// Motif avec * (n'importe quoi) et ? (un caractère), sans tenir compte des majuscules.
pub fn wildcard(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            pi += 1;
            mark = ti;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Rythme d'un glisser au premier plan (millisecondes, pixels), réglable dans oldstar.json.
#[derive(Clone, Copy, Debug)]
pub struct DragTiming {
    /// Curseur posé au point de départ, puis attente avant d'enfoncer le bouton.
    pub before_down_ms: u64,
    /// Bouton enfoncé sans bouger avant les premiers mouvements.
    pub hold_ms: u64,
    /// Pas des mouvements intermédiaires et attente entre deux pas.
    pub step_px: i32,
    pub step_ms: u64,
    /// Arrivé au point final, attente avant de relâcher (survol de la cible).
    pub before_up_ms: u64,
    /// Après le relâché, attente avant de remettre la souris à sa place : l'appli doit avoir
    /// traité le dépôt avec le curseur encore sur la cible.
    pub after_up_ms: u64,
}

impl Default for DragTiming {
    fn default() -> Self {
        Self { before_down_ms: 50, hold_ms: 200, step_px: 8, step_ms: 12, before_up_ms: 100, after_up_ms: 15 }
    }
}

static TIMING: std::sync::Mutex<Option<DragTiming>> = std::sync::Mutex::new(None);

pub fn set_drag_timing(t: DragTiming) {
    if let Ok(mut g) = TIMING.lock() {
        *g = Some(t);
    }
}

#[allow(dead_code)]
fn drag_timing() -> DragTiming {
    TIMING.lock().ok().and_then(|g| *g).unwrap_or_default()
}

/// Fenêtre qui correspond ; s'il y en a plusieurs, la principale : non réduite, la plus grande.
pub fn find(m: &Match) -> Option<isize> {
    list().into_iter().filter(|w| m.matches(w)).max_by_key(|w| (!w.minimized, w.area)).map(|w| w.id)
}

#[cfg(windows)]
pub use win::*;

#[cfg(target_os = "macos")]
#[path = "capture_mac.rs"]
mod mac;
#[cfg(target_os = "macos")]
pub use mac::*;

#[cfg(not(any(windows, target_os = "macos")))]
mod other {
    use super::Frame;
    use super::WinInfo;
    /// Capture et clics : seulement sous Windows et macOS pour l'instant.
    pub const AVAILABLE: bool = false;
    pub fn dpi_aware() {}
    pub fn list() -> Vec<WinInfo> {
        vec![]
    }
    pub fn is_window(_h: isize) -> bool {
        false
    }
    pub fn client_size(_h: isize) -> Option<(i32, i32)> {
        None
    }
    pub fn capture(_h: isize) -> Result<Frame, String> {
        Err("capture disponible seulement sous Windows et macOS".into())
    }
    pub fn input(_h: isize, _mode: &str, _kind: &str, _x: i32, _y: i32, _wheel: i32) -> Result<(), String> {
        Err("clics disponibles seulement sous Windows et macOS".into())
    }
}
#[cfg(not(any(windows, target_os = "macos")))]
pub use other::*;

#[cfg(windows)]
mod win {
    use super::Frame;
    use std::sync::Mutex;
    use windows::core::BOOL;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT, WPARAM};
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::Graphics::Gdi::{
        ClientToScreen, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, MapWindowPoints, ReleaseDC, SelectObject, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS, PW_CLIENTONLY};
    use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
        MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT, MOUSE_EVENT_FLAGS, VK_MENU,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        ChildWindowFromPointEx, EnumWindows, GetClientRect, GetCursorPos, GetForegroundWindow, GetSystemMetrics, GetWindowLongW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
        IsIconic, IsWindow, IsWindowVisible, PostMessageW, SetCursorPos, SetForegroundWindow, ShowWindow, CWP_SKIPDISABLED, CWP_SKIPINVISIBLE,
        CWP_SKIPTRANSPARENT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_RESTORE, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP,
        WM_MOUSEMOVE, WM_MOUSEWHEEL,
    };

    /// Rendu complet, contenu DirectX compris (constante absente des en-têtes anciens).
    const PW_RENDERFULLCONTENT: u32 = 2;

    fn hwnd(h: isize) -> HWND {
        HWND(h as *mut core::ffi::c_void)
    }

    /// Coordonnées en vrais pixels, quel que soit le zoom de l'écran (125 %, 150 %…).
    pub fn dpi_aware() {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    fn title_of(h: HWND) -> String {
        let mut buf = [0u16; 512];
        let n = unsafe { GetWindowTextW(h, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    pub const AVAILABLE: bool = true;

    /// Chemin complet du programme d'un processus.
    fn exe_path(pid: u32) -> String {
        unsafe {
            let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return String::new() };
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(h);
            if ok {
                String::from_utf16_lossy(&buf[..len as usize])
            } else {
                String::new()
            }
        }
    }

    /// Fenêtres de premier niveau visibles et titrées, hors OldStar et hors palettes d'outils.
    pub fn list() -> Vec<super::WinInfo> {
        unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
            let out = &mut *(l.0 as *mut Vec<HWND>);
            out.push(h);
            BOOL(1)
        }
        let mut all: Vec<HWND> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(cb), LPARAM(&mut all as *mut _ as isize));
        }
        let me = std::process::id();
        let mut paths: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        let mut out = Vec::new();
        for h in all {
            unsafe {
                if !IsWindowVisible(h).as_bool() {
                    continue;
                }
                if (GetWindowLongW(h, GWL_EXSTYLE) as u32) & WS_EX_TOOLWINDOW.0 != 0 {
                    continue;
                }
            }
            let title = title_of(h);
            if title.trim().is_empty() {
                continue;
            }
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
            if pid == me {
                continue;
            }
            let path = paths.entry(pid).or_insert_with(|| exe_path(pid)).clone();
            let exe = path.rsplit(['\\', '/']).next().unwrap_or("").to_string();
            let mut r = RECT::default();
            let area = if unsafe { GetWindowRect(h, &mut r) }.is_ok() { ((r.right - r.left) as i64 * (r.bottom - r.top) as i64).max(0) } else { 0 };
            let minimized = unsafe { IsIconic(h).as_bool() };
            out.push(super::WinInfo { id: h.0 as isize, title, exe, path, minimized, area });
        }
        out
    }

    pub fn is_window(h: isize) -> bool {
        unsafe { IsWindow(Some(hwnd(h))).as_bool() }
    }

    pub fn client_size(h: isize) -> Option<(i32, i32)> {
        let mut r = RECT::default();
        unsafe { GetClientRect(hwnd(h), &mut r).ok()? };
        Some((r.right - r.left, r.bottom - r.top))
    }

    pub fn capture(h: isize) -> Result<Frame, String> {
        let hw = hwnd(h);
        if unsafe { IsIconic(hw).as_bool() } {
            return Err("fenêtre réduite dans la barre des tâches".into());
        }
        let (w, hgt) = client_size(h).ok_or("taille de la fenêtre introuvable")?;
        if w <= 0 || hgt <= 0 {
            return Err("fenêtre vide".into());
        }
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let mut info = BITMAPINFO::default();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -hgt,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let result = match CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(bmp) => {
                    let old = SelectObject(mem, bmp.into());
                    let ok = PrintWindow(hw, mem, PRINT_WINDOW_FLAGS(PW_CLIENTONLY.0 | PW_RENDERFULLCONTENT)).as_bool();
                    let len = (w * hgt * 4) as usize;
                    let data = if ok && !bits.is_null() { Some(std::slice::from_raw_parts(bits as *const u8, len).to_vec()) } else { None };
                    SelectObject(mem, old);
                    let _ = DeleteObject(bmp.into());
                    data.map(|bgra| Frame { w: w as u32, h: hgt as u32, bgra }).ok_or_else(|| "PrintWindow a échoué".to_string())
                }
                Err(e) => Err(format!("image en mémoire impossible ({e})")),
            };
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            result
        }
    }

    fn mouse(flags: MOUSE_EVENT_FLAGS, dx: i32, dy: i32, data: i32) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: data as u32, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        }
    }

    fn key(up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_MENU, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() }, time: 0, dwExtraInfo: 0 } },
        }
    }

    /// Point de l'écran → coordonnées absolues de SendInput (0 à 65535 sur tout le bureau).
    fn absolute(p: POINT) -> (i32, i32) {
        unsafe {
            let (vx, vy) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
            let (vw, vh) = (GetSystemMetrics(SM_CXVIRTUALSCREEN).max(2), GetSystemMetrics(SM_CYVIRTUALSCREEN).max(2));
            (((p.x - vx) as i64 * 65535 / (vw - 1) as i64) as i32, ((p.y - vy) as i64 * 65535 / (vh - 1) as i64) as i32)
        }
    }

    fn bring_to_front(hw: HWND) {
        unsafe {
            if IsIconic(hw).as_bool() {
                let _ = ShowWindow(hw, SW_RESTORE);
            }
            if GetForegroundWindow() != hw {
                // Windows refuse le premier plan à un processus en arrière-plan, sauf juste après
                // une touche : un appui sur Alt débloque SetForegroundWindow.
                SendInput(&[key(false), key(true)], std::mem::size_of::<INPUT>() as i32);
                let _ = SetForegroundWindow(hw);
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
    }

    struct Drag {
        /// Position du curseur avant le clic (sendinput), à rétablir au relâché.
        cursor: Option<POINT>,
        /// Fenêtre enfant qui reçoit les messages (postmessage).
        child: Option<isize>,
    }

    static DRAG: Mutex<Option<Drag>> = Mutex::new(None);
    /// Dernière position envoyée pendant un glisser (écran), pour interpoler les déplacements.
    static LAST: Mutex<Option<POINT>> = Mutex::new(None);

    /// Déplacements intermédiaires de `from` à `to`, par pas de `step_px` espacés de `step_ms`
    /// (60 pas au plus) : certaines applis (Lake…) ne détectent un glisser qu'avec des
    /// mouvements continus, pas en rafale.
    fn glide(from: Option<POINT>, to: POINT) {
        let t = super::drag_timing();
        let abs = MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK | MOUSEEVENTF_MOVE;
        let Some(f) = from else {
            let (ax, ay) = absolute(to);
            unsafe { SendInput(&[mouse(abs, ax, ay, 0)], std::mem::size_of::<INPUT>() as i32) };
            return;
        };
        let dist = ((to.x - f.x).abs()).max((to.y - f.y).abs());
        let steps = (dist / t.step_px.max(1)).clamp(1, 60);
        for i in 1..=steps {
            let p = POINT { x: f.x + (to.x - f.x) * i / steps, y: f.y + (to.y - f.y) * i / steps };
            let (ax, ay) = absolute(p);
            unsafe { SendInput(&[mouse(abs, ax, ay, 0)], std::mem::size_of::<INPUT>() as i32) };
            if i < steps {
                std::thread::sleep(std::time::Duration::from_millis(t.step_ms));
            }
        }
    }

    fn lparam(x: i32, y: i32) -> LPARAM {
        LPARAM((((y as u32 & 0xffff) << 16) | (x as u32 & 0xffff)) as isize)
    }

    /// `kind` : down, move, up (glisser), click, dblclick, rclick (clic droit) ou wheel
    /// (`wheel` en crans de 120).
    pub fn input(h: isize, mode: &str, kind: &str, x: i32, y: i32, wheel: i32) -> Result<(), String> {
        let hw = hwnd(h);
        let mut p = POINT { x, y };
        unsafe {
            if !ClientToScreen(hw, &mut p).as_bool() {
                return Err("position introuvable".into());
            }
        }
        let mut drag = DRAG.lock().map_err(|_| "verrou")?;
        if mode == "postmessage" {
            let target = match (kind, drag.as_ref().and_then(|d| d.child)) {
                ("move" | "up", Some(c)) => hwnd(c),
                _ => deepest_child(hw, POINT { x, y }),
            };
            let mut pt = [POINT { x, y }];
            unsafe { MapWindowPoints(Some(hw), Some(target), &mut pt) };
            let (cx, cy) = (pt[0].x, pt[0].y);
            let r = unsafe {
                match kind {
                    "down" => {
                        *drag = Some(Drag { cursor: None, child: Some(target.0 as isize) });
                        let _ = PostMessageW(Some(target), WM_MOUSEMOVE, WPARAM(0), lparam(cx, cy));
                        PostMessageW(Some(target), WM_LBUTTONDOWN, WPARAM(1), lparam(cx, cy))
                    }
                    "move" => PostMessageW(Some(target), WM_MOUSEMOVE, WPARAM(if drag.is_some() { 1 } else { 0 }), lparam(cx, cy)),
                    "up" => {
                        *drag = None;
                        PostMessageW(Some(target), WM_LBUTTONUP, WPARAM(0), lparam(cx, cy))
                    }
                    "click" => {
                        *drag = None;
                        let _ = PostMessageW(Some(target), WM_MOUSEMOVE, WPARAM(0), lparam(cx, cy));
                        let _ = PostMessageW(Some(target), WM_LBUTTONDOWN, WPARAM(1), lparam(cx, cy));
                        PostMessageW(Some(target), WM_LBUTTONUP, WPARAM(0), lparam(cx, cy))
                    }
                    // Séquence d'un vrai double-clic pour une fenêtre CS_DBLCLKS.
                    "dblclick" => {
                        *drag = None;
                        let _ = PostMessageW(Some(target), WM_MOUSEMOVE, WPARAM(0), lparam(cx, cy));
                        let _ = PostMessageW(Some(target), WM_LBUTTONDOWN, WPARAM(1), lparam(cx, cy));
                        let _ = PostMessageW(Some(target), WM_LBUTTONUP, WPARAM(0), lparam(cx, cy));
                        let _ = PostMessageW(Some(target), WM_LBUTTONDBLCLK, WPARAM(1), lparam(cx, cy));
                        PostMessageW(Some(target), WM_LBUTTONUP, WPARAM(0), lparam(cx, cy))
                    }
                    "rclick" => {
                        *drag = None;
                        let _ = PostMessageW(Some(target), WM_MOUSEMOVE, WPARAM(0), lparam(cx, cy));
                        let _ = PostMessageW(Some(target), WM_RBUTTONDOWN, WPARAM(2), lparam(cx, cy));
                        PostMessageW(Some(target), WM_RBUTTONUP, WPARAM(0), lparam(cx, cy))
                    }
                    // La molette se donne en coordonnées de l'écran.
                    "wheel" => PostMessageW(Some(target), WM_MOUSEWHEEL, WPARAM(((wheel as i16 as u16 as usize) << 16) as usize), lparam(p.x, p.y)),
                    _ => Ok(()),
                }
            };
            return r.map_err(|e| e.to_string());
        }
        let (ax, ay) = absolute(p);
        let abs = MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK | MOUSEEVENTF_MOVE;
        let inputs: Vec<INPUT> = match kind {
            "down" => {
                let mut cur = POINT::default();
                let cursor = unsafe { GetCursorPos(&mut cur) }.ok().map(|_| cur);
                *drag = Some(Drag { cursor, child: None });
                bring_to_front(hw);
                if let Ok(mut l) = LAST.lock() {
                    *l = Some(p);
                }
                // Comme une main : curseur posé, petite pause, bouton enfoncé, maintien.
                let t = super::drag_timing();
                unsafe { SendInput(&[mouse(abs, ax, ay, 0)], std::mem::size_of::<INPUT>() as i32) };
                std::thread::sleep(std::time::Duration::from_millis(t.before_down_ms));
                let n = unsafe { SendInput(&[mouse(MOUSEEVENTF_LEFTDOWN, 0, 0, 0)], std::mem::size_of::<INPUT>() as i32) };
                std::thread::sleep(std::time::Duration::from_millis(t.hold_ms));
                if n == 0 {
                    return Err("SendInput refusé (fenêtre d'un programme lancé en administrateur ?)".into());
                }
                vec![]
            }
            // Glisser : mouvements intermédiaires depuis la position précédente.
            "move" => {
                let from = LAST.lock().ok().and_then(|mut l| l.replace(p));
                glide(from, p);
                vec![]
            }
            "up" => {
                let from = LAST.lock().ok().and_then(|mut l| l.take());
                glide(from, p);
                std::thread::sleep(std::time::Duration::from_millis(super::drag_timing().before_up_ms));
                vec![mouse(abs, ax, ay, 0), mouse(MOUSEEVENTF_LEFTUP, 0, 0, 0)]
            }
            // Clics complets en un seul envoi : le double-clic tombe bien sous GetDoubleClickTime().
            "click" | "dblclick" | "rclick" => {
                let mut cur = POINT::default();
                let cursor = unsafe { GetCursorPos(&mut cur) }.ok().map(|_| cur);
                *drag = Some(Drag { cursor, child: None });
                bring_to_front(hw);
                let (d, u) = if kind == "rclick" { (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP) } else { (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP) };
                let mut v = vec![mouse(abs, ax, ay, 0), mouse(d, 0, 0, 0), mouse(u, 0, 0, 0)];
                if kind == "dblclick" {
                    v.push(mouse(d, 0, 0, 0));
                    v.push(mouse(u, 0, 0, 0));
                }
                v
            }
            "wheel" => {
                bring_to_front(hw);
                vec![mouse(abs, ax, ay, 0), mouse(MOUSEEVENTF_WHEEL, 0, 0, wheel)]
            }
            _ => vec![],
        };
        let sent = if inputs.is_empty() { 0 } else { unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } };

        if matches!(kind, "up" | "click" | "dblclick" | "rclick") {
            if let Some(Drag { cursor: Some(c), .. }) = drag.take() {
                // Après un glisser, on laisse l'appli finir le dépôt avant de bouger la souris.
                let wait = if kind == "up" { super::drag_timing().after_up_ms } else { 15 };
                std::thread::sleep(std::time::Duration::from_millis(wait));
                unsafe {
                    let _ = SetCursorPos(c.x, c.y);
                }
            }
        }
        if (sent as usize) < inputs.len() {
            return Err("SendInput refusé (fenêtre d'un programme lancé en administrateur ?)".into());
        }
        Ok(())
    }

    /// Fenêtre enfant la plus profonde sous un point de la zone cliente.
    fn deepest_child(top: HWND, client: POINT) -> HWND {
        let mut cur = top;
        let mut pt = client;
        for _ in 0..16 {
            let child = unsafe { ChildWindowFromPointEx(cur, pt, CWP_SKIPINVISIBLE | CWP_SKIPDISABLED | CWP_SKIPTRANSPARENT) };
            if child.0.is_null() || child == cur {
                break;
            }
            let mut p = [pt];
            unsafe { MapWindowPoints(Some(cur), Some(child), &mut p) };
            pt = p[0];
            cur = child;
        }
        cur
    }
}
