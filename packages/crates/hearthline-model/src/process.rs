use crate::Text;

/// Typed process value exchanged between plant models, I/O, and controllers.
#[derive(Clone, Debug, PartialEq)]
pub enum SignalValue {
    Bool(bool),
    Analog(f64),
    Integer(i64),
    Text(Text<128>),
}

/// One named process signal with engineering metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessSignal {
    pub tag: Text<64>,
    pub value: SignalValue,
    pub quality_good: bool,
    pub timestamp_ms: u64,
}

/// Operator or controller command directed at an output tag.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessCommand {
    pub tag: Text<64>,
    pub value: SignalValue,
    pub source: Text<64>,
}

/// Events accepted by deterministic OT component models.
#[derive(Clone, Debug, PartialEq)]
pub enum ProcessEvent {
    Tick { elapsed_ms: u64 },
    Signal(ProcessSignal),
    Command(ProcessCommand),
    Trip { cause: Text<128> },
    Reset { authorized: bool },
}
