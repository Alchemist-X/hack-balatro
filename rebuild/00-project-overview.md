# 项目总览

## 目标

训练一个能在 [Balatro](https://www.playbalatro.com/)（扑克 roguelike 卡牌构建游戏）中达到**超人类通关率与连胜**的 AI 智能体。技术路线：Rust 游戏引擎 (pylatro) -> Gymnasium 封装 -> BC 预训练 -> PPO 微调 -> MCTS 增强。

## 技术路线

```
Phase 0  项目初始化与依赖搭建
Phase 1  环境封装 — pylatro (Rust) → Gymnasium, 454d obs / 86d action
Phase 2  基线智能体 — RandomAgent (0%) → GreedyAgent (~4%)
Phase 3  神经网络 — BalatroMLP (~934K) + Transformer (~15M)
Phase 4  PPO 训练 — clipped surrogate + GAE + action masking
Phase 5  行为克隆 + 课程学习 — GreedyAgent 轨迹 → BC → PPO
Phase 6  MCTS 搜索增强（计划中）
Phase 7  超人类评估（计划中）
```

## 文档索引

| 文件 | 内容 |
|------|------|
| [01-repositories-and-dependencies.md](01-repositories-and-dependencies.md) | 仓库、依赖、数据源、社区资源 |
| [02-simulation-environment.md](02-simulation-environment.md) | pylatro 引擎与 Gymnasium 封装 |
| [03-observation-space.md](03-observation-space.md) | 454 维观测空间逐维定义 |
| [04-action-space.md](04-action-space.md) | 86 维动作空间与 masking |
| [05-model-architecture.md](05-model-architecture.md) | MLP + Transformer 模型架构 |
| [06-agents.md](06-agents.md) | 5 种智能体的实现逻辑 |
| [07-training-pipeline.md](07-training-pipeline.md) | BC → PPO 训练流水线与超参 |
| [08-reward-design.md](08-reward-design.md) | 三层奖励结构设计 |
| [09-evaluation.md](09-evaluation.md) | 评估框架与指标定义 |
| [10-design-rationale.md](10-design-rationale.md) | 关键设计决策的原因与 rebuild 建议 |
| [11-balatro-game-rules.md](11-balatro-game-rules.md) | 游戏规则与数据获取方式 |
| [13-reproduction-guide.md](13-reproduction-guide.md) | 从零到训练的完整复现步骤 |
| [14-academic-references.md](14-academic-references.md) | 论文引用与基础设施文档 |
