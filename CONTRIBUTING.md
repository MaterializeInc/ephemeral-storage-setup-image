# Contributing guide

Thank you for your interest in contributing! This guide will help you understand the project structure, development workflow, and how to submit contributions effectively.

## Project Overview

Bootstrap LVM configures local instance store volumes on cloud instances and runs as a Kubernetes DaemonSet. It detects NVMe devices across different cloud providers (AWS, GCP, Azure), sets up LVM volume groups, and removes node taints to enable workload scheduling.

## Development Setup

### Prerequisites

- Git, Docker, Bash
- [Bats](https://bats-core.readthedocs.io/en/stable/installation.html) for testing
- [ShellCheck](https://github.com/koalaman/shellcheck) for shell script linting
- [pre-commit](https://pre-commit.com/) for automated checks

### Getting Started

1. Fork and clone the repository
2. Install pre-commit hooks: `pip install pre-commit && pre-commit install`

### Key Files

- `configure-disks.sh` - Main disk configuration script
- `remove-taint.sh` - Script to remove Kubernetes node taints
- `tests/` - Test suite using Bats framework
- `Dockerfile` - Container image definition

## Testing

We use the Bats (Bash Automated Testing System) framework for testing shell scripts.

### Running Tests

```bash
# Run all tests
./tests/run-tests.sh

# Run specific test file
bats tests/configure-disks.bats
```

### Writing Tests

When adding new features or fixing bugs, please include tests that:
- Test both success and failure scenarios
- Use the provided `test_helpers.sh` functions for creating mocks
- Don't require root privileges when possible

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
   kubectl logs -n disk-setup disk-setup-pod-name -c remove-taint
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

- Shell scripts should start with `#!/usr/bin/env bash` and use `set -euo pipefail`
- Follow [Google's Shell Style Guide](https://google.github.io/styleguide/shellguide.html)
- All scripts must pass ShellCheck linting
- Use descriptive variable names, add comments for complex logic, and implement proper error handling

## Documentation and Releases

- Update `README.md` with any new features or changes
- Document script parameters and include helpful code comments
- For releases, create a tag following semantic versioning (`git tag -a v0.2.0 -m v0.2.0`) and push the tag to the repository
- GitHub Actions will automatically build and publish Docker images from tags

## Getting Help

Create an issue on GitHub for bugs, feature requests, or questions.
