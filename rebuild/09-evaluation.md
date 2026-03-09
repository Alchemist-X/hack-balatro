# 评估方法

## 评估框架

文件：`eval/eval_policy.py`, `eval/compare_baselines.py`

### 核心函数

```python
from eval.eval_policy import evaluate_agent

metrics = evaluate_agent(
    agent=agent,
    env_factory=lambda seed: BalatroEnv(config),
    seeds=seeds,                # 100 个固定种子
    episodes_per_seed=1,        # 每个种子跑 1 局
)
```

### 评估流程

1. 加载固定种子集 (`eval/seeds.json`)
2. 对每个 seed 创建环境：`env = env_factory(seed)`
3. 运行 `_run_episode()`：
   ```python
   obs, info = env.reset(seed=seed)
   while not (terminated or truncated):
       action_mask = env.get_action_mask()
       action = agent.act(obs, info, action_mask)
       obs, reward, terminated, truncated, info = env.step(action)
   ```
4. 收集每局的 `game_won`, `blinds_passed`, `episode_reward`, `episode_length`
5. 汇总统计

## 评估指标

| 指标 | 定义 | 说明 |
|------|------|------|
| win_rate | wins / total_episodes | 通关率 |
| avg_blinds_passed | mean(blinds_passed) | 平均通过盲注数 |
| std_blinds_passed | std(blinds_passed) | 盲注数标准差 |
| avg_episode_length | mean(episode_length) | 平均步数 |
| avg_episode_reward | mean(episode_reward) | 平均回报 |
| max_win_streak | 最长连续通关次数 | 连胜 |
| avg_win_streak | 平均连胜长度 | 连胜稳定性 |
| streak_distribution | {1-2: n, 3-4: n, 5-9: n, 10+: n} | 连胜分布 |

## 固定种子集

文件：`eval/seeds.json`

包含 100 个预定义种子，确保评估可复现：

```json
[42, 137, 256, 314, 500, 628, 777, 888, 999, 1024,
 1111, 1234, 1337, 1500, 1776, 2000, 2023, 2048, 2222, 2500,
 ...]
```

使用固定种子意味着：
- 每局的初始牌组、Joker 出现顺序、Boss 效果完全相同
- 不同智能体在完全相同的条件下对比
- 结果可复现

注意：当前 pylatro 对种子控制不完整，`GameEngine()` 不接受种子参数，因此种子仅影响 Gymnasium 层面。

## 多智能体对比

文件：`eval/compare_baselines.py`

```python
from eval.compare_baselines import compare_agents

results = compare_agents(
    agents={"PPO": ppo_agent, "Greedy": greedy_agent, "Random": random_agent},
    env_factory=env_factory,
    seeds=seeds,
)
save_comparison(results, "results/comparison.json")
```

输出并排对比表：

```
Agent          | Win Rate | Avg Blinds | Avg Reward | Max Streak | Avg Streak
─────────────────────────────────────────────────────────────────────────────
GreedyAgent    |   4.0%   |    6.2     |    2.90    |     2      |    1.3
BC-Greedy      |   2.0%   |    3.8     |    1.58    |     1      |    1.0
PPO-BC         |   0.0%   |    1.5     |    0.24    |     0      |    0.0
RandomAgent    |   0.0%   |    0.3     |    0.02    |     0      |    0.0
```

## 评估脚本

```bash
# 评估随机智能体
python scripts/eval_run.py --agent random --episodes 100

# 评估 PPO 智能体
python scripts/eval_run.py --agent ppo --checkpoint checkpoints/best.pt --episodes 100

# 批量评估 GreedyAgent
python scripts/test_greedy.py --workers 100 --num-games 200

# 寻找 GreedyAgent 的一次胜利
python scripts/test_greedy.py --workers 100 --until-win
```

## 训练中的在线评估

PPO 训练过程中，每 `eval_interval` (100K-500K) 步执行一次 20-seed 快速评估：

```python
if total_steps % eval_interval == 0:
    metrics = evaluate_agent(agent, seeds[:20])
    log(f"Step {total_steps}: WR={metrics['win_rate']:.3f}")
    if metrics["win_rate"] > best_win_rate:
        agent.save("checkpoints/best.pt")
```
