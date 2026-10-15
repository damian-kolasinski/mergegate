use super::{JudgeSetting, Verdict};
use crate::item::ItemId;
use crate::project::ProjectName;
use crate::store::StoreError;

/// Each project's judge and the verdicts it gave.
pub trait JudgeStore {
    /// None when the project has no judge.
    fn judge_setting(&self, project: &ProjectName) -> Result<Option<JudgeSetting>, StoreError>;

    fn set_judge_setting(
        &mut self,
        project: &ProjectName,
        setting: Option<&JudgeSetting>,
    ) -> Result<(), StoreError>;

    /// The verdicts on `revision` from the judge whose [`JudgeSetting::key`]
    /// is `key`, in path order; none when that judge hasn't judged it.
    fn verdicts(
        &self,
        id: &ItemId,
        revision: u32,
        key: &str,
    ) -> Result<Option<Vec<Verdict>>, StoreError>;

    /// Replaces any verdicts on `revision`, whichever judge gave them.
    fn insert_verdicts(
        &mut self,
        id: &ItemId,
        revision: u32,
        key: &str,
        verdicts: &[Verdict],
    ) -> Result<(), StoreError>;
}
