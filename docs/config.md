# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

## MCP output budgets

Each `[mcp_servers.<server>.tools.<tool>]` entry can set a positive
`output_token_limit`. Plugin and user policies combine by taking the smaller
explicit limit, independently of approval policy. This fork counts the complete
serialized output and keeps the current model's byte/token policy and a 10K-token
ceiling in effect. The saved per-tool budget also applies to post-tool hook
feedback and resumed history; a lower current model policy can further reduce it.
Code Mode receives the original typed tool result.
Tool-result envelope metadata also consumes the context-item allowance. Oversized
envelopes fail before a model request is sent. A zero or sub-framing global budget
retains only the empty JSON string/array representation and internal status.

## Lifecycle hooks

Admins can set top-level `allow_managed_hooks_only = true` in
`requirements.toml` to ignore user, project, and session hook configs while
still allowing managed hooks from requirements and managed config layers. This
setting is only supported in `requirements.toml`; putting it in `config.toml`
does not enable managed-hooks-only mode.

Allowlisted executor cleanup hooks run asynchronously and use discovery from the
selected execution step. Stop, SubagentStop, and Interrupt requests report that step's model
and approval mode consistently with their MCP metadata. An interrupted turn with
no captured step uses its initial settings for local hooks and has no executor
cleanup discovery to reuse.
