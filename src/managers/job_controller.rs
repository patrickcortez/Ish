use std::process::Child;
use std::collections::HashMap;

pub struct JobController {
    jobs: HashMap<u32, Child>,
    next_job_id: u32,
    pub last_spawned_pid: Option<u32>,
}

impl JobController {
    pub fn new() -> Self {
        Self {
            jobs: HashMap::new(),
            next_job_id: 1,
            last_spawned_pid: None,
        }
    }

    pub fn add_job(&mut self, child: Child) -> u32 {
        let id = self.next_job_id;
        self.last_spawned_pid = Some(child.id());
        self.jobs.insert(id, child);
        self.next_job_id += 1;
        id
    }

    pub fn last_job_pid(&self) -> String {
        self.last_spawned_pid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "".to_string())
    }

    pub fn list_jobs(&mut self) -> String {
        self.cleanup();
        let mut out = String::new();
        if self.jobs.is_empty() {
            out.push_str("No active background jobs.\n");
        } else {
            for (id, child) in &self.jobs {
                out.push_str(&format!("[{}] Running: PID {}\n", id, child.id()));
            }
        }
        out
    }

    pub fn wait_job(&mut self, job_id: u32) -> Result<String, String> {
        if let Some(mut child) = self.jobs.remove(&job_id) {
            match child.wait() {
                Ok(status) => Ok(format!("[{}] Completed with status: {}", job_id, status)),
                Err(e) => Err(format!("Failed to wait for job [{}]: {}", job_id, e)),
            }
        } else {
            Err(format!("Job [{}] not found", job_id))
        }
    }

    pub fn kill_job(&mut self, job_id: u32) -> Result<String, String> {
        if let Some(mut child) = self.jobs.remove(&job_id) {
            match child.kill() {
                Ok(_) => Ok(format!("[{}] Killed", job_id)),
                Err(e) => Err(format!("Failed to kill job [{}]: {}", job_id, e)),
            }
        } else {
            Err(format!("Job [{}] not found", job_id))
        }
    }

    fn cleanup(&mut self) {
        // Remove jobs that have already exited
        self.jobs.retain(|_, child| {
            match child.try_wait() {
                Ok(Some(_)) => false, // Exited
                _ => true, // Still running or error checking
            }
        });
    }
}
