//! In-process background services. PG stores durable work; Tokio owns lifetime.
//! No second binary/container/broker and no actors pretending to be a durable queue.
use super::federation::http_signature::RsaHttpSigner;
use super::{
    crawler::{self, PublicHttp},
    db::Database,
    directory::Observation,
    storage::ObjectStore,
};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinHandle};

pub struct Services {
    stop: watch::Sender<bool>,
    supervisors: Vec<JoinHandle<()>>,
}
#[cfg(debug_assertions)]
pub async fn start_development_once(
    db: Database,
    media: Option<ObjectStore>,
    signer: Arc<RsaHttpSigner>,
) {
    static STARTED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
    STARTED
        .get_or_init(|| async move {
            let services = Services::start(db, media, signer);
            tokio::spawn(async move {
                shutdown_signal().await;
                services.shutdown().await;
                std::process::exit(0);
            });
        })
        .await;
}
impl Services {
    pub fn start(db: Database, media: Option<ObjectStore>, signer: Arc<RsaHttpSigner>) -> Self {
        let (stop, rx) = watch::channel(false);
        let mut supervisors = vec![supervise(
            "directory-scheduler",
            db.clone(),
            rx.clone(),
            scheduler,
        )];
        for _ in 0..5 {
            supervisors.push(supervise("directory-fetch", db.clone(), rx.clone(), worker));
        }
        if let Some(store) = media {
            let profile_store = store.clone();
            supervisors.push(supervise(
                "profile-refresh",
                db.clone(),
                rx.clone(),
                move |db, stop| profile_batches(db, profile_store.clone(), signer.clone(), stop),
            ));
            supervisors.push(supervise("media-cleanup", db, rx, move |db, stop| {
                media_cleanup(db, store.clone(), stop)
            }));
        }
        Self { stop, supervisors }
    }
    pub async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        // A bounded drain fits ordinary container termination windows. Hard-kill
        // or timed-out shutdown leaves leases for the next process to recover.
        let drained = tokio::time::timeout(Duration::from_secs(25), async {
            for task in &mut self.supervisors {
                let _ = task.await;
            }
        })
        .await;
        if drained.is_err() {
            for task in &self.supervisors {
                task.abort();
            }
        }
    }
}
impl Drop for Services {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}
struct AbortOnDrop(JoinHandle<()>);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
fn supervise<F, Fut>(
    name: &'static str,
    db: Database,
    mut stop: watch::Receiver<bool>,
    run: F,
) -> JoinHandle<()>
where
    F: Fn(Database, watch::Receiver<bool>) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        let mut backoff = 1;
        loop {
            if *stop.borrow() {
                return;
            }
            let started = std::time::Instant::now();
            let mut child = AbortOnDrop(tokio::spawn(run(db.clone(), stop.clone())));
            tokio::select! {
                _=&mut child.0=>{},
                _=stop.changed()=>{let _=(&mut child.0).await;return;}
            }
            if *stop.borrow() {
                return;
            }
            if started.elapsed() > Duration::from_secs(300) {
                backoff = 1;
            }
            // No error payload: a panic/query error may contain remote input.
            eprintln!("Background service {name} stopped; restarting in {backoff}s");
            if delay(&mut stop, Duration::from_secs(backoff)).await {
                return;
            }
            backoff = (backoff * 2).min(60);
        }
    })
}
async fn delay(stop: &mut watch::Receiver<bool>, duration: Duration) -> bool {
    if *stop.borrow() {
        return true;
    }
    tokio::select! {_=tokio::time::sleep(duration)=>false,_=stop.changed()=>true}
}
async fn scheduler(db: Database, mut stop: watch::Receiver<bool>) {
    while !*stop.borrow() {
        if db.enqueue_sites().await.is_err() || db.maintain_directory().await.is_err() {
            // Supervisor supplies backoff, rather than a busy error/retry loop.
            return;
        }
        if delay(&mut stop, Duration::from_secs(15)).await {
            return;
        }
    }
}
async fn media_cleanup(db: Database, store: ObjectStore, mut stop: watch::Receiver<bool>) {
    while !*stop.borrow() {
        let wait = match db.collect_retired_media(&store).await {
            Ok(true) => Duration::from_millis(100),
            Ok(false) => Duration::from_secs(5),
            Err(_) => return,
        };
        if delay(&mut stop, wait).await {
            return;
        }
    }
}
async fn profile_batches(
    db: Database,
    store: ObjectStore,
    signer: Arc<RsaHttpSigner>,
    mut stop: watch::Receiver<bool>,
) {
    while !*stop.borrow() {
        let job = match db.claim_profile_job().await {
            Ok(value) => value,
            Err(_) => return,
        };
        if *stop.borrow() {
            return;
        }
        let wait = if let Some(job) = job {
            if super::profile_refresh::run_batch_job(&db, &store, signer.clone(), job)
                .await
                .is_err()
            {
                return;
            }
            Duration::from_millis(100)
        } else {
            Duration::from_secs(5)
        };
        if delay(&mut stop, wait).await {
            return;
        }
    }
}
async fn worker(db: Database, mut stop: watch::Receiver<bool>) {
    while !*stop.borrow() {
        let job = match db.claim_site().await {
            Ok(Some(job)) => job,
            Ok(None) => {
                if delay(&mut stop, Duration::from_secs(5)).await {
                    return;
                }
                continue;
            }
            Err(_) => return,
        };
        if *stop.borrow() {
            return;
        }
        let result =
            tokio::time::timeout(Duration::from_secs(60), crawler::collect(&PublicHttp, &job))
                .await
                .unwrap_or_else(|_| Observation::failed("collection_timeout"));
        if db.finish_site(&job, &result).await.is_err() {
            return;
        }
    }
}
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}};
            return;
        }
    }
    if tokio::signal::ctrl_c().await.is_err() {
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn supervisor_restarts_failed_child_and_drains_on_shutdown() {
        let db = crate::backend::db::fixtures::database().await;
        let count = Arc::new(AtomicUsize::new(0));
        let observed = count.clone();
        let (stop, rx) = watch::channel(false);
        let task = supervise("test-child", db, rx, move |_, mut stop| {
            let count = count.clone();
            async move {
                if count.fetch_add(1, Ordering::SeqCst) == 0 {
                    panic!("simulated worker failure");
                }
                let _ = stop.changed().await;
            }
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while observed.load(Ordering::SeqCst) < 2 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        stop.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(observed.load(Ordering::SeqCst), 2);
    }
}
