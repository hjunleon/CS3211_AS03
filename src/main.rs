use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
    sync::{Arc, Mutex},
};

use task::{Task, TaskType};

#[tokio::main]
async fn main() {
    let (seed, starting_height, max_children) = get_args();

    eprintln!(
        "Using seed {}, starting height {}, max. children {}",
        seed, starting_height, max_children
    );

    let count_map = Arc::new(Mutex::new(HashMap::new())); 
    let mut taskq = VecDeque::from(Task::generate_initial(seed, starting_height, max_children));

    let output: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));

    let start = Instant::now();

    while !taskq.is_empty() {
        let mut results: Vec<tokio::task::JoinHandle<Vec<Task>>> = Vec::new();
        for t in taskq {
            results.push(tokio::task::spawn(run_task(t, output.clone(), count_map.clone())));
        }

        taskq = VecDeque::new();
        for res in results {
            taskq.extend(res.await.unwrap().into_iter());
        }
    }
    let end = Instant::now();

    eprintln!("Completed in {} s", (end - start).as_secs_f64());

    let o1 = output.lock().unwrap();
    let o2 = *count_map.lock().unwrap().get(&TaskType::Hash).unwrap_or(&0);
    let o3 = *count_map.lock().unwrap().get(&TaskType::Derive).unwrap_or(&0);
    let o4 = *count_map.lock().unwrap().get(&TaskType::Random).unwrap_or(&0);

    println!(
        "{},{},{},{}",
        o1, o2, o3, o4
    );
}

async fn run_task(task: Task, 
                    output: Arc<Mutex<u64>>,
                    count_map: Arc<Mutex<HashMap<TaskType, usize>>>) -> Vec<Task> {
    {
        *count_map.lock().unwrap().entry(task.typ).or_insert(0usize) += 1;
    }
    let result = task.execute();
    {
        *output.lock().unwrap() ^= result.0;
    }
    result.1
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
