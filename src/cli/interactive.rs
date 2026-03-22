use std::io::{self, Write};
use std::sync::Arc;
use std::time::Instant;

use sysinfo::System;
use parking_lot::{Mutex, RwLock};

use crate::core::transaction::{Address, Transaction};
use crate::node::NodeRuntime;
use crate::wallet::Wallet;

pub struct AppState {
    pub node_online: bool,
    pub peers_count: usize,
    pub hashrate_mhs: f64,
    pub wallet_balance: f64,
    pub block_height: u64,
    pub cpu_usage: f32,
    pub mem_usage: f32,
    pub uptime_sec: u64,
    pub wallet: Wallet,
    pub wallet_address: Address,
}

impl Default for AppState {
    fn default() -> Self {
        let wallet = Wallet::new();
        let wallet_address = parse_wallet_address(&wallet).unwrap_or([0u8; 20]);
        Self {
            node_online: false,
            peers_count: 0,
            hashrate_mhs: 0.0,
            wallet_balance: 0.0,
            block_height: 0,
            cpu_usage: 0.0,
            mem_usage: 0.0,
            uptime_sec: 0,
            wallet,
            wallet_address,
        }
    }
}

fn parse_wallet_address(wallet: &Wallet) -> Option<Address> {
    let trimmed = wallet.address.strip_prefix("spt")?;
    let bytes = hex::decode(trimmed).ok()?;
    if bytes.len() != 20 {
        return None;
    }
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes);
    Some(addr)
}

pub struct InteractiveCli {
    node: Arc<RwLock<Option<Arc<NodeRuntime>>>>,
    node_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    start_time: Instant,
}

impl InteractiveCli {
    pub fn new() -> Self {
        Self {
            node: Arc::new(RwLock::new(None)),
            node_handle: Arc::new(Mutex::new(None)),
            start_time: Instant::now(),
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut app = AppState::default();

        loop {
            self.update_metrics(&mut app);
            self.clear_screen();
            self.render_header(&app);
            self.render_main_menu();

            print!("Input pilihan: ");
            io::stdout().flush()?;

            let mut cmd = String::new();
            io::stdin().read_line(&mut cmd)?;
            match cmd.trim() {
                "1" => { self.node_menu(&mut app).await?; }
                "2" => { self.tx_menu(&mut app).await?; }
                "3" => { self.transaksi_menu(&mut app).await?; }
                "4" => { self.wallet_menu(&mut app).await?; }
                "5" => { self.contract_menu(&mut app).await?; }
                    "6" => { self.sdk_menu(&mut app).await?; }
                "7" => { self.network_menu().await?; }
                "8" => { self.explorer_menu().await?; }
                "9" => { self.help_menu().await?; }
                "0" => break,
                _ => {
                    println!("Pilihan tidak dikenal. Tekan Enter untuk coba lagi.");
                    let mut _tmp = String::new();
                    io::stdin().read_line(&mut _tmp)?;
                }
            }
        }

        Ok(())
    }

    fn clear_screen(&self) {
        print!("[2J[1;1H");
    }

    fn update_metrics(&self, app: &mut AppState) {
        let mut sys = System::new_all();
        sys.refresh_all();

        app.cpu_usage = sys.global_cpu_usage();
        app.mem_usage = if sys.total_memory() > 0 {
            sys.used_memory() as f32 / sys.total_memory() as f32 * 100.0
        } else {
            0.0
        };

        app.uptime_sec = self.start_time.elapsed().as_secs();

        if let Some(node) = self.node.read().as_ref() {
            app.node_online = true;
            app.peers_count = node.get_connection_count();
            app.block_height = node.get_current_height();
            app.hashrate_mhs = node.get_hash_rate() / 1_000_000.0;
            app.wallet_balance = node.get_balance(&app.wallet_address) as f64;
        } else {
            app.node_online = false;
            app.peers_count = 0;
            app.block_height = 0;
            app.hashrate_mhs = 0.0;
            app.wallet_balance = 0.0;
        }
    }

    fn render_header(&self, app: &AppState) {
        let node_status = if app.node_online { "ONLINE" } else { "OFFLINE" };
        let mining_status = if app.hashrate_mhs > 0.0 { "ACTIVE" } else { "INACTIVE" };
        let uptime = format!("{:02}:{:02}:{:02}", app.uptime_sec / 3600, (app.uptime_sec % 3600) / 60, app.uptime_sec % 60);

        let status_line = format!(
            "Peers: {} | Height: {} | CPU: {:.1}% | RAM: {:.1}% | Node: {} | Mining: {} | Uptime: {}",
            app.peers_count, app.block_height, app.cpu_usage, app.mem_usage, node_status, mining_status, uptime
        );

        let width = 80;

        let parts = if status_line.len() > width {
            let first = status_line[..width].to_string();
            let second = status_line[width..].trim_start().to_string();
            vec![first, second]
        } else {
            vec![status_line]
        };

        println!("+{:-<1$}+", "", width + 2);
        for line in &parts {
            println!("| {:<width$} |", line, width = width);
        }
        println!("+{:-<1$}+", "", width + 2);
        println!("");
    }

    fn render_main_menu(&self) {
        println!("--- Menu Utama Siput ---");
        println!("1. Node");
        println!("2. Tx");
        println!("3. Transaksi");
        println!("4. Wallet");
        println!("5. Smart Contract");
        println!("6. SDK");
        println!("7. Network");
        println!("8. Explorer");
        println!("9. Help");
        println!("0. Exit");
        println!("");
    }

    async fn node_menu(&self, app: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            self.clear_screen();
            self.render_header(app);
            println!("--- Node Menu ---");
            println!("1. Start Node");
            println!("2. Start Mining");
            println!("3. Status Node");
            println!("4. Status Mining");
            println!("5. Back");

            print!("Input: ");
            io::stdout().flush()?;

            let mut item = String::new();
            io::stdin().read_line(&mut item)?;
            match item.trim() {
                "1" => { self.start_node().await?; println!("Node started."); },
                "2" => {
                    print!("Masukkan Address Reward: ");
                    io::stdout().flush()?;
                    let mut reward = String::new();
                    io::stdin().read_line(&mut reward)?;
                    if reward.trim().is_empty() {
                        println!("Alamat reward kosong");
                    } else {
                        self.start_mining().await?;
                        println!("Mining started dengan reward ({})", reward.trim());
                    }
                }
                "3" => println!("Status Node: {}", if app.node_online { "ONLINE" } else { "OFFLINE" }),
                "4" => println!("Status Mining: {}", if app.hashrate_mhs > 0.0 { "ACTIVE" } else { "INACTIVE" }),
                "5" => break,
                _ => println!("Pilihan salah"),
            }
            println!("Tekan Enter untuk melanjutkan...");
            let mut _tmp = String::new();
            io::stdin().read_line(&mut _tmp)?;
        }
        Ok(())
    }

    async fn tx_menu(&self, app: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            self.clear_screen();
            self.render_header(app);
            println!("--- Tx Menu ---");
            println!("1. Send Coin");
            println!("2. History");
            println!("3. Back");

            print!("Input: ");
            io::stdout().flush()?;

            let mut item = String::new();
            io::stdin().read_line(&mut item)?;
            match item.trim() {
                "1" => {
                    if let Some(node) = self.node.read().as_ref() {
                        let to: Address = [0xFF; 20];
                        let nonce = node.get_nonce(&app.wallet_address);
                        let mut tx = Transaction::new_transfer(app.wallet_address, to, 1, nonce, 21_000, 1);
                        if let Err(err) = tx.sign(&app.wallet.private_key) {
                            println!("Sign error: {}", err);
                        } else if let Err(err) = node.add_transaction(tx).await {
                            println!("Add tx error: {}", err);
                        } else {
                            println!("Tx dikirim")
                        }
                    } else {
                        println!("Node belum aktif");
                    }
                }
                "2" => println!("History: belum tersedia"),
                "3" => break,
                _ => println!("Pilihan salah"),
            }
            println!("Tekan Enter untuk melanjutkan...");
            let mut _tmp = String::new();
            io::stdin().read_line(&mut _tmp)?;
        }
        Ok(())
    }

    async fn transaksi_menu(&self, app: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
        self.tx_menu(app).await
    }

    async fn wallet_menu(&self, app: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            self.clear_screen();
            self.render_header(app);
            println!("--- Wallet Menu ---");
            println!("1. Generate New Address");
            println!("2. Show Address");
            println!("3. Check Balance");
            println!("4. Back");

            print!("Input: ");
            io::stdout().flush()?;

            let mut item = String::new();
            io::stdin().read_line(&mut item)?;
            match item.trim() {
                "1" => {
                    app.wallet = Wallet::new();
                    app.wallet_address = parse_wallet_address(&app.wallet).unwrap_or([0u8; 20]);
                    println!("New address: {}", app.wallet.address);
                }
                "2" => println!("Address: {}", app.wallet.address),
                "3" => {
                    if let Some(node) = self.node.read().as_ref() {
                        println!("Balance: {}", node.get_balance(&app.wallet_address));
                    } else {
                        println!("Node offline");
                    }
                }
                "4" => break,
                _ => println!("Pilihan salah"),
            }
            println!("Tekan Enter untuk melanjutkan...");
            let mut _tmp = String::new();
            io::stdin().read_line(&mut _tmp)?;
        }
        Ok(())
    }

    async fn contract_menu(&self, app: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            self.clear_screen();
            self.render_header(app);
            println!("--- Smart Contract Menu ---");
            println!("1. Deploy Wasm++");
            println!("2. Back");

            print!("Input: ");
            io::stdout().flush()?;

            let mut item = String::new();
            io::stdin().read_line(&mut item)?;
            match item.trim() {
                "1" => {
                    if let Some(node) = self.node.read().as_ref() {
                        let mut contract_address = [0u8; 20];
                        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                        contract_address[..8].copy_from_slice(&timestamp.to_le_bytes());
                        let contract_code = vec![0x00, 0x01, 0x02, 0x03];
                        let mut contract_registry = node.contract_registry.lock();
                        match contract_registry.register_contract(contract_address, contract_code) {
                            Ok(_) => println!("Deployed contract at {}", hex::encode(contract_address)),
                            Err(err) => println!("Deploy failed: {}", err),
                        }
                    } else {
                        println!("Node offline");
                    }
                }
                "2" => break,
                _ => println!("Pilihan salah"),
            }
            println!("Tekan Enter untuk melanjutkan...");
            let mut _tmp = String::new();
            io::stdin().read_line(&mut _tmp)?;
        }
        Ok(())
    }

    async fn sdk_menu(&self, app: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            self.clear_screen();
            println!("--- SDK SIPUT ---");
            println!("1. Rust");
            println!("2. Golang");
            println!("3. Java");
            println!("4. Android");
            println!("5. Cara Menggunakan");
            println!("6. Exit");

            print!("Input: ");
            io::stdout().flush()?;

            let mut item = String::new();
            io::stdin().read_line(&mut item)?;
            let selection = item.trim();
            if ["1", "2", "3", "4"].contains(&selection) {
                let lang = match selection {
                    "1" => "Rust",
                    "2" => "Golang",
                    "3" => "Java",
                    "4" => "Android",
                    _ => "Unknown",
                };
                self.clear_screen();
                println!("--- SDK SIPUT ({}) ---", lang);

                if let Some(node) = self.node.read().as_ref() {
                    println!("[Engine] Node active");
                    println!("wallet balance: {}", node.get_balance(&app.wallet_address));
                    println!("current height: {}", node.get_current_height());
                    println!("peers: {}", node.get_connection_count());
                    println!("block count: {}", node.blockdag.read().get_all_blocks().len());
                } else {
                    println!("Node not active. Start node dulu agar SDK engine terhubung.");
                }

                println!("
Masuk ke terminal kosong (mode dev). Tekan Enter untuk kembali.");
                let mut _tmp = String::new();
                io::stdin().read_line(&mut _tmp)?;
                continue;
            }

            match selection {
                "5" => {
                    self.clear_screen();
                    println!("--- Cara Menggunakan SDK Siput ---");
                    println!("1. Buat wallet via Model internal (wallet module)");
                    println!("2. Buat & sign tx via Model internal (transaction module)");
                    println!("3. Deploy contract via ContractRegistry::deploy_contract");
                    println!("4. Gunakan Node Runtime / RPC untuk verify data");
                    println!("Tekan Enter untuk kembali...");
                    let mut _tmp = String::new();
                    io::stdin().read_line(&mut _tmp)?;
                }
                "6" => break,
                _ => {
                    println!("Pilihan tidak valid");
                    let mut _tmp = String::new();
                    io::stdin().read_line(&mut _tmp)?;
                }
            }
        }
        Ok(())
    }

    async fn network_menu(&self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            self.clear_screen();
            println!("--- Network Menu ---");
            println!("1. Status");
            println!("2. Back");

            print!("Input: ");
            io::stdout().flush()?;

            let mut item = String::new();
            io::stdin().read_line(&mut item)?;
            match item.trim() {
                "1" => {
                    if let Some(node) = self.node.read().as_ref() {
                        println!("Connected peers: {}", node.get_peer_count());
                        println!("Connection count: {}", node.get_connection_count());
                    } else {
                        println!("Node offline");
                    }
                }
                "2" => break,
                _ => println!("Pilihan salah"),
            }
            println!("Tekan Enter untuk melanjutkan...");
            let mut _tmp = String::new();
            io::stdin().read_line(&mut _tmp)?;
        }
        Ok(())
    }

    async fn explorer_menu(&self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            self.clear_screen();
            println!("--- Explorer Menu ---");
            println!("1. Info");
            println!("2. Back");

            print!("Input: ");
            io::stdout().flush()?;

            let mut item = String::new();
            io::stdin().read_line(&mut item)?;
            match item.trim() {
                "1" => println!("Explorer: dokumentasi / RPC endpoint."),
                "2" => break,
                _ => println!("Pilihan salah"),
            }
            println!("Tekan Enter untuk melanjutkan...");
            let mut _tmp = String::new();
            io::stdin().read_line(&mut _tmp)?;
        }
        Ok(())
    }

    async fn help_menu(&self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            self.clear_screen();
            println!("--- Help Menu ---");
            println!("1. Dokumentasi");
            println!("2. Back");

            print!("Input: ");
            io::stdout().flush()?;

            let mut item = String::new();
            io::stdin().read_line(&mut item)?;
            match item.trim() {
                "1" => println!("https://github.com/shaquel-it/siput"),
                "2" => break,
                _ => println!("Pilihan salah"),
            }
            println!("Tekan Enter untuk melanjutkan...");
            let mut _tmp = String::new();
            io::stdin().read_line(&mut _tmp)?;
        }
        Ok(())
    }

    async fn start_node(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.node.read().is_some() {
            return Ok(());
        }
        let node = Arc::new(NodeRuntime::new().await?);
        let node_clone = node.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = node_clone.run().await {
                eprintln!("Node runtime error: {}", e);
            }
        });
        *self.node_handle.lock() = Some(handle);
        *self.node.write() = Some(node);
        Ok(())
    }

    async fn start_mining(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(node) = self.node.read().as_ref() {
            node.start_mining().await?;
        }
        Ok(())
    }
}
