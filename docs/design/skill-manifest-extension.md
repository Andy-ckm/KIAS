# Skill Manifest 扩展方案

## 目标
扩展 SkillDef，支持完整的技能元数据管理（参考 skill-mcp manifest.json）。

## 当前 SkillDef
```rust
pub struct SkillDef {
    pub name: String,
    pub description: String,
    pub version: String,
    pub tags: Vec<String>,
    pub parameters: Option<serde_json::Value>,
}
```

## 扩展后 SkillDef
```rust
pub struct SkillDef {
    // 基础信息
    pub name: String,
    pub description: String,
    pub version: String,           // SemVer: "1.2.0"
    pub tags: Vec<String>,
    
    // 新增：依赖与权限
    pub dependencies: Vec<String>, // 依赖的其他技能
    pub permissions: Vec<String>,  // 所需权限（network, filesystem, etc）
    
    // 新增：入口与内容
    pub entry: String,             // 入口文件路径
    pub content_hash: String,      // 内容哈希（用于变更检测）
    
    // 新增：时间戳
    pub created_at: String,        // ISO 8601
    pub updated_at: String,        // ISO 8601
    
    // 保留
    pub parameters: Option<serde_json::Value>,
}
```

## 向后兼容
- 新字段全部有默认值
- 旧的 SkillDef JSON 仍可解析
- 新增 SkillManifest 结构体用于完整 manifest 管理

## 集成点
1. Workspace 的 load_skill / save_skill 使用新结构
2. Skill registry 支持按 tags/dependencies 搜索
3. 内容哈希用于变更检测和版本回滚

## 测试
1. 向后兼容解析测试
2. 内容哈希计算测试
3. 依赖关系验证测试
