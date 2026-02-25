# Tooling Roadmap (Utility-First, Security-First)

This roadmap defines the tool surface needed to make the agent genuinely useful for desktop operator workflows.

It complements `docs/ROADMAP.md` by focusing specifically on callable tools, their risk tiers, and delivery order.

## Design Principles
- Tools should be narrow and composable (small, auditable operations).
- Every tool must produce structured evidence.
- Read-only tools ship first to improve usefulness safely.
- Mutating tools require stronger consent UX and clearer previews.
- CLI and GUI should expose the same tool semantics (same RPC/tool names).

## Capability Tiers (Tool-Level)
- `ReadOnly`
  - No mutation of local system/app state
  - Safe by default in `BestEffort` mode (unless policy override)
- `LocalActions`
  - Mutates local files/apps/browser state in a bounded way
  - Explicit consent required by default
- `SystemActions`
  - High-impact actions (window focus/input injection/system ops)
  - Explicit consent + stronger confirmation semantics

## Phase A: Project + File Intelligence (Immediate Utility)
Goal: make the agent useful for reading, inspecting, and preparing data.

### ReadOnly tools (priority)
- `file.list`
- `file.read_text`
- `file.read_csv`
- `file.read_json`
- `file.search_text` (project-scoped search)
- `file.stat` (size/type/mtime)

### LocalActions tools (next)
- `file.write_text`
- `file.append_text`
- `file.mkdir`

Design notes
- All file tools should default to `project.open` root scope.
- Absolute paths or paths escaping project root should return structured errors.
- Evidence should include path, scope root, and operation summary.

## Phase B: Browser Automation (Fastest "Real Task" Utility)
Goal: supervised web workflows (forms, Google Sheets, internal dashboards).

### ReadOnly tools
- `browser.open`
- `browser.navigate`
- `browser.read_text`
- `browser.snapshot_dom`
- `browser.list_tabs`

### LocalActions tools
- `browser.type`
- `browser.click`
- `browser.select`
- `browser.press_key`

### Support tools
- `browser.wait_for`
- `browser.screenshot`
- `browser.find_elements`

Design notes
- Prefer DOM/selector-driven operations over screen coordinates.
- Mutating browser actions must include argument previews in consent UI.
- Evidence should include URL, selectors, and redacted inputs.

## Phase C: Data Entry / Spreadsheet Workflows
Goal: structured data import and supervised entry.

### ReadOnly tools
- `sheet.detect_columns` (for CSV/XLSX/web tables)
- `sheet.preview_rows`
- `mapping.suggest` (column -> field mapping suggestion)

### LocalActions tools
- `sheet.write_cells`
- `sheet.append_rows`
- `form.fill_fields`
- `form.submit`

Design notes
- Require previews showing affected rows/cells and redacted values.
- Keep "submit" distinct from "fill" for safer approvals.

## Phase D: Desktop App Operation (Windows + Linux)
Goal: move beyond browser-only workflows.

### ReadOnly tools
- `desktop.app.list`
- `desktop.window.list`
- `desktop.window.capture`

### LocalActions tools
- `desktop.app.activate`
- `desktop.window.focus`
- `desktop.input.type`
- `desktop.input.click`

### SystemActions tools (careful rollout)
- `desktop.shortcut.send` (global hotkeys)
- `desktop.clipboard.set`

Design notes
- Linux Wayland/X11 differences must be exposed in tool errors/evidence.
- Windows integrity/UAC boundaries must be explicit in failure reasons.

## Phase E: Project Engineering / Dev Productivity Tools
Goal: improve software/operator workflows using the same consent/audit model.

### ReadOnly tools
- `project.search`
- `project.read_file`
- `project.diff`
- `project.git.status`

### LocalActions tools
- `project.write_file`
- `project.patch_apply`
- `project.run_command` (bounded, allowlisted)

Design notes
- `run_command` should be heavily policy-gated and allowlisted.
- Structured diffs and command evidence are required.

## Phase F: Integrations (Plugins/MCP Servers)
Goal: externalize tool execution while preserving the same consent semantics.

### Examples
- Browser plugin (Playwright)
- Spreadsheet plugin
- Email/calendar plugins
- Internal enterprise tool connectors

Design notes
- Plugin tools must declare capabilities and schemas up front.
- MCP server lifecycle and provenance should be auditable.

## Immediate Next Implementation Batch (Recommended)
1. `file.read_json` (ReadOnly)
2. `file.search_text` (ReadOnly, project-scoped)
3. `file.stat` (ReadOnly)
4. `file.write_text` (LocalActions, consent-gated)

These increase utility immediately for "read data / inspect repo / prepare outputs" workflows and fit the current architecture well.
