# Changelog

## [Unreleased]
### Fixed
- Fixed an issue in `manager.rs` where downloading files with query parameters in their URLs (e.g. from Pexels) caused an unnecessary file conversion attempt to fail. The extension parsing now correctly ignores query strings and URL fragments.
- Made format conversion error handling more robust: if a conversion is unsupported or fails, it will now gracefully restore the original downloaded file and complete successfully, rather than abruptly failing the download after 100% completion.
