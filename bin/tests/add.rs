use mees::Requestable;

#[tokio::test]
pub async fn add() {
    mees::requests! {
        Add (i32, i32) -> i32
    };

    let mut responder = mees::Responder::new();
    responder.register(Add::handler(|add| async move { add.0 + add.1 }));
    tokio::spawn(async move {
        responder.run("localhost:6454").await.unwrap();
    });

    tokio::spawn(async move {
        mees_bin::run().await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let client = mees::Client::new("localhost:6454").await.unwrap();
    let res = Add(1, 2).request(&client).await.unwrap();
    assert_eq!(res, 3);
}
