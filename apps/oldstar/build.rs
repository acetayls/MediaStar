// Version de build (hash git, date) et icône de l'exécutable Windows : voir star-build.
// Si l'icône manque (copie partielle du dépôt), l'exe se construit sans.
fn main() {
    const ICON: &str = "assets/icons/oldstar.ico";
    if std::path::Path::new(ICON).exists() {
        star_build::app("OldStar", ICON);
    } else {
        println!("cargo:rerun-if-changed=build.rs");
        star_build::build_info();
    }
    // macOS : ScreenCaptureKit lié en « faible » pour que l'appli démarre aussi avant macOS 12.3
    // (la capture passe alors par CGWindowListCreateImage).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
    }
}
