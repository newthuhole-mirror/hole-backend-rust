use chrono::{offset::Local, DateTime};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sha2::{Digest, Sha256};

pub struct RandomHasher {
    pub salt: String,
    pub start_time: DateTime<Local>,
}

impl RandomHasher {
    pub fn get_random_one() -> RandomHasher {
        RandomHasher {
            salt: thread_rng()
                .sample_iter(&Alphanumeric)
                .take(16)
                .map(char::from)
                .collect(),
            start_time: Local::now(),
        }
    }

    pub fn hash_with_salt(&self, text: &str) -> String {
        let mut h = Sha256::new();
        h.update(text);
        h.update(&self.salt);
        format!("{:X}", h.finalize())[5..21].to_string()
    }

    pub fn get_tmp_token(&self) -> String {
        // 每15分钟变化一次
        self.hash_with_salt(&format!(
            "{}_{}",
            Local::now().timestamp() / 60 / 15,
            self.start_time.timestamp_subsec_nanos()
        ))
    }
}
