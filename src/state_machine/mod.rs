// Copyright 2026 秋毫mem Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// ============================================================================
// 连续监控状态机（Phase 2 核心）
//
// 三态模型：Normal ↔ Warning ↔ Critical
// 双条件转换（CR-02 架构师要求）：时间阈值 + 最少采集次数同时满足
// 紧急快速通道（CR-08）：available < 512MB || used > 98% → 立即 Critical
// ============================================================================

pub mod emergency;

use std::time::{Duration, Instant};

use crate::collector::{MemoryMetrics, PressureLevel};

// ============================================================================
// 状态与转换定义
// ============================================================================

/// 三态模型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MonitorState {
    Normal,
    Warning,
    Critical,
}

impl std::fmt::Display for MonitorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MonitorState::Normal => write!(f, "normal"),
            MonitorState::Warning => write!(f, "warning"),
            MonitorState::Critical => write!(f, "critical"),
        }
    }
}

/// 状态转换事件（决定输出 action）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StateTransition {
    /// 状态未变化，沿用上一次 action
    None,
    /// Normal → Warning
    EnterWarning,
    /// Warning → Critical（含紧急跃迁）
    EnterCritical,
    /// Warning → Normal
    RecoverToNormal,
    /// Critical → Warning
    RecoverToWarning,
    /// 紧急跃迁（立即 Critical，绕过时间窗口）
    Emergency,
}

/// 状态转换对应的 Agent action
impl StateTransition {
    pub fn action(&self) -> &'static str {
        match self {
            StateTransition::None => "no_change",
            StateTransition::EnterWarning => "monitor",
            StateTransition::EnterCritical => "abort_safely",
            StateTransition::RecoverToNormal => "ok",
            StateTransition::RecoverToWarning => "reduce_context",
            StateTransition::Emergency => "abort_safely",
        }
    }
}

// ============================================================================
// 状态机配置
// ============================================================================

/// 状态机配置（全部可调，默认值对应需求文档）
#[derive(Debug, Clone)]
pub struct StateMachineConfig {
    /// Normal → Warning 的最少持续秒数（默认 10）
    pub warning_seconds: u64,
    /// Warning → Critical 的最少持续秒数（默认 15）
    pub critical_seconds: u64,
    /// 恢复路径的最少持续秒数（默认 10）
    pub recovery_seconds: u64,

    /// Normal → Warning 的最少连续高压采样次数（CR-02，默认 3）
    pub min_samples_warning: u32,
    /// Warning → Critical 的最少连续 critical 采样次数（默认 5）
    pub min_samples_critical: u32,
    /// 恢复路径的最少连续低压采样次数（默认 3）
    pub min_samples_recovery: u32,

    /// 紧急触发：可用内存低于此值则立即 Critical（CR-08，默认 512 MB）
    pub emergency_available_mb: u64,
    /// 紧急触发：已用百分比高于此值则立即 Critical（CR-08，默认 98.0%）
    pub emergency_used_percent: f64,
}

impl Default for StateMachineConfig {
    fn default() -> Self {
        Self {
            warning_seconds: 10,
            critical_seconds: 15,
            recovery_seconds: 10,
            min_samples_warning: 3,
            min_samples_critical: 5,
            min_samples_recovery: 3,
            emergency_available_mb: 512,
            emergency_used_percent: 98.0,
        }
    }
}

// ============================================================================
// 状态机引擎
// ============================================================================

/// 连续监控状态机
///
/// # 工作原理
/// 1. 每次采集后调用 `update(metrics, now)`。
/// 2. 先检查紧急通道（CR-08）：可用内存 < 512MB 或已用 > 98% → 立即 Critical。
/// 3. 再检查正常转换路径，需要**同时**满足时间阈值和最少采集次数。
/// 4. 冷启动（首次采集）时，若 pressure ≥ High 则直接进入 Warning。
/// 5. 每轮 `--interval` / `--count` 开始时调用 `reset()` 归零。
#[derive(Debug, Clone)]
pub struct StateMachine {
    /// 当前状态
    state: MonitorState,
    /// 进入当前状态的时间
    state_entered_at: Instant,
    /// 是否已完成过首次采集（冷启动检测用）
    first_sample_taken: bool,
    /// 连续高压（High/Critical）采样计数器
    consecutive_high: u32,
    /// 连续 Critical 采样计数器
    consecutive_critical: u32,
    /// 连续低压（Low/Medium）采样计数器
    consecutive_ok: u32,
    /// 配置
    config: StateMachineConfig,
}

impl StateMachine {
    /// 创建新状态机，初始状态为 Normal，计时从当前时刻开始
    pub fn new(config: StateMachineConfig) -> Self {
        Self {
            state: MonitorState::Normal,
            state_entered_at: Instant::now(),
            first_sample_taken: false,
            consecutive_high: 0,
            consecutive_critical: 0,
            consecutive_ok: 0,
            config,
        }
    }

    /// 获取当前状态
    #[allow(dead_code)]
    pub fn current_state(&self) -> MonitorState {
        self.state
    }

    /// 重置状态机（每轮 --interval / --count 开始时调用）
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.state = MonitorState::Normal;
        self.state_entered_at = Instant::now();
        self.first_sample_taken = false;
        self.consecutive_high = 0;
        self.consecutive_critical = 0;
        self.consecutive_ok = 0;
    }

    /// 更新状态机，传入当前内存指标和时刻
    ///
    /// 返回 `StateTransition`，调用方根据该值决定输出 action。
    pub fn update(&mut self, metrics: &MemoryMetrics, now: Instant) -> StateTransition {
        // === 第 1 步：紧急快速通道（CR-08）===
        // 在一切之前检查，确保紧急状态不被延时条件阻塞
        if emergency::is_emergency(
            metrics,
            self.config.emergency_available_mb,
            self.config.emergency_used_percent,
        ) {
            return self.force_emergency(now);
        }

        // === 第 2 步：冷启动首次采集 ===
        // 首次采集时若 pressure ≥ High，立即进入 Warning（不等待连续计数）
        if !self.first_sample_taken {
            self.first_sample_taken = true;
            let pressure = &metrics.pressure;
            if *pressure == PressureLevel::High || *pressure == PressureLevel::Critical {
                return self.transition_to(
                    MonitorState::Warning,
                    now,
                    StateTransition::EnterWarning,
                );
            }
            // 首次采集状态正常，保持 Normal
            return StateTransition::None;
        }

        // === 第 3 步：正常状态转换 ===
        let duration = now.duration_since(self.state_entered_at);
        let pressure = &metrics.pressure;

        match self.state {
            MonitorState::Normal => {
                match pressure {
                    PressureLevel::High | PressureLevel::Critical => {
                        self.consecutive_high += 1;
                        self.consecutive_ok = 0;
                        if self.check_transition(
                            duration,
                            self.config.warning_seconds,
                            self.consecutive_high,
                            self.config.min_samples_warning,
                        ) {
                            self.transition_to(
                                MonitorState::Warning,
                                now,
                                StateTransition::EnterWarning,
                            )
                        } else {
                            StateTransition::None
                        }
                    }
                    _ => {
                        // 压力恢复正常时重置高压计数
                        self.consecutive_high = 0;
                        StateTransition::None
                    }
                }
            }

            MonitorState::Warning => {
                match pressure {
                    PressureLevel::Critical => {
                        self.consecutive_critical += 1;
                        self.consecutive_ok = 0;
                        if self.check_transition(
                            duration,
                            self.config.critical_seconds,
                            self.consecutive_critical,
                            self.config.min_samples_critical,
                        ) {
                            self.transition_to(
                                MonitorState::Critical,
                                now,
                                StateTransition::EnterCritical,
                            )
                        } else {
                            StateTransition::None
                        }
                    }
                    PressureLevel::Low | PressureLevel::Medium => {
                        self.consecutive_ok += 1;
                        self.consecutive_critical = 0;
                        if self.check_transition(
                            duration,
                            self.config.recovery_seconds,
                            self.consecutive_ok,
                            self.config.min_samples_recovery,
                        ) {
                            self.transition_to(
                                MonitorState::Normal,
                                now,
                                StateTransition::RecoverToNormal,
                            )
                        } else {
                            StateTransition::None
                        }
                    }
                    // High → 停留在 Warning，重置恢复计数
                    PressureLevel::High => {
                        self.consecutive_ok = 0;
                        StateTransition::None
                    }
                }
            }

            MonitorState::Critical => {
                match pressure {
                    PressureLevel::Critical => {
                        // 持续 Critical → 重置恢复计数，保持在 Critical
                        self.consecutive_ok = 0;
                        StateTransition::None
                    }
                    _ => {
                        // 压力回落 < Critical，开始恢复计数
                        self.consecutive_ok += 1;
                        if self.check_transition(
                            duration,
                            self.config.recovery_seconds,
                            self.consecutive_ok,
                            self.config.min_samples_recovery,
                        ) {
                            self.transition_to(
                                MonitorState::Warning,
                                now,
                                StateTransition::RecoverToWarning,
                            )
                        } else {
                            StateTransition::None
                        }
                    }
                }
            }
        }
    }

    // ========================================================================
    // 内部方法
    // ========================================================================

    /// 检查是否满足双条件：经过时间 >= 阈值 && 连续采样次数 >= 最小次数
    fn check_transition(
        &self,
        elapsed: Duration,
        required_seconds: u64,
        consecutive: u32,
        min_samples: u32,
    ) -> bool {
        elapsed >= Duration::from_secs(required_seconds) && consecutive >= min_samples
    }

    /// 执行状态跃迁
    fn transition_to(
        &mut self,
        new_state: MonitorState,
        now: Instant,
        transition: StateTransition,
    ) -> StateTransition {
        self.state = new_state;
        self.state_entered_at = now;
        // 重置所有计数器
        self.consecutive_high = 0;
        self.consecutive_critical = 0;
        self.consecutive_ok = 0;
        transition
    }

    /// 强制紧急跃迁（CR-08）
    /// 绕过时间窗口和连续计数，立即进入 Critical
    fn force_emergency(&mut self, now: Instant) -> StateTransition {
        let previous = self.state;
        self.state = MonitorState::Critical;
        self.state_entered_at = now;
        self.first_sample_taken = true; // 紧急跃迁后无需冷启动
        self.consecutive_high = 0;
        self.consecutive_critical = 0;
        self.consecutive_ok = 0;

        if previous == MonitorState::Critical {
            // 已在 Critical 状态，但紧急条件仍然满足 → 返回 None 避免重复报警
            // 但紧急条件是硬条件，如果上一轮已经是紧急 Critical，这里同上
            StateTransition::None
        } else {
            StateTransition::Emergency
        }
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    fn make_metrics(
        available_mb: u64,
        used_percent: f64,
        pressure: PressureLevel,
    ) -> MemoryMetrics {
        MemoryMetrics {
            total_mb: 16000,
            used_mb: (16000u64.saturating_sub(available_mb)),
            available_mb,
            used_percent,
            pressure,
        }
    }

    // UT-SM-001: 冷启动首次采集正常 → None
    #[test]
    fn test_cold_start_normal() {
        let mut sm = StateMachine::new(StateMachineConfig::default());
        let m = make_metrics(8000, 50.0, PressureLevel::Low);
        let result = sm.update(&m, Instant::now());
        assert_eq!(result, StateTransition::None);
        assert_eq!(sm.current_state(), MonitorState::Normal);
    }

    // UT-SM-002: 冷启动首次采集高压 → 立即 Warning
    #[test]
    fn test_cold_start_high_pressure() {
        let mut sm = StateMachine::new(StateMachineConfig::default());
        let m = make_metrics(3000, 80.0, PressureLevel::High);
        let result = sm.update(&m, Instant::now());
        assert_eq!(result, StateTransition::EnterWarning);
        assert_eq!(sm.current_state(), MonitorState::Warning);
    }

    // UT-SM-003: Normal 持续高压 → 满足时间和次数后进入 Warning
    #[test]
    fn test_normal_to_warning() {
        let config = StateMachineConfig {
            warning_seconds: 0, // 0秒，只靠次数条件
            min_samples_warning: 2,
            ..Default::default()
        };
        let mut sm = StateMachine::new(config);
        let now = Instant::now();

        // 冷启动（正常）
        let m = make_metrics(8000, 50.0, PressureLevel::Low);
        sm.update(&m, now);
        assert_eq!(sm.current_state(), MonitorState::Normal);

        // 第一次高压
        let m = make_metrics(3000, 80.0, PressureLevel::High);
        let r1 = sm.update(&m, now + Duration::from_secs(1));
        assert_eq!(r1, StateTransition::None, "第一次高压应保持 Normal");
        assert_eq!(sm.current_state(), MonitorState::Normal);

        // 第二次高压（满足 min_samples=2）
        let m = make_metrics(2000, 85.0, PressureLevel::High);
        let r2 = sm.update(&m, now + Duration::from_secs(2));
        assert_eq!(
            r2,
            StateTransition::EnterWarning,
            "第二次高压应进入 Warning"
        );
        assert_eq!(sm.current_state(), MonitorState::Warning);
    }

    // UT-SM-004: Warning 持续 Critical → 满足条件后进入 Critical
    #[test]
    fn test_warning_to_critical() {
        let config = StateMachineConfig {
            critical_seconds: 0,
            min_samples_critical: 3,
            ..Default::default()
        };
        let mut sm = StateMachine::new(config);
        let now = Instant::now();

        // 冷启动→Warning
        let m = make_metrics(3000, 80.0, PressureLevel::High);
        sm.update(&m, now);
        assert_eq!(sm.current_state(), MonitorState::Warning);

        // 3 次 Critical
        for i in 0..5 {
            let m = make_metrics(1000, 95.0, PressureLevel::Critical);
            let r = sm.update(&m, now + Duration::from_secs(i + 1));
            if i < 2 {
                assert_eq!(
                    r,
                    StateTransition::None,
                    "第{}次 Critical 应保持 Warning",
                    i + 1
                );
            } else {
                assert_eq!(
                    r,
                    StateTransition::EnterCritical,
                    "第{}次 Critical 应触发",
                    i + 1
                );
                assert_eq!(sm.current_state(), MonitorState::Critical);
                break;
            }
        }
    }

    // UT-SM-005: Warning 持续低压 → 恢复 Normal
    #[test]
    fn test_warning_to_normal() {
        let config = StateMachineConfig {
            recovery_seconds: 0,
            min_samples_recovery: 2,
            ..Default::default()
        };
        let mut sm = StateMachine::new(config);
        let now = Instant::now();

        // 冷启动→Warning
        let m = make_metrics(3000, 80.0, PressureLevel::High);
        sm.update(&m, now);

        // 2 次低压
        for i in 0..3 {
            let m = make_metrics(12000, 30.0, PressureLevel::Low);
            let r = sm.update(&m, now + Duration::from_secs(i + 1));
            if i == 0 {
                assert_eq!(r, StateTransition::None, "第一次低压应保持 Warning");
            } else {
                assert_eq!(
                    r,
                    StateTransition::RecoverToNormal,
                    "第二次低压应恢复 Normal"
                );
                assert_eq!(sm.current_state(), MonitorState::Normal);
                break;
            }
        }
    }

    // UT-SM-006: Critical 状态压力回落 → 恢复 Warning
    #[test]
    fn test_critical_to_warning() {
        let config = StateMachineConfig {
            recovery_seconds: 0,
            min_samples_recovery: 2,
            ..Default::default()
        };
        let mut sm = StateMachine::new(config);
        let now = Instant::now();

        // 直接放入 Critical
        sm.state = MonitorState::Critical;
        sm.state_entered_at = now;
        sm.first_sample_taken = true;

        // 第一次非 Critical（恢复计数 1/2）
        let m = make_metrics(5000, 60.0, PressureLevel::Medium);
        let r1 = sm.update(&m, now + Duration::from_secs(1));
        assert_eq!(r1, StateTransition::None, "第一次恢复应保持 Critical");

        // 第二次非 Critical（恢复计数 2/2 → RecoverToWarning）
        let m = make_metrics(6000, 50.0, PressureLevel::Low);
        let r2 = sm.update(&m, now + Duration::from_secs(2));
        assert_eq!(
            r2,
            StateTransition::RecoverToWarning,
            "第二次应恢复 Warning"
        );
        assert_eq!(sm.current_state(), MonitorState::Warning);
    }

    // UT-SM-007: 紧急通道：可用内存 < 512MB → 立即 Critical
    #[test]
    fn test_emergency_low_available() {
        let mut sm = StateMachine::new(StateMachineConfig::default());
        let now = Instant::now();

        // 冷启动（正常）
        let m = make_metrics(8000, 50.0, PressureLevel::Low);
        sm.update(&m, now);
        assert_eq!(sm.current_state(), MonitorState::Normal);

        // 紧急 → 立即 Critical
        let m = make_metrics(400, 95.0, PressureLevel::Critical);
        let r = sm.update(&m, now + Duration::from_secs(1));
        assert_eq!(r, StateTransition::Emergency, "400MB 应触发紧急跃迁");
        assert_eq!(sm.current_state(), MonitorState::Critical);
    }

    // UT-SM-008: 紧急通道：已用 > 98% → 立即 Critical
    #[test]
    fn test_emergency_high_used() {
        let mut sm = StateMachine::new(StateMachineConfig::default());
        let now = Instant::now();

        let m = make_metrics(8000, 50.0, PressureLevel::Low);
        sm.update(&m, now);

        let m = make_metrics(2000, 99.5, PressureLevel::Critical);
        let r = sm.update(&m, now + Duration::from_secs(1));
        assert_eq!(r, StateTransition::Emergency);
        assert_eq!(sm.current_state(), MonitorState::Critical);
    }

    // UT-SM-009: 紧急状态恢复：Critical → Warning
    #[test]
    fn test_emergency_recovery() {
        let config = StateMachineConfig {
            recovery_seconds: 0,
            min_samples_recovery: 2,
            ..Default::default()
        };
        let mut sm = StateMachine::new(config);
        let now = Instant::now();

        // 触发紧急
        let m = make_metrics(400, 95.0, PressureLevel::Critical);
        sm.update(&m, now);
        assert_eq!(sm.current_state(), MonitorState::Critical);

        // 第一次非紧急恢复
        let m = make_metrics(5000, 60.0, PressureLevel::Medium);
        let r1 = sm.update(&m, now + Duration::from_secs(1));
        assert_eq!(r1, StateTransition::None);

        // 第二次 → 恢复 Warning
        let m = make_metrics(6000, 50.0, PressureLevel::Low);
        let r2 = sm.update(&m, now + Duration::from_secs(2));
        assert_eq!(r2, StateTransition::RecoverToWarning);
        assert_eq!(sm.current_state(), MonitorState::Warning);
    }

    // UT-SM-010: Reset 后状态归零
    #[test]
    fn test_reset() {
        let mut sm = StateMachine::new(StateMachineConfig::default());
        let now = Instant::now();

        let m = make_metrics(3000, 80.0, PressureLevel::High);
        sm.update(&m, now);
        assert_eq!(sm.current_state(), MonitorState::Warning);

        sm.reset();
        assert_eq!(sm.current_state(), MonitorState::Normal);
    }

    // UT-SM-011: 双条件同时满足（时间 + 次数）
    #[test]
    fn test_dual_condition_both_required() {
        let config = StateMachineConfig {
            warning_seconds: 3,     // 要求至少 3 秒
            min_samples_warning: 3, // 要求至少 3 次
            ..Default::default()
        };
        let mut sm = StateMachine::new(config);
        let now = Instant::now();

        // 冷启动（正常）
        let m = make_metrics(8000, 50.0, PressureLevel::Low);
        sm.update(&m, now);

        // 连续 3 次高压，但时间未满 3 秒
        for i in 0..3 {
            let m = make_metrics(2000, 85.0, PressureLevel::High);
            let r = sm.update(&m, now + Duration::from_millis(100 * (i + 1)));
            // 只要时间不满，就不转换
            assert_eq!(r, StateTransition::None, "第{}次，时间应不足", i + 1);
        }

        // 第 4 次，时间已过 3 秒，次数满足
        let m = make_metrics(2000, 85.0, PressureLevel::High);
        let r = sm.update(&m, now + Duration::from_secs(4));
        assert_eq!(r, StateTransition::EnterWarning);
    }

    // UT-SM-012: StateTransition action 映射正确
    #[test]
    fn test_transition_actions() {
        assert_eq!(StateTransition::None.action(), "no_change");
        assert_eq!(StateTransition::EnterWarning.action(), "monitor");
        assert_eq!(StateTransition::EnterCritical.action(), "abort_safely");
        assert_eq!(StateTransition::RecoverToNormal.action(), "ok");
        assert_eq!(StateTransition::RecoverToWarning.action(), "reduce_context");
        assert_eq!(StateTransition::Emergency.action(), "abort_safely");
    }

    // UT-SM-013: 紧急后持续紧急 → Emergency 不重复触发
    #[test]
    fn test_emergency_already_critical() {
        let config = StateMachineConfig {
            recovery_seconds: 0,
            min_samples_recovery: 2,
            ..Default::default()
        };
        let mut sm = StateMachine::new(config);
        let now = Instant::now();

        // 触发紧急
        let m = make_metrics(400, 95.0, PressureLevel::Critical);
        let r1 = sm.update(&m, now);
        assert_eq!(r1, StateTransition::Emergency);
        assert_eq!(sm.current_state(), MonitorState::Critical);

        // 继续紧急条件 → 已 Critical，应返回 None 避免重复报警
        let m = make_metrics(300, 96.0, PressureLevel::Critical);
        let r2 = sm.update(&m, now + Duration::from_secs(1));
        assert_eq!(r2, StateTransition::None, "已在 Critical 不应重复紧急");
    }
}
