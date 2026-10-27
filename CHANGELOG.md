# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.1](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.5.0...v0.5.1) - 2026-10-27

### Changed

- Update the alpine base image to 3.24 and tonistiigi/xx to 1.10.0.
- Update and pin GitHub Actions to commit digests.

## [0.5.0](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.4.2...v0.5.0) - 2026-10-27

### Changed

- Replace `serde_yaml` with `serde-saphyr`.
- Update the Rust toolchain image to 1.99.1 and other Rust dependencies.
- Release with release-plz.

### Fixed

- On Azure, merge swap settings into the kubelet config file passed via `KUBELET_CONFIG_FILE_FLAGS` instead of overwriting it. This fixes AKS Node Auto Provisioning nodes losing their kubelet authentication and cluster DNS settings.

### Security

- Update openssl to 0.10.81.

## [0.4.2](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.4.1...v0.4.2) - 2026-07-10

### Changed

- Raise swap device priority from 10 to 2048, so disk swap is preferred over Bottlerocket's default zram swap (priority 1024).
- Update the alpine base image to 3.23 and the Rust toolchain image to 1.91.1.

## [0.4.1](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.4.0...v0.4.1) - 2025-12-06

### Added

- Example manifests.

### Changed

- Give all swap devices the same priority, so the kernel uses them round-robin.

## [0.4.0](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.3.4...v0.4.0) - 2025-09-08

### Changed

- **Breaking:** the default taint key is now `startup-taint.cluster-autoscaler.kubernetes.io/disk-unconfigured` (previously `disk-unconfigured`), so the GCP cluster autoscaler will scale up nodes with the taint.

## [0.3.4](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.3.3...v0.3.4) - 2025-09-03

### Added

- Debug and trace logging for device discovery.

## [0.3.3](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.3.2...v0.3.3) - 2025-08-27

### Fixed

- Identifying disks that are not labeled as NVMe.

## [0.3.2](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.3.1...v0.3.2) - 2025-08-27

### Added

- Restart kubelet after enabling swap.

### Fixed

- Detect disks on newer GCP node types that include `nvme` in the device ID.

## [0.3.1](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.3.0...v0.3.1) - 2025-08-07

### Added

- Argument to enable swap on Bottlerocket.

## [0.3.0](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.2.0...v0.3.0) - 2025-08-06

### Added

- Bottlerocket bootstrap container support.

## [0.2.0](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.1.2...v0.2.0) - 2025-08-04

### Changed

- Rewrite in Rust, using the Kubernetes API directly instead of `kubectl`.
- Removing the taint is now optional.

### Added

- Swap support.
- Configurable sysctl values.
- A sleep command, to avoid the need for pause images.

## [0.1.2](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.1.1...v0.1.2) - 2025-04-28

### Added

- Azure support.

## [0.1.1](https://github.com/MaterializeInc/ephemeral-storage-setup-image/compare/v0.1.0...v0.1.1) - 2025-04-22

### Changed

- Docker image tags include the `v` prefix.

## [0.1.0](https://github.com/MaterializeInc/ephemeral-storage-setup-image/releases/tag/v0.1.0) - 2025-04-21

### Added

- Initial release: shell scripts to configure local disks and remove the node taint.
