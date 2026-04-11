# Motrix Aria2 Configurations and Commands

This document outlines the Aria2 configurations, keys, and commands used by Motrix, which need to be supported by the future Sluice Rust download engine.

## 1. Engine Configuration (`aria2.conf`)

These are the default parameters passed to the aria2c engine upon initialization:

### RPC
- `enable-rpc=true`
- `rpc-allow-origin-all=true`
- `rpc-listen-all=true`

### File System
- `auto-save-interval=10`
- `disk-cache=64M`
- `file-allocation=none`
- `no-file-allocation-limit=64M`
- `save-session-interval=10`

### Task Parameters
- `check-certificate=false`
- `max-file-not-found=10`
- `max-tries=0`
- `retry-wait=10`
- `connect-timeout=10`
- `timeout=10`
- `min-split-size=1M`
- `http-accept-gzip=true`
- `remote-time=true`
- `summary-interval=0`
- `content-disposition-default-utf8=true`

### BitTorrent Task Parameters
- `bt-detach-seed-only=true`
- `bt-enable-lpd=true`
- `bt-hash-check-seed=true`
- `bt-max-peers=128`
- `bt-prioritize-piece=head`
- `bt-remove-unselected-file=true`
- `bt-seed-unverified=false`
- `bt-tracker-connect-timeout=10`
- `bt-tracker-timeout=10`
- `enable-dht=true`
- `enable-dht6=true`
- `enable-peer-exchange=true`
- `dht-entry-point=dht.transmissionbt.com:6881`
- `dht-entry-point6=dht.transmissionbt.com:6881`
- `peer-agent=Transmission/3.00`
- `peer-id-prefix=-TR3000-`

## 2. Dynamic Config Keys (`configKeys.js`)

Motrix tracks and dynamically updates the following properties:

### User Preferences (UI / Application Level)
`auto-check-update`, `auto-hide-window`, `auto-sync-tracker`, `cookie`, `enable-upnp`, `engine-bin-path`, `engine-max-connection-per-server`, `favorite-directories`, `hide-app-menu`, `history-directories`, `keep-seeding`, `keep-window-state`, `last-check-update-time`, `last-sync-tracker-time`, `locale`, `log-level`, `new-task-show-downloading`, `no-confirm-before-delete-task`, `open-at-login`, `protocols`, `proxy`, `resume-all-when-app-launched`, `run-mode`, `show-progress-bar`, `task-notification`, `theme`, `tracker-source`, `tray-speedometer`

### System Keys (Aria2 Global/Session Level)
These keys map directly to aria2c RPC global option updates (`aria2.changeGlobalOption` or `aria2.changeOption`):
`all-proxy-passwd`, `all-proxy-user`, `all-proxy`, `allow-overwrite`, `allow-piece-length-change`, `always-resume`, `async-dns`, `auto-file-renaming`, `bt-enable-hook-after-hash-check`, `bt-enable-lpd`, `bt-exclude-tracker`, `bt-external-ip`, `bt-force-encryption`, `bt-hash-check-seed`, `bt-load-saved-metadata`, `bt-max-peers`, `bt-metadata-only`, `bt-min-crypto-level`, `bt-prioritize-piece`, `bt-remove-unselected-file`, `bt-request-peer-speed-limit`, `bt-require-crypto`, `bt-save-metadata`, `bt-seed-unverified`, `bt-stop-timeout`, `bt-tracker-connect-timeout`, `bt-tracker-interval`, `bt-tracker-timeout`, `bt-tracker`, `check-integrity`, `checksum`, `conditional-get`, `connect-timeout`, `content-disposition-default-utf8`, `continue`, `dht-file-path`, `dht-file-path6`, `dht-listen-port`, `dir`, `dry-run`, `enable-http-keep-alive`, `enable-http-pipelining`, `enable-mmap`, `enable-peer-exchange`, `file-allocation`, `follow-metalink`, `follow-torrent`, `force-save`, `force-sequential`, `ftp-passwd`, `ftp-pasv`, `ftp-proxy-passwd`, `ftp-proxy-user`, `ftp-proxy`, `ftp-reuse-connection`, `ftp-type`, `ftp-user`, `gid`, `hash-check-only`, `header`, `http-accept-gzip`, `http-auth-challenge`, `http-no-cache`, `http-passwd`, `http-proxy-passwd`, `http-proxy-user`, `http-proxy`, `http-user`, `https-proxy-passwd`, `https-proxy-user`, `https-proxy`, `index-out`, `listen-port`, `lowest-speed-limit`, `max-concurrent-downloads`, `max-connection-per-server`, `max-download-limit`, `max-file-not-found`, `max-mmap-limit`, `max-overall-download-limit`, `max-overall-upload-limit`, `max-resume-failure-tries`, `max-tries`, `max-upload-limit`, `metalink-base-uri`, `metalink-enable-unique-protocol`, `metalink-language`, `metalink-location`, `metalink-os`, `metalink-preferred-protocol`, `metalink-version`, `min-split-size`, `no-file-allocation-limit`, `no-netrc`, `no-proxy`, `no-want-digest-header`, `out`, `parameterized-uri`, `pause-metadata`, `pause`, `piece-length`, `proxy-method`, `realtime-chunk-checksum`, `referer`, `remote-time`, `remove-control-file`, `retry-wait`, `reuse-uri`, `rpc-listen-port`, `rpc-save-upload-metadata`, `rpc-secret`, `seed-ratio`, `seed-time`, `select-file`, `split`, `ssh-host-key-md`, `stream-piece-selector`, `timeout`, `uri-selector`, `use-head`, `user-agent`

### Keys Requiring Engine Restart
`dht-listen-port`, `hide-app-menu`, `listen-port`, `rpc-listen-port`, `rpc-secret`
