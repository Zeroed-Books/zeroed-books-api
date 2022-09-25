use chrono::{Duration, DurationRound, Utc};
use redis::Commands;

use super::{RateLimitResult, RateLimiter};

/// A rate limiter that uses Redis as a backing store.
pub struct RedisRateLimiter {
    client: redis::Client,
}

impl RedisRateLimiter {
    /// Create a new rate limiter.
    ///
    /// # Arguments
    ///
    /// * `connection_uri` - The connection string used to connect to Redis.
    pub fn new(connection_uri: &str) -> anyhow::Result<Self> {
        Ok(Self {
            client: redis::Client::open(connection_uri)?,
        })
    }
}

impl RateLimiter for RedisRateLimiter {
    fn is_limited(&self, key: &str, max_req_per_min: u64) -> anyhow::Result<RateLimitResult> {
        // Rate limiting is implemented using the basic algorithm suggested by
        // the Redis documentation:
        // https://redis.com/redis-best-practices/basic-rate-limiting/

        let mut conn = self.client.get_connection()?;

        // We only do per-minute rate limiting. This means we can use the
        // current minute as our cache key because by the time it's used again,
        // the previous value will have expired 58 minutes ago.
        let now = Utc::now();
        let current_minute = now.format("%M").to_string();

        let cache_key = format!("{}:{}", key, current_minute);

        let hits: Option<u64> = conn.get(&cache_key)?;
        if let Some(hit_count) = hits {
            if hit_count > max_req_per_min {
                // Rate limit for the current minute has already been exceeded,
                // so just report the error along with the timestamp for when
                // the rate limit resets.
                //
                // Since we use a granularity of minutes for our cache key, make
                // sure that we truncate the expiration time to the last whole
                // minute
                let mut limit_expiration = now + Duration::minutes(1);
                limit_expiration = limit_expiration
                    .duration_trunc(Duration::minutes(1))
                    // Truncation only fails if the timestamp excedes the max
                    // representable timestamp in nanoseconds or if the duration
                    // exceeds the timestamp. Neither of those cases is true
                    // here, so we can just unwrap the result.
                    .expect("failed to truncate time");

                return Ok(RateLimitResult::LimitedUntil(limit_expiration));
            }
        }

        // The cache key either doesn't exist or is below the allowable rate
        // limit. Increment it by one, and ensure that the key has an expiration
        // time of one minute.
        //
        // Note that the "worst case" for expiration is if the key is
        // incremented the moment before the minute rolls over, meaning it will
        // expire the moment before the next minute rolls over. This gives us
        // the buffer of 58 minutes stated previously.
        redis::pipe()
            .atomic()
            .cmd("INCR")
            .arg(&cache_key)
            .ignore()
            .cmd("EXPIRE")
            .arg(&cache_key)
            .arg(59)
            .execute(&mut conn);

        Ok(RateLimitResult::NotLimited)
    }
}
