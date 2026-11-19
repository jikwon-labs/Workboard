// Windows: put the app icon (Resources/AppIcon.ico, made from
// Resources/AppIcon-256.png) and the name into workboard.exe, so File
// Explorer, the taskbar and shortcuts show the Workboard logo instead of a
// blank program icon. macOS takes its icon from the app bundle instead.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Resources/AppIcon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("Resources/AppIcon.ico");
        res.set("ProductName", "Workboard");
        res.set("FileDescription", "Workboard");
        res.compile()
            .expect("couldn't put the icon into workboard.exe");
    }
}
