// Scanning the QR code a computer shows to sign this iPhone in (Work
// together > Sign in on your phone: workboard://link?server=...&code=...),
// or an invite link's QR code. The camera reads QR codes only; anything
// that isn't a Workboard link says so and keeps scanning. Without a camera
// (the simulator) or without permission, it says how to type the code
// instead.

import AVFoundation
import SwiftUI
import UIKit
import WorkboardKit

struct QRScanView: View {
    /// Called once with the link that was scanned; the sheet closes itself.
    var onLink: (InviteLink) -> Void

    @Environment(\.dismiss) private var dismiss
    @Environment(\.openURL) private var openURL
    @State private var access: AVAuthorizationStatus = AVCaptureDevice.authorizationStatus(for: .video)
    @State private var wrong = false
    @State private var done = false

    var body: some View {
        NavigationStack {
            content
                .navigationTitle("Scan QR code")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { dismiss() }
                            .accessibilityIdentifier("scanCancelButton")
                    }
                }
        }
        .task {
            if access == .notDetermined {
                _ = await AVCaptureDevice.requestAccess(for: .video)
                access = AVCaptureDevice.authorizationStatus(for: .video)
            }
        }
    }

    @ViewBuilder var content: some View {
        if AVCaptureDevice.default(for: .video) == nil {
            message("This iPhone has no camera to scan with. Type the six-character code instead: press Show code next to the QR code on your computer.", settings: false)
        } else {
            switch access {
            case .authorized:
                ZStack(alignment: .bottom) {
                    QRCameraView { text in found(text) }
                        .ignoresSafeArea(edges: .bottom)
                    Text(wrong ? "That isn't a Workboard sign-in code. On your computer, open Work together > Sign in on your phone." : "Point the camera at the QR code on your computer: Work together > Sign in on your phone.")
                        .font(.callout)
                        .multilineTextAlignment(.center)
                        .padding(14)
                        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
                        .padding(20)
                        .accessibilityIdentifier("scanHint")
                }
            case .notDetermined:
                ProgressView()
            default:
                message("Workboard isn't allowed to use the camera. Allow it in Settings, or type the six-character code instead: press Show code next to the QR code on your computer.", settings: true)
            }
        }
    }

    func message(_ text: LocalizedStringKey, settings: Bool) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "qrcode.viewfinder").font(.system(size: 48)).foregroundStyle(.secondary)
            Text(text).multilineTextAlignment(.center).accessibilityIdentifier("scanMessage")
            if settings {
                Button("Open Settings") {
                    if let u = URL(string: UIApplication.openSettingsURLString) { openURL(u) }
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    func found(_ text: String) {
        guard !done else { return }
        guard let link = InviteLink.parse(text) else {
            wrong = true
            return
        }
        done = true
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        dismiss()
        onLink(link)
    }
}

/// The camera, reporting each QR code it reads.
struct QRCameraView: UIViewControllerRepresentable {
    var onCode: (String) -> Void

    func makeUIViewController(context: Context) -> Controller {
        let c = Controller()
        c.onCode = onCode
        return c
    }

    func updateUIViewController(_ c: Controller, context: Context) {
        c.onCode = onCode
    }

    final class Controller: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
        var onCode: ((String) -> Void)?
        private let session = AVCaptureSession()
        private var preview: AVCaptureVideoPreviewLayer?
        private var last: String?

        override func viewDidLoad() {
            super.viewDidLoad()
            view.backgroundColor = .black
            guard let camera = AVCaptureDevice.default(for: .video),
                  let input = try? AVCaptureDeviceInput(device: camera),
                  session.canAddInput(input) else { return }
            session.addInput(input)
            let output = AVCaptureMetadataOutput()
            guard session.canAddOutput(output) else { return }
            session.addOutput(output)
            output.setMetadataObjectsDelegate(self, queue: .main)
            output.metadataObjectTypes = [.qr]
            let layer = AVCaptureVideoPreviewLayer(session: session)
            layer.videoGravity = .resizeAspectFill
            view.layer.addSublayer(layer)
            preview = layer
        }

        override func viewDidLayoutSubviews() {
            super.viewDidLayoutSubviews()
            preview?.frame = view.bounds
        }

        override func viewWillAppear(_ animated: Bool) {
            super.viewWillAppear(animated)
            let s = session
            DispatchQueue.global(qos: .userInitiated).async { if !s.isRunning { s.startRunning() } }
        }

        override func viewWillDisappear(_ animated: Bool) {
            super.viewWillDisappear(animated)
            let s = session
            DispatchQueue.global(qos: .userInitiated).async { if s.isRunning { s.stopRunning() } }
        }

        func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
            guard let text = objects.compactMap({ ($0 as? AVMetadataMachineReadableCodeObject)?.stringValue }).first,
                  text != last else { return }
            // the same code is read many times a second: report it once
            last = text
            onCode?(text)
        }
    }
}
