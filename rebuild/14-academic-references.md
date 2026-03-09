# 学术引用与技术文档

## 核心 RL 算法

### PPO — Proximal Policy Optimization

- **论文**: Schulman, Wolski, Dhariwal, Radford, Klimov (2017). "Proximal Policy Optimization Algorithms"
- **arXiv**: https://arxiv.org/abs/1707.06347
- **OpenAI 博客**: https://openai.com/index/openai-baselines-ppo/
- **与本项目的关系**: PPO 是项目的核心训练算法。使用 clipped surrogate objective、多 epoch mini-batch 更新、并行环境收集 rollout。

### GAE — Generalized Advantage Estimation

- **论文**: Schulman, Moritz, Levine, Jordan, Abbeel (2016). "High-Dimensional Continuous Control Using Generalized Advantage Estimation"
- **arXiv**: https://arxiv.org/abs/1506.02438
- **与本项目的关系**: PPO 训练中使用 GAE(lambda=0.95) 计算 advantage，平衡 bias-variance tradeoff。实现在 `training/rollout.py` 的 `compute_advantages()` 中。

### AlphaZero — Self-Play + MCTS

- **论文**: Silver, Hubert, Schrittwieser et al. (2018). "A general reinforcement learning algorithm that masters chess, shogi, and Go through self-play"
- **arXiv**: https://arxiv.org/abs/1712.01815
- **Science**: DOI 10.1126/science.aar6404
- **与本项目的关系**: MCTSAgent 的设计参考了 AlphaZero 的 MCTS + neural network policy 架构。计划中的 Phase 6 将实现 AlphaZero 风格的"MCTS 生成训练目标"。

## 模仿学习

### DAgger — Dataset Aggregation

- **论文**: Ross, Gordon, Bagnell (2011). "A Reduction of Imitation Learning and Structured Prediction to No-Regret Online Learning"
- **arXiv**: https://arxiv.org/abs/1011.0686
- **AISTATS 2011**: https://proceedings.mlr.press/v15/ross11a/ross11a.pdf
- **与本项目的关系**: 当前使用的 BC (Behavior Cloning) 存在分布漂移问题。DAgger 通过迭代收集学习策略运行时的 expert label 来缓解此问题，是计划中的下一步改进方向。

### Behavior Cloning 基础理论

- **关键概念**: 分布漂移 (distribution shift) — 训练数据来自 expert 分布，但测试时 agent 自身的错误将其带入未见过的状态
- **量化**: 每步错误率 epsilon 在 T 步 episode 中累积为 O(T^2 * epsilon)（DAgger 将其降为 O(T * epsilon)）

## 牌类 / 卡牌游戏 AI

### Pluribus — 超人类多人扑克 AI

- **论文**: Brown, Sandholm (2019). "Superhuman AI for multiplayer poker"
- **Science**: DOI 10.1126/science.aay2400
- **Nature 报道**: https://www.nature.com/articles/d41586-019-02156-9
- **与本项目的关系**: Pluribus 在六人无限注德州扑克中击败职业选手。Balatro 同样涉及不完全信息和概率推理，但 Balatro 是单人游戏且没有对手建模需求，因此 RL 方法比 CFR 更适用。

### Libratus — 超人类单挑扑克 AI

- **论文**: Brown, Sandholm (2018). "Superhuman AI for heads-up no-limit poker: Libratus beats top professionals"
- **Science**: DOI 10.1126/science.aao1733
- **与本项目的关系**: Libratus 使用 Counterfactual Regret Minimization (CFR)。其"blueprint + subgame solving"的分层决策思想启发了 MCTSAgent 的"关键决策点搜索"设计。

## 同类游戏 RL 实践

### Slay the Spire RL

- **Maskable PPO 实现**: https://github.com/krystianrusin/slay-the-spire-rl
- **多模型方法 (DQN + NN)**: https://milesoram.github.io/slay-the-spire-ml-project.html
- **自动化 AI**: https://github.com/xaved88/bottled_ai
- **与本项目的关系**: Slay the Spire 是最接近 Balatro 的同类游戏——都是 roguelike 卡牌构建、都有多阶段决策（战斗/商店/事件）、都有稀疏奖励。Maskable PPO 方案与本项目技术栈高度一致。

### Hearthstone / TCG AI

- **Hearthstone RL**: 大量 OpenAI Gym 环境和 RL agent，但 Hearthstone 是对战游戏（双人零和），与 Balatro 的单人优化问题结构不同
- **参考价值**: 卡牌效果建模、deck building 策略抽象

## 基础设施文档

### Gymnasium

- **文档**: https://gymnasium.farama.org/
- **GitHub**: https://github.com/Farama-Foundation/Gymnasium
- **与本项目的关系**: `BalatroEnv` 实现了 `gymnasium.Env` 接口，包括 `reset()`, `step()`, `observation_space`, `action_space`

### PyTorch

- **文档**: https://pytorch.org/docs/stable/
- **与本项目的关系**: 所有模型（BalatroMLP, CardEncoder, JokerEncoder, PolicyValueNet）使用 PyTorch 实现

### PyO3

- **文档**: https://pyo3.rs/
- **GitHub**: https://github.com/PyO3/pyo3
- **与本项目的关系**: pylatro 通过 PyO3 将 Rust 游戏引擎暴露为 Python 模块

### maturin

- **文档**: https://www.maturin.rs/
- **GitHub**: https://github.com/PyO3/maturin
- **与本项目的关系**: 构建和安装 pylatro 的工具。`maturin develop --release` 编译 Rust 并安装为 Python 包

### Weights & Biases (wandb)

- **文档**: https://docs.wandb.ai/
- **与本项目的关系**: 实验追踪、训练曲线可视化、超参记录
