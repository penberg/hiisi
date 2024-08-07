use inferno::server;

fn main() {
    init_logger();

    let nr_threads = 1;

    let threads: Vec<_> = (0..nr_threads)
        .map(|_| {
            std::thread::spawn(|| {
                let mut rt = runtime();
                rt.block_on(async {
                    let addr = [0, 0, 0, 0];
                    let port = 8080;
                    log::info!("🔥 Inferno is listening to 0.0.0.0:{} ...", port);
                    if let Err(e) = server::serve(addr.into(), port).await {
                        log::error!("Error: {}", e);
                    }
                    log::info!("Http server stopped");
                });
            })
        })
        .collect();

    threads.into_iter().for_each(|t| {
        let _ = t.join();
    });
}

fn init_logger() {
    let env = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(env).init();
}

#[cfg(not(target_os = "linux"))]
fn runtime() -> monoio::Runtime<monoio::time::TimeDriver<monoio::LegacyDriver>> {
    monoio::RuntimeBuilder::<monoio::LegacyDriver>::new()
        .enable_timer()
        .build()
        .expect("Failed building the Runtime")
}

#[cfg(target_os = "linux")]
fn runtime() -> monoio::Runtime<monoio::time::TimeDriver<monoio::IoUringDriver>> {
    monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
        .enable_timer()
        .build()
        .expect("Failed building the Runtime")
}
