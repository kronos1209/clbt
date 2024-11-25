# usage

想定した使い方

- 管理構造体に追加した bot を起動して、すべての output が準備完了になる、もしくはタイムアウトによって強制終了されたものが返却される

```rust

enum RunResult {
    Finish(Vec<Bot::Output>),
    Timeout,
}

let bot_manger = BotManager::new()
    .spawn_rand(/* bot 生成を行う間隔を指定する trait obj */)
    .with_timeout(Duration::from_secs(30));

bot_manager
    .init_fn(/* some initialize function */)
    .add_bot(Bot::new(/* some config */))
    .add_bot(Bot::new(/* some config */));

let fut: Box<dyn Future<Output = RunResult> + Sync + Send> = bot_manager.run();

let results = fut.await;
```

- 終了した bot があれば、即座に結果を返す

```rust 
let bot_manger = BotManager::new()
    .spawn_rand(/* bot 生成を行う間隔を指定する trait obj */)
    .with_timeout(Duration::from_secs(30));

bot_manager
    .init_fn(/* some initialize function */)
    .add_bot(Bot::new(/* some config */))
    .add_bot(Bot::new(/* some config */));

let mut stream: Box<dyn Stream<Item = RunResult> + Sync + Send> = bot_manager.run_stream();

while let Some(result) = stream.next().awwait {
    /* result handling */
};
```