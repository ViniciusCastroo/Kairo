fn main() {
    println!("cargo:rerun-if-changed=assets");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR do Cargo"));
    let status = std::process::Command::new("glib-compile-resources")
        .arg("--sourcedir=assets")
        .arg("--target")
        .arg(output.join("kairo.gresource"))
        .arg("assets/kairo.gresource.xml")
        .status()
        .expect("Instale glib-compile-resources (libglib2.0-dev-bin no Ubuntu)");
    assert!(status.success(), "Falha ao compilar os recursos do Kairo");
}
