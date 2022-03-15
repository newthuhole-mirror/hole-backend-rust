use rand::{distributions::Alphanumeric, thread_rng, Rng};

pub struct RandomHasher {
    salt: String,
}

impl RandomHasher {
    pub fn get_random_one() -> RandomHasher {
        RandomHasher {
            salt: thread_rng()
                .sample_iter(&Alphanumeric)
                .take(16)
                .map(char::from)
                .collect(),
        }
    }

    pub fn hash_with_salt(&self, text: &str) -> String {
        // TODO
        format!("hash({}+{})", self.salt, text)
    }
}
