use clap::Parser;
use siput_core::{cli::cli::{Cli, CliHandler, Commands}, observability};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize comprehensive observability
    let observability_config = observability::ObservabilityConfig::default();
    observability::init_observability(observability_config).await?;

    println!("╔═══════════════════════════════════════════════════╗");
    println!("║ Siput Blockchain Node                       ║");
    println!("║ BlockDAG + GHOSTDAG + State Execution + Finality ║");
    println!("╚═══════════════════════════════════════════════════╝\n");

    // If no CLI arguments are provided, start the interactive menu.
    // This keeps compatibility with existing CLI subcommands.
    let args: Vec<String> = std::env::args().collect();
    if args.len() <= 1 {
        // Interactive mode
        let interactive = siput_core::cli::interactive::InteractiveCli::new();
        interactive.run().await?;
        return Ok(());
    }

    // Parse CLI arguments (legacy mode)
    let cli = Cli::parse();
    let handler = CliHandler::new();

    // Handle commands
    match cli.command {
        Commands::Node { node_command } => match node_command {
            siput_core::cli::cli::NodeCommands::Start {
                listen_addr,
                rpc_addr,
            } => {
                handler.handle_node_start(&listen_addr, &rpc_addr).await?;
            }
        },
        Commands::Wallet { wallet_command } => match wallet_command {
            siput_core::cli::cli::WalletCommands::Create { output } => {
                handler.handle_wallet_create(output.as_deref()).await?;
            }
            siput_core::cli::cli::WalletCommands::Balance { address, rpc_url } => {
                handler.handle_wallet_balance(&address, &rpc_url).await?;
            }
        },
        Commands::Tx { tx_command } => match tx_command {
            siput_core::cli::cli::TxCommands::Send {
                wallet,
                to,
                amount,
                rpc_url,
            } => {
                handler
                    .handle_tx_send(&wallet, &to, amount, &rpc_url)
                    .await?;
            }
        },
        Commands::Contract { contract_command } => match contract_command {
            siput_core::cli::cli::ContractCommands::Deploy {
                wasm,
                wallet,
                rpc_url,
            } => {
                handler
                    .handle_contract_deploy(&wasm, wallet.as_deref(), &rpc_url)
                    .await?;
            }
            siput_core::cli::cli::ContractCommands::Call {
                contract,
                method,
                args,
                wallet,
                rpc_url,
            } => {
                handler
                    .handle_contract_call(
                        &contract,
                        &method,
                        args.as_deref(),
                        wallet.as_deref(),
                        &rpc_url,
                    )
                    .await?;
            }
        },
    }

    // Shutdown observability systems
    observability::shutdown_observability().await?;

    Ok(())
}
