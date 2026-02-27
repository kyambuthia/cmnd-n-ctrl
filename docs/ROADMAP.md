# Product Roadmap (Windows + Linux First)

This project is no longer just a scaffold. The goal is a useful, secure desktop operator agent that can execute supervised tasks across local applications and browser workflows.

This roadmap is organized by delivery phases with explicit outcomes and "done" criteria.

## North Star
- User gives a request in CLI TUI or desktop app (same backend protocol).
- Agent proposes a bounded plan and required actions.
- User reviews and approves/denies via explicit consent flow.
- Agent executes actions (desktop/browser/file) with audit trail and evidence.
- Failures are visible, recoverable, and logged.

## Product Principles
- Security by default: explicit consent, least privilege, auditable actions.
- Shared backend contract: CLI and GUI use the same JSON-RPC methods and semantics.
- Desktop-first usefulness: Windows + Linux workflows before broadening scope.
- Reliability before "magic": deterministic tools and evidence over vague agent behavior.

## Phase 0: Stabilize The Current Foundation (1-2 weeks)
Goal: make existing features dependable and easier to operate.

Current Status
- Done:
  - `cli` defaults to interactive mode with TUI + REPL fallback.
  - Consent TTL/expiry/replay checks are implemented and tested.
  - Storage write locking is implemented.
  - MCP runtime lifecycle/status reconciliation implemented.
  - `system.health`/`doctor` diagnostics include MCP runtime probes.
- In progress:
  - Full Tauri parity and release hardening.

Done Criteria
- `npm run tauri:dev` works on Windows and Linux developer machines.
- Consent approve/deny/replay/expiry behavior is tested and deterministic.
- No frequent storage corruption/race issues with concurrent CLI + GUI usage.

## Phase 1: Make Browser-Based Workflows Useful (Fastest Value) (2-4 weeks)
Goal: supervised browser automation for real tasks (Google Sheets/forms/web apps) before deep desktop automation.

Current Status
- Done:
  - MCP stdio runtime request path exists (`probe`, `tools`, `call`, `tool_call`).
  - MCP tool invocation is integrated into chat via:
    - `mcp.tool_call`
    - dynamic aliases `mcp.server.<id>.<tool>`
  - OpenAI-compatible tool-call ID handling implemented.
- Remaining:
  - Real Playwright-backed browser MCP server integration.
  - Browser-specific evidence and richer selector/URL previews.

Planned Scope
- Add Playwright-backed browser tool plugin (or equivalent local browser automation plugin):
  - `browser.open`
  - `browser.navigate`
  - `browser.read_text`
  - `browser.type`
  - `browser.click`
  - `browser.wait_for`
- Consent policy:
  - Read operations (`read_text`) can be `ReadOnly`
  - Mutating operations (`type`, `click`) require confirmation by default
- Evidence capture:
  - URL, selector summary, timestamp, redacted input previews, optional screenshot path/reference
- GUI/CLI parity for browser action proposals and approvals queue

Done Criteria
- User can complete a supervised Google Sheets or form-entry workflow end-to-end.
- Audit log shows proposed actions, approvals, executions, and evidence summaries.
- CLI TUI and GUI both surface and control the same approval queue.

## Phase 2: File + Spreadsheet Input Workflows (3-5 weeks)
Goal: support common "read data, enter data" tasks reliably.

Scope
- Add real file tools (desktop-safe, scoped):
  - `file.list`
  - `file.read_text`
  - `file.read_csv`
  - `file.read_xlsx` (initial parser-only support)
- Add project/workspace scoping for file access:
  - bind file tools to `project.open` root unless user explicitly expands scope
- Add schema-aware previews in consent UI:
  - row counts
  - column names
  - redacted samples
- Add mapping assistance (agent proposes column -> field mapping, user confirms)

Done Criteria
- User can import CSV/XLSX data and populate a web form or Google Sheet under supervision.
- File reads are auditable and constrained to project scope unless overridden by explicit consent.

## Phase 3: Real Desktop App Automation (Windows + Linux) (4-8+ weeks)
Goal: move from browser-only usefulness to desktop-app usefulness.

Current Status
- Done:
  - `desktop.open_url` best-effort real OS dispatchers.
  - `desktop.app.activate` best-effort platform adapters.
- Remaining:
  - Real `desktop.app.list` implementation.
  - Better window targeting and stronger evidence capture.

Planned Scope
- Implement non-stub Windows and Linux backends for:
  - `desktop.app.list`
  - `desktop.app.activate`
  - window/focus/state inspection
- Add desktop input tools (initially conservative):
  - `desktop.input.type`
  - `desktop.input.click`
  - `desktop.screen.capture` (evidence/perception)
- Platform-specific policy and constraints:
  - Windows UAC/session integrity boundaries
  - Linux Wayland vs X11 limitations and portal fallback paths
- Evidence:
  - active window/app metadata
  - coordinates/selectors (where applicable)
  - redacted screenshots/snippets

Done Criteria
- User can activate a target app and perform simple supervised input/click flows.
- Behavior is reliable enough for repeated demo scenarios on both Windows and Linux.

## Phase 4: Perception + Recovery (4-8 weeks)
Goal: make the agent resilient, not just scripted.

Scope
- DOM-first and UI-state extraction abstractions
- OCR fallback for non-browser apps (Linux/Windows)
- Wait/retry primitives and structured action failure reasons
- Plan execution state machine:
  - pending
  - awaiting consent
  - executing
  - blocked
  - failed
  - completed
- User-visible recovery prompts:
  - "I cannot find button X, retry/search/cancel?"

Done Criteria
- Agent recovers from minor UI timing issues without unsafe behavior.
- Failures are explained and actionable, not silent.

## Phase 5: UX Overhaul (GUI + CLI) (parallel, starts now)
Goal: turn the current functional UI into an efficient operator interface.

GUI Priorities (Tauri)
- Replace panel-heavy layout with task-first workflow:
  - left: sessions/tasks
  - center: request + current step + approvals
  - right: collapsible audit/evidence/debug
- Improve visual hierarchy:
  - calmer spacing
  - reduced chrome/noise
  - clear action cards and risk badges
- Dedicated screens/views:
  - providers/auth
  - approvals queue
  - audit browser with filters
  - session history/messages
- Native app ergonomics:
  - keyboard shortcuts
  - window state persistence
  - notifications for pending approvals (later)

CLI Priorities (TUI/REPL)
- Done:
  - interactive fallback REPL when TUI unavailable
  - natural-language-only chat UX in CLI/TUI/REPL
  - feed-style rendering improvements in both REPL and TUI
- In progress:
  - deeper visual parity with desktop feed blocks
  - searchable execution history

Done Criteria
- A new user can perform a supervised workflow without reading source code or raw JSON.
- Debug views are available but hidden by default.

## Phase 6: Hardening and Trust (ongoing)
Goal: make the tool safe to use regularly.

Scope
- Audit tamper-evidence (hash chaining or signing)
- Storage encryption at rest for provider tokens (OS keychain integration later)
- Plugin isolation controls and allowlists
- Capability presets and organization policy packs
- Exportable audit records for review/compliance

Done Criteria
- Consent and audit records are trustworthy and hard to tamper with silently.
- Provider secrets are not stored as plain JSON in long-term production mode.

## Phase 7: Performance and Efficiency (ongoing)
Goal: make the tool fast enough for real daily use.

Scope
- Reduce roundtrips in tool execution loops
- Streaming responses / progressive plan updates
- Cached UI state snapshots and tool registry queries
- Background task execution with bounded concurrency
- Benchmark and profile hot paths (IPC, storage, TUI refresh, GUI bridge)

Done Criteria
- Common workflows feel responsive and do not block the UI unnecessarily.

## Immediate Next 5 Milestones (Recommended)
1. Playwright browser MCP server integration and end-to-end supervised workflow test.
2. Real `desktop.app.list` implementation (Linux first, Windows next).
3. Shared execution feed projection adoption in Tauri frontend render path.
4. Keychain-backed provider secret storage.
5. Windows + Linux CI job for `tauri-app` feature build and smoke tests.

## Definition of "Useful" (v1)
The tool is "useful" when a user can reliably do the following in a supervised flow:
- "Read this CSV from my project."
- "Open this Google Sheet."
- "Fill these rows/fields."
- Review and approve risky actions.
- See exactly what was done in the audit log.

That is the target to optimize for before expanding into broader desktop automation.
