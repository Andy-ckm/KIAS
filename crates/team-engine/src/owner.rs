use async_trait::async_trait;
use kias_common::KiasResult;
use super::state::TeamState;

/// Owner - 控制面（借鉴 MiniMax 设计）
/// 
/// 职责：
/// 1. 理解用户目标
/// 2. 拆分子任务
/// 3. 决定执行顺序
/// 4. 分配 Worker
/// 5. 合并结果
/// 6. 控制停止条件
#[async_trait]
pub trait Owner: Send + Sync {
    /// 理解用户目标
    async fn understand_goal(&self, input: &str) -> KiasResult<String>;
    
    /// 拆分子任务
    async fn decompose_tasks(&self, goal: &str) -> KiasResult<Vec<String>>;
    
    /// 决定执行顺序
    async fn determine_order(&self, tasks: &[String]) -> KiasResult<Vec<usize>>;
    
    /// 合并结果
    async fn merge_results(&self, results: &[String]) -> KiasResult<String>;
    
    /// 控制停止条件
    fn should_stop(&self, state: &TeamState) -> bool;
}

pub struct DefaultOwner;

impl Default for DefaultOwner {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultOwner {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Owner for DefaultOwner {
    async fn understand_goal(&self, input: &str) -> KiasResult<String> {
        // TODO: 调用 LLM 理解目标
        Ok(input.to_string())
    }

    async fn decompose_tasks(&self, goal: &str) -> KiasResult<Vec<String>> {
        // TODO: 调用 LLM 拆分任务
        Ok(vec![goal.to_string()])
    }

    async fn determine_order(&self, tasks: &[String]) -> KiasResult<Vec<usize>> {
        // 默认顺序执行
        Ok((0..tasks.len()).collect())
    }

    async fn merge_results(&self, results: &[String]) -> KiasResult<String> {
        // TODO: 调用 LLM 合并结果
        Ok(results.join("\n"))
    }

    fn should_stop(&self, state: &TeamState) -> bool {
        // 所有任务都验证通过
        state.tasks.iter().all(|t| {
            t.status == super::state::TaskStatus::Verified
        })
    }
}
