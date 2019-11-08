#![no_std]
extern crate alloc;
use {
    alloc::{boxed::Box, sync::Arc},
    core::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    },
    spin::Mutex,
    woke::{waker_ref, Woke},
};

use smallvec::*;

#[macro_use]
extern crate lazy_static;

// our executor just holds one task
pub struct Executor {
    tasks: SmallVec<[Arc<Task>;1024]>,
}

// Our task holds onto a future the executor can poll
struct Task {
    pub future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
}

// specify how we want our tasks to wake up
impl Woke for Task {
    fn wake_by_ref(_: &Arc<Self>) {
        // run the executor again because something finished!
        Executor::run()
    }
}


impl Executor {
    pub fn spawn(future: impl Future<Output = ()> + 'static + Send) {
        // store our task in global state
        let task = Arc::new(Task {
            future: Mutex::new(Some(Box::pin(future))),
        });
        let mut e = get_executor().lock();
        let mut v = SmallVec::new();
        v.push(task);
        e.tasks = v;

        // we drop this early because otherwise run() will cause a mutex lock
        core::mem::drop(e);

        // get things going!
        Executor::run();
    }
    fn run() {
        // get our task from global state
        let mut e = get_executor().lock();
        let count = e.tasks.len();
        for _ in 0..count {
            let task = e.tasks.remove(0);
            let mut is_pending = false;
            {
                let mut future_slot = task.future.lock();
                if let Some(mut future) = future_slot.take() {
                    // make a waker for our task
                    let waker = waker_ref(&task);
                    // poll our future and give it a waker
                    let context = &mut Context::from_waker(&*waker);
                    if let Poll::Pending = future.as_mut().poll(context) {
                        *future_slot = Some(future);
                        is_pending = true;
                    }
                }
            }
            if is_pending {
                e.tasks.push(task);
            }
        }
    }
}



lazy_static! {
    static ref INSTANCE: Mutex<Executor> = {
        Mutex::new(Executor {
            tasks: SmallVec::new(),
        })
    };
}

// get a global holder of our one task
fn get_executor() -> &'static Mutex<Executor> {
    &INSTANCE
}
