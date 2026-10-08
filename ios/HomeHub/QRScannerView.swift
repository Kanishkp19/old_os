import SwiftUI
import AVFoundation

struct QRScannerView: UIViewControllerRepresentable {
    let onCode: (String) -> Void
    func makeUIViewController(context: Context) -> ScannerVC { let vc = ScannerVC(); vc.onCode = onCode; return vc }
    func updateUIViewController(_ vc: ScannerVC, context: Context) {}
    static func dismantleUIViewController(_ vc: ScannerVC, coordinator: ()) { vc.stop() }
    final class ScannerVC: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
        var onCode: ((String) -> Void)?
        private let session = AVCaptureSession(), captureQueue = DispatchQueue(label: "homehub.camera")
        private var preview: AVCaptureVideoPreviewLayer?, configured = false, lastCode: String?
        override func viewDidLoad() {
            super.viewDidLoad()
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                guard granted, let self else { return }
                self.captureQueue.async { self.configure() }
            }
        }
        private func configure() {
            guard !configured, let camera = AVCaptureDevice.default(for: .video), let input = try? AVCaptureDeviceInput(device: camera), session.canAddInput(input) else { return }
            session.beginConfiguration(); session.addInput(input)
            let output = AVCaptureMetadataOutput()
            guard session.canAddOutput(output) else { session.commitConfiguration(); return }
            session.addOutput(output); output.setMetadataObjectsDelegate(self, queue: .main); output.metadataObjectTypes = [.qr]
            session.commitConfiguration(); configured = true
            DispatchQueue.main.async {
                let preview = AVCaptureVideoPreviewLayer(session: self.session); preview.videoGravity = .resizeAspectFill; preview.frame = self.view.bounds
                self.view.layer.addSublayer(preview); self.preview = preview
            }
            session.startRunning()
        }
        override func viewDidLayoutSubviews() { super.viewDidLayoutSubviews(); preview?.frame = view.bounds }
        override func viewWillDisappear(_ animated: Bool) { super.viewWillDisappear(animated); stop() }
        func stop() { captureQueue.async { if self.session.isRunning { self.session.stopRunning() } } }
        func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
            guard let code = (objects.first as? AVMetadataMachineReadableCodeObject)?.stringValue, code != lastCode else { return }
            lastCode = code; onCode?(code)
            // Allow retry after a temporary pairing error or a new token.
            DispatchQueue.main.asyncAfter(deadline: .now() + 5) { self.lastCode = nil }
        }
    }
}
