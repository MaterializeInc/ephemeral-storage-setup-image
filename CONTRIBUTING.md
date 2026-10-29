# Contributing guide

Thank you for your interest in contributing! This guide will help you understand the project structure, development workflow, and how to submit contributions effectively.

## Project Overview

`ephemeral-storage-setup` is a Rust binary that configures local instance store volumes on cloud instances and runs as a Kubernetes DaemonSet. It detects NVMe devices across different cloud providers (AWS, GCP, Azure), sets them up as either an LVM volume group or swap, and removes node taints to enable workload scheduling.

## Development Setup

### Prerequisites

- Git, Docker
- A [Rust toolchain](https://rustup.rs/)
- [pre-commit](https://pre-commit.com/) and [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) for linting

### Getting Started

1. Fork and clone the repository
2. Install the pre-push hooks: `pip install -r requirements.txt && pre-commit install`

### Key Files

- `src/` - The `ephemeral-storage-setup` binary (`lvm`, `swap`, and `sleep` subcommands)
- `lvm.conf` - LVM configuration installed in the image
- `testdata/` - `lsblk` output fixtures used by the unit tests
- `examples/` - Example node configurations for each cloud provider
- `Dockerfile` - Container image definition

## Testing

CI runs the unit tests, linting, and a Docker build on every PR. To run them locally:

```bash
# Unit tests
cargo test

# rustfmt, clippy, and cargo-deny
./bin/lint
```

### Writing Tests

When adding new features or fixing bugs, please include tests that:
- Test both success and failure scenarios
- Use `lsblk` fixtures in `testdata/` rather than real devices
- Don't require root privileges

## Manual Testing

### Building and Publishing Your Test Image

To thoroughly test your changes in a real environment, you should build a custom Docker image and test it on actual cloud infrastructure. This process allows you to verify that your modifications work correctly with real NVMe devices.

1. Build your Docker image locally:
   ```bash
   docker build -t yourdockerhub/ephemeral-storage-setup-image:test .
   ```

2. Push the image to your Docker Hub account:
   ```bash
   # Log in to Docker Hub
   docker login

   # Push your test image
   docker push yourdockerhub/ephemeral-storage-setup-image:test
   ```

3. Test with real infrastructure by deploying a Kubernetes cluster with your custom image. You can use the Materialize Google Terraform module for this purpose by setting the `disk_setup_image` variable to your custom image:

   ```hcl
   module "materialize" {
     source = "github.com/MaterializeInc/terraform-google-materialize"

     # Other configuration variables...

     # Specify your custom disk setup image
     disk_setup_image = "yourdockerhub/ephemeral-storage-setup-image:test"
   }
   ```

4. After deployment, verify that:
   - NVMe disks are correctly detected for your cloud provider
   - LVM volume groups are properly created
   - The node taint is successfully removed
   - Workloads can be scheduled on the nodes

5. Check the logs of the DaemonSet pods to troubleshoot any issues:
   ```bash
   kubectl logs -n disk-setup disk-setup-pod-name -c disk-setup
   kubectl describe pod disk-setup-pod-name -n disk-setup
   ```

## Submitting Changes

### Pull Request Process

1. Fork the repository and clone your fork
1. Create a branch from `main` for your changes (`feature/your-feature-name`)
1. Make changes and commit with clear messages (`Add support for detecting devices on Azure`)
1. Push your branch to your fork and create a Pull Request to the main repository
1. Ensure all CI checks pass and address any review feedback

Write commit messages in present tense, imperative mood, and reference any related issues.

## Coding Standards

- Code must be formatted with `rustfmt` and pass `clippy` with no warnings
- New dependencies must pass `cargo-deny` checks (see `deny.toml`)
- Use descriptive names, add comments for complex logic, and implement proper error handling

## Documentation and Releases

- Update `README.md` with any new features or changes
- Document new CLI flags and include helpful code comments
- Releases are managed by [release-plz](https://release-plz.dev): merging the release PR it opens tags the release and publishes the Docker image

## Getting Help

Create an issue on GitHub for bugs, feature requests, or questions.
