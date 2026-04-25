# claude-guardian

A lightweight background daemon that intercepts Claude Code hooks to detect and mask PII before requests reach Anthropic servers.

## What it does

- Runs a local HTTP server that Claude Code hooks POST events to
- Scans all hook payloads for secrets (Anthropic/GitHub/AWS API keys, bearer tokens, env secrets, IP addresses)
- Masks sensitive data in-place before it leaves your machine
- Stores a log of intercepted events in a local SQLite database
- Provides a web UI to inspect activity and review what was masked

## License

MIT
