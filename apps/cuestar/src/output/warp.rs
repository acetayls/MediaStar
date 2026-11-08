//! Mapping 4 coins : homographie appliquée au GPU (compositeur, ou shader utilisateur mpv,
//! hook OUTPUT), points intermédiaires en option (grille 3 × 3), bord adouci, et repères du
//! réglage dessinés sur la sortie.

use std::path::PathBuf;

type M3 = [[f64; 3]; 3];

/// Homographie qui envoie le carré unité (u, v) sur le quadrilatère donné
/// (haut-gauche, haut-droit, bas-droit, bas-gauche). Méthode de Heckbert.
fn square_to_quad(q: [[f32; 2]; 4]) -> M3 {
    let [x0, x1, x2, x3] = q.map(|p| p[0] as f64);
    let [y0, y1, y2, y3] = q.map(|p| p[1] as f64);
    let (dx1, dx2, dx3) = (x1 - x2, x3 - x2, x0 - x1 + x2 - x3);
    let (dy1, dy2, dy3) = (y1 - y2, y3 - y2, y0 - y1 + y2 - y3);
    let det = dx1 * dy2 - dx2 * dy1;
    let (g, h) = if det.abs() < 1e-12 { (0.0, 0.0) } else { ((dx3 * dy2 - dx2 * dy3) / det, (dx1 * dy3 - dx3 * dy1) / det) };
    [[x1 - x0 + g * x1, x3 - x0 + h * x3, x0], [y1 - y0 + g * y1, y3 - y0 + h * y3, y0], [g, h, 1.0]]
}

/// Position à l'écran du point (u, v) de l'image (aperçu de la régie).
pub fn map_point(corners: [[f32; 2]; 4], u: f32, v: f32) -> [f32; 2] {
    let m = square_to_quad(corners);
    let (u, v) = (u as f64, v as f64);
    let w = m[2][0] * u + m[2][1] * v + m[2][2];
    [((m[0][0] * u + m[0][1] * v + m[0][2]) / w) as f32, ((m[1][0] * u + m[1][1] * v + m[1][2]) / w) as f32]
}

fn invert(m: M3) -> Option<M3> {
    let [[a, b, c], [d, e, f], [g, h, i]] = m;
    let (ca, cb, cc) = (e * i - f * h, -(d * i - f * g), d * h - e * g);
    let det = a * ca + b * cb + c * cc;
    if det.abs() < 1e-12 {
        return None;
    }
    let k = 1.0 / det;
    Some([
        [ca * k, -(b * i - c * h) * k, (b * f - c * e) * k],
        [cb * k, (a * i - c * g) * k, -(a * f - c * d) * k],
        [cc * k, -(a * h - b * g) * k, (a * e - b * d) * k],
    ])
}

/// Homographie inverse (point de l'écran vers point de l'image), pour le compositeur.
/// None : mapping neutre.
pub fn inverse(corners: Option<[[f32; 2]; 4]>) -> [[f32; 3]; 3] {
    let id = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    corners.and_then(|c| invert(square_to_quad(c))).map(|m| m.map(|r| r.map(|v| v as f32))).unwrap_or(id)
}

/// Fichiers de shader successifs : mpv met en cache par chemin, chaque réglage prend donc un nom neuf.
pub struct ShaderFiles {
    n: u64,
    last: Option<PathBuf>,
}

impl ShaderFiles {
    pub fn new() -> Self {
        Self { n: 0, last: None }
    }

    pub fn write(&mut self, src: &str) -> std::io::Result<PathBuf> {
        self.n += 1;
        let p = std::env::temp_dir().join(format!("cuestar-warp-{}-{}.glsl", std::process::id(), self.n));
        std::fs::write(&p, src)?;
        self.clear_previous();
        self.last = Some(p.clone());
        Ok(p)
    }

    fn clear_previous(&mut self) {
        if let Some(old) = self.last.take() {
            let _ = std::fs::remove_file(old);
        }
    }
}

impl Drop for ShaderFiles {
    fn drop(&mut self) {
        self.clear_previous();
    }
}

// ------------------------------------------------------------------ grille 3 × 3 et repères

use crate::protocol::{WarpGeom, WarpGuides};

/// Place des points intermédiaires dans l'image : milieux des bords haut, droit, bas, gauche, puis centre.
pub const NODES: [[f32; 2]; 5] = [[0.5, 0.0], [1.0, 0.5], [0.5, 1.0], [0.0, 0.5], [0.5, 0.5]];

/// Bases de Lagrange aux nœuds 0, ½ et 1.
fn lagrange(t: f64) -> [f64; 3] {
    [2.0 * (t - 0.5) * (t - 1.0), 4.0 * t * (1.0 - t), 2.0 * t * (t - 0.5)]
}

/// Position à l'écran du point (u, v) de l'image : perspective des quatre coins, plus la
/// correction des points intermédiaires (biquadratique, nulle aux coins). Même calcul que
/// `wfwd` dans les shaders.
pub fn forward(g: &WarpGeom, u: f32, v: f32) -> [f32; 2] {
    let mut q = map_point(g.corners, u, v);
    if let Some(m) = &g.mesh {
        let (lu, lv) = (lagrange(u as f64), lagrange(v as f64));
        let w = [lu[1] * lv[0], lu[2] * lv[1], lu[1] * lv[2], lu[0] * lv[1], lu[1] * lv[1]];
        for (k, d) in m.iter().enumerate() {
            q[0] += (w[k] * d[0] as f64) as f32;
            q[1] += (w[k] * d[1] as f64) as f32;
        }
    }
    q
}

/// Les poignées à l'écran : quatre coins, puis les cinq points intermédiaires s'il y en a.
pub fn handles(g: &WarpGeom) -> Vec<[f32; 2]> {
    let mut v = g.corners.to_vec();
    if g.mesh.is_some() {
        v.extend(NODES.iter().map(|n| forward(g, n[0], n[1])));
    }
    v
}

/// La déformation garde l'image à l'endroit : coins en quadrilatère convexe dans le même sens
/// que l'écran, et (avec les points intermédiaires) aucun repli nulle part dans l'image.
pub fn valid(g: &WarpGeom) -> bool {
    let c = g.corners;
    for i in 0..4 {
        let (a, b, d) = (c[i], c[(i + 1) % 4], c[(i + 2) % 4]);
        let cross = (b[0] - a[0]) * (d[1] - b[1]) - (b[1] - a[1]) * (d[0] - b[0]);
        if cross <= 1e-6 {
            return false;
        }
    }
    if g.mesh.is_none() {
        return true;
    }
    let n = 16;
    let e = 1e-3;
    for i in 0..=n {
        for j in 0..=n {
            let (u, v) = (i as f32 / n as f32 * (1.0 - e), j as f32 / n as f32 * (1.0 - e));
            let p = forward(g, u, v);
            let pu = forward(g, u + e, v);
            let pv = forward(g, u, v + e);
            let det = (pu[0] - p[0]) * (pv[1] - p[1]) - (pu[1] - p[1]) * (pv[0] - p[0]);
            if det <= 0.0 {
                return false;
            }
        }
    }
    true
}

/// Paramètres des shaders (13 vec4) : perspective, points intermédiaires, poignées, réglages.
/// `wx[12].zw` (taille de l'écran en pixels) est rempli au moment de dessiner.
pub fn params(geom: Option<&WarpGeom>, guides: Option<&WarpGuides>) -> [[f32; 4]; 13] {
    let mut wx = [[0.0f32; 4]; 13];
    let g = geom.copied().unwrap_or(WarpGeom { corners: crate::prefs::Warp::IDENTITY, mesh: None, feather: 0.0 });
    let h = square_to_quad(g.corners);
    for r in 0..3 {
        wx[r] = [h[r][0] as f32, h[r][1] as f32, h[r][2] as f32, 0.0];
    }
    if let Some(m) = &g.mesh {
        wx[3] = [m[0][0], m[0][1], m[1][0], m[1][1]];
        wx[4] = [m[2][0], m[2][1], m[3][0], m[3][1]];
        wx[5] = [m[4][0], m[4][1], 0.0, 0.0];
    }
    let hs = handles(&g);
    for (k, p) in hs.iter().enumerate() {
        let r = &mut wx[6 + k / 2];
        r[(k % 2) * 2] = p[0];
        r[(k % 2) * 2 + 1] = p[1];
    }
    wx[11] = [g.mesh.is_some() as u8 as f32, if geom.is_some() { g.feather.max(0.0) } else { 0.0 }, guides.is_some() as u8 as f32, guides.map_or(-1.0, |x| x.selected as f32)];
    wx[12] = [guides.is_some_and(|x| x.grid) as u8 as f32, hs.len() as f32, 1920.0, 1080.0];
    wx
}

/// Calcul commun aux shaders, écrit en GLSL (`wx` : les 13 vec4 de `params`).
/// `hlsl` le traduit pour Direct3D (types et deux fonctions renommés).
const CORE: &str = r#"
vec2 wfwd(vec2 t) {
    vec3 t3 = vec3(t, 1.0);
    vec2 q = vec2(dot(wx[0].xyz, t3), dot(wx[1].xyz, t3)) / dot(wx[2].xyz, t3);
    if (wx[11].x > 0.5) {
        vec3 lu = vec3(2.0 * (t.x - 0.5) * (t.x - 1.0), 4.0 * t.x * (1.0 - t.x), 2.0 * t.x * (t.x - 0.5));
        vec3 lv = vec3(2.0 * (t.y - 0.5) * (t.y - 1.0), 4.0 * t.y * (1.0 - t.y), 2.0 * t.y * (t.y - 0.5));
        q += lu.y * lv.x * wx[3].xy + lu.z * lv.y * wx[3].zw + lu.y * lv.z * wx[4].xy + lu.x * lv.y * wx[4].zw + lu.y * lv.y * wx[5].xy;
    }
    return q;
}
vec2 winv(vec2 p, vec2 t) {
    if (wx[11].x > 0.5) {
        for (int k = 0; k < 10; k++) {
            vec2 f0 = wfwd(t);
            vec2 f = f0 - p;
            vec2 du = (wfwd(t + vec2(0.001, 0.0)) - f0) * 1000.0;
            vec2 dv = (wfwd(t + vec2(0.0, 0.001)) - f0) * 1000.0;
            float det = du.x * dv.y - du.y * dv.x;
            if (abs(det) < 1e-9) break;
            t -= vec2(dv.y * f.x - dv.x * f.y, du.x * f.y - du.y * f.x) / det;
        }
    }
    return t;
}
float wcover(vec2 p, vec2 t, float w) {
    // Distance au bord de l'image en pixels de l'écran, calculée sur la déformation elle-même
    // (pas de dérivées d'écran) : signée, négative hors de l'image.
    vec2 size = wx[12].zw;
    vec2 f0 = wfwd(t);
    vec2 ju = (wfwd(t + vec2(0.001, 0.0)) - f0) * 1000.0 * size;
    vec2 jv = (wfwd(t + vec2(0.0, 0.001)) - f0) * 1000.0 * size;
    float det = abs(ju.x * jv.y - ju.y * jv.x);
    vec2 d = min(t, 1.0 - t);
    float e = min(d.x * det / max(length(jv), 1e-6), d.y * det / max(length(ju), 1e-6));
    if (!(w > 0.0)) return 0.0;
    if (wx[11].x > 0.5 && length(f0 - p) > 0.002) return 0.0;
    // Bord adouci : rampe centrée sur le bord exact (anticrénelage), large de `feather` pixels.
    if (wx[11].y > 0.0) return clamp(e / wx[11].y + 0.5, 0.0, 1.0);
    return e >= 0.0 ? 1.0 : 0.0;
}
float wdigit(int n, vec2 q) {
    if (q.x < 0.0 || q.y < 0.0 || q.x >= 3.0 || q.y >= 5.0) return 0.0;
    int pat = n == 1 ? 29850 : n == 2 ? 29671 : n == 3 ? 31207 : n == 4 ? 18925 : n == 5 ? 31183 : n == 6 ? 31695 : n == 7 ? 18727 : n == 8 ? 31727 : 31215;
    int i = int(q.y) * 3 + int(q.x);
    return float((pat >> i) & 1);
}
vec4 wguides(vec2 p, vec2 t, float cover) {
    vec2 fw = max(fwidth(t), 1e-7);
    vec4 o = vec4(0.0, 0.0, 0.0, 0.0);
    if (wx[11].z < 0.5) return o;
    vec2 size = wx[12].zw;
    vec3 acc = vec3(1.0, 0.62, 0.1);
    vec2 d = min(t, 1.0 - t) / fw;
    if (cover > 0.0 && wx[12].x > 0.5) {
        vec2 g = abs(fract(t * 8.0 + 0.5) - 0.5) / (8.0 * fw);
        if (min(g.x, g.y) < 0.8) o = vec4(0.85, 0.85, 0.85, 0.7);
    }
    if (cover > 0.0 && min(d.x, d.y) < 3.0) o = vec4(acc, 1.0);
    int n = int(wx[12].y + 0.5);
    for (int k = 0; k < 9; k++) {
        if (k >= n) break;
        vec4 pp = wx[6 + k / 2];
        vec2 h = (k % 2 == 0) ? pp.xy : pp.zw;
        vec2 dp = (p - h) * size;
        float r = length(dp);
        bool sel = abs(float(k) - wx[11].w) < 0.5;
        vec3 c = sel ? vec3(1.0, 1.0, 1.0) : acc;
        if (r < 16.0 && r > 10.0) o = vec4(0.0, 0.0, 0.0, 1.0);
        if (abs(r - 13.0) < 1.6) o = vec4(c, 1.0);
        if (sel && r < 8.0) o = vec4(acc, 1.0);
        if (r < 2.5) o = vec4(1.0, 1.0, 1.0, 1.0);
        vec2 org = vec2(h.x < 0.5 ? 20.0 : -32.0, h.y < 0.5 ? 20.0 : -40.0);
        vec2 q = (dp - org) / 4.0;
        if (q.x >= -1.0 && q.y >= -1.0 && q.x < 4.0 && q.y < 6.0) {
            o = vec4(0.0, 0.0, 0.0, 0.85);
            if (wdigit(k + 1, q) > 0.5) o = vec4(c, 1.0);
        }
    }
    return o;
}
"#;

/// Calcul commun en GLSL, à placer après la déclaration de `wx`.
#[allow(dead_code)]
pub fn glsl_core() -> String {
    CORE.to_string()
}

/// Même calcul en HLSL.
pub fn hlsl_core() -> String {
    CORE.replace("vec2", "float2").replace("vec3", "float3").replace("vec4", "float4").replace("fract(", "frac(")
}

/// Shader mpv (hook OUTPUT) du mapping et des repères, constantes écrites dedans. Avec `fade`,
/// un paramètre réglable à chaud (`glsl-shader-opts=fade=…`, vo gpu-next) assombrit l'image
/// jusqu'au noir : il sert aux transitions. None : rien à appliquer.
pub fn hook_shader(geom: Option<&WarpGeom>, guides: Option<&WarpGuides>, fade: bool) -> Option<String> {
    if geom.is_none() && guides.is_none() && !fade {
        return None;
    }
    let corners = geom.map_or(crate::prefs::Warp::IDENTITY, |g| g.corners);
    let m = invert(square_to_quad(corners))?;
    let wx = params(geom, guides);
    let f = |v: f64| format!("{v:.9}");
    let arr = wx.iter().map(|r| format!("vec4({}, {}, {}, {})", f(r[0] as f64), f(r[1] as f64), f(r[2] as f64), f(r[3] as f64))).collect::<Vec<_>>().join(",\n    ");
    let (param, mul) = if fade {
        ("//!PARAM fade\n//!DESC Niveau de l'image (0 = noir)\n//!TYPE float\n//!MINIMUM 0.0\n//!MAXIMUM 1.0\n1.0\n\n", " * fade")
    } else {
        ("", "")
    };
    // La taille de l'écran (wx[12].zw) vient de mpv.
    Some(format!(
        "{param}//!HOOK OUTPUT
//!BIND HOOKED
//!DESC CueStar mapping
vec4 wx[13];
{core}
vec4 hook() {{
    vec4 wx0[13] = vec4[13](
    {arr});
    for (int k = 0; k < 13; k++) wx[k] = wx0[k];
    wx[12].zw = target_size;
    vec2 p = HOOKED_pos;
    float w = {g} * p.x + {h} * p.y + {i};
    vec2 t = winv(p, vec2({a} * p.x + {b} * p.y + {c}, {d} * p.x + {e} * p.y + {ff}) / w);
    float cov = wcover(p, t, w);
    vec3 col = HOOKED_tex(clamp(t, 0.0, 1.0)).rgb;
    col = cov > 0.0 ? col * cov{mul} : vec3(0.0, 0.0, 0.0);
    vec4 gd = wguides(p, t, cov);
    return vec4(mix(col, gd.rgb, gd.a), 1.0);
}}
",
        core = CORE,
        a = f(m[0][0]),
        b = f(m[0][1]),
        c = f(m[0][2]),
        d = f(m[1][0]),
        e = f(m[1][1]),
        ff = f(m[1][2]),
        g = f(m[2][0]),
        h = f(m[2][1]),
        i = f(m[2][2]),
    ))
}

#[cfg(test)]
mod mesh_tests {
    use super::*;

    fn geom(mesh: Option<[[f32; 2]; 5]>) -> WarpGeom {
        WarpGeom { corners: [[0.1, 0.05], [0.9, 0.0], [1.0, 1.0], [0.0, 0.9]], mesh, feather: 1.5 }
    }

    #[test]
    fn mesh_passes_through_its_points() {
        let mut m = [[0.0f32; 2]; 5];
        m[0] = [0.0, 0.05];
        m[4] = [0.03, -0.02];
        let g = geom(Some(m));
        // Coins inchangés, points intermédiaires décalés de leur place sur la perspective.
        assert_eq!(forward(&g, 0.0, 0.0), g.corners[0]);
        let flat = map_point(g.corners, 0.5, 0.0);
        let top = forward(&g, 0.5, 0.0);
        assert!((top[0] - flat[0]).abs() < 1e-5 && (top[1] - flat[1] - 0.05).abs() < 1e-5);
        assert!(valid(&g));
    }

    #[test]
    fn flipped_shapes_are_refused() {
        assert!(valid(&geom(None)));
        // Bas au-dessus du haut.
        let mut g = geom(None);
        g.corners = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        assert!(!valid(&g));
        // Coins croisés (nœud papillon).
        g.corners = [[0.0, 0.0], [1.0, 1.0], [1.0, 0.0], [0.0, 1.0]];
        assert!(!valid(&g));
        // Milieu du haut tiré sous le bas : la grille se replie.
        let mut m = [[0.0f32; 2]; 5];
        m[0] = [0.0, 1.5];
        assert!(!valid(&geom(Some(m))));
    }

    /// `CUESTAR_DUMP_SHADER=<fichier> cargo test -- --ignored dump_hook_shader` : écrit le shader
    /// mpv (mapping courbé, repères) pour l'essayer dans une vraie libmpv.
    #[test]
    #[ignore]
    fn dump_hook_shader() {
        let mut m = [[0.0f32; 2]; 5];
        m[4] = [0.03, -0.02];
        let s = hook_shader(Some(&geom(Some(m))), Some(&WarpGuides { selected: 4, grid: true }), true).unwrap();
        std::fs::write(std::env::var("CUESTAR_DUMP_SHADER").unwrap(), s).unwrap();
    }

    #[test]
    fn hook_shader_has_constants() {
        let s = hook_shader(Some(&geom(None)), Some(&WarpGuides { selected: 2, grid: true }), true).unwrap();
        assert!(s.contains("//!HOOK OUTPUT") && s.contains("wguides") && s.contains("fade"));
        assert!(hook_shader(None, None, false).is_none());
        assert!(!hlsl_core().contains("vec2") && hlsl_core().contains("frac("));
    }
}
