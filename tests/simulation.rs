#![feature(ptr_internals)]
use delay_timer::timer::timer_core::get_timestamp;
use delay_timer::{
    create_async_fn_body,
    delay_timer::DelayTimer,
    timer::{
        runtime_trace::task_handle::DelayTaskHandler,
        task::{Frequency, TaskBuilder},
    },
    utils::functions::{create_default_delay_task_handler, create_delay_task_handler},
};
use futures::future;
use smol::{channel, future as SmolFuture, LocalExecutor, Task, Timer};
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;
use std::thread::Thread;
use std::{
    ptr::Unique,
    sync::{
        atomic::{
            AtomicBool, AtomicUsize,
            Ordering::{AcqRel, Acquire, Release, SeqCst},
        },
        Arc,
    },
    thread::{current, park, park_timeout},
    time::{Duration, Instant},
};
use surf;

#[test]
fn go_works() {
    let mut delay_timer = DelayTimer::new();
    let mut task_builder = TaskBuilder::default();
    let share_num = Arc::new(AtomicUsize::new(0));
    let share_num_bunshin = share_num.clone();

    //每次 +1
    //第一次任务会在，1秒后执行， 之后每次在6秒后执行
    let body = move || {
        share_num_bunshin.fetch_add(1, Release);
        println!("task 1 ,1s run");
        create_default_delay_task_handler()
    };

    task_builder.set_frequency(Frequency::CountDown(3, "0/6 * * * * * *"));
    task_builder.set_task_id(1);
    let task = task_builder.spawn(body);
    delay_timer.add_task(task);

    let mut i = 0;

    loop {
        i = i + 1;
        park_timeout(Duration::from_secs(5));

        //检测，任务是否执行的符合预期
        assert_eq!(i, share_num.load(Acquire));

        if i == 3 {
            break;
        }
    }
}

#[test]
fn tests_countdown() {
    let mut delay_timer = DelayTimer::new();
    let mut task_builder = TaskBuilder::default();
    let share_num = Arc::new(AtomicUsize::new(3));
    let share_num_bunshin = share_num.clone();
    let body = move || {
        share_num_bunshin.fetch_sub(1, Release);
        println!("task 1 ,1s run");
        create_default_delay_task_handler()
    };

    task_builder.set_frequency(Frequency::CountDown(3, "* * * * * * *"));
    task_builder.set_task_id(1);
    let task = task_builder.spawn(body);
    delay_timer.add_task(task);

    let mut i = 0;

    loop {
        i = i + 1;
        park_timeout(Duration::from_secs(1));

        if i == 6 {
            //task 一共运行3次，每秒运行一次，6秒后从最多减到0
            assert_eq!(0, share_num.load(Acquire));
            break;
        }
    }
}

#[test]
fn demo_it() {
    //TODO:Remember close terminal can speed up because of
    //printnl! block process if stand-pipe if full.

    let mut delay_timer = DelayTimer::new();
    let mut task_builder = TaskBuilder::default();
    let mut run_flag = Arc::new(AtomicUsize::new(0));
    let run_flag_ref: Option<Unique<Arc<AtomicUsize>>> = Unique::new(&mut run_flag);

    let thread = current();

    let body = move || {
        println!("running....");
        let local_run_flag = run_flag_ref.unwrap().as_ptr();

        unsafe {
            (*local_run_flag).fetch_add(1, SeqCst);
        }
        create_default_delay_task_handler()
    };
    let end_body = move || {
        let local_run_flag = run_flag_ref.unwrap().as_ptr();
        unsafe {
            println!(
                "end time {}, result {}",
                get_timestamp(),
                (*local_run_flag).load(SeqCst)
            );
        }
        thread.unpark();
        create_default_delay_task_handler()
    };

    let async_body = create_async_fn_body!({
        let mut res = surf::get("https://httpbin.org/get").await.unwrap();
        let body_str = res.body_string().await.unwrap();
        println!("{}", body_str);
        Ok(())
    });

    task_builder.set_frequency(Frequency::CountDown(1, "30 * * * * * *"));
    task_builder.set_maximum_running_time(90);

    println!("start time {}", get_timestamp());
    for i in 0..10000 {
        task_builder.set_task_id(i);

        let task = task_builder.spawn(body);
        delay_timer.add_task(task);
    }

    task_builder.set_frequency(Frequency::CountDown(1, "59 * * * * * *"));
    for i in 10000..13000 {
        task_builder.set_task_id(i);

        let task = task_builder.spawn(async_body);
        delay_timer.add_task(task);
    }

    task_builder.set_task_id(88888);
    task_builder.set_frequency(Frequency::CountDown(1, "* 2 * * * * *"));
    let task = task_builder.spawn(end_body);
    delay_timer.add_task(task);

    park();
}