use std::io::ErrorKind;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

use clap::{CommandFactory, Parser, Subcommand};

use ephemeral_storage_setup::detect::DiskDetector;
use ephemeral_storage_setup::lvm::LvmController;
use ephemeral_storage_setup::swap::SwapController;
use ephemeral_storage_setup::{CloudProvider, Commander};

#[derive(Parser)]
#[clap(name = "disk-setup")]
struct CliArgs {
    /// What action to take.
    ///
    /// If not provided, we attempt to detect the arguments from user-data files.
    /// This case is useful for BottleRocket and other environments that allow
    /// configuration with containers, but don't allow args to be passed to them.
    ///
    /// If using user-data, the files must contain a json array of args.
    /// Do not include the executable name in the array.
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Lvm {
        #[clap(flatten)]
        common_args: CommonArgs,

        /// Name of the LVM volume group to create.
        #[arg(long, env, default_value = "instance-store-vg")]
        vg_name: String,
    },
    Swap {
        #[clap(flatten)]
        common_args: CommonArgs,

        /// Enable swap on bottlerocket nodes using its apiclient.
        #[clap(long, env, group = "swap-hacks")]
        bottlerocket_enable_swap: bool,

        /// Enable swap by hackily modifying the kubelet config and restarting it.
        #[clap(long, env, group = "swap-hacks")]
        hack_restart_kubelet_enable_swap: bool,

        /// Apply sysctl settings to make swap more effective and safer.
        ///
        /// This doesn't work on bottlerocket.
        #[clap(long, env)]
        apply_sysctls: bool,

        /// Controls the weight of application data vs filesystem cache
        /// when moving data out of memory and into swap.
        /// 0 effectively disables swap, 100 treats them equally.
        /// For Materialize uses, they are equivalent, so we set it to 100.
        #[arg(long, env, default_value_t = 100)]
        vm_swappiness: usize,

        /// Always reserve a minimum amount of actual free RAM.
        /// Setting this value to 1GiB makes it much less likely that we hit OOM
        /// while we still have swap space available we could have used.
        #[arg(long, env, default_value_t = 1048576)]
        vm_min_free_kbytes: usize,

        /// Increase the aggressiveness of kswapd.
        /// Higher values will cause kswapd to swap more and earlier.
        #[arg(long, env, default_value_t = 100)]
        vm_watermark_scale_factor: usize,
    },
    /// Don't do anything, just sleep.
    /// This allows us to not need a separate image just to keep
    /// the daemonset alive after we have initialized things.
    Sleep,
}

#[derive(Parser)]
struct CommonArgs {
    #[clap(long, env)]
    cloud_provider: CloudProvider,

    /// Name of the Kubernetes node we are running on.
    /// This is required if removing the taint.
    #[clap(long, env)]
    node_name: Option<String>,

    /// Name of the taint to remove.
    #[clap(long, env, default_value = "disk-unconfigured")]
    taint_key: String,

    #[clap(long, env, requires_if("true", "node_name"))]
    remove_taint: bool,
}

fn print_help_and_exit() -> ! {
    CliArgs::command().print_help().unwrap();
    exit(2)
}

fn main() {
    let args = CliArgs::parse();
    let command = args.command.unwrap_or_else(|| {
        // If they didn't pass a command, try to detect if we're a bottlerocket
        // bootstrap container with the args in user-data.
        let userdata_path = "/.bottlerocket/bootstrap-containers/current/user-data";
        match std::fs::read_to_string(userdata_path) {
            Ok(userdata) => {
                println!("Found userdata at '{userdata_path}'");
                let args: Vec<String> =
                    serde_json::from_str(&userdata).expect("Userdata must be a json array of args");
                CliArgs::parse_from(
                    // Clap expects the first argument to be the name of the executable,
                    // but it doesn't really make sense for that to be set by the user here.
                    std::iter::once("ephemeral-storage-setup")
                        .chain(args.iter().map(|s| s.as_str())),
                )
                .command
                .unwrap_or_else(|| {
                    print_help_and_exit();
                })
            }
            Err(e) if e.kind() == ErrorKind::NotFound => print_help_and_exit(),
            Err(e) => panic!("{e:?}"),
        }
    });
    let commander = Commander::default();
    match command {
        Commands::Lvm {
            common_args:
                CommonArgs {
                    cloud_provider,
                    node_name,
                    taint_key,
                    remove_taint,
                },
            vg_name,
        } => {
            let disk_detector = DiskDetector::new(commander.clone(), cloud_provider);
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(
                    LvmController {
                        commander,
                        disk_detector,
                        node_name,
                        taint_key,
                        remove_taint,
                        vg_name,
                    }
                    .setup(),
                )
        }
        Commands::Swap {
            common_args:
                CommonArgs {
                    cloud_provider,
                    node_name,
                    taint_key,
                    remove_taint,
                },
            bottlerocket_enable_swap,
            hack_restart_kubelet_enable_swap,
            apply_sysctls,
            vm_swappiness,
            vm_min_free_kbytes,
            vm_watermark_scale_factor,
        } => {
            let disk_detector = DiskDetector::new(commander.clone(), cloud_provider);
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(
                    SwapController {
                        cloud_provider,
                        commander,
                        disk_detector,
                        node_name,
                        taint_key,
                        remove_taint,
                        bottlerocket_enable_swap,
                        hack_restart_kubelet_enable_swap,
                        apply_sysctls,
                        vm_swappiness,
                        vm_min_free_kbytes,
                        vm_watermark_scale_factor,
                    }
                    .setup(),
                )
        }
        Commands::Sleep => loop {
            sleep(Duration::from_secs(3600));
        },
    }
}
