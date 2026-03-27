# Agent Style

## 当前阶段的唯一目标

- 当前阶段的目标不是“做一个大致能玩的 Balatro 风格环境”。
- 当前阶段的目标是：造出与原版 Balatro 一致的环境，并且用真实客户端 trajectory 证明它一致。

## 不接受的标准

- 不接受“最终分数差不多”。
- 不接受“多数 case 正确”。
- 不接受“看起来差不多能玩”。
- 不接受“先用近似 simulator 顶着，后面再说”。

## 接受标准

- 真实客户端是唯一金标准。
- 所有 replay / trajectory 验证都必须优先对齐本机合法持有的 `Balatro.love` 与真实客户端行为。
- 只有在同一 seed、同一动作序列下实现：
  - 动作合法性一致
  - 状态转移一致
  - chips / mult / dollars 一致
  - hand / deck / discard / jokers / consumables 一致
  - blind / shop / reroll / skip side effects 一致
  - Joker 触发顺序一致
  - RNG 结果一致
- 才能把环境视为通过当前阶段。

## 工程策略

- 每次发现 replay 与 Lua / 真实客户端预期不一致，优先改引擎和动作语义，不用解释掩盖偏差。
- snapshot 只够做粗对比；关键事件链必须保留，便于定位 Joker 顺序、blind side effect、RNG 消耗顺序。
- 所有可见 demo、CLI、viewer、日志都只是验证手段，不是 correctness 的替代品。

## 行为约束

- 遇到 fidelity gap 时，应明确写出 gap，而不是把 gap 包装成 feature。
- 任何阶段门槛都必须向“原版一致”收敛，而不是向“更方便训练”收敛。
