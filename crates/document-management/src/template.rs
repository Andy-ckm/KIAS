//! 文档模板系统
//! 预定义医药企业常用文档模板

use serde::{Deserialize, Serialize};

/// 文档模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentTemplate {
    pub id: String,
    pub name: String,
    pub category: TemplateCategory,
    pub description: String,
    pub sections: Vec<TemplateSection>,
    pub required_signatures: Vec<String>,
    pub applicable_doc_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateCategory {
    SOP,           // 标准操作规程
    Protocol,      // 验证方案
    Report,        // 报告
    Policy,        // 政策
    WorkInstruction, // 工作指导
    Form,          // 表格
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSection {
    pub title: String,
    pub content: String,
    pub required: bool,
    pub order: u32,
}

/// 预定义模板库
pub struct TemplateLibrary;

impl TemplateLibrary {
    /// SOP 模板
    pub fn sop_template() -> DocumentTemplate {
        DocumentTemplate {
            id: "tpl-sop-001".into(),
            name: "标准操作规程模板".into(),
            category: TemplateCategory::SOP,
            description: "医药企业标准操作规程通用模板".into(),
            sections: vec![
                TemplateSection { title: "目的".into(), content: "描述本SOP的目的和适用范围".into(), required: true, order: 1 },
                TemplateSection { title: "范围".into(), content: "本SOP适用于...".into(), required: true, order: 2 },
                TemplateSection { title: "职责".into(), content: "列出相关人员职责".into(), required: true, order: 3 },
                TemplateSection { title: "程序".into(), content: "详细操作步骤".into(), required: true, order: 4 },
                TemplateSection { title: "参考文件".into(), content: "相关法规和标准".into(), required: false, order: 5 },
                TemplateSection { title: "修订历史".into(), content: "版本变更记录".into(), required: true, order: 6 },
            ],
            required_signatures: vec!["编写人".into(), "审核人".into(), "批准人".into()],
            applicable_doc_types: vec!["SOP".into()],
        }
    }

    /// 验证方案模板
    pub fn validation_protocol_template() -> DocumentTemplate {
        DocumentTemplate {
            id: "tpl-vp-001".into(),
            name: "验证方案模板".into(),
            category: TemplateCategory::Protocol,
            description: "IQ/OQ/PQ 验证方案通用模板".into(),
            sections: vec![
                TemplateSection { title: "验证目的".into(), content: "本验证方案的目的是...".into(), required: true, order: 1 },
                TemplateSection { title: "验证范围".into(), content: "验证范围包括...".into(), required: true, order: 2 },
                TemplateSection { title: "验证策略".into(), content: "验证方法和接受标准".into(), required: true, order: 3 },
                TemplateSection { title: "人员职责".into(), content: "验证团队成员及职责".into(), required: true, order: 4 },
                TemplateSection { title: "测试用例".into(), content: "详细测试步骤和预期结果".into(), required: true, order: 5 },
                TemplateSection { title: "偏差处理".into(), content: "偏差处理流程".into(), required: true, order: 6 },
                TemplateSection { title: "结论".into(), content: "验证结论和建议".into(), required: true, order: 7 },
            ],
            required_signatures: vec!["验证人员".into(), "QA审核".into(), "批准人".into()],
            applicable_doc_types: vec!["ValidationProtocol".into()],
        }
    }

    /// 获取所有内置模板
    pub fn all_templates() -> Vec<DocumentTemplate> {
        vec![
            Self::sop_template(),
            Self::validation_protocol_template(),
        ]
    }
}
