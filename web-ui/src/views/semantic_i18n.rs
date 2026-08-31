use crate::label_or;
use crate::state::UiLang;

pub(crate) type SemanticLang = UiLang;

pub(crate) fn localized_status(lang: SemanticLang, status: &str) -> String {
    if lang == SemanticLang::En {
        return match status {
            "active_trigger_failed" => "Active, trigger failed",
            other => other,
        }
        .to_string();
    }
    match status {
        "approved" => "已批准",
        "rejected" => "已拒绝",
        "pending" => "待审核",
        "pending_review" => "待审核",
        "materialized" => "已发布",
        "candidate" => "候选",
        "failed_gate" => "闸门失败",
        "superseded" => "已替代",
        "rolled_back" => "已回滚",
        "archived" => "已归档",
        "customer_grade" => "客户级",
        "production_like_pilot" => "生产试点",
        "repo_controlled" => "仓库控制",
        "ready" => "就绪",
        "approval" | "approval_required" | "write_approval_required" => "需审批",
        "needs_review" => "需复核",
        "blocked" => "已阻塞",
        "pilot_ready" => "试点就绪",
        "active" => "运行中",
        "active_trigger_failed" => "运行中，触发失败",
        "executing" => "执行中",
        "finalizing" => "收尾中",
        "cancel_requested" => "取消中",
        "outcome_unknown" => "结果待对账",
        "canceled" | "cancelled" => "已取消",
        "completed" => "已完成",
        "proposal_only" => "仅提案",
        "profiled" => "已画像",
        "proposed" => "已提议",
        "compiled" => "已生成",
        "referenced" => "被引用",
        "profile_unset" => "事务未设置",
        "mode_unset" => "模式未设置",
        "read_only" => "只读",
        "" => "未设置",
        other => other,
    }
    .to_string()
}

pub(crate) fn localized_source_mode(lang: SemanticLang, source_mode: &str) -> String {
    if lang == SemanticLang::En {
        return match source_mode {
            "demo_ecommerce" | "demo" | "" => "Sample data",
            "demo_insurance" => "Insurance sample",
            other => label_or(other, "demo"),
        }
        .to_string();
    }
    match source_mode {
        "demo_ecommerce" | "demo" => "示例数据",
        "demo_insurance" => "保险示例",
        "" => "示例",
        other => other,
    }
    .to_string()
}

pub(crate) fn localized_risk(lang: SemanticLang, risk: &str) -> String {
    if lang == SemanticLang::En {
        return risk.to_string();
    }
    match risk {
        "low" => "低风险",
        "medium" => "中风险",
        "high" => "高风险",
        "needs_review" => "需复核",
        "approval_required" => "需审批",
        "merge" => "合并",
        "pii_review" => "PII 复核",
        "merge_review_required" => "合并复核",
        "possible_match" => "可能匹配",
        "blocked" => "已阻塞",
        "risk_unset" => "风险未设置",
        "" => "未设置",
        other => other,
    }
    .to_string()
}

pub(crate) fn localized_proposal_type(lang: SemanticLang, proposal_type: &str) -> String {
    if lang == SemanticLang::En {
        return proposal_type.to_ascii_uppercase();
    }
    match proposal_type {
        "object" => "业务对象",
        "relation" => "关系 Link",
        "metric" => "指标 Metric",
        "logic" | "logic_rule" => "规则 Logic",
        "action" => "动作 Action",
        other => other,
    }
    .to_string()
}

pub(crate) fn localized_node_type(lang: SemanticLang, node_type: &str) -> String {
    if lang == SemanticLang::En {
        return node_type.to_string();
    }
    match node_type {
        "dataset" => "资料表",
        "object" => "业务对象",
        "metric" => "指标",
        "logic" => "规则",
        "action" => "动作",
        "tool" => "工具",
        "subgraph" => "子图",
        "merge_candidate" => "合并候选",
        other => other,
    }
    .to_string()
}

pub(crate) fn localized_edge_type(lang: SemanticLang, edge_type: &str) -> String {
    if lang == SemanticLang::En {
        return edge_type.to_string();
    }
    match edge_type {
        "maps_to" => "映射为",
        "relates_to" => "业务关联",
        "uses_metric" => "计算指标",
        "depends_on" => "依赖",
        "validates" => "校验",
        "acts_on" => "作用于",
        "compiles_to" => "生成工具",
        "groups" => "归入子图",
        "merge_suggests" => "建议合并",
        other => other,
    }
    .to_string()
}

pub(crate) fn localized_evidence_key<'a>(lang: SemanticLang, key: &'a str) -> &'a str {
    if lang == SemanticLang::En {
        return key;
    }
    match key {
        "approval_required" => "审批",
        "contract_source" => "动作来源",
        "definition_evidence" => "定义证据",
        "domain_scope" => "领域",
        "effect_count" => "影响数",
        "enum_candidates" => "枚举候选",
        "execution_mode" => "执行模式",
        "expression" => "指标公式",
        "field_null_rates" => "空值率",
        "industry" => "行业",
        "join_success_rate" => "关联成功率",
        "pii_candidates" => "PII 候选",
        "primary_key" => "主键",
        "primary_key_candidates" => "主键候选",
        "references_field" => "目标字段",
        "references_table" => "目标表",
        "row_count" => "样本行",
        "seed_ontology_match" => "种子对象",
        "seed_relation_match" => "种子关系",
        "semantic_model" => "语义模型",
        "source_field" => "来源字段",
        "source_mode" => "来源模式",
        "source_table" => "来源表",
        "table" => "资料表",
        "target_object" => "目标对象",
        "time_dimensions" => "时间维度",
        "tool_namespace" => "工具命名空间",
        "transaction_profile" => "事务策略",
        other => other,
    }
}
