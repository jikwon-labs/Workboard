// A few threads for work that waits on the outside world: outgoing
// webhooks and slash commands (curl, up to 10 s each), iPhone pushes
// (APNs, up to 20 s) and reset mails. One process-wide set of WORKERS
// threads serves every workspace, so a busy channel or a slow address
// can't make the server start a thread per message.
//
// A job goes to the thread its key picks, so jobs with the same key (one
// webhook, one device) run one after another in the order they came;
// jobs with different keys usually run side by side.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;

/// How many threads run jobs.
pub(crate) const WORKERS: usize = 8;

type Job = Box<dyn FnOnce() + Send>;

static QUEUES: OnceLock<Vec<Sender<Job>>> = OnceLock::new();

fn queues() -> &'static [Sender<Job>] {
    QUEUES.get_or_init(|| {
        (0..WORKERS)
            .map(|i| {
                let (tx, rx) = channel::<Job>();
                std::thread::Builder::new()
                    .name(format!("wb-jobs-{}", i))
                    .spawn(move || {
                        for job in rx {
                            // a job that panics (it says so on stderr) doesn't
                            // take the jobs behind it down with it
                            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                        }
                    })
                    .expect("couldn't start a job thread");
                tx
            })
            .collect()
    })
}

/// Run `job` on a job thread, after the jobs with the same `key` sent before it.
pub(crate) fn run(key: &str, job: impl FnOnce() + Send + 'static) {
    let mut h = DefaultHasher::new();
    key.hash(&mut h);
    let q = &queues()[(h.finish() % WORKERS as u64) as usize];
    // the threads never stop, so the queue is always there
    let _ = q.send(Box::new(job));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn jobs_with_one_key_run_in_order() {
        let seen = Arc::new(Mutex::new(vec![]));
        let (done, wait) = mpsc::channel();
        for i in 0..200 {
            let (seen, done) = (seen.clone(), done.clone());
            run("jobs-test:one-key", move || {
                if i % 50 == 0 {
                    std::thread::sleep(Duration::from_millis(5));
                }
                seen.lock().unwrap().push(i);
                if i == 199 {
                    let _ = done.send(());
                }
            });
        }
        wait.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(*seen.lock().unwrap(), (0..200).collect::<Vec<_>>());
    }

    #[test]
    fn many_jobs_share_a_few_threads_and_a_panic_stops_none() {
        let threads = Arc::new(Mutex::new(HashSet::new()));
        let (done, wait) = mpsc::channel();
        run("jobs-test:panics", || panic!("on purpose"));
        for i in 0..500 {
            let (threads, done) = (threads.clone(), done.clone());
            run(&format!("jobs-test:{}", i % 37), move || {
                let name = std::thread::current().name().unwrap_or("").to_string();
                threads.lock().unwrap().insert(name);
                let _ = done.send(());
            });
        }
        let (after, wait_after) = mpsc::channel();
        run("jobs-test:panics", move || {
            let _ = after.send(());
        });
        for _ in 0..500 {
            wait.recv_timeout(Duration::from_secs(10)).unwrap();
        }
        wait_after.recv_timeout(Duration::from_secs(10)).unwrap();
        let threads = threads.lock().unwrap();
        assert!(threads.len() <= WORKERS, "{:?}", threads);
        assert!(
            threads.iter().all(|t| t.starts_with("wb-jobs-")),
            "{:?}",
            threads
        );
    }
}
