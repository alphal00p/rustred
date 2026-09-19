//! Single-field terminal progress and explicitly requested plain stderr logs.

use std::io::Write;
use std::time::{Duration, Instant};

use crate::{FamilyCloseGenerationStage, FamilyCloseProgress};

use super::terminal::TerminalSession;

/// Presentation failures never change artifact generation or output errors.
pub(crate) struct FamilyCloseProgressMonitor<W: Write> {
    terminal: Option<TerminalSession<W>>,
    plain: Option<W>,
    color: bool,
    last_update: Option<Instant>,
    last_was_generating: bool,
}

impl<W: Write> FamilyCloseProgressMonitor<W> {
    pub(crate) fn new(writer: W, terminal: bool, force: bool, no_color: bool) -> Self {
        let (terminal, plain) = if terminal {
            (TerminalSession::try_new_line(writer).ok(), None)
        } else {
            (None, force.then_some(writer))
        };
        Self {
            terminal,
            plain,
            color: !no_color,
            last_update: None,
            last_was_generating: false,
        }
    }

    pub(crate) fn observe(&mut self, event: FamilyCloseProgress) {
        self.observe_at(event, Instant::now());
    }

    fn observe_at(&mut self, event: FamilyCloseProgress, now: Instant) {
        if self.terminal.is_none() && self.plain.is_none() {
            return;
        }
        let generating = matches!(event, FamilyCloseProgress::Generating { .. });
        // Share one generation throttle across worker sectors and stages:
        // alternating worker phases must not bypass the rate limit. Every
        // major boundary and the first generation event after it stay visible.
        if generating
            && self.last_was_generating
            && self
                .last_update
                .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_millis(100))
        {
            return;
        }
        self.last_was_generating = generating;
        self.write(&format_event(event), now);
    }

    pub(crate) fn finish(&mut self, success: bool) {
        self.write(
            if success {
                "RustRed | output written"
            } else {
                "RustRed | failed (see error)"
            },
            Instant::now(),
        );
        if let Some(terminal) = &mut self.terminal {
            terminal.close();
        }
    }

    fn write(&mut self, text: &str, now: Instant) {
        self.last_update = Some(now);
        if let Some(terminal) = &mut self.terminal {
            if terminal.render_line(text, self.color).is_err() {
                self.terminal = None;
            }
        } else if let Some(writer) = &mut self.plain
            && writeln!(writer, "{text}")
                .and_then(|()| writer.flush())
                .is_err()
        {
            self.plain = None;
        }
    }
}

fn format_event(event: FamilyCloseProgress) -> String {
    use FamilyCloseProgress::*;
    let (elapsed, status) = match event {
        Preparing { arity, elapsed } => (elapsed, format!("preparing K={arity}")),
        Prepared {
            sectors,
            zero_sectors,
            global_zero_sectors,
            elapsed,
        } => (
            elapsed,
            format!(
                "generate {sectors} sectors; {zero_sectors} scoped zero; {global_zero_sectors} global zero proofs"
            ),
        ),
        Generating {
            sector,
            stage,
            elapsed,
            ..
        } => {
            let phase = match stage {
                FamilyCloseGenerationStage::Case { pending } => format!("case; {pending} pending"),
                FamilyCloseGenerationStage::Discovery { depth, seeds, rows } => {
                    format!("discovery depth={depth} seeds={seeds} rows={rows}")
                }
                FamilyCloseGenerationStage::ExactMaterialization => "exact lift".into(),
                FamilyCloseGenerationStage::Canonicalization => "canonicalization".into(),
                FamilyCloseGenerationStage::GuardExtraction => "guards".into(),
                FamilyCloseGenerationStage::ExceptionalGeometry => "exceptional geometry".into(),
                FamilyCloseGenerationStage::RuleFound { pending } => {
                    format!("rule found; {pending} pending")
                }
                FamilyCloseGenerationStage::Numerical { cases } => {
                    format!("finite search; {cases} cases")
                }
            };
            (elapsed, format!("sector={sector} {phase}"))
        }
        GeneratedSector {
            sector,
            rules,
            finite_residuals,
            elapsed,
            ..
        } => (
            elapsed,
            format!("sector={sector} generated {rules} rules; {finite_residuals} finite residuals"),
        ),
        CheckingSector {
            sector,
            rules,
            elapsed,
            ..
        } => (elapsed, format!("replay sector={sector} {rules} rules")),
        CheckingRule {
            sector,
            ordinal,
            total,
            elapsed,
        } => (
            elapsed,
            format!("checking sector={sector} rule={}/{total}", ordinal + 1),
        ),
        CheckedSector {
            sector,
            replayed_rules,
            uncovered_boxes,
            issues,
            elapsed,
            ..
        } => (
            elapsed,
            format!(
                "replayed sector={sector} rules={replayed_rules} uncovered={uncovered_boxes} issues={issues}"
            ),
        ),
        LoweringRule {
            sector,
            ordinal,
            total,
            elapsed,
        } => (
            elapsed,
            format!("lowering sector={sector} rule={}/{total}", ordinal + 1),
        ),
        LoweredSector {
            sector,
            cells,
            elapsed,
        } => (elapsed, format!("lowered sector={sector} cells={cells}")),
        Installing {
            sectors,
            rule_cells,
            terminals,
            elapsed,
        } => (
            elapsed,
            format!("installing {sectors} sectors; {rule_cells} cells; {terminals} terminals"),
        ),
        Installed { elapsed } => (elapsed, "installed in memory".into()),
        Encoding { elapsed } => (elapsed, "encoding output".into()),
        Encoded { bytes, elapsed } => (elapsed, format!("encoded {bytes} bytes")),
    };
    format!("RustRed | {:.1}s | {status}", elapsed.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirected_progress_is_opt_in_and_plain() {
        let event = FamilyCloseProgress::Preparing {
            arity: 3,
            elapsed: Duration::ZERO,
        };
        let mut quiet = FamilyCloseProgressMonitor::new(Vec::new(), false, false, false);
        quiet.observe(event);
        assert!(quiet.plain.is_none() && quiet.terminal.is_none());
        let mut plain = FamilyCloseProgressMonitor::new(Vec::new(), false, true, false);
        plain.observe(event);
        plain.finish(true);
        let output = String::from_utf8(plain.plain.unwrap()).unwrap();
        assert!(output.contains("preparing K=3"));
        assert!(output.contains("output written"));
        assert!(!output.contains('\u{1b}') && !output.contains('\r'));
    }

    #[test]
    fn replay_observation_is_not_formatted_as_publication() {
        let output = format_event(FamilyCloseProgress::CheckedSector {
            ordinal: 0,
            sector: 7,
            replayed_rules: 2,
            uncovered_boxes: 1,
            issues: 1,
            elapsed: Duration::ZERO,
        });
        assert!(output.contains("uncovered=1 issues=1"));
        assert!(!output.contains("closed") && !output.contains("written"));
    }

    #[test]
    fn consecutive_rule_checks_are_visible_inside_the_throttle_window() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        monitor.observe(FamilyCloseProgress::CheckingSector {
            ordinal: 0,
            sector: 214,
            rules: 161,
            elapsed: Duration::ZERO,
        });
        for ordinal in [0, 1, 160] {
            // Reset the last-render timestamp explicitly: the assertion must
            // not depend on how quickly this test process is scheduled.
            monitor.last_update = Some(Instant::now());
            monitor.observe(FamilyCloseProgress::CheckingRule {
                sector: 214,
                ordinal,
                total: 161,
                elapsed: Duration::ZERO,
            });
        }
        let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
        assert_eq!(output.lines().count(), 4);
        for expected in ["rule=1/161", "rule=2/161", "rule=161/161"] {
            assert!(output.contains(expected), "missing {expected}: {output}");
        }
        assert!(!output.contains("closed") && !output.contains("written"));
    }

    #[test]
    fn alternating_worker_stages_share_one_generation_throttle() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        let now = Instant::now();
        let event = |ordinal: usize| FamilyCloseProgress::Generating {
            ordinal: ordinal % 6,
            sector: (ordinal % 6) as u64,
            stage: match ordinal % 4 {
                0 => FamilyCloseGenerationStage::Case { pending: ordinal },
                1 => FamilyCloseGenerationStage::Discovery {
                    depth: 1,
                    seeds: ordinal,
                    rows: ordinal,
                },
                2 => FamilyCloseGenerationStage::ExactMaterialization,
                _ => FamilyCloseGenerationStage::GuardExtraction,
            },
            elapsed: Duration::ZERO,
        };
        monitor.observe_at(
            FamilyCloseProgress::Prepared {
                sectors: 6,
                zero_sectors: 0,
                global_zero_sectors: 0,
                elapsed: Duration::ZERO,
            },
            now,
        );
        for ordinal in 0..10_000 {
            monitor.observe_at(event(ordinal), now);
        }
        assert_eq!(
            monitor
                .plain
                .as_ref()
                .unwrap()
                .iter()
                .filter(|&&b| b == b'\n')
                .count(),
            2,
        );
        monitor.observe_at(event(1), now + Duration::from_millis(99));
        assert_eq!(
            monitor
                .plain
                .as_ref()
                .unwrap()
                .iter()
                .filter(|&&b| b == b'\n')
                .count(),
            2,
        );
        let next_tick = now + Duration::from_millis(100);
        monitor.observe_at(event(2), next_tick);
        monitor.observe_at(
            FamilyCloseProgress::GeneratedSector {
                ordinal: 0,
                sector: 0,
                rules: 1,
                finite_residuals: 1,
                elapsed: Duration::ZERO,
            },
            next_tick,
        );
        // A major boundary is followed immediately by one visible generation
        // event, even when another worker is already inside the same tick.
        monitor.observe_at(event(3), next_tick);
        monitor.observe_at(event(4), next_tick);
        let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
        assert_eq!(output.lines().count(), 5);
        assert!(output.contains("exact lift"));
        assert!(output.contains("generated 1 rules"));
        assert!(output.contains("sector=3 guards"));
    }

    #[test]
    fn adjacent_phase_boundaries_are_never_throttled() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        for event in [
            FamilyCloseProgress::GeneratedSector {
                ordinal: 0,
                sector: 7,
                rules: 2,
                finite_residuals: 1,
                elapsed: Duration::ZERO,
            },
            FamilyCloseProgress::CheckingSector {
                ordinal: 0,
                sector: 7,
                rules: 2,
                elapsed: Duration::ZERO,
            },
            FamilyCloseProgress::CheckingRule {
                sector: 7,
                ordinal: 0,
                total: 2,
                elapsed: Duration::ZERO,
            },
            FamilyCloseProgress::CheckedSector {
                ordinal: 0,
                sector: 7,
                replayed_rules: 2,
                uncovered_boxes: 0,
                issues: 0,
                elapsed: Duration::ZERO,
            },
            FamilyCloseProgress::LoweringRule {
                sector: 7,
                ordinal: 0,
                total: 2,
                elapsed: Duration::ZERO,
            },
            FamilyCloseProgress::LoweredSector {
                sector: 7,
                cells: 2,
                elapsed: Duration::ZERO,
            },
        ] {
            monitor.observe(event);
        }
        let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
        for expected in [
            "generated 2",
            "replay sector=7",
            "checking sector=7 rule=1/2",
            "replayed sector=7",
            "lowering sector=7 rule=1/2",
            "lowered sector=7",
        ] {
            assert!(output.contains(expected), "missing {expected}: {output}");
        }
    }
}
