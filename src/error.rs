use thiserror::Error;
use tokio_cron_scheduler::JobSchedulerError;

#[derive(Error, Debug)]
pub enum Error {
    #[error("scheduler error: {0}")]
    Scheduler(#[from] JobSchedulerError),
    #[error("other error: {0}")]
    Other(String),
}