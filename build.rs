// Windows: put the app icon (Resources/AppIcon.ico, made from
// Resources/AppIcon-256.png) and the name into workboard.exe, so File
// Explorer, the taskbar and shortcuts show the Workboard logo instead of a
// blank program icon. macOS takes its icon from the app bundle instead.
//
// iPad: the app links Swift libraries (through LiveKit's WebRTC), which a
// real iPad keeps in /usr/lib/swift; without that search path the app ended
// at launch with "Library not loaded: @rpath/libswiftCoreMedia.dylib".
//
// iPad simulator: the push entitlement goes inside the program (the
// __TEXT,__entitlements section, as Xcode does for simulator builds), so the
// simulator gives the app a push token; signed with it the way a device app
// is, the simulator refuses to start the app (scripts/ipad-sim).
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,/usr/lib/swift");
        if std::env::var("CARGO_CFG_TARGET_ABI").as_deref() == Ok("sim") {
            let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
            let file = out.join("desk-sim.entitlements");
            let plist = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\"><dict><key>aps-environment</key><string>development</string></dict></plist>\n";
            if std::fs::write(&file, plist).is_ok() {
                println!(
                    "cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__entitlements,{}",
                    file.display()
                );
            }
        }
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
