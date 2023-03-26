use mees::Requestable;
use serde::{Deserialize, Serialize};

#[tokio::test]
async fn test_parse() {
    #[derive(Serialize, Deserialize)]
    struct FooResponse {
        baz: String,
    }
    mees::requests! {
        Foo { bar: String } -> FooResponse
        /// Add two numbers
        Add (i32, i32) -> i32
    };

    let mut responder = mees::Responder::new();
    responder.register(Foo::handler(|_| async move {
        FooResponse {
            baz: "world".to_string(),
        }
    }));
    responder.register(Add::handler(|add| async move { add.0 + add.1 }));

    let res = Add(1, 2).handle_local(&responder).await.unwrap();
    println!("res: {:?}", res);
    println!("path: {:?}", Add::path());
}
