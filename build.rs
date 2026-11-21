// Windows: put the app icon (Resources/AppIcon.ico, made from
// Resources/AppIcon-256.png) and the name into workboard.exe, so File
// Explorer, the taskbar and shortcuts show the Workboard logo instead of a
// blank program icon. macOS takes its icon from the app bundle instead.
//
// iPad: the app links Swift libraries (through LiveKit's WebRTC), which a
// real iPad keeps in /usr/lib/swift; without that search path the app ended
// at launch with "Library not loaded: @rpath/libswiftCoreMedia.dylib".
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,/usr/lib/swift");
    }
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
