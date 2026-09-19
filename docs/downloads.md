# Downloads & Releases

Get the latest pre-compiled binaries for Pincer Engine, inspect versions, or report bugs.

---

## Latest Release

<div id="release-card" class="release-card">
  <div class="release-header">
    <span class="release-badge" id="release-tag">Fetching latest version...</span>
    <span class="release-license-badge">GPL-3.0</span>
    <span class="release-date" id="release-date"></span>
  </div>
  <p id="release-description">Loading release details from GitHub...</p>
  <div class="download-actions" id="download-actions">
    <a class="github-release-btn" href="https://github.com/im-adnan/pincer-engine/releases/latest" target="_blank" rel="noopener noreferrer">
      View Release on GitHub →
    </a>
  </div>
</div>

---

## Quick Installation (macOS)

### 1. Install via Homebrew

You can install Pincer using our custom Homebrew tap:

```bash
brew trust im-adnan/pincer/pincer
brew tap im-adnan/pincer
brew install pincer
```

### 2. Verify Installation

Check that Pincer is installed and view its version:

```bash
pincer --version
```

Start the daemon or interactive CLI:

```bash
# Run in daemon mode with RPC on port 6800
pincer --daemon --port 6800

# Or download a file directly
pincer "https://example.com/file.zip"
```

---

## All Releases & Versions

To view past release notes, changelogs, and historical binaries:

- Browse all releases: [GitHub Releases Archive](https://github.com/im-adnan/pincer-engine/releases)
- Review changes in source: [CHANGELOG.md](https://github.com/im-adnan/pincer-engine/blob/main/CHANGELOG.md)

---

## Found a Bug or Have a Feature Request?

We welcome community feedback, bug reports, and suggestions!

<div class="bug-report-box">
  <h3>🐛 Report an Issue</h3>
  <p>Encountered unexpected behavior, download errors, or a crash? Open a bug report directly on our GitHub issue tracker with your CLI log or system details.</p>
  <div class="bug-report-actions">
    <a class="bug-btn" href="https://github.com/im-adnan/pincer-engine/issues/new" target="_blank" rel="noopener noreferrer">
      Open an Issue on GitHub
    </a>
    <a class="issues-list-link" href="https://github.com/im-adnan/pincer-engine/issues" target="_blank" rel="noopener noreferrer">
      View Existing Issues →
    </a>
  </div>
</div>
