use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Tokens of one model call or a sum of calls. `input` is fresh input only: cache reads and
/// writes are counted apart, as Claude Code reports them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Usage {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
}

impl Usage {
    /// Everything sent to the model in the call: the context it read.
    pub fn context(&self) -> u64 {
        self.input + self.cache_read + self.cache_write
    }

    pub fn total(&self) -> u64 {
        self.context() + self.output
    }

    /// Each field's larger value (one message's lines repeat its usage, output growing).
    pub fn max(self, o: Self) -> Self {
        Self { input: self.input.max(o.input), cache_read: self.cache_read.max(o.cache_read), cache_write: self.cache_write.max(o.cache_write), output: self.output.max(o.output) }
    }
}

impl std::ops::Add for Usage {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self { input: self.input + o.input, cache_read: self.cache_read + o.cache_read, cache_write: self.cache_write + o.cache_write, output: self.output + o.output }
    }
}

impl std::iter::Sum for Usage {
    fn sum<I: Iterator<Item = Self>>(it: I) -> Self {
        it.fold(Self::default(), |a, b| a + b)
    }
}
