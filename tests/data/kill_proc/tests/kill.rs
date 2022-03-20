#[test]
fn spawn_test_and_kill() {
    let _child: ChildWrapper = test_bin::get_test_bin("kill_proc")
    .spawn()
    .unwrap()
    .into();
    
    run_test().unwrap();
}


fn run_test() -> Result<(), tokio::time::error::Elapsed> {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(healthy_or_timeout())
}

async fn healthy_or_timeout() -> Result<(), tokio::time::error::Elapsed> {
    tokio::time::timeout(std::time::Duration::from_secs(5), wait_for_healthy()).await
}

async fn wait_for_healthy() {
    loop {
        if let Ok(response) = http_call().await {
            if response.status() == http::StatusCode::OK {
                return;
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

async fn http_call() -> Result<hyper::Response<hyper::Body>, hyper::Error> {
    let url = "http://localhost:18080/hello".parse().unwrap();
    let client = hyper::client::Client::new();
    client.get(url).await
}

struct ChildWrapper {
    child: std::process::Child,
}

impl ChildWrapper {
    fn new(child: std::process::Child) -> Self {
        Self {
            child,
        }
    }
}

impl Drop for ChildWrapper {
    fn drop(&mut self) {
        let pid = self.child.id();
        let pid = nix::unistd::Pid::from_raw(pid.try_into().unwrap());
        let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM);
    }
}

impl From<std::process::Child> for ChildWrapper {
    fn from(child: std::process::Child) -> Self {
        ChildWrapper::new(child)
    }
}
