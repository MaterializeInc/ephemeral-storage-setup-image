use clap::{Parser, Subcommand};

use ephemeral_storage_setup::detect::DiskDetector;
use ephemeral_storage_setup::lvm::LvmController;
use ephemeral_storage_setup::swap::SwapController;
use ephemeral_storage_setup::{CloudProvider, Commander};

#[derive(Parser)]
#[clap(name = "disk-setup")]
struct CliArgs {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Lvm {
        #[clap(flatten)]
        common_args: CommonArgs,

        #[arg(long, env, default_value = "instance-store-vg")]
        vg_name: String,
    },
    Swap {
        #[clap(flatten)]
        common_args: CommonArgs,

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
}

#[derive(Parser)]
struct CommonArgs {
    #[clap(long, env)]
    cloud_provider: CloudProvider,

    #[clap(long, env)]
    node_name: String,

    #[clap(long, env, default_value = "disk-unconfigured")]
    taint_key: String,
}

fn main() {
    let args = CliArgs::parse();
    let commander = Commander::default();
    match args.command {
        Commands::Lvm {
            common_args,
            vg_name,
        } => {
            let disk_detector = DiskDetector::new(commander.clone(), common_args.cloud_provider);
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(
                    LvmController {
                        commander,
                        disk_detector,
                        node_name: common_args.node_name,
                        taint_key: common_args.taint_key,
                        vg_name,
                    }
                    .setup(),
                )
        }
        Commands::Swap {
            common_args,
            vm_swappiness,
            vm_min_free_kbytes,
            vm_watermark_scale_factor,
        } => {
            let disk_detector = DiskDetector::new(commander.clone(), common_args.cloud_provider);
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(
                    SwapController {
                        commander,
                        disk_detector,
                        node_name: common_args.node_name,
                        taint_key: common_args.taint_key,
                        vm_swappiness,
                        vm_min_free_kbytes,
                        vm_watermark_scale_factor,
                    }
                    .setup(),
                )
        }
    }
}
