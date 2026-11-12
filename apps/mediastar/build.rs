// Icône de l'exécutable Windows et version de build (hash git, date) : voir star-build.
fn main() {
    star_build::app("MediaStar", "assets/icons/mediastar.ico");
    // macOS : comme pour l'exe OldStar, ScreenCaptureKit lié en « faible » pour que MediaStar
    // démarre aussi avant macOS 12.3 (la capture d'OldStar passe alors par CGWindowListCreateImage).
    // Un build.rs ne règle que l'édition de liens de son propre paquet : à répéter ici.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg-bins=-Wl,-weak_framework,ScreenCaptureKit");
    }
}
