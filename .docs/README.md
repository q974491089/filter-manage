# Agent 内部文档

### v0.6.2 — 2026-11-15 · 方案颜色稳定性与预览配套（发布中）

基于无登录分支整合客户端颜色修复与内置场景预览，保留线上已有官网分享页构建/路由。用户已于 2026-11-15 审阅公告并授权发布，当前按 release-workflow 执行配套服务、官网及客户端正式发布。

| 类型 | 端 | 说明与根因 | 关键文件 | 文档 |
|---|---|---|---|---|
| 修复 | 客户端后端 | 方案重复 ICC 关联与中间曲线写入触发短时回退；实时查询目标输出配置、同名跳过关联、最终一次写入，保留有限偏差恢复 | icc.rs、color_backend.rs、nvidia.rs | [ICC](./api/icc.md) |
| 修复 | 客户端后端 | 游戏启动/模式变化/前台切换可能覆盖曲线；按输出身份和操作代号有限恢复，手动操作取消旧任务 | process_watcher.rs、color_recovery.rs、foreground_watch.rs | [ICC](./api/icc.md) |
| 修复 | 客户端后端 | AMD PnP 枚举旧监视器在前造成 ADLX 错屏；只选择活动监视器 | nvidia.rs、display_identity.rs | [AMD](./api/amd.md) |
| 改进 | 客户端前后端 | 不支持的分辨率不阻断颜色应用，导入时剔除不支持的分辨率/刷新率 | icc.rs、ImportShareModal.tsx | [ICC](./api/icc.md) |
| 新功能 | 客户端前后端 | 三场景本地渲染、缓存和切换；优化切换组件位置与图片稳定性 | builtin_preview.rs、builtinPreview.ts、BuiltinPreviewPicker.tsx、SegmentedTabs.tsx | [公告草稿](./releases/v0.6.2-announcement.md) |
| 修复 | 官网 | 无登录分支缺少主分支已部署的 /s 构建钩子/Edge 路由；恢复静态壳与元数据注入，场景图片作为无封面时的预览卡片 | copyShareLanding.mjs、share-shell.html、edge-functions/s/[[default]].js | [发布计划](./releases/v0.6.2-release-plan.md) |
| 修复 | 配套 API | 投稿 JSON 的 previewScene 未接收/保存；补齐投稿及条目列、所有读取、审核通过新建/既有条目保留逻辑，空字段兼容旧客户端 | api/market.go、market/model.go、market/repo.go | API 仓库 docs/submission-preview-scene.md |
| 修复 | 管理后台 | users.ts 换行被 NUL 替换及测试断句导致构建失败；还原换行和断言，保留数字账号 ID 及原有布局调整 | users.ts、users/index.test.tsx | [发布计划](./releases/v0.6.2-release-plan.md) |
| 改进 | 管理后台 | 审核及条目列表接收场景，展示场景名称；无上传封面时使用本地场景原图 | services/icc.ts、SubmissionReview.tsx、icc/index.tsx、public/game-previews | API 仓库 docs/submission-preview-scene.md |

用户最终报告：13 次方案点击，14 次跳过相同 ICC 关联；24 次曲线保持检查，0 次补写；21 次延迟回读偏差均为 0，0 次颜色操作错误，用户确认已恢复正常。此证据来自标准后端；NVIDIA 实验模式默认开关仍关闭，不宣称全部硬件已验证。

客户端不引入登录命令、凭据依赖或账号界面。API 现有账号功能可先于客户端上线；本轮不改账号迁移，不再次执行线上迁移。旧账号/管理员分支的功能已整合到 API main，本轮修复基于该 main，不重复合并旧分支。

验证结果将在发布准备完成后记录于 [发布计划](./releases/v0.6.2-release-plan.md)。

### 开发中 — 2026-11-14 · 无登录颜色修复分支

基线为 v0.6.1 登录接入前的 `b2afd7f`，移植分辨率兼容、分享导入分辨率预检与监听颜色有限恢复，并提取后续修复。

| 功能 | 说明 | 文档 |
|---|---|---|
| NVIDIA ICC 后端 | 本机后端选择、能力查询、严格远程开关、输出身份及所有权恢复 | [icc.md](./api/icc.md) |
| 自动监听颜色恢复 | 所有权快照、事件重开检查窗口、总预算与手动优先 | [icc.md](./api/icc.md) |
| AMD 饱和度错屏修复 | PnP 选择活动监视器，跳过残留旧设备 | [amd.md](./api/amd.md) |

本分支不引入客户端登录、凭据依赖或内置效果图功能。整合后的构建与模拟测试结果由主 Agent 记录；真实 NVIDIA 输出效果需相应硬件验证。
验证：Windows cargo check --all-targets --locked 无错误、无警告；cargo test --lib --locked 82 通过，1 个真机测试默认忽略；显式 AMD 只读探针 1 通过，识别活动 Q27G41ZDE 并读到饱和度 0–200（默认/当前 100）。pnpm run build 通过（保留既有 Browserslist 与包体积提示）。未验证 NVIDIA 真机调色效果，未发布安装包。

### 开发中 — 2026-11-14 · 手动方案切换排查与系统校准开关修正

用户报告中 13 次手动方案点击均有执行，最终参数一致，系统即时回读没有报错；视频显示方案滑块/选中状态切换，但不能据此断言画面已采用这些参数。旧路径每次方案以四次 LUT 写入逐项修改，并先复位 DVC 再写目标值；当前分支已统一应用。新查出无 VCGT 的 ICC 路径重复关闭再开启系统校准管理开关，现改为当前滑块曲线的同步应用。新增仅调试会话启用的方案应用后延迟回读，以区分后续系统曲线覆盖与系统回读一致但画面无效。旧高级颜色位不能单独证明 HDR。

| 改动 | 说明 | 文档 |
|---|---|---|
| 无 VCGT ICC | 同步移除旧基底、保留滑块，失败回滚 | [icc.md](./api/icc.md) |
| 延迟回读 | 250/1000ms，当前会话与代号限制，只读 | [debug.md](./api/debug.md) |

用户笔记本的真实屏幕回归仍需要新构建，不能把即时/延迟回读一致宣称为画面验证。

验证于 2026-11-15：cargo test --lib --locked 86 通过、1 个 AMD 真机只读测试默认忽略；cargo check --all-targets --locked 无错误与编译警告；pnpm exec tauri build --no-bundle 成功，生成包含本轮修正与延迟诊断的测试 EXE。用户笔记本画面效果仍待复测。

### 开发中 — 2026-11-15 · 手动方案曲线短时重置修正

用户新日志三次确认：最终中间调从 168 在约 250ms 内回到 128，操作代号不变；HDR 关闭，SDR 高级颜色开启。修正标准方案路径重复关联同一个 ICC、写旧曲线和中途 DVC 复位；改为实时关联查询、按需更新，目标 DVC 后写最终曲线。手动方案增加 250/750ms 两次有限偏差补写，门闩内复核操作代号、稳定身份和后端；滑块/新方案操作取消旧任务，读取失败不猜测。登录仍不包含在此分支。

| 改动 | 说明 | 文档 |
|---|---|---|
| ICC 关联和准备 | 相同有效关联不重设，准备不写曲线 | [icc.md](./api/icc.md) |
| 手动方案短时补写 | 偏差大于 2 才补写，最大两次，新操作取消 | [icc.md](./api/icc.md) |
| 调试日志 | 区分关联变化、曲线保持和有限补写 | [debug.md](./api/debug.md) |

验证：cargo test --lib --locked 94 通过、1 个 AMD 真机只读测试默认忽略；cargo check --all-targets --locked 无错误与 Rust 编译警告；pnpm exec tauri build --no-bundle 成功，前端 tsc/Vite 与 Windows release EXE 构建完成。新增关联复用/外部变更/查询失败与补写取消/量化/读写失败回归验证；用户实际屏幕效果仍待 v2 测试包复测。

### 开发中 — 2026-11-15 · ICC 查询兼容与单向闪动排查

用户确认 v2 方案可以持续应用，但自定义方案仍单向闪动。新日志五次均为曲线被重置后在 250ms 补写，后续保持；默认曲线接近中性所以变化不明显。关联查询每次都失败，重复注册没有被跳过。修正 ICC 默认配置子类型为 CPST_NONE，WCS 无默认记录时从实际目标 GDI 输出读取当前 ICC，按实时结果决定是否注册；诊断区分实际输出配置与旧桌面注册表缺省标签。保留有限补写和现有 AMD/监听修复，继续无登录。

| 改动 | 说明 | 文档 |
|---|---|---|
| ICC 默认查询 | 正确子类型，缺少 WCS 记录时查询目标 DC | [icc.md](./api/icc.md) |
| ICC 诊断 | 实际输出文件路径，旧注册表标签不再冒充当前配置 | [debug.md](./api/debug.md) |

验证：cargo test --lib --locked 98 通过、2 个真机测试默认忽略；显式只读 live_primary_icc_query_probe 1 通过，本机读取到系统 sRGB，重复查询同一有效配置不触发 setter。cargo check --all-targets --locked 无错误与 Rust 警告；pnpm exec tauri build --no-bundle 成功，前端及 Windows release EXE 构建完成。真实用户笔记本闪动效果待 v3 复测。
