//! Context publication and request-window policy, independent of session execution.

mod additional_context;
mod compaction_reduction;
mod compaction_replacement;
mod context_fingerprint;
mod request_budget;

pub use additional_context::AdditionalContextSnapshot;
pub use additional_context::AdditionalContextStore;
pub use compaction_reduction::CompactionItem;
pub use compaction_reduction::CompactionReduction;
pub use compaction_reduction::CompactionReductionExhausted;
pub use compaction_reduction::LocalCompactionReduction;
pub use compaction_reduction::plan_compaction_reduction;
pub use compaction_replacement::CompactionReplacement;
pub use compaction_replacement::ReplacementExceedsWindow;
pub use compaction_replacement::fit_compaction_replacement;
pub use request_budget::RequestBudget;
pub use request_budget::RequestExceedsWindow;
pub use request_budget::RequestFit;
