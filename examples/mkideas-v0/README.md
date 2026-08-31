# MK Ideas Agents

The directory keeps its original `mkideas-v0` path so existing local imports do
not break, but the pack now contains the five approved MK Ideas personas:

- Guest Researcher
- Outreach Drafter
- Interview Producer
- Content / Clip Copilot
- Operations Briefing Assistant

Validate it with:

```sh
buzz pack validate examples/mkideas-v0
node examples/mkideas-v0/tests/validate-agent-contract-fixtures.mjs
```

Import the personas through Buzz's existing owner-reviewed agent creation flow. Each runtime identity must be registered as a managed agent on the private community. The relay rejects kind `48201` proposals from ordinary human identities.

The pack intentionally declares no model, API key, provider, or external MCP
server. Local deterministic fixtures are the default integration surface until
an owner separately approves a provider, network policy, and budget.

Agents publish only capability-allowed proposals (`48201`), informational
summaries (`48203`), and runner lifecycle status (`48204`). They never publish
addressable MK state or human approval actions. Reviewers approve or reject a
pending proposal from the product, producing a separate human-signed action.
The machine-readable boundaries live in
[`contracts/agent-capabilities.v2.json`](contracts/agent-capabilities.v2.json).

The JSON fixtures under `fixtures/agents/v2/` are unsigned payload examples,
not relay events. They use fixed IDs, timestamps, fake/local provider metadata,
and synthetic sources so contract tests are deterministic and perform no
network calls.

For a repeat-safe staging dataset, run `buzz mk-ideas seed-demo` with an owner or admin identity. The command uses stable entity UUIDs, queries each current head before writing, preserves a source identifier, and clearly labels every record as synthetic. Running it again reports the existing records instead of creating duplicates.
