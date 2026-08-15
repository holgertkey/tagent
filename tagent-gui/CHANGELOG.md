# Changelog

All notable changes to `tagent-gui` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`tagent-gui` versions independently of `tagent-cli` (see the root
[CHANGELOG.md](../CHANGELOG.md) for that application's history) and independently of
the `tagent` library. See [`tagent-gui development plan.md`](../.debug/tagent-gui%20development%20plan.md)
for the roadmap and design decisions behind this project.

## [Unreleased]

### Changed
- Established `tagent-gui` as a fully independent application from `tagent-cli`:
  own interface, own configuration (roadmap), own feature set, own versioning —
  built only on the `tagent` library. This file starts tracking notable changes
  from this point forward.

## [0.13.0] - 2026-08-05

### Added
- Initial `tagent-gui` prototype: Slint desktop GUI, translate-only, built directly
  on the `tagent` library. Not tracked by this changelog prior to this entry.
