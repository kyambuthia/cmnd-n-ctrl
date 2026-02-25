# Permissions Matrix

## Overview
The assistant uses capability tiers enforced by policy. Platform-specific actions differ substantially and must be authorized per tool call.

## Desktop Platforms

### Windows
- Input injection and desktop automation may require elevated privileges depending on API and target app.
- UAC prompts and secure desktop contexts can block automation.
- Signed binaries may be required in enterprise environments.
- Window focus/activation can fail across integrity levels or when apps run elevated.
- Desktop application enumeration may differ between classic Win32, UWP, and virtual desktops.

### macOS
- Accessibility permissions are required for input automation.
- Screen recording permission may be required for observation/screenshot tools.
- TCC prompts must be explicit and user-granted per app bundle.

### Linux
- X11 allows more legacy automation but is less secure.
- Wayland often blocks global input injection/screen capture without compositor support.
- Prefer portals (xdg-desktop-portal) for user-mediated actions and file access.
- Window activation/focus is compositor-dependent; many Wayland compositors restrict arbitrary focus stealing.
- Desktop app listing may require compositor/window-manager specific integrations and should degrade gracefully.

## Mobile Platforms

### Android
- Overlays, accessibility, notifications, and usage stats may require special permissions and OEM-dependent behavior.
- Background execution is constrained by battery optimizations.
- Some automation features may require AccessibilityService and explicit user opt-in.

### iOS
- General-purpose input injection is not available for App Store apps.
- Sandboxing limits cross-app automation and background behavior.
- Siri Shortcuts / App Intents style integrations are safer fit for many use cases.

## Capability Tiers (Example)
- `ReadOnly`: local metadata/no side effects (`time.now`, `echo`, local transforms)
- `LocalActions`: desktop-scoped actions with user impact (e.g. `desktop.app.list`)
- `SystemActions`: higher-risk OS/application control (e.g. `desktop.app.activate`) requiring explicit confirmation

## Policy Guidance
- Default-deny unknown tools.
- Require confirmation for `LocalActions` and `SystemActions` by default.
- Log every authorization decision and executed action with evidence.
