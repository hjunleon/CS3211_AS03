#![allow(unused_imports)]
use core::num;
use std::{
    thread,
    collections::{HashMap, VecDeque},
    time::Instant, sync::mpsc::channel,
};

use std::sync::{
    atomic::{
        AtomicUsize,
        Ordering::Relaxed, 
        Ordering::SeqCst,
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

static INPUT_CNT:  AtomicUsize = AtomicUsize::new(0);
static OUTPUT_CNT: AtomicUsize = AtomicUsize::new(0);

// static ONE_HEIGHT_CNT: AtomicUsize = AtomicUsize::new(0);
// static FINAL_CNT: AtomicUsize = AtomicUsize::new(0);

static CPU_CNT: AtomicUsize = AtomicUsize::new(4);
// static CHAN_SIZE: AtomicUsize = AtomicUsize::new(4);
const Q_SIZE_MULTI: usize = 16;



fn main() {
    CPU_CNT.store(num_cpus::get(), Relaxed);
    println!("CPU_CNT: {}", CPU_CNT.load(Relaxed));
    
    let (seed, starting_height, max_children) = get_args();

    eprintln!(
        "Using seed {}, starting height {}, max. children {}",
        seed, starting_height, max_children
    );
    let main_cpu_cnt = CPU_CNT.load(Relaxed);

    let is_done_cond = Arc::new((Mutex::new(false), Condvar::new()));

    let pool = ThreadPool::new(main_cpu_cnt);
    // CHAN_SIZE.store(main_cpu_cnt * Q_SIZE_MULTI, Relaxed);
    let (tx, rx) = channel::unbounded::<Task>(); //channel::bounded::<Task>(CHAN_SIZE.load(Relaxed));

    // let mut count_map = HashMap::new(); // Dont need, split  into 3 usize variables so that tasks of different types wont wait on each other to update the count
    let mut taskq = VecDeque::from(Task::generate_initial(seed, starting_height, max_children));

    println!("taskq has {} tasks initially.", taskq.len());

    INPUT_CNT.fetch_add(taskq.len(), SeqCst);

    // let mut output: u64 = 0;

    // create a barrier that waits for all jobs plus the starter thread
    // let barrier = Arc::new(Barrier::new(n_jobs + 1));
    let start = Instant::now();
    // while let Some(next) = taskq.pop_front() {
    //     match next.typ {
    //         TaskType::Derive => DERIVE_COUNT.fetch_add(1, Relaxed),
    //         TaskType::Hash => HASH_COUNT.fetch_add(1, Relaxed),
    //         TaskType::Random => RAND_COUNT.fetch_add(1, Relaxed),
    //     };
    //     // *count_map.entry(next.typ).or_insert(0usize) += 1;
    //     let result = next.execute();
    //     println!("Result length: {}",result.1.len());
    //     if result.1.len() > 0 {
    //         println!("New height: {}",result.1[0].height);
    //     }
        
    //     // output ^= result.0;
    //     OUTPUT.fetch_xor(result.0, Relaxed);
    //     if result.1.len() == 0 {
    //         FINAL_CNT.fetch_add(1, Relaxed);
    //     }
    //     taskq.extend(result.1.into_iter());
    // }

    while let Some(init_next) = taskq.pop_front() {
        let is_done_cond2 = Arc::clone(&is_done_cond);
        let t_tx = tx.clone();
        let t_rx = rx.clone();
        pool.execute(move || {
            // let mut final_cnt = FINAL_CNT.load(Relaxed);
            // let mut one_cnt: usize = ONE_HEIGHT_CNT.load(Relaxed);
            // one_cnt == 0 || final_cnt < one_cnt
            while  INPUT_CNT.load(SeqCst) > OUTPUT_CNT.load(SeqCst) {
                let next = t_rx.recv().unwrap();
                println!("Thread: Received task at height {}", next.height);
                match next.typ {
                    TaskType::Derive => DERIVE_COUNT.fetch_add(1, Relaxed),
                    TaskType::Hash => HASH_COUNT.fetch_add(1, Relaxed),
                    TaskType::Random => RAND_COUNT.fetch_add(1, Relaxed),
                };
                let result = next.execute();
                OUTPUT.fetch_xor(result.0, Relaxed);
                INPUT_CNT.fetch_add(result.1.len(), SeqCst);
                OUTPUT_CNT.fetch_add(1, SeqCst);
                // if result.1.len() > 0 && result.1[0].height == 1 {
                //     // println!("Thread: I have reached height 1!");
                //     ONE_HEIGHT_CNT.fetch_add(result.1.len(), SeqCst);
                // }
                // if next.height == 0 {
                //     println!("Thread: I have reached height 0!");
                //     FINAL_CNT.fetch_add(1, SeqCst);
                // }
                // println!("Pushing {} new tasks ", result.1.len());
                for new_task in result.1.iter() {
                    // t_tx.se
                    t_tx.send(new_task.clone()).unwrap();
                }
                // one_cnt = ONE_HEIGHT_CNT.load(Relaxed);
                // final_cnt = FINAL_CNT.load(Relaxed);
                // println!("one_cnt: {one_cnt}");
                // println!("final_cnt: {final_cnt}");
                println!("Input vs out: {}, {}", INPUT_CNT.load(SeqCst), OUTPUT_CNT.load(SeqCst));
            }

            let (lock, cvar) = &*is_done_cond2;
            let mut is_done = lock.lock().unwrap();
            *is_done = true;
            cvar.notify_one();
        });
        tx.send(init_next.clone()).unwrap();
    }

    // while FINAL_CNT.load(Relaxed) < CPU_CNT.load(Relaxed) {

    // }
    let (lock, cvar) = &*is_done_cond;
    let mut is_done = lock.lock().unwrap();

    while !*is_done {
        is_done = cvar.wait(is_done).unwrap();
    }
    
    println!("FINAL Input vs out: {}, {}", INPUT_CNT.load(SeqCst), OUTPUT_CNT.load(SeqCst));

    let end = Instant::now();

    eprintln!("Completed in {} s", (end - start).as_secs_f64());

    println!(
        "{},{},{},{}",
        OUTPUT.load(Relaxed),
        HASH_COUNT.load(Relaxed),
        DERIVE_COUNT.load(Relaxed),
        RAND_COUNT.load(Relaxed)
    );

    // count_map.get(&TaskType::Hash).unwrap_or(&0),
    // count_map.get(&TaskType::Derive).unwrap_or(&0),
    // count_map.get(&TaskType::Random).unwrap_or(&0)
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
