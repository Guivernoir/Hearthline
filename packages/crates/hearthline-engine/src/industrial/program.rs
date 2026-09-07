use heapless::Vec as FixedList;
use hearthline_model::Text;

use crate::capacity::{SEQUENCE_OUTPUT_CAPACITY, SEQUENCE_STEP_CAPACITY};
const _: () = assert!(SEQUENCE_OUTPUT_CAPACITY <= u32::BITS as usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceCondition {
    StartPermitted,
    TimerElapsed { duration_ms: u64 },
    ResetPermitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceTransition {
    pub condition: SequenceCondition,
    pub target: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceAssignment {
    pub variable: Text<64>,
    pub value: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceStep {
    pub number: i64,
    pub assignments: FixedList<SequenceAssignment, SEQUENCE_OUTPUT_CAPACITY>,
    pub transition: Option<SequenceTransition>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompiledSequenceStep {
    number: i64,
    values: [i64; SEQUENCE_OUTPUT_CAPACITY],
    assigned: u32,
    transition: Option<SequenceTransition>,
}

impl CompiledSequenceStep {
    fn assignment(&self, index: usize) -> Option<i64> {
        (self.assigned & (1 << index) != 0).then_some(self.values[index])
    }
}

impl SequenceStep {
    pub fn new(
        number: i64,
        assignments: impl IntoIterator<Item = SequenceAssignment>,
        transition: Option<SequenceTransition>,
    ) -> Option<Self> {
        let mut bounded = FixedList::new();
        for assignment in assignments {
            bounded.push(assignment).ok()?;
        }
        Some(Self {
            number,
            assignments: bounded,
            transition,
        })
    }

    pub fn assignment(&self, variable: &str) -> Option<i64> {
        self.assignments
            .iter()
            .find(|assignment| assignment.variable.as_str() == variable)
            .map(|assignment| assignment.value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceProgram {
    pub name: Text<64>,
    pub scan_interval_ms: u64,
    pub idle_step: i64,
    pub fault_step: i64,
    variables: FixedList<Text<64>, SEQUENCE_OUTPUT_CAPACITY>,
    steps: FixedList<CompiledSequenceStep, SEQUENCE_STEP_CAPACITY>,
}

impl SequenceProgram {
    pub fn new(
        name: Text<64>,
        scan_interval_ms: u64,
        idle_step: i64,
        fault_step: i64,
        steps: impl IntoIterator<Item = SequenceStep>,
    ) -> Option<Self> {
        if scan_interval_ms == 0 {
            return None;
        }
        let mut variables: FixedList<Text<64>, SEQUENCE_OUTPUT_CAPACITY> = FixedList::new();
        let mut bounded: FixedList<CompiledSequenceStep, SEQUENCE_STEP_CAPACITY> = FixedList::new();
        for step in steps {
            if bounded
                .iter()
                .any(|candidate| candidate.number == step.number)
            {
                return None;
            }
            let mut values = [0; SEQUENCE_OUTPUT_CAPACITY];
            let mut assigned = 0_u32;
            for assignment in step.assignments {
                let index = if let Some(index) = variables
                    .iter()
                    .position(|variable| variable == &assignment.variable)
                {
                    index
                } else {
                    variables.push(assignment.variable).ok()?;
                    variables.len() - 1
                };
                let bit = 1_u32 << index;
                if assigned & bit != 0 {
                    return None;
                }
                assigned |= bit;
                values[index] = assignment.value;
            }
            bounded
                .push(CompiledSequenceStep {
                    number: step.number,
                    values,
                    assigned,
                    transition: step.transition,
                })
                .ok()?;
        }
        if !bounded.iter().any(|step| step.number == idle_step)
            || !bounded.iter().any(|step| step.number == fault_step)
        {
            return None;
        }
        for step in &bounded {
            if let Some(transition) = step.transition
                && !bounded
                    .iter()
                    .any(|candidate| candidate.number == transition.target)
            {
                return None;
            }
        }
        Some(Self {
            name,
            scan_interval_ms,
            idle_step,
            fault_step,
            variables,
            steps: bounded,
        })
    }

    fn step(&self, number: i64) -> Option<&CompiledSequenceStep> {
        self.steps.iter().find(|step| step.number == number)
    }

    fn assignment(&self, step: i64, variable: &str) -> Option<i64> {
        let index = self
            .variables
            .iter()
            .position(|candidate| candidate.as_str() == variable)?;
        self.step(step)?.assignment(index)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SequenceInputs {
    pub start_request: bool,
    pub reset_request: bool,
    pub safety_ready: bool,
    pub trip_active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceScan {
    pub previous_step: i64,
    pub current_step: i64,
    pub step_changed: bool,
    pub cycle_completed: bool,
}

#[derive(Clone, Debug)]
pub struct SequenceRuntime {
    program: SequenceProgram,
    current_step: i64,
    step_elapsed_ms: u64,
    scan_remainder_ms: u64,
    scan_count: u64,
    cycle_count: u64,
    running: bool,
}

impl SequenceRuntime {
    pub fn new(program: SequenceProgram) -> Self {
        let current_step = program.idle_step;
        Self {
            program,
            current_step,
            step_elapsed_ms: 0,
            scan_remainder_ms: 0,
            scan_count: 0,
            cycle_count: 0,
            running: false,
        }
    }

    pub const fn current_step(&self) -> i64 {
        self.current_step
    }

    pub const fn step_elapsed_ms(&self) -> u64 {
        self.step_elapsed_ms
    }

    pub const fn scan_count(&self) -> u64 {
        self.scan_count
    }

    pub const fn cycle_count(&self) -> u64 {
        self.cycle_count
    }

    pub const fn running(&self) -> bool {
        self.running
    }

    pub const fn program(&self) -> &SequenceProgram {
        &self.program
    }

    pub fn current_assignment(&self, variable: &str) -> Option<i64> {
        self.program.assignment(self.current_step, variable)
    }

    pub fn time_to_next_scan_ms(&self) -> u64 {
        self.program
            .scan_interval_ms
            .saturating_sub(self.scan_remainder_ms)
    }

    pub fn elapse(&mut self, elapsed_ms: u64, inputs: SequenceInputs) -> Option<SequenceScan> {
        self.elapse_with_timer_override(elapsed_ms, inputs, None)
    }

    pub fn elapse_with_timer_override(
        &mut self,
        elapsed_ms: u64,
        inputs: SequenceInputs,
        timer_override_ms: Option<u64>,
    ) -> Option<SequenceScan> {
        assert!(
            elapsed_ms <= self.time_to_next_scan_ms(),
            "sequence elapsed slice cannot cross more than one scan boundary"
        );
        self.step_elapsed_ms = self.step_elapsed_ms.saturating_add(elapsed_ms);
        self.scan_remainder_ms = self.scan_remainder_ms.saturating_add(elapsed_ms);
        if self.scan_remainder_ms < self.program.scan_interval_ms {
            return None;
        }
        self.scan_remainder_ms = 0;
        Some(self.execute_scan_with_timer_override(inputs, timer_override_ms))
    }

    pub fn execute_scan(&mut self, inputs: SequenceInputs) -> SequenceScan {
        self.execute_scan_with_timer_override(inputs, None)
    }

    fn execute_scan_with_timer_override(
        &mut self,
        inputs: SequenceInputs,
        timer_override_ms: Option<u64>,
    ) -> SequenceScan {
        self.scan_count = self.scan_count.saturating_add(1);
        let previous_step = self.current_step;
        if inputs.trip_active {
            self.current_step = self.program.fault_step;
        } else if let Some(transition) = self
            .program
            .step(self.current_step)
            .and_then(|step| step.transition)
            && transition_matches(
                transition.condition,
                self.step_elapsed_ms,
                inputs,
                timer_override_ms,
            )
        {
            self.current_step = transition.target;
        }
        let step_changed = previous_step != self.current_step;
        let cycle_completed = step_changed
            && previous_step != self.program.idle_step
            && self.current_step == self.program.idle_step
            && self.running;
        if step_changed {
            self.step_elapsed_ms = 0;
        }
        if cycle_completed {
            self.cycle_count = self.cycle_count.saturating_add(1);
        }
        self.running = self.current_step != self.program.idle_step
            && self.current_step != self.program.fault_step;
        SequenceScan {
            previous_step,
            current_step: self.current_step,
            step_changed,
            cycle_completed,
        }
    }

    pub fn force_fault(&mut self) -> SequenceScan {
        self.execute_scan(SequenceInputs {
            trip_active: true,
            ..SequenceInputs::default()
        })
    }
}

fn transition_matches(
    condition: SequenceCondition,
    step_elapsed_ms: u64,
    inputs: SequenceInputs,
    timer_override_ms: Option<u64>,
) -> bool {
    match condition {
        SequenceCondition::StartPermitted => {
            inputs.start_request && inputs.safety_ready && !inputs.trip_active
        }
        SequenceCondition::TimerElapsed { duration_ms } => {
            step_elapsed_ms >= timer_override_ms.unwrap_or(duration_ms)
        }
        SequenceCondition::ResetPermitted => {
            inputs.reset_request && inputs.safety_ready && !inputs.trip_active
        }
    }
}
