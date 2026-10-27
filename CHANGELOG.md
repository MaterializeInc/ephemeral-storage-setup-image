# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.2](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.4.0...v0.4.2) - 2026-10-27

### Other

- Replace serde_yaml with serde-saphyr
- Use k8s-openapi v1_35 feature
- Update Rust dependencies
- Fix ETXTBSY race between mock writes and spawns in tests
- Merge pull request #72 from MaterializeInc/renovate/openssl-0.x-lockfile
- Merge pull request #67 from MaterializeInc/release-plz
- Update Rust crate openssl to v0.10.81
- Merge pull request #63 from MaterializeInc/azure-existing-kubelet-config
- Bump and pin actions in release-plz workflow
- Update Rust crate openssl to v0.10.80 [SECURITY]
- Release with release-plz
- Add renovate.json
- Add .github/CODEOWNERS
- Bump docker/metadata-action from 5 to 6
- Bump rust from 1.91.1-alpine3.22 to 1.99.1-alpine3.22
- Update cc and move to shlex 2
- Find the Azure kubelet config file with systemctl show
- Merge swap settings into an existing Azure kubelet config file
- Move update_kubelet_config to a free function
