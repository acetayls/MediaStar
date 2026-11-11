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
}
