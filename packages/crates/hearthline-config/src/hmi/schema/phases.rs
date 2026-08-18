use super::HmiProcessPhase;

pub(in crate::hmi) const FORMING_PHASES: [HmiProcessPhase; 15] = [
    HmiProcessPhase {
        key: "idle",
        label: "Ready",
    },
    HmiProcessPhase {
        key: "mould-filling",
        label: "Fill",
    },
    HmiProcessPhase {
        key: "air-pressurizing",
        label: "Apply air pressure",
    },
    HmiProcessPhase {
        key: "pressure-dwell",
        label: "Pressure hold",
    },
    HmiProcessPhase {
        key: "depressurizing",
        label: "Depressurize",
    },
    HmiProcessPhase {
        key: "excess-slip-drain",
        label: "Drain slip",
    },
    HmiProcessPhase {
        key: "release-water",
        label: "Release water",
    },
    HmiProcessPhase {
        key: "release-air",
        label: "Release air",
    },
    HmiProcessPhase {
        key: "mould-opening",
        label: "Open mould",
    },
    HmiProcessPhase {
        key: "robot-pickup",
        label: "Robot pickup",
    },
    HmiProcessPhase {
        key: "operator-delivery",
        label: "Operator handoff",
    },
    HmiProcessPhase {
        key: "mould-wash",
        label: "Wash",
    },
    HmiProcessPhase {
        key: "cleaning-air-purge",
        label: "Cleaning air",
    },
    HmiProcessPhase {
        key: "vacuum-dry",
        label: "Vacuum dry",
    },
    HmiProcessPhase {
        key: "mould-closing",
        label: "Close mould",
    },
];

pub(in crate::hmi) const SLIP_PREPARATION_PHASES: [HmiProcessPhase; 15] = [
    HmiProcessPhase {
        key: "idle",
        label: "Ready",
    },
    HmiProcessPhase {
        key: "water-charge",
        label: "Charge water",
    },
    HmiProcessPhase {
        key: "deflocculant-charge",
        label: "Dose deflocculants",
    },
    HmiProcessPhase {
        key: "ball-clay-charge",
        label: "Charge ball clay",
    },
    HmiProcessPhase {
        key: "kaolin-charge",
        label: "Charge kaolin",
    },
    HmiProcessPhase {
        key: "feldspar-charge",
        label: "Charge feldspar",
    },
    HmiProcessPhase {
        key: "quartz-charge",
        label: "Charge quartz",
    },
    HmiProcessPhase {
        key: "wet-mixing",
        label: "Wet mixing",
    },
    HmiProcessPhase {
        key: "screening",
        label: "Screen",
    },
    HmiProcessPhase {
        key: "magnetic-separation",
        label: "Remove tramp iron",
    },
    HmiProcessPhase {
        key: "conditioning-ageing",
        label: "Condition / age",
    },
    HmiProcessPhase {
        key: "rheology-quality-release",
        label: "Rheology release",
    },
    HmiProcessPhase {
        key: "temperature-trim",
        label: "Temperature trim",
    },
    HmiProcessPhase {
        key: "transfer-to-forming",
        label: "Transfer to Forming",
    },
    HmiProcessPhase {
        key: "batch-complete",
        label: "Batch complete",
    },
];

pub(in crate::hmi) const WATER_PREPARATION_PHASES: [HmiProcessPhase; 10] = [
    HmiProcessPhase {
        key: "idle",
        label: "Ready",
    },
    HmiProcessPhase {
        key: "raw-water-intake",
        label: "Raw intake",
    },
    HmiProcessPhase {
        key: "equalization",
        label: "Equalize",
    },
    HmiProcessPhase {
        key: "multimedia-filtration",
        label: "Media filter",
    },
    HmiProcessPhase {
        key: "activated-carbon",
        label: "Carbon filter",
    },
    HmiProcessPhase {
        key: "ion-exchange-softening",
        label: "Soften",
    },
    HmiProcessPhase {
        key: "reverse-osmosis-blend",
        label: "RO and blend",
    },
    HmiProcessPhase {
        key: "process-water-quality-release",
        label: "Quality release",
    },
    HmiProcessPhase {
        key: "treated-water-transfer",
        label: "Product transfer",
    },
    HmiProcessPhase {
        key: "treatment-cycle-complete",
        label: "Complete",
    },
];

pub(in crate::hmi) const RETURN_WATER_PHASES: [HmiProcessPhase; 9] = [
    HmiProcessPhase {
        key: "idle",
        label: "Ready",
    },
    HmiProcessPhase {
        key: "segregated-return-collection",
        label: "Collect by origin",
    },
    HmiProcessPhase {
        key: "return-equalization",
        label: "Equalize",
    },
    HmiProcessPhase {
        key: "coagulation-flocculation",
        label: "Coagulate / flocculate",
    },
    HmiProcessPhase {
        key: "lamella-clarification",
        label: "Clarify",
    },
    HmiProcessPhase {
        key: "filter-press-dewatering",
        label: "Dewater solids",
    },
    HmiProcessPhase {
        key: "polishing-filtration",
        label: "Polish",
    },
    HmiProcessPhase {
        key: "reuse-quality-routing",
        label: "Quality route",
    },
    HmiProcessPhase {
        key: "recovery-cycle-complete",
        label: "Complete",
    },
];

pub(in crate::hmi) const GLAZE_PREPARATION_PHASES: [HmiProcessPhase; 12] = [
    HmiProcessPhase {
        key: "idle",
        label: "Ready",
    },
    HmiProcessPhase {
        key: "glaze-water-charge",
        label: "Charge water",
    },
    HmiProcessPhase {
        key: "seven-powder-weighing",
        label: "Weigh powders",
    },
    HmiProcessPhase {
        key: "glaze-dispersant-charge",
        label: "Dose dispersant",
    },
    HmiProcessPhase {
        key: "glaze-wet-milling",
        label: "Wet mill",
    },
    HmiProcessPhase {
        key: "63-micrometre-screening",
        label: "Screen 63 um",
    },
    HmiProcessPhase {
        key: "glaze-magnetic-separation",
        label: "Remove tramp iron",
    },
    HmiProcessPhase {
        key: "density-flow-adjustment",
        label: "Adjust properties",
    },
    HmiProcessPhase {
        key: "glaze-quality-release",
        label: "Quality release",
    },
    HmiProcessPhase {
        key: "agitated-glaze-storage",
        label: "Agitated storage",
    },
    HmiProcessPhase {
        key: "transfer-to-glazing",
        label: "Transfer to glazing",
    },
    HmiProcessPhase {
        key: "glaze-batch-complete",
        label: "Complete",
    },
];
