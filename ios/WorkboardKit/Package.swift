// swift-tools-version: 5.10
import PackageDescription

let package = Package(
    name: "WorkboardKit",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [
        .library(name: "WorkboardKit", targets: ["WorkboardKit"]),
        .executable(name: "wb-fake-server", targets: ["wb-fake-server"]),
        .executable(name: "wb-cli", targets: ["wb-cli"]),
    ],
    targets: [
        // Data (raw JSON tree), change stamps and merge (ported from src/sync.rs),
        // new ids, and the server protocol client.
        .target(name: "WorkboardKit"),
        // A test-only stand-in for workboard-server: holds one snapshot and
        // echoes every push back to everyone as an update. No merging.
        .target(name: "WorkboardFakeServer", dependencies: ["WorkboardKit"]),
        .executableTarget(name: "wb-fake-server", dependencies: ["WorkboardFakeServer", "WorkboardKit"]),
        // A command-line client for manual checks against a server.
        .executableTarget(name: "wb-cli", dependencies: ["WorkboardKit"]),
        .testTarget(name: "WorkboardKitTests", dependencies: ["WorkboardKit", "WorkboardFakeServer"]),
    ]
)
