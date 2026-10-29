# 长历史限定探针

供隔离原生候选使用，正常 release 配置不包含此入口。

`bun x vite build --config scripts/long-history-qa/vite.qa.ts` 在既有前端入口之外生成 `/long-history-qa.js`。应由带独立应用身份和合成 provider 的本机候选消费；不要将日常 AppData 或真实模型会话用作 fixture。构建不要与会覆盖 `dist/` 的测试并行。

模块提供 `metrics()`、`send(text)`、`observeStream(name)`、`result(name)`。`observeStream` 记录 5 次预热后 30 对输入/滚动第二 rAF 样本、挂载规模、Worker 调度与草稿/焦点；结果写在当前隔离 WebView 的 localStorage。它会修改 composer 和滚动位置，只能在合成会话执行。

调用方必须另行断言所有 `sampleWindows.busy` 为 true、预期逻辑消息数、provider 500 块及原生最终文本 SHA。未挂载的尾部不证明格式收敛；缓存命中不冒称冷解析耗时。真实 Stop、选择、媒体、窗口缩放和鼠标焦点由物理输入单独验证。计数/采样不能替代这些检查。

本轮输入、范围、通过口径和原始文件哈希见[交付记录](../../docs/performance-evidence/long-history-closeout-20261029/README.md)。后续仅在相关源码变化时重采对应项。
