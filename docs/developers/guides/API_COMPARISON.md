# API Comparison

This document tracks how Pincer's API methods map to standard download management patterns, demonstrating full coverage.

> **See also**: [API Overview](../api/README.md) · [Feature Status](../FEATURE_STATUS.md)

---

## Task Addition & Management

| Standard Method Name | Pincer Method Name | Status |
|:---|:---|:---|
| `addUri` | `pin.addUri` | ✅ Fully Covered |
| `addTorrent` | `pin.addTorrent` | ✅ Fully Covered |
| `addMetalink` | `pin.addMetalink` | ✅ Fully Covered |
| `remove` | `pin.remove` | ✅ Fully Covered |
| `forceRemove` | `pin.forceRemove` | ✅ Fully Covered |
| `pause` | `pin.pause` | ✅ Fully Covered |
| `pauseAll` | `pin.pauseAll` | ✅ Fully Covered |
| `forcePause` | `pin.forcePause` | ✅ Fully Covered |
| `forcePauseAll` | `pin.forcePauseAll` | ✅ Fully Covered |
| `unpause` | `pin.unpause` | ✅ Fully Covered |
| `unpauseAll` | `pin.unpauseAll` | ✅ Fully Covered |
| `changePosition` | `pin.changePosition` | ✅ Fully Covered |
| `changeUri` | `pin.changeUri` | ✅ Fully Covered |

---

## Status & Monitoring

| Function | Pincer Equivalent | Status |
|:---|:---|:---|
| `tellStatus` | `pin.tellStatus` | ✅ Fully Covered |
| `tellActive` | `pin.tellActive` | ✅ Fully Covered |
| `tellWaiting` | `pin.tellWaiting` | ✅ Fully Covered |
| `tellStopped` | `pin.tellStopped` | ✅ Fully Covered |
| `getGlobalStat` | `pin.getGlobalStat` | ✅ Fully Covered |
| `getUris` | `pin.getUris` | ✅ Fully Covered |
| `getFiles` | `pin.getFiles` | ✅ Fully Covered |
| `getPeers` | `pin.getPeers` | ✅ Fully Covered |
| `getServers` | `pin.getServers` | ✅ Fully Covered |

---

## Configuration, History & System

| Function | Pincer Equivalent | Status |
|:---|:---|:---|
| `changeOption` | `pin.changeOption` | ✅ Fully Covered |
| `getOption` | `pin.getOption` | ✅ Fully Covered |
| `changeGlobalOption` | `pin.changeGlobalOption` | ✅ Fully Covered |
| `getGlobalOption` | `pin.getGlobalOption` | ✅ Fully Covered |
| `purgeDownloadResult` | `pin.purgeDownloadResult` | ✅ Fully Covered |
| `removeDownloadResult` | `pin.removeDownloadResult` | ✅ Fully Covered |
| `getVersion` | `pin.getVersion` | ✅ Fully Covered |
| `getSessionInfo` | `pin.getSessionInfo` | ✅ Fully Covered |
| `shutdown` | `pin.shutdown` | ✅ Fully Covered |
| `forceShutdown` | `pin.forceShutdown` | ✅ Fully Covered |
| `saveSession` | `pin.saveSession` | ✅ Fully Covered |
| `system.multicall` | `system.multicall` | ✅ Fully Covered |
| `system.listMethods` | `system.listMethods` | ✅ Fully Covered |
| `system.listNotifications` | `system.listNotifications` | ✅ Fully Covered |

---

## Events

| Event | Pincer Equivalent | Status |
|:---|:---|:---|
| `onDownloadStart` | `pin.onDownloadStart` | ✅ Fully Covered |
| `onDownloadPause` | `pin.onDownloadPause` | ✅ Fully Covered |
| `onDownloadComplete` | `pin.onDownloadComplete` | ✅ Fully Covered |
| `onDownloadError` | `pin.onDownloadError` | ✅ Fully Covered |
| `onDownloadStop` | `pin.onDownloadStop` | ✅ Fully Covered |
| `onBtDownloadComplete` | `pin.onBtDownloadComplete` | ✅ Fully Covered |
