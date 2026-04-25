# claude-guardian

A lightweight background daemon that intercepts Claude Code hooks to detect and mask PII before requests reach Anthropic servers.

## Install

### Homebrew (macOS)

```bash
brew install Nihal-Ahamed-MS/claude-guardian/claude-guardian
```

Or tap first, then install:

```bash
brew tap Nihal-Ahamed-MS/claude-guardian https://github.com/Nihal-Ahamed-MS/claude-guardian
brew install claude-guardian
```

### Manual

Download the binary for your platform from [Releases](https://github.com/Nihal-Ahamed-MS/claude-guardian/releases), then:

```bash
chmod +x claude-guardian-*
sudo mv claude-guardian-* /usr/local/bin/claude-guardian
claude-guardian start
```

## Usage

```bash
claude-guardian start   # install Claude Code hooks and start daemon
claude-guardian stop    # remove hooks and stop daemon
claude-guardian logs    # open monitoring UI at http://localhost:7422
```

## What it does

- Runs a local HTTP server that Claude Code hooks POST events to
- Scans all hook payloads for secrets (Anthropic/GitHub/AWS API keys, bearer tokens, env secrets, IP addresses)
- Masks sensitive data in-place before it leaves your machine
- For `PreToolUse` hooks, returns the masked `tool_input` so Claude proceeds with redacted values
- Blocks `UserPromptSubmit` entirely if secrets are detected (HTTP 403 → exit 2)
- Stores a log of intercepted events in a local SQLite database
- Provides a web UI to inspect activity and review what was masked

## Ports

| Port | Purpose |
|------|---------|
| 7421 | Hook receiver (Claude Code → guardian) |
| 7422 | Web monitoring UI |

## License
MIT
