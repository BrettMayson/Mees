use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};

use mees::Client;
use mees::Requestable;

mees::requests! {
    Add (i32, i32) -> i32
}

// Here we have an async function to benchmark
async fn do_something(client: &Client) {
    Add(1, 2).request(client).await.unwrap();
}

fn setup(c: &mut Criterion) {
    let tokio = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // Set up the server
    tokio.spawn(async move {
        mees_bin::run("localhost:6454".parse().expect("invalid address")).await;
    });

    // Set up the responder
    tokio.spawn(async move {
        let mut responder = mees::Responder::new();
        responder.register(Add::handler(|add| async move { add.0 + add.1 }));
        responder
            .run("localhost:6454".parse().expect("invalid address"))
            .await
            .unwrap();
    });

    {
        let _guard = tokio.enter();
        tokio.block_on(tokio::time::sleep(std::time::Duration::from_millis(500)));
    }

    let client = tokio
        .block_on(mees::Client::new(
            "localhost:6454".parse().expect("invalid address"),
        ))
        .unwrap();
    c.bench_with_input(
        BenchmarkId::new("send_add", &client),
        &client,
        |b, client| {
            b.to_async(&tokio).iter(|| do_something(client));
        },
    );
}

criterion_group!(benches, setup);
criterion_main!(benches);
