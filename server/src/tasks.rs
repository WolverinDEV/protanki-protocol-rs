use std::{task::{Waker, Poll, self}, pin::Pin, cell::RefCell};

use futures::{Future, FutureExt};
use tracing::trace;

use crate::client::{ClientComponent, Client};

trait RegisteredTask : Send {
    fn poll(&mut self, client: &mut Client, cx: &mut task::Context) -> Poll<()>;
}

struct RegisteredTaskImpl<T, F: Future<Output = T>, C: FnOnce(&mut Client, T) -> ()> {
    task: Pin<Box<F>>,
    callback: Option<C>,
}

impl<T, F: Future<Output = T> + Send, C: FnOnce(&mut Client, T) -> () + Send> RegisteredTask for RegisteredTaskImpl<T, F, C> {
    fn poll(&mut self, client: &mut Client, cx: &mut task::Context) -> Poll<()> {
        match self.task.as_mut().poll(cx) {
            Poll::Ready(value) => {
                if let Some(callback) = self.callback.take() {
                    (callback)(client, value);
                }
                Poll::Ready(())
            },
            Poll::Pending => Poll::Pending
        }
    }
}

pub struct Tasks {
    tasks: RefCell<Vec<Box<dyn RegisteredTask>>>,
    new_tasks: RefCell<Vec<Box<dyn RegisteredTask>>>,

    waker: RefCell<Option<Waker>>,
}

impl Tasks {
    pub fn new() -> Self {
        Self {
            tasks: RefCell::new(Vec::with_capacity(100)),
            new_tasks: RefCell::new(Vec::new()),

            waker: RefCell::new(None),
        }
    }

    pub fn enqueue<T: 'static, F: Future<Output = T> + Send + 'static, C: FnOnce(&mut Client, T) -> () + Send + 'static>(&self, task: F, callback: C) {
        let task = Box::new(RegisteredTaskImpl{
            task: Box::pin(task),
            callback: Some(callback),
        });

        match self.tasks.try_borrow_mut() {
            Ok(mut tasks) => {
                tasks.push(task);
            },
            Err(_) => {
                let mut tasks = self.new_tasks.borrow_mut();
                tasks.push(task);
            }
        }

        if let Some(waker) = self.waker.take() {
            waker.wake_by_ref();
        }
    }

    pub fn poll(&self, client: &mut crate::client::Client, cx: &mut std::task::Context) -> anyhow::Result<()> {
        *self.waker.borrow_mut() = Some(cx.waker().clone());
        
        loop {
            self.tasks.borrow_mut().retain_mut(|task| {
                if let Poll::Ready(_) = task.poll(client, cx) {
                    false
                } else {
                    true
                }
            });
            
            let mut new_tasks = self.new_tasks.borrow_mut();
            if new_tasks.is_empty() {
                return Ok(());
            }
            
            let new_tasks: Vec<_> = std::mem::replace(new_tasks.as_mut(), Default::default());
            let mut task_queue = self.tasks.borrow_mut();
            task_queue.extend(new_tasks.into_iter());
        }
    }
}