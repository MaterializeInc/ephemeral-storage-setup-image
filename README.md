# Bootstrap Ephemeral NVMe Drives with Swap or LVM

This bootstrap container provides a solution for configuring local instance store volumes on cloud instances, by either combining them into an LVM volume group or using them as swap.

## Supported Cloud Providers

- **AWS**: Detects Amazon EC2 NVMe Instance Storage devices, with special handling for Bottlerocket OS
- **GCP**: Detects Google Cloud local SSD devices at the `/dev/disk/by-id/google-local-ssd-*` path
- **Azure**: Detects Azure ephemeral disks at the `/dev/` path

##### GCP note
In GCP the `konnectivity-agent` pods are needed to retrieve any pod logs.
If those run only on nodes with this taint and they do not tolerate it, all pod logs will be inaccessible until the taint is removed.
In the case of failure of the ephemeral disk setup pods, it may be difficult to debug them, as their logs will be inaccessible.

Configuring the `konnectivity-agent` pods to either tolerate the `disk-unconfigured` taint, or to run on nodes without that taint will allow logs to be accessible as normal. A separate pool of nodes for system daemons without ephemeral disks should work fine.

##### Azure note
Azure AKS nodes do not support removing taints.
The issue for fixing this was closed (https://github.com/Azure/AKS/issues/2934), so it is unlikely Microsoft will support this any time soon.
As such, you should not pass the `--remove-taint` argument to the ephemeral disk setup pods, and should not configure your nodes to start with the `disk-unconfigured` taint.
During the time between the node launching and the ephemeral volumes being configured, workloads that rely on those volumes may fail.
This is a sad state of affairs for Azure Kubernetes, and we recommend that you contact your Azure support representative to encourage them to fix this.

There is a work around possible by using an admission controller to apply the taint when the node is created, rather than configuring it using AKS. This is unfortunately out of the scope of this tool for now.

## Usage

### LVM
```bash
Usage: ephemeral-storage-setup lvm [OPTIONS] --cloud-provider <CLOUD_PROVIDER>

Options:
      --cloud-provider <CLOUD_PROVIDER>
          [env: CLOUD_PROVIDER=] [possible values: aws, gcp, azure, generic]
      --node-name <NODE_NAME>
          Name of the Kubernetes node we are running on. This is required if removing the taint [env: NODE_NAME=]
      --taint-key <TAINT_KEY>
          Name of the taint to remove [env: TAINT_KEY=] [default: disk-unconfigured]
      --remove-taint
          [env: REMOVE_TAINT=]
      --vg-name <VG_NAME>
          Name of the LVM volume group to create [env: VG_NAME=] [default: instance-store-vg]
```

### Swap

```bash
Usage: ephemeral-storage-setup swap [OPTIONS] --cloud-provider <CLOUD_PROVIDER>

Options:
      --cloud-provider <CLOUD_PROVIDER>
          [env: CLOUD_PROVIDER=] [possible values: aws, gcp, azure, generic]
      --node-name <NODE_NAME>
          Name of the Kubernetes node we are running on. This is required if removing the taint [env: NODE_NAME=]
      --taint-key <TAINT_KEY>
          Name of the taint to remove [env: TAINT_KEY=] [default: disk-unconfigured]
      --remove-taint
          [env: REMOVE_TAINT=]
```

## Kubernetes Integration

This solution is designed to be deployed as a Kubernetes DaemonSet to automatically configure instance store volumes on nodes.

### Deployment Process

1. The DaemonSet runs on nodes with the `materialize.cloud/disk=true` label
2. The init container runs with privileged access to configure the disks
3. Once disks are configured, the node taint `disk-unconfigured` is removed
4. Pods can then be scheduled on the node

It is recommended that any daemonsets required for networking or logs run on other nodes, or be configured to tolerate this taint.

### Terraform Example (swap)

The example below is for configuring disks as swap space. If you would like to use the disks as an LVM volume group, simply replace the `swap` argument with `lvm`.

```hcl
resource "kubernetes_daemonset" "disk_setup" {
  count = var.enable_disk_setup ? 1 : 0
  metadata {
    name      = "disk-setup"
    namespace = kubernetes_namespace.disk_setup[0].metadata[0].name
    labels = {
      "app.kubernetes.io/managed-by" = "terraform"
      "app.kubernetes.io/part-of"    = "materialize"
      "app"                          = "disk-setup"
    }
  }
  spec {
    selector {
      match_labels = {
        app = "disk-setup"
      }
    }
    template {
      metadata {
        labels = {
          app = "disk-setup"
        }
      }
      spec {
        security_context {
          run_as_non_root = false
          run_as_user     = 0
          fs_group        = 0
          seccomp_profile {
            type = "RuntimeDefault"
          }
        }
        affinity {
          node_affinity {
            required_during_scheduling_ignored_during_execution {
              node_selector_term {
                match_expressions {
                  key      = "materialize.cloud/disk"
                  operator = "In"
                  values   = ["true"]
                }
              }
            }
          }
        }
        # Node taint to prevent regular workloads from being scheduled until disks are configured
        toleration {
          key      = "disk-unconfigured"
          operator = "Exists"
          effect   = "NoSchedule"
        }
        # Use host network and PID namespace
        host_network = true
        host_pid     = true
        init_container {
          name    = "disk-setup"
          image   = var.disk_setup_image
          command = ["ephemeral-storage-setup"]
          args    = [
            "swap",
            "--cloud-provider",
            var.cloud_provider,
            "--remove-taint",
          ]
          resources {
            limits = {
              memory = "128Mi"
            }
            requests = {
              memory = "128Mi"
              cpu    = "50m"
            }
          }
          security_context {
            privileged  = true
            run_as_user = 0
          }
          env {
            name = "NODE_NAME"
            value_from {
              field_ref {
                field_path = "spec.nodeName"
              }
            }
          }
          # Mount all necessary host paths
          volume_mount {
            name       = "dev"
            mount_path = "/dev"
          }
          volume_mount {
            name       = "host-root"
            mount_path = "/host"
          }
        }
        container {
          name  = "pause"
          image = var.disk_setup_image
          command = ["ephemeral-storage-setup"]
          args    = ["sleep"]
          resources {
            limits = {
              memory = "8Mi"
            }
            requests = {
              memory = "8Mi"
              cpu    = "1m"
            }
          }
          security_context {
            allow_privilege_escalation = false
            read_only_root_filesystem  = true
            run_as_non_root            = true
            run_as_user                = 65534
          }
        }
        volume {
          name = "dev"
          host_path {
            path = "/dev"
          }
        }
        volume {
          name = "host-root"
          host_path {
            path = "/"
          }
        }
        service_account_name = kubernetes_service_account.disk_setup[0].metadata[0].name
      }
    }
  }
}

# Service account for the disk setup daemon
resource "kubernetes_service_account" "disk_setup" {
  count = var.enable_disk_setup ? 1 : 0
  metadata {
    name      = "disk-setup"
    namespace = kubernetes_namespace.disk_setup[0].metadata[0].name
  }
}

# RBAC role to allow removing taints
resource "kubernetes_cluster_role" "disk_setup" {
  count = var.enable_disk_setup ? 1 : 0
  metadata {
    name = "disk-setup"
  }
  rule {
    api_groups = [""]
    resources  = ["nodes"]
    verbs      = ["get", "patch", "update"]
  }
}

# Bind the role to the service account
resource "kubernetes_cluster_role_binding" "disk_setup" {
  count = var.enable_disk_setup ? 1 : 0
  metadata {
    name = "disk-setup"
  }
  role_ref {
    api_group = "rbac.authorization.k8s.io"
    kind      = "ClusterRole"
    name      = kubernetes_cluster_role.disk_setup[0].metadata[0].name
  }
  subject {
    kind      = "ServiceAccount"
    name      = kubernetes_service_account.disk_setup[0].metadata[0].name
    namespace = kubernetes_namespace.disk_setup[0].metadata[0].name
  }
}
```

## Contributing

We welcome contributions! Please follow the [Contributing Guide](CONTRIBUTING.md) for details on how to contribute to this project.
