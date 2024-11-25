//! bot.rs
//! 
//! タスクを実行する bot とそのタスクのインターフェースを定義する
//! 
//! ## example
//! 
//! ```ignore
//! /// 10 秒間待機するタスクを定義する
//! #[derive(Clone)]
//! pub struct SampleTask {}
//! impl BotTask for SampleTask {
//!     type Output = (),
//!     type Error = Box<dyn std::error::Error + Send + Sync>
//!     type Future = Pin<Box<dyn Future<Output=Result<Self::Output,Self::Error>> + Send>>
//!     fn call(&self) -> Self::Future {
//!         async {
//!             tokio::time::sleep(Duration::from_secs(10)).await;
//!             Ok(())
//!         }
//!     }
//!     // タスクを強制終了した際に実行するコールバックを登録する
//!     // デフォルトでは何もしないため、必要に応じて上書きする
//!     fn on_abort(&mut self) {
//!         println!("on abort");
//!     }
//! }
//! 
//! // Bot::new で実行するタスクを注入する
//! let bot = Bot::new(SampleTask {});
//! 
//! // タスクを実行する
//! let future = bot.start();
//! let result = future.await;
//! ```
use std::future::Future;

use anyhow::bail;
#[allow(unused_imports)]
pub use tokio::task::{AbortHandle, JoinHandle};

#[allow(dead_code)]         
pub struct Bot<T> {
    pub(crate) inner: T,
    pub(crate) abort: Option<AbortHandle>,
}

impl<T> Bot<T> {
    #[allow(dead_code)]
    pub fn new(inner: T) -> Self {        
        Self { inner,abort: None }
    }
}

#[allow(dead_code)]
impl<T> Bot<T> 
where
    T: BotTask,
    T::Output: Send + 'static,
    T::Error:  Send + 'static,
    T::Future: Send +  'static,
{
    pub fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(abort) = &self.abort {
            abort.abort();
            self.inner.on_abort();
            return Ok(());
        };
        // TODO: 専用のエラー型を用意するかもしれない
        bail!("BOT not started yet")
        
    }
    pub fn start(
        &mut self,
    ) -> JoinHandle<Result<<T as BotTask>::Output, <T as BotTask>::Error>>

    {
        
        let fut = self.inner.call();
        tokio::spawn(async move { fut.await })  
    }
}

/// bot が実行するタスクのインターフェース
pub trait BotTask: Clone{
    type Output;
    type Error;
    type Future: Future<Output = Result<Self::Output, Self::Error>>;


    // provided function
    // Task を強制終了する際に実行する処理
    // デフォルトでは何も行わないため、
    // なにか処理を行いたい場合は abort メソッドを上書きする
    fn on_abort(&mut self){}

    // required function
    // Task を future に変換する
    fn call(&self) -> Self::Future;

}

#[cfg(test)]
mod tests {
    use std::{pin::Pin, time::Duration};

    use super::*;
    use std::sync::Arc;
    use tokio::{select, sync::Mutex};

    #[async_trait::async_trait]
    trait StateBotExt {
        async fn state(&self) -> State;
    } 
    #[async_trait::async_trait]
    impl StateBotExt for Bot<TestTask> {
        async fn state(&self) -> State {
            let state = self.inner.state.lock().await;
            state.clone()
        }
    }
    #[derive(Clone,Debug,PartialEq)]
    enum State {
        NotStart,
        Finish,
        Abort,
    }

    #[derive(Clone)]
    struct TestTask {
        sleep: Duration,
        state: Arc<tokio::sync::Mutex<State>>,
    }
    struct NoError;
    impl BotTask for TestTask {
        type Error = NoError;
        type Output = ();
        type Future = Pin<Box<dyn Future<Output = Result<Self::Output,Self::Error>> + Send>>;

        fn on_abort(&mut self) {
            let mut state = self.state.blocking_lock();
            *state = State::Abort
        }
        fn call(&self) -> Self::Future {
            let sleep = tokio::time::sleep(self.sleep);
            let state = Arc::clone(&self.state);
            Box::pin(async move{
                println!("start task");
                sleep.await;

                let mut state = state.lock().await;
                *state = State::Finish;
                Ok(())
            })
        }
    }
    #[tokio::test]
    async fn test_bot() -> anyhow::Result<()> {
        let task = TestTask {
            sleep: Duration::from_secs(1),
            state: Arc::new(Mutex::new(State::NotStart))
        };

        let not_start_task = task.clone();
        let bot = Bot::new(not_start_task);
        assert_eq!(bot.state().await,State::NotStart);

        let finished_task = task.clone();
        let mut bot = Bot::new(finished_task);
        let timeout = Box::pin(tokio::time::sleep(Duration::from_secs(10)));
        select! {
            _ = timeout => {
                println!("timeout");
                let _ = bot.stop();

            }
            _ = bot.start() => {
                println!("finished")
            }
        }; 
        assert_eq!(bot.state().await,State::Finish);

        let abort_task = task.clone();
        let mut bot = Bot::new(abort_task);
        let timeout = Box::pin(tokio::time::sleep(Duration::from_millis(100)));
        select! {
            _ = timeout => {
                println!("timeout");
                let _ = bot.stop();

            }
            _ = bot.start() => {
                println!("finished")
            }
        }; 
        assert_eq!(bot.state().await,State::Finish);

        Ok(())
    }

}
