fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use httptest::{matchers::*, responders::*, Expectation, Server};
    use reqwest::Error as ReqWestError;
    use std::time::Duration;
    // Builds a reqwest blocking client
    fn build_client() -> Result<reqwest::blocking::Client, ReqWestError> {
        const TIMEOUT_IN_SECS: u64 = 5;
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_IN_SECS))
            .build()
    }

    #[test]
    fn test_httptest() {
        let server = Server::run();
        // Expect request of given method on specified url
        let request_path = request::method_path("GET", "/foo");
        let expectation = Expectation::matching(all_of![request_path]);
        server.expect(expectation.respond_with(status_code(200)));

        let url = server.url("/foo");
        let port = url.port_u16().unwrap();
        let host = url.host().unwrap().to_string();
        let uri = format!("http://{}:{}/{}", host, port, "foo");

        let client = build_client().unwrap();
        let req_builder = client.get(&uri);
        let response = req_builder.send().unwrap();
        assert!(response.status().is_success());
    }
}
