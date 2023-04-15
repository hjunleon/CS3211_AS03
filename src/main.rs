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

static CUR_HEIGHT: AtomicUsize = AtomicUsize::new(0);

static CUR_HEIGHT_CNT: AtomicUsize = AtomicUsize::new(0);
static NEXT_HEIGHT_CNT: AtomicUsize = AtomicUsize::new(0);

static INPUT_CNT:  AtomicUsize = AtomicUsize::new(0);
static OUTPUT_CNT: AtomicUsize = AtomicUsize::new(0);

static FINAL_H_CNT: AtomicUsize = AtomicUsize::new(0);

static CPU_CNT: AtomicUsize = AtomicUsize::new(4);


fn main() {
    CPU_CNT.store(num_cpus::get(), Relaxed);
    println!("CPU_CNT: {}", CPU_CNT.load(Relaxed));
    
    let (seed, starting_height, max_children) = get_args();

    eprintln!(
        "Using seed {}, starting height {}, max. children {}",
        seed, starting_height, max_children
    );

    CUR_HEIGHT.store(starting_height, SeqCst);

    let main_cpu_cnt = CPU_CNT.load(Relaxed);

    let is_done_cond = Arc::new((Mutex::new(0), Condvar::new()));

    let pool = ThreadPool::new(main_cpu_cnt);
    // CHAN_SIZE.store(main_cpu_cnt * Q_SIZE_MULTI, Relaxed);
    let (tx1, rx1) = channel::unbounded::<Task>(); //channel::bounded::<Task>(CHAN_SIZE.load(Relaxed));

    let (tx2, rx2) = channel::unbounded::<Task>();

    // let mut count_map = HashMap::new(); // Dont need, split  into 3 usize variables so that tasks of different types wont wait on each other to update the count
    let mut taskq = VecDeque::from(Task::generate_initial(seed, starting_height, max_children));

    println!("taskq has {} tasks initially.", taskq.len());

    INPUT_CNT.fetch_add(taskq.len(), SeqCst);
    CUR_HEIGHT_CNT.fetch_add(taskq.len(), SeqCst);
    let start = Instant::now();
    while let Some(init_next) = taskq.pop_front() {
        match starting_height % 2 {
            0 => tx1.send(init_next.clone()).unwrap(),
            1 => tx2.send(init_next.clone()).unwrap(),
            _ => todo!()
        }
    }

    for _ in 0..main_cpu_cnt{
        let is_done_cond2 = Arc::clone(&is_done_cond);
        let (t_tx1, t_rx1) = (tx1.clone(), rx1.clone());
        let (t_tx2, t_rx2) = (tx2.clone(), rx2.clone());
        pool.execute(move || {
            while INPUT_CNT.load(SeqCst) > OUTPUT_CNT.load(SeqCst) {
                let cur_height = CUR_HEIGHT.load(SeqCst);
                // Consume all current height tasks
                println!("Thread: Consume all current height tasks {}", cur_height);
                let (cur_tx, cur_rx) = match cur_height % 2 {
                    0 => (&t_tx2, &t_rx1),
                    1 => (&t_tx1, &t_rx2),
                    _ => todo!()
                };
                while !cur_rx.is_empty(){
                    let next = cur_rx.recv().unwrap();
                    // println!("Thread: Received task at height {}", next.height);
                    match next.typ {
                        TaskType::Derive => DERIVE_COUNT.fetch_add(1, Relaxed),
                        TaskType::Hash => HASH_COUNT.fetch_add(1, Relaxed),
                        TaskType::Random => RAND_COUNT.fetch_add(1, Relaxed),
                    };
                    let result = next.execute();
                    OUTPUT.fetch_xor(result.0, Relaxed);
                    INPUT_CNT.fetch_add(result.1.len(), SeqCst);
                    NEXT_HEIGHT_CNT.fetch_add(result.1.len(), SeqCst);
                    OUTPUT_CNT.fetch_add(1, SeqCst);

                    // Fill with all next height tasks
                    for new_task in result.1.iter() {
                        cur_tx.send(new_task.clone()).unwrap();
                    }
                    CUR_HEIGHT_CNT.fetch_sub(1, SeqCst);
                }
                while CUR_HEIGHT_CNT.load(SeqCst) > 0 {}
                // println!("Finished all on this level");
                if cur_height == 0 {
                    println!("Finished all levels!");
                    println!("Unlocking the exit");
                    let (lock, cvar) = &*is_done_cond2;
                    let mut is_done = lock.lock().unwrap();
                    *is_done += 1; // true
                    cvar.notify_one();
                    FINAL_H_CNT.fetch_add(1, SeqCst);
                    return;
                    // break;
                }
                match CUR_HEIGHT.compare_exchange(cur_height, cur_height - 1, SeqCst, SeqCst){
                    Ok(_) => {}
                    Err(_) => {}
                };
                let next_height = NEXT_HEIGHT_CNT.load(SeqCst);
                match CUR_HEIGHT_CNT.compare_exchange(0, NEXT_HEIGHT_CNT.load(SeqCst), SeqCst, SeqCst){
                    Ok(_) => {},
                    Err(_) => {}
                };

                match NEXT_HEIGHT_CNT.compare_exchange(next_height, 0, SeqCst, SeqCst){
                    Ok(_) => {},
                    Err(_) => {}
                };

                // println!("Input vs out: {}, {}", INPUT_CNT.load(SeqCst), OUTPUT_CNT.load(SeqCst));
                // println!("NEXT_HEIGHT_CNT vs CUR_HEIGHT_CNT: {}, {}", NEXT_HEIGHT_CNT.load(SeqCst), CUR_HEIGHT_CNT.load(SeqCst));
            }
        });
    }

    // let (lock, cvar) = &*is_done_cond;
    // let mut is_done = lock.lock().unwrap();

    
    // while *is_done < main_cpu_cnt {  
    //     is_done = cvar.wait(is_done).unwrap();
    //     println!("Isdone is now {is_done}");
    // }

    while FINAL_H_CNT.load(SeqCst) < main_cpu_cnt {
        println!("FINAL_H_CNT: {}", FINAL_H_CNT.load(SeqCst));
        thread::sleep(Duration::from_secs(2));
    };
    
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
