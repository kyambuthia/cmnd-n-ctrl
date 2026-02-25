package com.example.assistant

// UniFFI bridge placeholder. TODO: replace with generated bindings + JNI wiring.
class CoreBridge {
    fun ping(): String = "core-bridge-stub"

    fun chatRequest(messagesJson: String, requireConfirmation: Boolean): String {
        return "{\"status\":\"stub\",\"messages\":$messagesJson,\"requireConfirmation\":$requireConfirmation}"
    }
}
