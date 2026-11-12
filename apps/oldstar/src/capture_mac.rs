//! Fenêtres macOS (Intel et Apple Silicon) : liste, capture et clics.
//!
//! - Liste et taille : `CGWindowListCopyWindowInfo` (fenêtres de niveau normal, hors OldStar).
//!   Les titres des fenêtres des autres applis n'apparaissent qu'avec l'autorisation
//!   « Enregistrement de l'écran » ; sans elle, le nom de l'appli sert de titre.
//! - Capture : ScreenCaptureKit (`SCScreenshotManager`, macOS 14 et plus), même fenêtre cachée
//!   derrière d'autres ; avant macOS 14, `CGWindowListCreateImage`. Image en points (pas en
//!   pixels Retina) : zones, tailles et touchers sont tous en points.
//! - Clics : `CGEvent`, autorisation « Accessibilité » nécessaire. Premier plan (`sendinput`) :
//!   l'appli passe devant, vrais clics, le curseur revient ensuite à sa place. Arrière-plan
//!   (`postmessage`) : événements envoyés au seul processus de la fenêtre (`CGEventPostToPid`),
//!   sans la faire passer devant ; certaines applis les ignorent.
//!
//! Les identifiants de fenêtre sont les `CGWindowID`.

use super::{Frame, WinInfo};
use std::ptr::NonNull;
use std::sync::mpsc;
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::AnyThread;
use objc2_core_foundation::{CFArray, CFDictionary, CFRetained, CFString, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    kCGWindowBounds, kCGWindowIsOnscreen, kCGWindowLayer, kCGWindowName, kCGWindowNumber, kCGWindowOwnerName, kCGWindowOwnerPID, CGBitmapContextCreate,
    CGColorSpace, CGContext, CGEvent, CGEventField, CGEventTapLocation, CGEventType, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo,
    CGMouseButton, CGPreflightScreenCaptureAccess, CGRectMakeWithDictionaryRepresentation, CGRequestScreenCaptureAccess, CGScrollEventUnit,
    CGWarpMouseCursorPosition, CGWindowImageOption, CGWindowListCopyWindowInfo, CGWindowListOption,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};
use objc2_screen_capture_kit::{SCContentFilter, SCScreenshotManager, SCShareableContent, SCStreamConfiguration, SCWindow};

pub const AVAILABLE: bool = true;

/// Rien à préparer au lancement : les autorisations sont demandées au premier usage.
pub fn dpi_aware() {}

/* ---------- Autorisations ---------- */

const SCREEN_HINT: &str = "autorisez OldStar dans Réglages Système › Confidentialité et sécurité › Enregistrement de l'écran, puis relancez OldStar";
const AX_HINT: &str = "autorisez OldStar dans Réglages Système › Confidentialité et sécurité › Accessibilité";

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrustedWithOptions(options: *const CFDictionary) -> bool;
    static kAXTrustedCheckOptionPrompt: &'static CFString;
}

/// Enregistrement de l'écran : demandé une fois (fenêtre du système), effectif après relance.
fn screen_allowed() -> bool {
    static ASK: Once = Once::new();
    if CGPreflightScreenCaptureAccess() {
        return true;
    }
    ASK.call_once(|| {
        crate::journal::warn(&format!("Capture des fenêtres : {SCREEN_HINT}."));
        CGRequestScreenCaptureAccess();
    });
    false
}

/// Accessibilité (clics simulés) : la première fois, macOS propose d'ouvrir les réglages.
fn input_allowed() -> bool {
    static ASK: Once = Once::new();
    let mut prompt = false;
    ASK.call_once(|| prompt = true);
    let ok = unsafe {
        if prompt {
            let key: &NSString = cf_str(kAXTrustedCheckOptionPrompt);
            let opts = NSDictionary::<NSString, AnyObject>::from_slices(&[key], &[NSNumber::new_bool(true).as_ref()]);
            AXIsProcessTrustedWithOptions(Retained::as_ptr(&opts) as *const CFDictionary)
        } else {
            AXIsProcessTrustedWithOptions(std::ptr::null())
        }
    };
    if !ok && prompt {
        crate::journal::warn(&format!("Clics dans les fenêtres : {AX_HINT}."));
    }
    ok
}

/* ---------- Liste des fenêtres ---------- */

/// CFString constant → NSString (même objet, « toll-free bridging »).
fn cf_str(s: &CFString) -> &NSString {
    unsafe { &*(s as *const CFString as *const NSString) }
}

struct Info {
    id: u32,
    pid: i32,
    title: String,
    owner: String,
    bounds: CGRect,
    layer: i64,
    onscreen: bool,
}

fn number(d: &NSDictionary<NSString, AnyObject>, key: &CFString) -> Option<Retained<NSNumber>> {
    d.objectForKey(cf_str(key))?.downcast::<NSNumber>().ok()
}

fn string(d: &NSDictionary<NSString, AnyObject>, key: &CFString) -> String {
    d.objectForKey(cf_str(key)).and_then(|o| o.downcast::<NSString>().ok()).map(|s| s.to_string()).unwrap_or_default()
}

fn infos(option: CGWindowListOption, id: u32) -> Vec<Info> {
    let Some(arr) = CGWindowListCopyWindowInfo(option, id) else { return vec![] };
    let arr: &NSArray<NSDictionary<NSString, AnyObject>> = unsafe { &*(CFRetained::as_ptr(&arr).as_ptr() as *const CFArray as *const NSArray<NSDictionary<NSString, AnyObject>>) };
    let mut out = Vec::new();
    for d in arr.iter() {
        let Some(id) = number(&d, unsafe { kCGWindowNumber }) else { continue };
        let mut bounds = CGRect::default();
        if let Some(b) = d.objectForKey(cf_str(unsafe { kCGWindowBounds })) {
            let dict = Retained::as_ptr(&b) as *const CFDictionary;
            unsafe { CGRectMakeWithDictionaryRepresentation(Some(&*dict), &mut bounds) };
        }
        out.push(Info {
            id: id.unsignedIntValue(),
            pid: number(&d, unsafe { kCGWindowOwnerPID }).map(|n| n.intValue()).unwrap_or(0),
            title: string(&d, unsafe { kCGWindowName }),
            owner: string(&d, unsafe { kCGWindowOwnerName }),
            bounds,
            layer: number(&d, unsafe { kCGWindowLayer }).map(|n| n.longLongValue()).unwrap_or(0),
            onscreen: number(&d, unsafe { kCGWindowIsOnscreen }).map(|n| n.boolValue()).unwrap_or(false),
        });
    }
    out
}

fn info(id: isize) -> Option<Info> {
    infos(CGWindowListOption::OptionIncludingWindow, id as u32).into_iter().find(|i| i.id == id as u32)
}

extern "C" {
    fn proc_pidpath(pid: i32, buffer: *mut core::ffi::c_void, size: u32) -> i32;
}

/// Chemin complet du programme d'un processus.
fn exe_path(pid: i32) -> String {
    let mut buf = [0u8; 4096];
    let n = unsafe { proc_pidpath(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    if n <= 0 {
        return String::new();
    }
    String::from_utf8_lossy(&buf[..n as usize]).into_owned()
}

/// Fenêtres de niveau normal (pas le Dock, les menus ni les bulles), hors OldStar.
pub fn list() -> Vec<WinInfo> {
    let _ = screen_allowed();
    let me = std::process::id() as i32;
    let mut paths: std::collections::HashMap<i32, String> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for i in infos(CGWindowListOption::OptionAll | CGWindowListOption::ExcludeDesktopElements, 0) {
        if i.layer != 0 || i.pid == me || i.bounds.size.width < 40.0 || i.bounds.size.height < 40.0 {
            continue;
        }
        // Sans l'autorisation d'enregistrement de l'écran, pas de titre : le nom de l'appli.
        let title = if i.title.trim().is_empty() { i.owner.clone() } else { i.title.clone() };
        if title.trim().is_empty() {
            continue;
        }
        let path = paths.entry(i.pid).or_insert_with(|| exe_path(i.pid)).clone();
        let exe = path.rsplit('/').next().unwrap_or("").to_string();
        let area = (i.bounds.size.width * i.bounds.size.height) as i64;
        // Hors écran : réduite dans le Dock, ou sur un autre bureau (Spaces).
        out.push(WinInfo { id: i.id as isize, title, exe, path, minimized: !i.onscreen, area });
    }
    out
}

pub fn is_window(h: isize) -> bool {
    info(h).is_some()
}

/// Taille de la fenêtre en points (cadre compris : macOS ne distingue pas de zone cliente).
pub fn client_size(h: isize) -> Option<(i32, i32)> {
    let i = info(h)?;
    Some((i.bounds.size.width.round() as i32, i.bounds.size.height.round() as i32))
}

/* ---------- Capture ---------- */

/// Objet Objective-C passé d'un fil à l'autre (les réponses de ScreenCaptureKit arrivent sur
/// une file à lui ; on n'y touche qu'une fois reçu).
struct Sendable<T>(T);
unsafe impl<T> Send for Sendable<T> {}

/// Fenêtres ScreenCaptureKit, gardées un instant : les demander coûte quelques millisecondes.
type ScList = Sendable<Retained<NSArray<SCWindow>>>;
static SC_WINDOWS: Mutex<Option<(Instant, ScList)>> = Mutex::new(None);

fn sc_window(id: u32) -> Result<Retained<SCWindow>, String> {
    let find = |list: &NSArray<SCWindow>| list.iter().find(|w| unsafe { w.windowID() } == id);
    let mut cache = SC_WINDOWS.lock().map_err(|_| "verrou")?;
    if let Some((at, list)) = cache.as_ref() {
        if at.elapsed() < Duration::from_secs(3) {
            if let Some(w) = find(&list.0) {
                return Ok(w);
            }
        }
    }
    let (tx, rx) = mpsc::channel::<Result<ScList, String>>();
    let block = block2::RcBlock::new(move |content: *mut SCShareableContent, err: *mut objc2_foundation::NSError| {
        let r = match unsafe { Retained::retain(content) } {
            Some(c) => Ok(Sendable(unsafe { c.windows() })),
            None => Err(unsafe { err.as_ref() }.map(|e| e.localizedDescription().to_string()).unwrap_or_else(|| "liste des fenêtres refusée".into())),
        };
        let _ = tx.send(r);
    });
    unsafe { SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(true, false, &block) };
    let list = rx.recv_timeout(Duration::from_secs(3)).map_err(|_| "ScreenCaptureKit ne répond pas".to_string())??;
    let w = find(&list.0);
    *cache = Some((Instant::now(), list));
    w.ok_or_else(|| "fenêtre introuvable pour ScreenCaptureKit".into())
}

/// macOS 14 et plus : `SCScreenshotManager` existe.
fn has_screenshot_manager() -> bool {
    objc2::runtime::AnyClass::get(c"SCScreenshotManager").is_some()
}

fn capture_sck(id: u32, w: usize, h: usize) -> Result<CFRetained<CGImage>, String> {
    let window = sc_window(id)?;
    let filter = unsafe { SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &window) };
    let config = unsafe { SCStreamConfiguration::new() };
    unsafe {
        config.setWidth(w);
        config.setHeight(h);
        config.setShowsCursor(false);
        config.setIgnoreShadowsSingleWindow(true);
    }
    let (tx, rx) = mpsc::channel::<Result<Sendable<CFRetained<CGImage>>, String>>();
    let block = block2::RcBlock::new(move |img: *mut CGImage, err: *mut objc2_foundation::NSError| {
        let r = match NonNull::new(img) {
            Some(p) => Ok(Sendable(unsafe { CFRetained::retain(p) })),
            None => Err(unsafe { err.as_ref() }.map(|e| e.localizedDescription().to_string()).unwrap_or_else(|| "capture refusée".into())),
        };
        let _ = tx.send(r);
    });
    unsafe { SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(&filter, &config, Some(&block)) };
    rx.recv_timeout(Duration::from_secs(3)).map_err(|_| "ScreenCaptureKit ne répond pas".to_string())?.map(|s| s.0)
}

#[allow(deprecated)]
fn capture_legacy(id: u32) -> Result<CFRetained<CGImage>, String> {
    // Rectangle nul = juste la fenêtre ; résolution nominale = en points.
    let null = CGRect { origin: CGPoint { x: f64::INFINITY, y: f64::INFINITY }, size: CGSize { width: 0.0, height: 0.0 } };
    objc2_core_graphics::CGWindowListCreateImage(
        null,
        CGWindowListOption::OptionIncludingWindow,
        id,
        CGWindowImageOption::BoundsIgnoreFraming | CGWindowImageOption::NominalResolution,
    )
    .ok_or_else(|| "capture refusée".into())
}

pub fn capture(h: isize) -> Result<Frame, String> {
    if !screen_allowed() {
        return Err(SCREEN_HINT.into());
    }
    let i = info(h).ok_or("fenêtre fermée")?;
    if !i.onscreen {
        return Err("fenêtre réduite dans le Dock ou sur un autre bureau".into());
    }
    let (w, hgt) = (i.bounds.size.width.round() as usize, i.bounds.size.height.round() as usize);
    if w == 0 || hgt == 0 {
        return Err("fenêtre vide".into());
    }
    let img = if has_screenshot_manager() { capture_sck(i.id, w, hgt)? } else { capture_legacy(i.id)? };
    to_bgra(&img, w, hgt)
}

/// Image quelconque → BGRA w × h (dessinée dans un tampon au format voulu).
fn to_bgra(img: &CGImage, w: usize, h: usize) -> Result<Frame, String> {
    let mut bgra = vec![0u8; w * h * 4];
    let space = CGColorSpace::new_device_rgb().ok_or("espace de couleurs")?;
    // Octets B, G, R, A : alpha en premier, entiers 32 bits petit-boutistes.
    let bitmap = CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0;
    let ctx = unsafe { CGBitmapContextCreate(bgra.as_mut_ptr().cast(), w, h, 8, w * 4, Some(&space), bitmap) }.ok_or("image en mémoire impossible")?;
    CGContext::draw_image(Some(&ctx), CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize { width: w as f64, height: h as f64 } }, Some(img));
    drop(ctx);
    Ok(Frame { w: w as u32, h: h as u32, bgra })
}

/* ---------- Clics ---------- */

struct Drag {
    /// Position du curseur avant le clic (premier plan), à rétablir au relâché.
    cursor: Option<CGPoint>,
    /// Arrière-plan : processus qui reçoit les événements.
    pid: Option<i32>,
}

static DRAG: Mutex<Option<Drag>> = Mutex::new(None);
/// Dernière position envoyée pendant un glisser, pour interpoler les déplacements.
static LAST: Mutex<Option<CGPoint>> = Mutex::new(None);

fn cursor_pos() -> Option<CGPoint> {
    let e = CGEvent::new(None)?;
    Some(CGEvent::location(Some(&e)))
}

fn mouse_event(kind: CGEventType, p: CGPoint, button: CGMouseButton, clicks: i64) -> Option<CFRetained<CGEvent>> {
    let e = CGEvent::new_mouse_event(None, kind, p, button)?;
    if clicks > 0 {
        CGEvent::set_integer_value_field(Some(&e), CGEventField::MouseEventClickState, clicks);
    }
    Some(e)
}

fn post(e: Option<CFRetained<CGEvent>>, pid: Option<i32>) {
    let Some(e) = e else { return };
    match pid {
        Some(pid) => CGEvent::post_to_pid(pid, Some(&e)),
        None => CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&e)),
    }
}

fn bring_to_front(pid: i32) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        #[allow(deprecated)]
        app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
        std::thread::sleep(Duration::from_millis(60));
    }
}

/// Déplacements intermédiaires de `from` à `to` (bouton enfoncé), au rythme réglé.
fn glide(from: Option<CGPoint>, to: CGPoint, pid: Option<i32>) {
    let t = super::drag_timing();
    let Some(f) = from else {
        post(mouse_event(CGEventType::LeftMouseDragged, to, CGMouseButton::Left, 0), pid);
        return;
    };
    let dist = (to.x - f.x).abs().max((to.y - f.y).abs());
    let steps = ((dist / t.step_px.max(1) as f64) as i32).clamp(1, 60);
    for i in 1..=steps {
        let k = i as f64 / steps as f64;
        let p = CGPoint { x: f.x + (to.x - f.x) * k, y: f.y + (to.y - f.y) * k };
        post(mouse_event(CGEventType::LeftMouseDragged, p, CGMouseButton::Left, 0), pid);
        if i < steps {
            std::thread::sleep(Duration::from_millis(t.step_ms));
        }
    }
}

/// `kind` : down, move, up (glisser), click, dblclick, rclick (clic droit) ou wheel
/// (`wheel` en crans de 120, positif vers le haut). `x`, `y` en points dans la fenêtre.
pub fn input(h: isize, mode: &str, kind: &str, x: i32, y: i32, wheel: i32) -> Result<(), String> {
    if !input_allowed() {
        return Err(AX_HINT.into());
    }
    let i = info(h).ok_or("fenêtre fermée")?;
    let p = CGPoint { x: i.bounds.origin.x + x as f64, y: i.bounds.origin.y + y as f64 };
    let background = mode == "postmessage";
    let target = if background { Some(i.pid) } else { None };
    let mut drag = DRAG.lock().map_err(|_| "verrou")?;
    let t = super::drag_timing();
    let save_cursor = |drag: &mut Option<Drag>| {
        *drag = Some(Drag { cursor: if background { None } else { cursor_pos() }, pid: target });
        if !background {
            bring_to_front(i.pid);
        }
    };
    match kind {
        "down" => {
            save_cursor(&mut drag);
            if let Ok(mut l) = LAST.lock() {
                *l = Some(p);
            }
            // Comme une main : curseur posé, petite pause, bouton enfoncé, maintien.
            post(mouse_event(CGEventType::MouseMoved, p, CGMouseButton::Left, 0), target);
            std::thread::sleep(Duration::from_millis(t.before_down_ms));
            post(mouse_event(CGEventType::LeftMouseDown, p, CGMouseButton::Left, 1), target);
            std::thread::sleep(Duration::from_millis(t.hold_ms));
        }
        "move" => {
            let pid = drag.as_ref().and_then(|d| d.pid).or(target);
            let from = LAST.lock().ok().and_then(|mut l| l.replace(p));
            if drag.is_some() {
                glide(from, p, pid);
            } else {
                post(mouse_event(CGEventType::MouseMoved, p, CGMouseButton::Left, 0), pid);
            }
        }
        "up" => {
            let pid = drag.as_ref().and_then(|d| d.pid).or(target);
            let from = LAST.lock().ok().and_then(|mut l| l.take());
            glide(from, p, pid);
            std::thread::sleep(Duration::from_millis(t.before_up_ms));
            post(mouse_event(CGEventType::LeftMouseUp, p, CGMouseButton::Left, 1), pid);
        }
        "click" | "dblclick" | "rclick" => {
            save_cursor(&mut drag);
            let (d, u, b) = if kind == "rclick" {
                (CGEventType::RightMouseDown, CGEventType::RightMouseUp, CGMouseButton::Right)
            } else {
                (CGEventType::LeftMouseDown, CGEventType::LeftMouseUp, CGMouseButton::Left)
            };
            post(mouse_event(CGEventType::MouseMoved, p, CGMouseButton::Left, 0), target);
            post(mouse_event(d, p, b, 1), target);
            post(mouse_event(u, p, b, 1), target);
            if kind == "dblclick" {
                // Deuxième clic marqué comme tel (compteur de clics à 2).
                post(mouse_event(d, p, b, 2), target);
                post(mouse_event(u, p, b, 2), target);
            }
        }
        "wheel" => {
            if !background {
                bring_to_front(i.pid);
                post(mouse_event(CGEventType::MouseMoved, p, CGMouseButton::Left, 0), None);
            }
            let lines = (wheel / 40).clamp(-10, 10);
            let lines = if lines == 0 { wheel.signum() } else { lines };
            post(CGEvent::new_scroll_wheel_event2(None, CGScrollEventUnit::Line, 1, lines, 0, 0), target);
        }
        _ => {}
    }
    if matches!(kind, "up" | "click" | "dblclick" | "rclick") {
        if let Some(Drag { cursor: Some(c), .. }) = drag.take() {
            let wait = if kind == "up" { t.after_up_ms } else { 15 };
            std::thread::sleep(Duration::from_millis(wait));
            let _ = CGWarpMouseCursorPosition(c);
        }
    }
    Ok(())
}
