# Threat Model

## Goals
- Prevent silent execution of high-risk actions.
- Preserve a tamper-evident audit trail of tool usage and policy decisions.
- Isolate provider and plugin integrations to reduce supply-chain blast radius.

## Key Threats

### Prompt Injection
- Risk: Untrusted content (web pages, documents, chat messages) attempts to coerce unsafe tool execution.
- Mitigations:
  - Policy gate authorizes every tool call before execution.
  - Explicit user confirmation for sensitive capability tiers.
  - Tool schemas constrain argument shapes.
  - Evidence capture records what was executed and why.

### Supply Chain (Providers / Plugins / Native Dependencies)
- Risk: Malicious or compromised dependencies, provider SDKs, or plugins.
- Mitigations:
  - Provider integrations are process-isolated via JSON-RPC over stdio (plugin process model).
  - Optional Wasm plugin host stub for stronger sandboxing in future work.
  - Minimize default capabilities and use signed releases for production builds.
  - Review and pin dependency versions in CI before promotion.

### Plugin Isolation Failures
- Risk: Plugin escapes process boundaries or gains broad filesystem/network access.
- Mitigations:
  - Capability tiers mapped to explicit policy decisions.
  - Brokered tool execution through core action backends only.
  - Prefer allowlisted methods and validate JSON-RPC envelopes.

### Audit Log Tampering / Loss
- Risk: Actions occur without accountability or logs are modified after the fact.
- Mitigations:
  - Append-only audit events with per-request `audit_id`.
  - Record policy outcomes, consent prompts, tool arguments (redacted where needed), and evidence references.
  - Forward logs to OS logging / secure storage in platform shells (future work).

## Trust Boundaries
- User UI (desktop/mobile/CLI)
- Core orchestrator + policy engine
- Action backends (OS/platform integrations)
- Provider plugins / model backends (potentially untrusted)
- External services / networks

## Residual Risks
- Stubbed provider/plugin implementations are not sandboxed yet.
- No cryptographic audit signing in scaffold.
- No runtime secret management beyond placeholders.
