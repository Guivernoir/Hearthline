use hearthline_model::{Angle, AngularSpeed, LinearSpeed, Percentage, Position};

const FULL_PERCENT: i64 = 10_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RobotPose {
    pub x: Position,
    pub y: Position,
    pub z: Position,
    pub w: Angle,
    pub p: Angle,
    pub r: Angle,
}

impl RobotPose {
    pub const fn new(x: Position, y: Position, z: Position, w: Angle, p: Angle, r: Angle) -> Self {
        Self { x, y, z, w, p, r }
    }

    fn interpolate(self, target: Self, elapsed: u64, duration: u64) -> Self {
        Self {
            x: Position::from_raw(lerp_raw(self.x.raw(), target.x.raw(), elapsed, duration)),
            y: Position::from_raw(lerp_raw(self.y.raw(), target.y.raw(), elapsed, duration)),
            z: Position::from_raw(lerp_raw(self.z.raw(), target.z.raw(), elapsed, duration)),
            w: Angle::from_raw(lerp_raw(self.w.raw(), target.w.raw(), elapsed, duration)),
            p: Angle::from_raw(lerp_raw(self.p.raw(), target.p.raw(), elapsed, duration)),
            r: Angle::from_raw(lerp_raw(self.r.raw(), target.r.raw(), elapsed, duration)),
        }
    }

    fn maximum_scaled_delta(self, target: Self) -> u64 {
        [
            absolute_delta(target.x.raw(), self.x.raw()),
            absolute_delta(target.y.raw(), self.y.raw()),
            absolute_delta(target.z.raw(), self.z.raw()),
            absolute_delta(target.w.raw(), self.w.raw()).saturating_mul(8),
            absolute_delta(target.p.raw(), self.p.raw()).saturating_mul(8),
            absolute_delta(target.r.raw(), self.r.raw()).saturating_mul(8),
        ]
        .into_iter()
        .max()
        .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RobotJoints {
    pub axes: [Angle; 6],
}

impl RobotJoints {
    pub const fn new(axes: [Angle; 6]) -> Self {
        Self { axes }
    }

    fn interpolate(self, target: Self, elapsed: u64, duration: u64) -> Self {
        let mut axes = [Angle::ZERO; 6];
        let mut index = 0;
        while index < axes.len() {
            axes[index] = Angle::from_raw(lerp_raw(
                self.axes[index].raw(),
                target.axes[index].raw(),
                elapsed,
                duration,
            ));
            index += 1;
        }
        Self { axes }
    }

    fn maximum_delta(self, target: Self) -> u64 {
        self.axes
            .into_iter()
            .zip(target.axes)
            .map(|(current, requested)| absolute_delta(requested.raw(), current.raw()))
            .max()
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RobotWorkspace {
    pub minimum: RobotPose,
    pub maximum: RobotPose,
    pub joint_minimum: RobotJoints,
    pub joint_maximum: RobotJoints,
}

impl RobotWorkspace {
    pub fn contains_pose(self, pose: RobotPose) -> bool {
        in_range(pose.x.raw(), self.minimum.x.raw(), self.maximum.x.raw())
            && in_range(pose.y.raw(), self.minimum.y.raw(), self.maximum.y.raw())
            && in_range(pose.z.raw(), self.minimum.z.raw(), self.maximum.z.raw())
            && in_range(pose.w.raw(), self.minimum.w.raw(), self.maximum.w.raw())
            && in_range(pose.p.raw(), self.minimum.p.raw(), self.maximum.p.raw())
            && in_range(pose.r.raw(), self.minimum.r.raw(), self.maximum.r.raw())
    }

    pub fn contains_joints(self, joints: RobotJoints) -> bool {
        joints
            .axes
            .into_iter()
            .zip(self.joint_minimum.axes)
            .zip(self.joint_maximum.axes)
            .all(|((value, minimum), maximum)| in_range(value.raw(), minimum.raw(), maximum.raw()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RobotMotionKind {
    Rapid,
    Linear,
    Joint,
    Jog,
}

impl RobotMotionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rapid => "rapid",
            Self::Linear => "linear",
            Self::Joint => "joint",
            Self::Jog => "jog",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RobotCartesianAxis {
    X,
    Y,
    Z,
    W,
    P,
    R,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RobotCartesianIncrement {
    Linear(Position),
    Angular(Angle),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RobotMotionError {
    OutsideWorkspace,
    InvalidSpeed,
    InvalidIncrement,
    MotionActive,
}

#[derive(Clone, Copy, Debug)]
pub struct RobotMotionRuntime {
    workspace: RobotWorkspace,
    home: RobotPose,
    current_pose: RobotPose,
    start_pose: RobotPose,
    target_pose: RobotPose,
    current_joints: RobotJoints,
    start_joints: RobotJoints,
    target_joints: RobotJoints,
    kind: RobotMotionKind,
    elapsed_ms: u64,
    duration_ms: u64,
    speed_percent: Percentage,
    max_linear_speed: LinearSpeed,
    max_joint_speed: AngularSpeed,
    active: bool,
}

impl RobotMotionRuntime {
    pub fn new(
        workspace: RobotWorkspace,
        home: RobotPose,
        max_linear_speed: LinearSpeed,
        max_joint_speed: AngularSpeed,
    ) -> Result<Self, RobotMotionError> {
        if !workspace.contains_pose(home) {
            return Err(RobotMotionError::OutsideWorkspace);
        }
        if max_linear_speed <= LinearSpeed::ZERO || max_joint_speed <= AngularSpeed::ZERO {
            return Err(RobotMotionError::InvalidSpeed);
        }
        let joints = projected_joints(home, workspace);
        Ok(Self {
            workspace,
            home,
            current_pose: home,
            start_pose: home,
            target_pose: home,
            current_joints: joints,
            start_joints: joints,
            target_joints: joints,
            kind: RobotMotionKind::Rapid,
            elapsed_ms: 0,
            duration_ms: 0,
            speed_percent: Percentage::ZERO,
            max_linear_speed,
            max_joint_speed,
            active: false,
        })
    }

    pub fn command_pose(
        &mut self,
        target: RobotPose,
        kind: RobotMotionKind,
        speed_percent: Percentage,
    ) -> Result<(), RobotMotionError> {
        validate_speed(speed_percent)?;
        if !self.workspace.contains_pose(target) {
            return Err(RobotMotionError::OutsideWorkspace);
        }
        if self.active {
            return Err(RobotMotionError::MotionActive);
        }
        self.start_pose = self.current_pose;
        self.target_pose = target;
        self.start_joints = self.current_joints;
        self.target_joints = projected_joints(target, self.workspace);
        let duration_ms = duration(
            self.current_pose.maximum_scaled_delta(target),
            self.max_linear_speed.raw(),
            speed_percent,
        );
        self.begin(kind, speed_percent, duration_ms);
        Ok(())
    }

    pub fn command_joints(
        &mut self,
        target: RobotJoints,
        speed_percent: Percentage,
    ) -> Result<(), RobotMotionError> {
        validate_speed(speed_percent)?;
        if !self.workspace.contains_joints(target) {
            return Err(RobotMotionError::OutsideWorkspace);
        }
        if self.active {
            return Err(RobotMotionError::MotionActive);
        }
        self.start_pose = self.current_pose;
        self.target_pose = projected_pose(target, self.workspace);
        self.start_joints = self.current_joints;
        self.target_joints = target;
        let duration_ms = duration(
            self.current_joints.maximum_delta(target),
            self.max_joint_speed.raw(),
            speed_percent,
        );
        self.begin(RobotMotionKind::Joint, speed_percent, duration_ms);
        Ok(())
    }

    pub fn jog_cartesian(
        &mut self,
        axis: RobotCartesianAxis,
        increment: RobotCartesianIncrement,
        speed_percent: Percentage,
    ) -> Result<(), RobotMotionError> {
        let mut target = self.current_pose;
        match (axis, increment) {
            (RobotCartesianAxis::X, RobotCartesianIncrement::Linear(value)) => {
                target.x = target.x.saturating_add(value)
            }
            (RobotCartesianAxis::Y, RobotCartesianIncrement::Linear(value)) => {
                target.y = target.y.saturating_add(value)
            }
            (RobotCartesianAxis::Z, RobotCartesianIncrement::Linear(value)) => {
                target.z = target.z.saturating_add(value)
            }
            (RobotCartesianAxis::W, RobotCartesianIncrement::Angular(value)) => {
                target.w = target.w.saturating_add(value)
            }
            (RobotCartesianAxis::P, RobotCartesianIncrement::Angular(value)) => {
                target.p = target.p.saturating_add(value)
            }
            (RobotCartesianAxis::R, RobotCartesianIncrement::Angular(value)) => {
                target.r = target.r.saturating_add(value)
            }
            _ => return Err(RobotMotionError::InvalidIncrement),
        }
        self.command_pose(target, RobotMotionKind::Jog, speed_percent)
    }

    pub fn jog_joint(
        &mut self,
        axis: usize,
        increment: Angle,
        speed_percent: Percentage,
    ) -> Result<(), RobotMotionError> {
        let mut target = self.current_joints;
        let Some(value) = target.axes.get_mut(axis) else {
            return Err(RobotMotionError::OutsideWorkspace);
        };
        *value = value.saturating_add(increment);
        self.command_joints(target, speed_percent)
    }

    pub fn tick(&mut self, elapsed_ms: u64) -> bool {
        if !self.active {
            return false;
        }
        self.elapsed_ms = self
            .elapsed_ms
            .saturating_add(elapsed_ms)
            .min(self.duration_ms);
        self.current_pose =
            self.start_pose
                .interpolate(self.target_pose, self.elapsed_ms, self.duration_ms);
        self.current_joints =
            self.start_joints
                .interpolate(self.target_joints, self.elapsed_ms, self.duration_ms);
        if self.elapsed_ms < self.duration_ms {
            return false;
        }
        self.active = false;
        true
    }

    pub fn stop(&mut self) {
        self.start_pose = self.current_pose;
        self.target_pose = self.current_pose;
        self.start_joints = self.current_joints;
        self.target_joints = self.current_joints;
        self.elapsed_ms = 0;
        self.duration_ms = 0;
        self.speed_percent = Percentage::ZERO;
        self.active = false;
    }

    pub const fn pose(&self) -> RobotPose {
        self.current_pose
    }
    pub const fn target_pose(&self) -> RobotPose {
        self.target_pose
    }
    pub const fn joints(&self) -> RobotJoints {
        self.current_joints
    }
    pub const fn target_joints(&self) -> RobotJoints {
        self.target_joints
    }
    pub const fn home(&self) -> RobotPose {
        self.home
    }
    pub const fn workspace(&self) -> RobotWorkspace {
        self.workspace
    }
    pub const fn motion_kind(&self) -> RobotMotionKind {
        self.kind
    }
    pub const fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }
    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
    pub const fn speed_percent(&self) -> Percentage {
        self.speed_percent
    }
    pub const fn active(&self) -> bool {
        self.active
    }

    pub fn progress(&self) -> Percentage {
        if !self.active || self.duration_ms == 0 {
            return Percentage::from_raw(FULL_PERCENT);
        }
        Percentage::from_raw(
            ((self.elapsed_ms as u128) * (FULL_PERCENT as u128) / self.duration_ms as u128) as i64,
        )
    }

    fn begin(&mut self, kind: RobotMotionKind, speed_percent: Percentage, duration_ms: u64) {
        self.kind = kind;
        self.elapsed_ms = 0;
        self.duration_ms = duration_ms;
        self.speed_percent = speed_percent;
        self.active = duration_ms > 0;
        if !self.active {
            self.current_pose = self.target_pose;
            self.current_joints = self.target_joints;
        }
    }
}

fn validate_speed(speed: Percentage) -> Result<(), RobotMotionError> {
    if speed.raw() > 0 && speed.raw() <= FULL_PERCENT {
        Ok(())
    } else {
        Err(RobotMotionError::InvalidSpeed)
    }
}

fn duration(distance: u64, maximum_per_second: i64, speed: Percentage) -> u64 {
    if distance == 0 || maximum_per_second <= 0 || speed.raw() <= 0 {
        return 0;
    }
    let numerator = (distance as u128) * 1_000 * FULL_PERCENT as u128;
    let denominator = (maximum_per_second as u128) * speed.raw() as u128;
    ((numerator / denominator).min(u64::MAX as u128) as u64).max(100)
}

fn projected_joints(pose: RobotPose, workspace: RobotWorkspace) -> RobotJoints {
    let values = [
        pose.y.raw(),
        pose.z.raw(),
        pose.x.raw(),
        pose.w.raw(),
        pose.p.raw(),
        pose.r.raw(),
    ];
    let minimums = [
        workspace.minimum.y.raw(),
        workspace.minimum.z.raw(),
        workspace.minimum.x.raw(),
        workspace.minimum.w.raw(),
        workspace.minimum.p.raw(),
        workspace.minimum.r.raw(),
    ];
    let maximums = [
        workspace.maximum.y.raw(),
        workspace.maximum.z.raw(),
        workspace.maximum.x.raw(),
        workspace.maximum.w.raw(),
        workspace.maximum.p.raw(),
        workspace.maximum.r.raw(),
    ];
    let mut axes = [Angle::ZERO; 6];
    let mut index = 0;
    while index < axes.len() {
        axes[index] = Angle::from_raw(lerp_ratio(
            workspace.joint_minimum.axes[index].raw(),
            workspace.joint_maximum.axes[index].raw(),
            normalized(values[index], minimums[index], maximums[index]),
        ));
        index += 1;
    }
    RobotJoints { axes }
}

fn projected_pose(joints: RobotJoints, workspace: RobotWorkspace) -> RobotPose {
    let ratio = |index: usize| {
        normalized(
            joints.axes[index].raw(),
            workspace.joint_minimum.axes[index].raw(),
            workspace.joint_maximum.axes[index].raw(),
        )
    };
    RobotPose {
        x: Position::from_raw(lerp_ratio(
            workspace.minimum.x.raw(),
            workspace.maximum.x.raw(),
            ratio(2),
        )),
        y: Position::from_raw(lerp_ratio(
            workspace.minimum.y.raw(),
            workspace.maximum.y.raw(),
            ratio(0),
        )),
        z: Position::from_raw(lerp_ratio(
            workspace.minimum.z.raw(),
            workspace.maximum.z.raw(),
            ratio(1),
        )),
        w: Angle::from_raw(lerp_ratio(
            workspace.minimum.w.raw(),
            workspace.maximum.w.raw(),
            ratio(3),
        )),
        p: Angle::from_raw(lerp_ratio(
            workspace.minimum.p.raw(),
            workspace.maximum.p.raw(),
            ratio(4),
        )),
        r: Angle::from_raw(lerp_ratio(
            workspace.minimum.r.raw(),
            workspace.maximum.r.raw(),
            ratio(5),
        )),
    }
}

fn normalized(value: i64, minimum: i64, maximum: i64) -> i64 {
    let span = maximum.saturating_sub(minimum);
    if span <= 0 {
        return 0;
    }
    let offset = value.saturating_sub(minimum).clamp(0, span);
    ((offset as i128) * (FULL_PERCENT as i128) / (span as i128)) as i64
}

fn lerp_ratio(start: i64, target: i64, ratio: i64) -> i64 {
    let delta = target.saturating_sub(start);
    let adjustment =
        (delta as i128) * (ratio.clamp(0, FULL_PERCENT) as i128) / (FULL_PERCENT as i128);
    start.saturating_add(adjustment as i64)
}

fn lerp_raw(start: i64, target: i64, elapsed: u64, duration: u64) -> i64 {
    if duration == 0 {
        return target;
    }
    let delta = target.saturating_sub(start);
    let adjustment = (delta as i128) * (elapsed.min(duration) as i128) / (duration as i128);
    start.saturating_add(adjustment as i64)
}

fn absolute_delta(left: i64, right: i64) -> u64 {
    left.abs_diff(right)
}

const fn in_range(value: i64, minimum: i64, maximum: i64) -> bool {
    value >= minimum && value <= maximum
}
