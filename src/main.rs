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
            LvmController {
                commander,
                disk_detector,
                node_name: common_args.node_name,
                taint_key: common_args.taint_key,
                vg_name,
            }
            .setup()
        }
        Commands::Swap { common_args } => {
            let disk_detector = DiskDetector::new(commander.clone(), common_args.cloud_provider);
            SwapController {
                commander,
                disk_detector,
                node_name: common_args.node_name,
                taint_key: common_args.taint_key,
            }
            .setup()
        }
    }
}
