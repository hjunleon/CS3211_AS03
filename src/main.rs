#![allow(unused_imports)]
use core::num;
use std::{
    thread,
    collections::{HashMap, VecDeque},
    time::{Instant, Duration}, sync::mpsc::channel,
};

use std::sync::{
    atomic::{
        AtomicUsize,
        Ordering::Relaxed, 
        Ordering::SeqCst,
        Ordering::Release,
        Ordering::Acquire,
        AtomicU64
    },
    Arc,
    Barrier,
    Condvar,
    Mutex
};

use crossbeam::{atomic, channel};
use task::{Task, TaskType};

use threadpool::ThreadPool;

// Goal: Use a decentralized approach where all worker threads are allowed to issue tasks to other threads and execute tasks assigned to them. ( spawn more rust hardware threads each time but be wary of memory usage and computation overhead )
// Or Use a shared task pool with no explicitly designated master thread. 

// Q: No tasks have to wait on other tasks right? Seemingly doesnt seem to be an issue

static HASH_COUNT: AtomicUsize = AtomicUsize::new(0);
static DERIVE_COUNT: AtomicUsize = AtomicUsize::new(0);
static RAND_COUNT: AtomicUsize = AtomicUsize::new(0);

static OUTPUT: AtomicU64 = AtomicU64::new(0);


fn task_handler(next: &Task) -> u64{
    println!("Thread: Received task at height {}", next.height);
    match next.typ {
        TaskType::Derive => DERIVE_COUNT.fetch_add(1, Relaxed),
        TaskType::Hash => HASH_COUNT.fetch_add(1, Relaxed),
        TaskType::Random => RAND_COUNT.fetch_add(1, Relaxed),
    };
    let mut result = next.execute();

    for new_task in result.1.iter() {
        result.0 ^= task_handler(new_task);
    }
    result.0
}
fn main() {
    
    
    let (seed, starting_height, max_children) = get_args();

    eprintln!(
        "Using seed {}, starting height {}, max. children {}",
        seed, starting_height, max_children
    );

    let main_cpu_cnt = num_cpus::get(); 
    println!("CPU_CNT: {}", main_cpu_cnt);

    let taskq = VecDeque::from(Task::generate_initial(seed, starting_height, max_children));

    let init_taskq_len = taskq.len();

    println!("taskq has {} tasks initially.", taskq.len());

    let taskq = Arc::new(Mutex::new(taskq));

    let start = Instant::now();

    let mut handles = Vec::new();
    for worker_id in 0..main_cpu_cnt{
        println!("Thread number  {worker_id}");
        let t_q = Arc::clone(&taskq);
        let handle = thread::spawn(move || {
            let t_q2 = t_q.lock().expect("Mutex poisoned");
            for i in (worker_id..init_taskq_len).step_by(main_cpu_cnt){
                let cur_task = &t_q2[i];
                let output = task_handler(cur_task);
                OUTPUT.fetch_xor(output, Relaxed);
            }
        });
        handles.push(handle);
    }


    for handle in handles {
        handle.join().unwrap();
    }
    

    let end = Instant::now();

    eprintln!("Completed in {} s", (end - start).as_secs_f64());

    println!(
        "{},{},{},{}",
        OUTPUT.load(Relaxed),
        HASH_COUNT.load(Relaxed),
        DERIVE_COUNT.load(Relaxed),
        RAND_COUNT.load(Relaxed)
    );
}


// There should be no need to modify anything below

fn get_args() -> (u64, usize, usize) {
    let mut args = std::env::args().skip(1);
    (
        args.next()
            .map(|a| a.parse().expect("invalid u64 for seed"))
            .unwrap_or_else(|| rand::Rng::gen(&mut rand::thread_rng())),
        args.next()
            .map(|a| a.parse().expect("invalid usize for starting_height"))
            .unwrap_or(5),
        args.next()
            .map(|a| a.parse().expect("invalid u64 for seed"))
            .unwrap_or(5),
    )
}

mod task;
