use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};

use task::{Task, TaskType};

fn main() {
    let (seed, starting_height, max_children) = get_args();

    eprintln!(
        "Using seed {}, starting height {}, max. children {}",
        seed, starting_height, max_children
    );

    let mut count_map = HashMap::new();
    let mut taskq = VecDeque::from(Task::generate_initial(seed, starting_height, max_children));

    let mut output: u64 = 0;

    let start = Instant::now();
    while let Some(next) = taskq.pop_front() {
        *count_map.entry(next.typ).or_insert(0usize) += 1;
        let result = next.execute();
        output ^= result.0;
        taskq.extend(result.1.into_iter());
    }
    let end = Instant::now();

    eprintln!("Completed in {} s", (end - start).as_secs_f64());

    println!(
        "{},{},{},{}",
        output,
        count_map.get(&TaskType::Hash).unwrap_or(&0),
        count_map.get(&TaskType::Derive).unwrap_or(&0),
        count_map.get(&TaskType::Random).unwrap_or(&0)
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
