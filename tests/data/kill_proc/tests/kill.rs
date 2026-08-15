#[test]
fn spawn_test_and_kill() {
    let _child: ChildWrapper = std::process::Command::new(env!("CARGO_BIN_EXE_kill_proc"))
        .spawn()
        .expect("kill_proc test binary should start")
        .into();

    run_test().expect("kill_proc should become healthy before the timeout");
}

fn run_test() -> Result<(), tokio::time::error::Elapsed> {
    tokio::runtime::Runtime::new()
        .expect("kill test Tokio runtime should start")
        .block_on(healthy_or_timeout())
}

async fn healthy_or_timeout() -> Result<(), tokio::time::error::Elapsed> {
    // The tracee's Tokio clock advances while ptrace stops it, so this deadline must allow for
    // time spent collecting coverage as well as time spent running the application.
    tokio::time::timeout(std::time::Duration::from_secs(30), wait_for_healthy()).await
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
    let url = "http://localhost:18080/hello"
        .parse()
        .expect("health check URL should be valid");
    let client = hyper::client::Client::new();
    client.get(url).await
}

struct ChildWrapper {
    child: std::process::Child,
}

impl ChildWrapper {
    fn new(child: std::process::Child) -> Self {
        Self { child }
    }
}

impl Drop for ChildWrapper {
    fn drop(&mut self) {
        let pid = self.child.id();
        let pid = nix::unistd::Pid::from_raw(
            pid.try_into()
                .expect("child process ID should fit in a platform pid_t"),
        );
        let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM);
    }
}

impl From<std::process::Child> for ChildWrapper {
    fn from(child: std::process::Child) -> Self {
        ChildWrapper::new(child)
    }
}
