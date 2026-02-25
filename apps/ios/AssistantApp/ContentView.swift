import SwiftUI

struct ContentView: View {
    @State private var prompt: String = ""
    @State private var requireConfirmation: Bool = true
    @State private var response: String = "No response yet."
    private let bridge = CoreBridge()

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Assistant (iOS Stub)")
                .font(.title2)
            TextField("Prompt", text: $prompt)
                .textFieldStyle(.roundedBorder)
            Toggle("Require confirmation", isOn: $requireConfirmation)
            Button("Send") {
                response = bridge.chatRequest(prompt: prompt, requireConfirmation: requireConfirmation)
            }
            Text(response)
                .frame(maxWidth: .infinity, alignment: .leading)
            Spacer()
        }
        .padding()
    }
}

#Preview {
    ContentView()
}
