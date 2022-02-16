use warp::Filter;

#[tokio::main]
async fn main() {
    let hello = warp::path("hello")
        .and(warp::get())
        .map(|| String::from("Hello"));

    warp::serve(hello)
        .run(([127, 0, 0, 1], 18080))
        .await;
}
