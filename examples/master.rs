use nnxt_master::MasterServer;
use tracing::info;
use nnxt_utils::setup_log;

const MASTER_ADDR: &str = "ipc:///tmp/nnxt/master";
const HEARTBEAT_TIMEOUT_NS: u64 = 5_000_000_000;

fn main() {
    let _ = setup_log();
    std::fs::create_dir_all("/tmp/nnxt").expect("create ipc dir");
    info!("starting master addr=[{}]", MASTER_ADDR);

    let mut server = MasterServer::new(MASTER_ADDR, HEARTBEAT_TIMEOUT_NS)
        .expect("master server create failed");
    server.run().expect("master run failed");
}
