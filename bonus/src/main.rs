use std::{
    thread,
    time::Instant
};

use std::sync::{
    atomic::{
        AtomicUsize,
        Ordering::Relaxed, 
        AtomicU64
    },
    Arc,
    RwLock
};

use task::{Task, TaskType};

// Goal: Use a decentralized approach where all worker threads are allowed to issue tasks to other threads and execute tasks assigned to them. ( spawn more rust hardware threads each time but be wary of memory usage and computation overhead )
// Or Use a shared task pool with no explicitly designated master thread. 

// Q: No tasks have to wait on other tasks right? Seemingly doesnt seem to be an issue

static HASH_COUNT: AtomicUsize = AtomicUsize::new(0);
static DERIVE_COUNT: AtomicUsize = AtomicUsize::new(0);
static RAND_COUNT: AtomicUsize = AtomicUsize::new(0);

static OUTPUT: AtomicU64 = AtomicU64::new(0);

static THREAD_STACK_SIZE: usize = 4 * 1024 * 1024 * 1024;


fn task_handler(next: &Task) -> u64{
    // println!("Thread: Received task at height {}", next.height);
    match next.typ {
        TaskType::Derive => DERIVE_COUNT.fetch_add(1, Relaxed),
        TaskType::Hash => HASH_COUNT.fetch_add(1, Relaxed),
        TaskType::Random => RAND_COUNT.fetch_add(1, Relaxed),
    };
    let mut result = next.execute();
    drop(next);
    for new_task in result.1 {
        result.0 ^= task_handler(&new_task);
        drop(new_task);
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
    // println!("CPU_CNT: {}", main_cpu_cnt);

    let taskq: Vec<Task> = Task::generate_initial(seed, starting_height, max_children).collect();

    // let taskq:  Vec<Task>  = VecDeque::from();
// VecDeque::from(
    let init_taskq_len = taskq.len();

    // println!("taskq has {} tasks initially.", taskq.len());

    let taskq = Arc::new(RwLock::new(taskq));

    let start = Instant::now();

    let mut handles = Vec::new();
    for worker_id in 0..main_cpu_cnt{
        // println!("Thread number  {worker_id}");
        let t_q = Arc::clone(&taskq);
        let handle = thread::Builder::new().stack_size(THREAD_STACK_SIZE).spawn(move || {
            // println!("Hello from thread {worker_id}");
            let t_q2 = t_q.read().expect("Mutex poisoned");
            for i in (worker_id..init_taskq_len).step_by(main_cpu_cnt){
                // println!("Thread {worker_id} processing initial task {i}");
                let cur_task = &t_q2[i];
                let output = task_handler(cur_task);
                // println!("Returned to top level!");
                OUTPUT.fetch_xor(output, Relaxed);
            }
        }).unwrap();
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
