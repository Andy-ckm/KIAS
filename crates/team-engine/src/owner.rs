use super::state::TeamState;
use async_trait::async_trait;
use kias_common::KiasResult;

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
        // 非 LLM 占位符：简单清理和规范化输入
        let cleaned = input.split_whitespace().collect::<Vec<_>>().join(" ");

        // 简单关键词提取：识别动作动词和关键名词
        let action_words = [
            "create",
            "build",
            "implement",
            "fix",
            "update",
            "add",
            "remove",
            "refactor",
            "test",
            "deploy",
            "configure",
            "setup",
            "analyze",
            "design",
            "review",
        ];
        let lower = cleaned.to_lowercase();

        let mut understood = cleaned.to_string();

        // 检查是否包含已知动作词
        for action in action_words {
            if lower.contains(action) {
                understood = format!("[{}] {}", action.to_uppercase(), cleaned);
                break;
            }
        }

        // 如果没有识别到动作，添加默认前缀
        if !action_words.iter().any(|a| lower.contains(a)) && lower.len() > 10 {
            understood = format!("[PROCESS] {}", cleaned);
        }

        Ok(understood)
    }

    async fn decompose_tasks(&self, goal: &str) -> KiasResult<Vec<String>> {
        // 非 LLM 占位符：基于标点符号和模式拆分任务

        // 移除理解goal时添加的前缀
        let goal_clean = goal
            .trim_start_matches(|c: char| {
                c == '[' || c == ']' || c.is_ascii_uppercase() || c == ' '
            })
            .trim();

        let mut tasks = Vec::new();

        // 尝试按数字编号拆分：1. 2. 3.
        if goal_clean.len() > 3 {
            let mut chars = goal_clean.chars().peekable();
            let mut current_num = String::new();
            let mut in_numbered = false;

            while let Some(c) = chars.next() {
                if c.is_ascii_digit() {
                    current_num.push(c);
                    in_numbered = true;
                } else if c == '.' && in_numbered && !current_num.is_empty() {
                    // 找到编号，提取后续内容
                    let mut rest = String::new();
                    let mut space_started = false;
                    for nc in chars.by_ref() {
                        if nc.is_whitespace() {
                            space_started = true;
                        } else if space_started || nc == ' ' || nc == '\t' {
                            rest.push(nc);
                            if !rest.trim().is_empty() && rest.len() > 1 {
                                break;
                            }
                        } else {
                            rest.push(nc);
                        }
                    }
                    let task = rest.trim().to_string();
                    if !task.is_empty() {
                        tasks.push(task);
                    }
                    current_num.clear();
                    in_numbered = false;
                } else {
                    in_numbered = false;
                    current_num.clear();
                }
            }
        }

        // 尝试按分号拆分
        if tasks.len() <= 1 && goal_clean.contains(';') {
            let parts: Vec<&str> = goal_clean.split(';').collect();
            if parts.len() > 1 {
                tasks.clear();
                for part in parts {
                    let trimmed = part.trim();
                    if !trimmed.is_empty() {
                        tasks.push(trimmed.to_string());
                    }
                }
            }
        }

        // 尝试按句子拆分（。或.或!或?结尾）
        if tasks.len() <= 1 {
            let mut current = String::new();
            for c in goal_clean.chars() {
                current.push(c);
                if ".!?".contains(c) {
                    let task = current.trim().to_string();
                    if !task.is_empty() {
                        tasks.push(task);
                    }
                    current.clear();
                }
            }
            // 处理最后一个句子（如果没有标点结尾）
            if !current.trim().is_empty() && tasks.is_empty() {
                tasks.push(current.trim().to_string());
            }
        }

        // 如果还是为空（简短输入），返回原始goal作为单一任务
        if tasks.is_empty() {
            let single = goal_clean.trim().to_string();
            if !single.is_empty() {
                tasks.push(single);
            }
        }

        Ok(tasks)
    }

    async fn determine_order(&self, tasks: &[String]) -> KiasResult<Vec<usize>> {
        // 默认顺序执行
        Ok((0..tasks.len()).collect())
    }

    async fn merge_results(&self, results: &[String]) -> KiasResult<String> {
        // Filter out empty results
        let non_empty: Vec<&String> = results.iter().filter(|r| !r.trim().is_empty()).collect();

        if non_empty.is_empty() {
            return Ok(String::new());
        }

        if non_empty.len() == 1 {
            return Ok(non_empty[0].clone());
        }

        // Smart merge: deduplicate lines across results, preserve structure
        let mut seen_lines: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut merged_sections: Vec<String> = Vec::new();

        for (idx, result) in non_empty.iter().enumerate() {
            let mut unique_lines: Vec<String> = Vec::new();
            for line in result.lines() {
                let normalized = line.trim().to_lowercase();
                // Keep section headers and non-trivial unique content
                if normalized.is_empty()
                    || normalized.starts_with('#')
                    || normalized.starts_with("==")
                    || normalized.starts_with("--")
                    || !seen_lines.contains(&normalized)
                {
                    seen_lines.insert(normalized);
                    unique_lines.push(line.to_string());
                }
            }

            if !unique_lines.is_empty() {
                if non_empty.len() > 2 {
                    merged_sections.push(format!(
                        "--- Result {} ---\n{}",
                        idx + 1,
                        unique_lines.join("\n")
                    ));
                } else {
                    merged_sections.push(unique_lines.join("\n"));
                }
            }
        }

        Ok(merged_sections.join("\n\n"))
    }

    fn should_stop(&self, state: &TeamState) -> bool {
        // 所有任务都验证通过
        state
            .tasks
            .iter()
            .all(|t| t.status == super::state::TaskStatus::Verified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_merge_empty_results() {
        let owner = DefaultOwner::new();
        let result = owner.merge_results(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_merge_single_result() {
        let owner = DefaultOwner::new();
        let result = owner
            .merge_results(&["single result".to_string()])
            .await
            .unwrap();
        assert_eq!(result, "single result");
    }

    #[tokio::test]
    async fn test_merge_filters_empty_strings() {
        let owner = DefaultOwner::new();
        let result = owner
            .merge_results(&[
                "".to_string(),
                "actual content".to_string(),
                "  ".to_string(),
            ])
            .await
            .unwrap();
        assert_eq!(result, "actual content");
    }

    #[tokio::test]
    async fn test_merge_two_results_deduplicates() {
        let owner = DefaultOwner::new();
        let result = owner
            .merge_results(&[
                "line 1\nline 2\nline 3".to_string(),
                "line 2\nline 3\nline 4".to_string(),
            ])
            .await
            .unwrap();
        // Should contain all unique lines
        assert!(result.contains("line 1"));
        assert!(result.contains("line 4"));
        // Duplicate lines should appear only once
        let count_line2 = result.matches("line 2").count();
        assert_eq!(count_line2, 1, "line 2 should appear only once");
    }

    #[tokio::test]
    async fn test_merge_preserves_headers() {
        let owner = DefaultOwner::new();
        let result = owner
            .merge_results(&[
                "# Section A\ncontent a".to_string(),
                "# Section B\ncontent b".to_string(),
            ])
            .await
            .unwrap();
        assert!(result.contains("# Section A"));
        assert!(result.contains("# Section B"));
        assert!(result.contains("content a"));
        assert!(result.contains("content b"));
    }

    #[tokio::test]
    async fn test_merge_three_results_adds_labels() {
        let owner = DefaultOwner::new();
        let result = owner
            .merge_results(&[
                "result one".to_string(),
                "result two".to_string(),
                "result three".to_string(),
            ])
            .await
            .unwrap();
        // With 3+ results, should add "--- Result N ---" labels
        assert!(result.contains("--- Result 1 ---"));
        assert!(result.contains("--- Result 2 ---"));
        assert!(result.contains("--- Result 3 ---"));
    }

    #[tokio::test]
    async fn test_merge_two_results_no_labels() {
        let owner = DefaultOwner::new();
        let result = owner
            .merge_results(&["a".to_string(), "b".to_string()])
            .await
            .unwrap();
        // With 2 results, should NOT add labels
        assert!(!result.contains("--- Result"));
    }
}
