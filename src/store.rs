use rand::{Rng, RngCore};
use sha2::{Digest, Sha256};

#[derive(Deserialize, Serialize)]
pub struct Account {
    pub email: String,
    pub secret: Secret,
}

#[derive(Deserialize, Serialize)]
pub struct Secret {
    pub hash: Vec<u8>,
    pub salt: Vec<u8>,
}

impl Secret {
    pub fn encode(rng: &mut RngCore, password: &str) -> Secret {
        let salt: Vec<u8> = rng.gen::<[u8; 32]>().as_ref().into();

        let mut hasher = Sha256::new();
        hasher.input(password.as_bytes());
        hasher.input(&salt);

        let hash: Vec<u8> = hasher.result().as_ref().into();

        Secret { hash, salt }
    }

    pub fn contains(&self, password: &str) -> bool {
        let mut hasher = Sha256::new();
        hasher.input(password.as_bytes());
        hasher.input(&self.salt);

        self.hash == hasher.result().as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{prng::hc128::Hc128Rng, SeedableRng};

    #[test]
    fn test_secret_should_match() {
        let seed = [0; 32];
        let mut rng = Hc128Rng::from_seed(seed);
        
        let password = String::from("hunter2");
        let secret = Secret::encode(&mut rng, &password);
        assert!(secret.contains(&password));
    }

    #[test]
    fn test_secret_should_not_match() {
        let seed = [0; 32];
        let mut rng = Hc128Rng::from_seed(seed);
        
        let password = String::from("hunter2");
        let secret = Secret::encode(&mut rng, &password);
        assert!(!secret.contains("qwerty"));
    }
}