//! MAIS, Multi-Agent Investigative Systems: reads what a multi-agent run's CLIs and orchestrator
//! recorded, ties every model call to its agent and that agent's place in the team, and explains
//! where the run's tokens and context went.
pub mod adapters;
pub mod link;
pub mod model;
