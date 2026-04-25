class ClaudeGuardian < Formula
  desc "Local observer daemon that intercepts Claude Code hooks and masks PII"
  homepage "https://github.com/Nihal-Ahamed-MS/claude-guardian"
  version "0.1.2"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/Nihal-Ahamed-MS/claude-guardian/releases/download/v0.1.2/claude-guardian-aarch64-apple-darwin"
      sha256 "3b15608658739ba99917d1514a1dabcf0f80b85759acaa6389aa91d02526ae58"
    else
      url "https://github.com/Nihal-Ahamed-MS/claude-guardian/releases/download/v0.1.2/claude-guardian-x86_64-apple-darwin"
      sha256 "c24d1dc538ebab8f046ad20f91df263c20280ec036468898d5bcab7f8dc70d87"
    end
  end

  def install
    bin.install stable.url.split("/").last => "claude-guardian"
  end

  def post_install
    system "#{bin}/claude-guardian", "start"
  rescue
    # Daemon start is best-effort at install time.
  end

  def caveats
    <<~EOS
      claude-guardian has been installed and started as a background daemon.

      It intercepts Claude Code hooks on port 7421 and serves the monitoring
      UI at http://localhost:7422

      Useful commands:
        claude-guardian start   # install hooks and start daemon
        claude-guardian stop    # remove hooks and stop daemon
        claude-guardian logs    # open the web UI
    EOS
  end

  test do
    assert_match "claude-guardian", shell_output("#{bin}/claude-guardian --version")
  end
end
