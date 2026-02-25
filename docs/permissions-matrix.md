# Permissions Matrix

## Overview
The assistant uses capability tiers enforced by policy. Platform-specific actions differ substantially and must be authorized per tool call.

## Desktop Platforms

### Windows
- Input injection and desktop automation may require elevated privileges depending on API and target app.
- UAC prompts and secure desktop contexts can block automation.
- Signed binaries may be required in enterprise environments.

### macOS
- Accessibility permissions are required for input automation.
- Screen recording permission may be required for observation/screenshot tools.
- TCC prompts must be explicit and user-granted per app bundle.

### Linux
- X11 allows more legacy automation but is less secure.
- Wayland often blocks global input injection/screen capture without compositor support.
- Prefer portals (xdg-desktop-portal) for user-mediated actions and file access.

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
- Tier 0: Read-only local metadata / no side effects
- Tier 1: User-scoped file operations via system pickers/portals
- Tier 2: Network access / provider calls / plugin IPC
- Tier 3: Desktop automation or OS-integrated actions requiring explicit confirmation each time

## Policy Guidance
- Default-deny unknown tools.
- Require confirmation for Tier 2+ by default in consumer builds.
- Log every authorization decision and executed action with evidence.
