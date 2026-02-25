import Foundation

// UniFFI bridge placeholder. TODO: replace with generated bindings and Rust FFI wiring.
struct CoreBridge {
    func ping() -> String {
        "core-bridge-stub"
    }

    func chatRequest(prompt: String, requireConfirmation: Bool) -> String {
        let mode = requireConfirmation ? "RequireConfirmation" : "BestEffort"
        return "iOS bridge stub -> mode=\(mode), prompt=\(prompt)"
    }
}
