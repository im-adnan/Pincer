// Custom JavaScript for Pincer Engine Documentation

document.addEventListener('DOMContentLoaded', () => {
  // 1. Inject "Report a Bug" and "Downloads" buttons into the top navigation bar
  const rightButtons = document.querySelector('.right-buttons');
  if (rightButtons) {
    // Bug report button
    const bugLink = document.createElement('a');
    bugLink.href = 'https://github.com/im-adnan/pincer-engine/issues/new';
    bugLink.target = '_blank';
    bugLink.rel = 'noopener noreferrer';
    bugLink.title = 'Report a bug or issue on GitHub';
    bugLink.className = 'nav-action-btn nav-btn-bug';
    bugLink.innerHTML = '<span>🐛 Report a Bug</span>';
    rightButtons.insertBefore(bugLink, rightButtons.firstChild);

    // Downloads button
    const downloadLink = document.createElement('a');
    // Calculate relative path to downloads.html based on current nesting
    const depth = (window.location.pathname.match(/\//g) || []).length;
    downloadLink.href = 'https://github.com/im-adnan/pincer-engine/releases/latest';
    downloadLink.title = 'View latest downloads and releases';
    downloadLink.className = 'nav-action-btn nav-btn-download';
    downloadLink.innerHTML = '<span>⬇ Releases</span>';
    rightButtons.insertBefore(downloadLink, bugLink);
  }

  // 2. Fetch latest release details for the Downloads page
  const releaseCard = document.getElementById('release-card');
  if (releaseCard) {
    const releaseTagEl = document.getElementById('release-tag');
    const releaseDateEl = document.getElementById('release-date');
    const releaseDescEl = document.getElementById('release-description');
    const downloadBtn = document.getElementById('primary-download-btn');
    const assetNameEl = document.getElementById('asset-name');

    fetch('https://api.github.com/repos/im-adnan/pincer-engine/releases/latest')
      .then((response) => {
        if (!response.ok) {
          throw new Error(`GitHub API returned status ${response.status}`);
        }
        return response.json();
      })
      .then((release) => {
        const tagName = release.tag_name || 'v1.0.0';
        if (releaseTagEl) releaseTagEl.textContent = `Latest: ${tagName}`;

        if (release.published_at && releaseDateEl) {
          const pubDate = new Date(release.published_at);
          releaseDateEl.textContent = `Released on ${pubDate.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })}`;
        }

        if (releaseDescEl) {
          releaseDescEl.textContent = release.name ? `${release.name}` : `Automated build for macOS (${tagName}).`;
        }

        // Find pincer-macos.zip asset
        if (Array.isArray(release.assets)) {
          const macAsset = release.assets.find((a) => a.name.includes('macos') || a.name.endsWith('.zip'));
          if (macAsset) {
            if (downloadBtn) downloadBtn.href = macAsset.browser_download_url;
            if (assetNameEl) {
              const sizeMb = (macAsset.size / (1024 * 1024)).toFixed(1);
              assetNameEl.textContent = `${macAsset.name} (${sizeMb} MB)`;
            }
          }
        }
      })
      .catch((err) => {
        // Fallback gracefully to defaults
        if (releaseTagEl) releaseTagEl.textContent = 'Latest: v1.0.0';
        if (releaseDateEl) releaseDateEl.textContent = 'Stable Release';
        if (releaseDescEl) {
          releaseDescEl.textContent = 'Pre-compiled binaries are available directly from the GitHub releases archive.';
        }
      });
  }
});
