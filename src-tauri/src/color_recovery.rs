//! 监听方案颜色恢复策略（纯逻辑，无 Win32 调用）。
//!
//! 背景见 `.docs/plans/2026-11-06-process-color-recovery.md`：游戏进程触发方案后，
//! 游戏初始化、显示模式切换或其他程序可能重写显卡 gamma 曲线。本模块承载恢复
//! 的全部策略决策——期限、冷却、预算、激活序号、高字节比较——真实的回读与写入
//! 由 process_watcher 的调度 tick 执行；测试用可控时间与假 read/write 驱动状态机，
//! 不真实 sleep、不接触桌面。
//!
//! 关键边界（与计划一致）：
//! - 回读一致只说明 OS 层一致，不能证明硬件扫描输出持续采用该曲线，所以事件后
//!   允许一次有预算的重写，而不是只依赖缓存比较；
//! - 恢复只写缓存的最终 LUT：不重跑整组方案、不切分辨率、不重新注册 ICC、
//!   不重复 DVC；
//! - 预算耗尽即停手，不得持续重应用压过游戏或用户。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ─── 常量与恢复窗口（Task 3）──────────────────────────────────────────────────

/// 回读节流：每秒最多一次
pub(crate) const VERIFY_INTERVAL: Duration = Duration::from_secs(1);
/// 激活成功后的启动窗口（覆盖原报告约 80 秒的游戏启动过程）
pub(crate) const STARTUP_WINDOW: Duration = Duration::from_secs(120);
/// 颜色环境事件（前台 / 显示模式）重新开启的检查窗口
pub(crate) const EVENT_WINDOW: Duration = Duration::from_secs(10);
/// 事件后重写前的稳定窗口（跳过连发风暴的中间态）
pub(crate) const EVENT_SETTLE: Duration = Duration::from_millis(800);
/// 两次恢复写入之间的最小间隔
pub(crate) const WRITE_COOLDOWN: Duration = Duration::from_secs(5);
/// 每次激活最多额外写入次数（成功或失败的尝试均计预算）
pub(crate) const MAX_RECOVERY_WRITES: u8 = 3;

/// 运行时高字节差超过该值才判定为可恢复的偏离；一致或仅 1 级量化差不因比较触发写入
pub(crate) const DIFF_TOLERANCE: u8 = 1;

/// 一次激活的有限恢复窗口。所有字段由调度 tick 依据 `Instant`（单调时钟）推进，
/// 测试以合成时间点驱动，避免真实 sleep。
#[derive(Debug)]
pub(crate) struct RecoveryWindow {
    /// 到期后休眠但保留所有权；新事件开窗，累计预算与冷却不重置。
    active: bool,
    pub(crate) deadline: Instant,
    pub(crate) next_check: Instant,
    pub(crate) last_write: Option<Instant>,
    pub(crate) writes_used: u8,
    pub(crate) reassert_after_event: bool,
}

// ─── 恢复所有权（Task 2）──────────────────────────────────────────────────────

/// 一次规则激活的恢复所有权。只有它与 `WatcherState.applied` 的当前方案一致时，
/// 恢复调度才被允许写屏；active_rules 中已被后来方案盖住的规则不在保护范围。
#[derive(Debug, Clone)]
pub(crate) struct RecoveryOwnership {
    /// 捕获时的写入后端；B 的事件恢复不得写入传统 GDI LUT。
    pub(crate) backend: crate::color_backend::Backend,
    pub(crate) rule_id: String,
    /// 全局递增的激活序号：每次激活唯一，旧激活无法冒充
    pub(crate) activation_seq: u64,
    /// 实际目标显示器的 GDI 设备名（\\.\DISPLAYn）
    pub(crate) device_id: String,
    /// 激活时该设备的 PnP 身份；显示器重新连接且设备名复用时用它拒绝旧身份续写
    pub(crate) pnp_id: String,
    /// 方案整体应用结束后的最终代号（APPLY_GENERATION）
    pub(crate) generation: u64,
    /// 最终合成的 768 项 LUT（含 ICC vcgt 与 RGB 增益）
    pub(crate) expected_lut: [u16; 768],
}

static ACTIVATION_SEQ: AtomicU64 = AtomicU64::new(0);

/// 每次规则激活分配一个唯一序号（从 1 开始）
pub(crate) fn next_activation_seq() -> u64 {
    ACTIVATION_SEQ.fetch_add(1, Ordering::SeqCst) + 1
}

// ─── 调度决策 ─────────────────────────────────────────────────────────────────

/// 恢复写入的触发原因（用于日志区分）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WriteReason {
    /// 回读发现高字节差超限（可恢复的偏离）
    DeviationDetected,
    /// 颜色环境事件（前台 / 显示模式变化）后的重 assert
    EventReassert,
}

impl WriteReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            WriteReason::DeviationDetected => "deviation detected",
            WriteReason::EventReassert => "event reassert",
        }
    }
}

/// 窗口关闭原因（预算耗尽 / 到期时记一次日志用）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseReason {
    /// 启动窗口或事件窗口到期
    WindowExpired,
    /// 3 次写入预算耗尽
    BudgetExhausted,
}

impl CloseReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            CloseReason::WindowExpired => "window expired",
            CloseReason::BudgetExhausted => "budget exhausted (3 writes used)",
        }
    }
}

/// 调度 tick 询问"现在该做什么"的回答
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryRequest {
    /// 未到检查时间（next_check 已在窗口内部维护）
    Wait,
    /// 执行一次只读回读，结果交回 finish_verify
    Verify,
    /// 待办的重写现在可以执行（800ms 安定期与写入冷却均已满足）
    Write,
    /// 窗口应关闭（到期仅休眠；预算耗尽结束激活；调度器记录原因）
    Close(CloseReason),
}

/// 一次只读回读完成后的后续动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerifyFollowup {
    /// 只读：一致 / 仅量化差 / 读取失败，等下一次检查
    None,
    /// 立即重写（冷却已过、预算有余）
    WriteNow(WriteReason),
    /// 检测到偏离但冷却未过：置待办并顺延到冷却结束，届时由 Write 请求执行
    WriteAfterCooldown,
    /// 窗口应关闭
    CloseWindow(CloseReason),
}

impl RecoveryWindow {
    /// 激活成功后开启启动窗口：120 秒期限，1 秒后第一次回读
    pub(crate) fn open_startup(now: Instant) -> Self {
        Self {
            active: true,
            deadline: now + STARTUP_WINDOW,
            next_check: now + VERIFY_INTERVAL,
            last_write: None,
            writes_used: 0,
            reassert_after_event: false,
        }
    }

    /// 颜色环境事件到达（目标进程前台 / 显示模式变化）：合并重写待办，把窗口
    /// 延长到至少 `now + EVENT_WINDOW`，安定 800ms 后再检查。
    /// 事件风暴自然合并：待办只有一个布尔位，安定时间取最后一个事件；
    /// 总预算不因事件重置。
    pub(crate) fn note_event(&mut self, now: Instant) {
        if self.writes_used >= MAX_RECOVERY_WRITES {
            return; // 事件不能复活已耗尽预算的激活
        }
        self.active = true;
        if now + EVENT_WINDOW > self.deadline {
            self.deadline = now + EVENT_WINDOW;
        }
        self.reassert_after_event = true;
        self.next_check = now + EVENT_SETTLE;
    }

    fn cooldown_elapsed(&self, now: Instant) -> bool {
        self.last_write.is_none_or(|t| now >= t + WRITE_COOLDOWN)
    }

    /// 冷却结束时刻（仅在 last_write 已存在时调用路径才可达）
    fn cooldown_end(&self) -> Instant {
        self.last_write
            .expect("cooldown_end requires a previous write")
            + WRITE_COOLDOWN
    }

    /// 调度 tick 询问：现在该做什么？除 Wait 外的请求都应在调用方完成对应动作
    /// （回读 / 写入 / 关窗）后再推进窗口状态。
    pub(crate) fn due_request(&mut self, now: Instant) -> RecoveryRequest {
        if self.writes_used >= MAX_RECOVERY_WRITES {
            return RecoveryRequest::Close(CloseReason::BudgetExhausted);
        }
        if !self.active {
            return RecoveryRequest::Wait;
        }
        if now >= self.deadline {
            self.active = false;
            self.reassert_after_event = false;
            return RecoveryRequest::Close(CloseReason::WindowExpired);
        }
        if now < self.next_check {
            return RecoveryRequest::Wait;
        }
        if self.reassert_after_event {
            if !self.cooldown_elapsed(now) {
                // 冷却未过：顺延到冷却结束；届时若已过窗口期限会先按到期关闭
                self.next_check = self.cooldown_end();
                return RecoveryRequest::Wait;
            }
            return RecoveryRequest::Write;
        }
        RecoveryRequest::Verify
    }

    /// 一次只读回读完成。`diff` = 期望 LUT 与实际回读的高字节最大差；
    /// `None` = 读取失败（仅记录并等待后续检查，不伪称覆盖、不直接写入）。
    pub(crate) fn finish_verify(&mut self, now: Instant, diff: Option<u8>) -> VerifyFollowup {
        match diff {
            None => {
                self.next_check = now + VERIFY_INTERVAL;
                VerifyFollowup::None
            }
            Some(d) if d <= DIFF_TOLERANCE => {
                // 一致或仅 1 级量化差：只读，不因比较触发写入
                self.next_check = now + VERIFY_INTERVAL;
                VerifyFollowup::None
            }
            Some(_) => {
                if self.writes_used >= MAX_RECOVERY_WRITES {
                    return VerifyFollowup::CloseWindow(CloseReason::BudgetExhausted);
                }
                if !self.cooldown_elapsed(now) {
                    self.reassert_after_event = true;
                    self.next_check = self.cooldown_end();
                    return VerifyFollowup::WriteAfterCooldown;
                }
                VerifyFollowup::WriteNow(WriteReason::DeviationDetected)
            }
        }
    }

    /// 一次写入尝试完成（成功或失败都计预算）。返回 true 表示预算耗尽，
    /// 调度器应结束该次激活的自动恢复并记录原因。
    pub(crate) fn finish_write(&mut self, now: Instant) -> bool {
        self.writes_used += 1;
        self.last_write = Some(now);
        self.reassert_after_event = false;
        self.next_check = now + VERIFY_INTERVAL;
        self.writes_used >= MAX_RECOVERY_WRITES
    }
}

// ─── 曲线比较（Task 1）────────────────────────────────────────────────────────

/// 期望与实际 gamma ramp 的高字节最大差（0..=255）。
/// 只比高字节：低字节是 16 位标度的量化噪声，不影响 8 位级视觉效果。
pub(crate) fn ramp_high_byte_diff(expected: &[u16; 768], actual: &[u16; 768]) -> u8 {
    expected
        .iter()
        .zip(actual)
        .map(|(e, a)| ((e >> 8) as u8).abs_diff((a >> 8) as u8))
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[test]
    fn detects_a_reset_but_ignores_low_byte_quantization() {
        let expected = [100u16 << 8; 768];
        let mut actual = expected;
        actual[128] += 255;
        assert_eq!(ramp_high_byte_diff(&expected, &actual), 0);
        actual[128] = 128u16 << 8;
        assert_eq!(ramp_high_byte_diff(&expected, &actual), 28);
    }
}

// ─── 调度场景测试（可控时间，不真实 sleep）────────────────────────────────────

#[cfg(test)]
mod scheduler_tests {
    use super::*;

    /// 场景：激活后每秒回读；80 秒时被外部重置 → 检出偏离并在预算内重写一次
    #[test]
    fn reset_at_80s_is_detected_and_rewritten() {
        let t0 = Instant::now();
        let mut win = RecoveryWindow::open_startup(t0);
        let mut t = t0 + VERIFY_INTERVAL;
        // 前 79 秒回读一致，只读
        while t < t0 + Duration::from_secs(80) {
            assert_eq!(win.due_request(t), RecoveryRequest::Verify);
            assert_eq!(win.finish_verify(t, Some(0)), VerifyFollowup::None);
            t += VERIFY_INTERVAL;
        }
        // 80 秒：曲线被写回 identity → 高字节差超限 → 立即重写
        assert_eq!(win.due_request(t), RecoveryRequest::Verify);
        assert_eq!(
            win.finish_verify(t, Some(40)),
            VerifyFollowup::WriteNow(WriteReason::DeviationDetected)
        );
        assert!(!win.finish_write(t), "第一次写入后预算未耗尽");
        assert_eq!(win.writes_used, 1);
        assert_eq!(win.last_write, Some(t));
        // 写入后回到一致
        assert_eq!(
            win.due_request(t + VERIFY_INTERVAL),
            RecoveryRequest::Verify
        );
        assert_eq!(
            win.finish_verify(t + VERIFY_INTERVAL, Some(0)),
            VerifyFollowup::None
        );
    }

    /// 场景：120 秒到期只关闭一次；之后休眠，不重复关闭或轮询
    #[test]
    fn window_stops_after_120s() {
        let t0 = Instant::now();
        let mut win = RecoveryWindow::open_startup(t0);
        // 到期前的最后一秒仍可回读
        let t = t0 + STARTUP_WINDOW - VERIFY_INTERVAL;
        assert_eq!(win.due_request(t), RecoveryRequest::Verify);
        assert_eq!(win.finish_verify(t, Some(0)), VerifyFollowup::None);
        assert_eq!(
            win.due_request(t0 + STARTUP_WINDOW),
            RecoveryRequest::Close(CloseReason::WindowExpired)
        );
        assert_eq!(
            win.due_request(t0 + STARTUP_WINDOW + Duration::from_secs(5)),
            RecoveryRequest::Wait
        );
    }

    /// 场景：两次写入至少间隔 WRITE_COOLDOWN；冷却期间检出偏离只置待办
    #[test]
    fn writes_are_cooldown_gated() {
        let t0 = Instant::now();
        let mut win = RecoveryWindow::open_startup(t0);
        let t1 = t0 + Duration::from_secs(2);
        assert_eq!(
            win.finish_verify(t1, Some(30)),
            VerifyFollowup::WriteNow(WriteReason::DeviationDetected)
        );
        assert!(!win.finish_write(t1));
        // 写入 1 秒后再次偏离：冷却未过 → 置待办并顺延到冷却结束，不立即写
        let t2 = t1 + VERIFY_INTERVAL;
        assert_eq!(
            win.finish_verify(t2, Some(30)),
            VerifyFollowup::WriteAfterCooldown
        );
        assert!(win.reassert_after_event);
        assert_eq!(
            win.due_request(t2 + Duration::from_secs(1)),
            RecoveryRequest::Wait,
            "冷却结束前不得写"
        );
        let t3 = t1 + WRITE_COOLDOWN;
        assert_eq!(
            win.due_request(t3),
            RecoveryRequest::Write,
            "冷却结束后待办重写可执行"
        );
        assert!(!win.finish_write(t3));
        assert_eq!(win.writes_used, 2);
    }

    /// 场景：第三次写入后预算耗尽，停止该次激活的自动写入并给出原因
    #[test]
    fn budget_exhausts_after_three_writes() {
        let t0 = Instant::now();
        let mut win = RecoveryWindow::open_startup(t0);
        let mut t = t0 + VERIFY_INTERVAL;
        for i in 0..3 {
            assert_eq!(win.due_request(t), RecoveryRequest::Verify);
            assert_eq!(
                win.finish_verify(t, Some(50)),
                VerifyFollowup::WriteNow(WriteReason::DeviationDetected)
            );
            // 真实 tick 在检出偏离后立即写；成功或失败都计预算
            assert_eq!(win.finish_write(t), i == 2, "第 3 次写入耗尽预算");
            // 推进到冷却结束之后 + 一个回读间隔
            t += WRITE_COOLDOWN + VERIFY_INTERVAL;
        }
        assert_eq!(win.writes_used, 3);
        assert_eq!(
            win.finish_verify(t, Some(50)),
            VerifyFollowup::CloseWindow(CloseReason::BudgetExhausted),
            "预算耗尽后继续偏离只关窗，不再写"
        );
        assert_eq!(
            win.due_request(t),
            RecoveryRequest::Close(CloseReason::BudgetExhausted)
        );
    }

    /// 场景：读取失败只记录并等待后续检查，不伪称覆盖、不直接写入
    #[test]
    fn read_failure_never_triggers_a_write() {
        let t0 = Instant::now();
        let mut win = RecoveryWindow::open_startup(t0);
        let t = t0 + VERIFY_INTERVAL;
        assert_eq!(win.due_request(t), RecoveryRequest::Verify);
        assert_eq!(win.finish_verify(t, None), VerifyFollowup::None);
        assert_eq!(win.writes_used, 0);
        assert_eq!(
            win.next_check,
            t + VERIFY_INTERVAL,
            "读取失败后等下一轮检查"
        );
    }

    /// 场景：仅 1 级量化差不触发写入（DIFF_TOLERANCE 边界）
    #[test]
    fn quantization_noise_does_not_trigger_write() {
        let t0 = Instant::now();
        let mut win = RecoveryWindow::open_startup(t0);
        let t = t0 + VERIFY_INTERVAL;
        assert_eq!(win.due_request(t), RecoveryRequest::Verify);
        assert_eq!(
            win.finish_verify(t, Some(DIFF_TOLERANCE)),
            VerifyFollowup::None
        );
        assert_eq!(win.writes_used, 0);
    }

    /// 场景：事件风暴合并——连发多条事件只安排一次安定后重写，且不重置总预算
    #[test]
    fn event_storms_merge_into_one_reassert() {
        let t0 = Instant::now();
        let mut win = RecoveryWindow::open_startup(t0);
        let mut t = t0 + Duration::from_millis(200);
        // 第一场 5 连发风暴（如 Win+P 过程 / Alt+Tab 连切）
        for _ in 0..5 {
            win.note_event(t);
            t += Duration::from_millis(100);
        }
        // 安定期内（最后一个事件 + 800ms 之前）不得写
        assert_eq!(win.due_request(t), RecoveryRequest::Wait);
        let settled = t + EVENT_SETTLE;
        assert_eq!(win.due_request(settled), RecoveryRequest::Write);
        assert!(!win.finish_write(settled), "风暴合并为一次写入");
        assert_eq!(win.writes_used, 1);
        assert!(!win.reassert_after_event);
        // 第二场风暴（拉开冷却间隔）：预算不重置，总共只会再写一次
        let t2 = settled + WRITE_COOLDOWN + Duration::from_secs(1);
        win.note_event(t2);
        win.note_event(t2 + Duration::from_millis(100));
        let settled2 = t2 + Duration::from_millis(100) + EVENT_SETTLE;
        assert_eq!(win.due_request(settled2), RecoveryRequest::Write);
        assert!(!win.finish_write(settled2));
        assert_eq!(win.writes_used, 2, "事件不重置总预算");
    }

    /// 场景：事件窗口把期限延长；此后无事件则按延长后的期限到期关闭
    #[test]
    fn event_extends_deadline_by_event_window() {
        let t0 = Instant::now();
        let mut win = RecoveryWindow::open_startup(t0);
        // 启动窗口 120 秒；110 秒处来一次模式变化事件
        let t_event = t0 + Duration::from_secs(110);
        win.note_event(t_event);
        // 原期限（120s）已被事件窗口（110+10=120s）顶齐，再晚 1 秒的事件顶到 121s
        let t_event2 = t0 + Duration::from_secs(111);
        win.note_event(t_event2);
        // 安定后执行事件重写（回读即使一致也要重 assert 一次）
        let settled = t_event2 + EVENT_SETTLE;
        assert_eq!(win.due_request(settled), RecoveryRequest::Write);
        assert!(!win.finish_write(settled));
        // 120s 原期限已到，但事件窗口把它顶到了 121s —— 仍活跃（next_check 已过，
        // 允许回读；若回读一致则只读等待，到 121s 才关闭）
        assert_eq!(
            win.due_request(t0 + Duration::from_secs(120)),
            RecoveryRequest::Verify,
            "事件窗口延长了期限，120s 处仍活跃"
        );
        assert_eq!(
            win.due_request(t0 + Duration::from_secs(121) + Duration::from_millis(1)),
            RecoveryRequest::Close(CloseReason::WindowExpired)
        );
    }
}

/// 用假 read/write 驱动完整状态机，复现真实观察：规则激活后先回读一致，
/// 约 80 秒时被外部写回 identity ramp，调度器检出偏离并自动恢复（Task 5 的
/// 模拟验证）。真实 I/O 在 process_watcher 的 tick 中，这里驱动同一组纯决策函数。
#[cfg(test)]
mod simulated_flow_tests {
    use super::*;

    #[test]
    fn match_then_reset_at_80s_then_auto_recovery() {
        let t0 = Instant::now();
        let mut win = RecoveryWindow::open_startup(t0);

        // 假 read：80 秒处被覆盖一次（高字节差 255），其余时间一致
        // 假 write：记录写入时刻；写入后曲线恢复一致
        let mut writes: Vec<Instant> = Vec::new();
        let mut covered = false;
        let mut t = t0 + VERIFY_INTERVAL;

        while t < t0 + STARTUP_WINDOW {
            // 外部覆盖发生在激活后 80 秒（仅一次）
            if t == t0 + Duration::from_secs(80) {
                covered = true;
            }
            match win.due_request(t) {
                RecoveryRequest::Wait => {}
                RecoveryRequest::Verify => {
                    let diff = if covered { 255u8 } else { 0u8 };
                    if let VerifyFollowup::WriteNow(_) = win.finish_verify(t, Some(diff)) {
                        writes.push(t);
                        covered = false; // 写入后回读恢复一致
                        win.finish_write(t);
                    }
                }
                RecoveryRequest::Write => {
                    writes.push(t);
                    covered = false;
                    win.finish_write(t);
                }
                RecoveryRequest::Close(_) => break,
            }
            t += VERIFY_INTERVAL;
        }

        assert_eq!(writes.len(), 1, "80 秒处的覆盖应在预算内恢复一次");
        assert_eq!(
            writes[0] - t0,
            Duration::from_secs(80),
            "覆盖发生在约 80 秒时被检出"
        );
        assert_eq!(win.writes_used, 1);
        // 窗口继续运行直到 120 秒到期关闭
        assert_eq!(
            win.due_request(t0 + STARTUP_WINDOW),
            RecoveryRequest::Close(CloseReason::WindowExpired)
        );
    }

    #[test]
    fn expired_window_rearms_and_keeps_budget_and_cooldown() {
        let t0 = Instant::now();
        let mut win = RecoveryWindow::open_startup(t0);
        let last = t0 + STARTUP_WINDOW - Duration::from_secs(1);
        assert!(!win.finish_write(last));
        let expired = t0 + STARTUP_WINDOW;
        assert_eq!(
            win.due_request(expired),
            RecoveryRequest::Close(CloseReason::WindowExpired)
        );
        assert_eq!(
            win.due_request(expired + VERIFY_INTERVAL),
            RecoveryRequest::Wait
        );
        let event = expired + Duration::from_secs(2);
        win.note_event(event);
        assert_eq!(win.writes_used, 1);
        assert_eq!(win.last_write, Some(last));
        assert_eq!(
            win.due_request(event + EVENT_SETTLE),
            RecoveryRequest::Wait,
            "沿用上一窗口的冷却"
        );
        assert_eq!(
            win.due_request(last + WRITE_COOLDOWN),
            RecoveryRequest::Write
        );
        assert!(!win.finish_write(last + WRITE_COOLDOWN));
        assert_eq!(win.writes_used, 2);
    }
}
