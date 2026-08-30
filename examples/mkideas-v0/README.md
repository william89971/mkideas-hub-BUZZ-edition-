# MK Ideas V0 Agents

This persona pack contains the two contextual agents required by the V0 architecture demonstration:

- Guest Researcher
- Content / Clip Copilot

Validate it with:

```sh
buzz pack validate examples/mkideas-v0
```

Import the personas through Buzz's existing owner-reviewed agent creation flow. Each runtime identity must be registered as a managed agent on the private community. The relay rejects kind `48201` proposals from ordinary human identities.

The agents publish proposals only. Reviewers approve or reject them from Today or Studio, which creates separate human-signed approval state and action events.

For a repeat-safe staging dataset, run `buzz mk-ideas seed-demo` with an owner or admin identity. The command uses stable entity UUIDs, queries each current head before writing, preserves a source identifier, and clearly labels every record as synthetic. Running it again reports the existing records instead of creating duplicates.
