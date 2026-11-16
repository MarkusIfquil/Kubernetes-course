use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub port: u16,
    pub greeting: String,
    pub redis_url: String,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let port = parse_port(&env::var("PORT").unwrap_or_else(|_| "8080".to_string()))?;

        Ok(Self {
            port,
            greeting: env::var("GREETING")
                .unwrap_or_else(|_| "hello from my container".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379/0".to_string()),
        })
    }
}

pub fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port >= 1)
        .ok_or_else(|| format!("invalid PORT {value:?}: must be an integer between 1 and 65535"))
}

#[cfg(test)]
mod tests {
    use super::parse_port;

    #[test]
    fn accepts_valid_ports() {
        assert_eq!(parse_port("8080"), Ok(8080));
        assert_eq!(parse_port("1"), Ok(1));
        assert_eq!(parse_port("65535"), Ok(65535));
    }

    #[test]
    fn rejects_invalid_ports() {
        assert!(parse_port("").is_err());
        assert!(parse_port("abc").is_err());
        assert!(parse_port("0").is_err());
        assert!(parse_port("65536").is_err());
        assert!(parse_port("-1").is_err());
    }
}
