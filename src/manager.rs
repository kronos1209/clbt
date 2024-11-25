use core::panic;
use std::{pin::Pin, time::Duration};

use futures::{future, stream, Stream, StreamExt as _};

use crate::bot::{Bot, BotTask};

enum RunResult<R> {
    Finish(R),
    Timeout,
    Other(Box<dyn std::error::Error + Send + Sync>)
}
trait Manager<T> 
where 
    T: BotTask
{
    fn init_fn<F>(&mut self,f: F) 
    where 
        F: FnOnce() + 'static;
    fn bot_num(&mut self,num: usize);
    fn timeout(&mut self,timeout: Duration);
    fn run(&mut self) -> Pin<Box<dyn Stream<Item = RunResult<Result<<T as BotTask>::Output,<T as BotTask>::Error>>>>>;
    fn stop(&mut self) -> anyhow::Result<()>;
}

pub struct  BotManager<T> {
    bots: Vec<Bot<T>>,
    init_fn: Option<Box<dyn FnOnce()>>,
    num: usize,
    task: T,
    timeout: Option<Duration>,
}
impl<T> BotManager<T> {
    pub fn new(task:  T) -> Self{
        Self {
            task: task,
            init_fn: None,
            bots: vec![],
            num: 0,
            timeout: None
        }
    }
}
impl<T> Manager<T> for BotManager<T> 
where 
    T: BotTask,
    T::Output: Send + 'static,
    T::Error:  Send + 'static,
    T::Future: Send +  'static,
{
    fn init_fn<F>(&mut self,f: F) 
        where 
            F: FnOnce() + 'static
    {
        self.init_fn = Some(Box::new(f))    
    }
    fn bot_num(&mut self,num: usize) {
        self.num = num
    }
    fn timeout(&mut self,timeout: Duration) {
        if let Some(_) = &self.timeout {
            panic!("aleady set timeout")
        }
        self.timeout = Some(timeout)
    }
    fn run(&mut self) -> Pin<Box<dyn Stream<Item = RunResult<Result<<T as BotTask>::Output,<T as BotTask>::Error>>>>>{
        let mut bots: Vec<Bot<T>> = (0..self.num).map(|_| {
            Bot::new(self.task.clone())
        }).collect();


        let (tx,mut rx) = tokio::sync::mpsc::channel(1024);
        
        // tokio::spawn() でバックグランド上でタスクを実行する
        // 実行した結果はチャネル経由でストリームされる
        bots.iter_mut().for_each(|bot|  {
            let fut = bot.start();
            let tx_clone = tx.clone();
            tokio::spawn(async  move {
                let res = match fut.await {
                    Ok(result) => {
                        RunResult::Finish(result)
                    }
                    Err(err) => RunResult::Other(Box::new(err))
                 };
                tx_clone.send(res).await
            });
        });
        self.bots = bots;

        Box::pin(async_stream::stream! {while let Some(result) = rx.recv().await {
            yield result
        }})
            
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        self.bots.iter_mut().for_each(|bot| {
            let _ = bot.stop();
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{future::Future, pin::Pin, sync::Arc};

    use tokio::sync::Mutex;

    use super::*;
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

            println!("finished task");
                Ok(())
            })
        }
    }
    #[tokio::test]
    async fn test_bot_manager_with_successful() {
        let mut manager = BotManager::new(TestTask {
            sleep: Duration::from_millis(100),
            state: Arc::new(Mutex::new(State::NotStart))
        });
        manager.bot_num(10);

        let mut stream = manager.run();

        while let Some(result) = stream.next().await {
            match result {
                RunResult::Other( ..) => assert!(false,"happen something wrong"),
                _ => {} 
            }
        }

        for bot in manager.bots.iter() {
            assert_eq!(bot.state().await,State::Finish)
        }
    }
    async fn test_bot_manager_with_timeout() {
    }
}