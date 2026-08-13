#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RobotPose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
    pub p: f64,
    pub r: f64,
}

impl RobotPose {
    pub const fn new(x: f64, y: f64, z: f64, w: f64, p: f64, r: f64) -> Self {
        Self { x, y, z, w, p, r }
    }

    fn interpolate(self, target: Self, progress: f64) -> Self {
        Self {
            x: lerp(self.x, target.x, progress),
            y: lerp(self.y, target.y, progress),
            z: lerp(self.z, target.z, progress),
            w: lerp(self.w, target.w, progress),
            p: lerp(self.p, target.p, progress),
            r: lerp(self.r, target.r, progress),
        }
    }

    fn maximum_scaled_delta(self, target: Self) -> f64 {
        [
            (target.x - self.x).abs(),
            (target.y - self.y).abs(),
            (target.z - self.z).abs(),
            (target.w - self.w).abs() * 8.0,
            (target.p - self.p).abs() * 8.0,
            (target.r - self.r).abs() * 8.0,
        ]
        .into_iter()
        .fold(0.0, f64::max)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RobotJoints {
    pub axes: [f64; 6],
}

impl RobotJoints {
    pub const fn new(axes: [f64; 6]) -> Self {
        Self { axes }
    }

    fn interpolate(self, target: Self, progress: f64) -> Self {
        let mut axes = [0.0; 6];
        let mut index = 0;
        while index < axes.len() {
            axes[index] = lerp(self.axes[index], target.axes[index], progress);
            index += 1;
        }
        Self { axes }
    }

    fn maximum_delta(self, target: Self) -> f64 {
        self.axes
            .into_iter()
            .zip(target.axes)
            .map(|(current, requested)| (requested - current).abs())
            .fold(0.0, f64::max)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RobotWorkspace {
    pub minimum: RobotPose,
    pub maximum: RobotPose,
    pub joint_minimum: RobotJoints,
    pub joint_maximum: RobotJoints,
}

impl RobotWorkspace {
    pub fn contains_pose(self, pose: RobotPose) -> bool {
        in_range(pose.x, self.minimum.x, self.maximum.x)
            && in_range(pose.y, self.minimum.y, self.maximum.y)
            && in_range(pose.z, self.minimum.z, self.maximum.z)
            && in_range(pose.w, self.minimum.w, self.maximum.w)
            && in_range(pose.p, self.minimum.p, self.maximum.p)
            && in_range(pose.r, self.minimum.r, self.maximum.r)
    }

    pub fn contains_joints(self, joints: RobotJoints) -> bool {
        joints
            .axes
            .into_iter()
            .zip(self.joint_minimum.axes)
            .zip(self.joint_maximum.axes)
            .all(|((value, minimum), maximum)| in_range(value, minimum, maximum))
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
pub enum RobotMotionError {
    OutsideWorkspace,
    InvalidSpeed,
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
    speed_percent: f64,
    max_linear_speed_mm_s: f64,
    max_joint_speed_deg_s: f64,
    active: bool,
}

impl RobotMotionRuntime {
    pub fn new(
        workspace: RobotWorkspace,
        home: RobotPose,
        max_linear_speed_mm_s: f64,
        max_joint_speed_deg_s: f64,
    ) -> Result<Self, RobotMotionError> {
        if !workspace.contains_pose(home) {
            return Err(RobotMotionError::OutsideWorkspace);
        }
        if max_linear_speed_mm_s <= 0.0 || max_joint_speed_deg_s <= 0.0 {
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
            speed_percent: 0.0,
            max_linear_speed_mm_s,
            max_joint_speed_deg_s,
            active: false,
        })
    }

    pub fn command_pose(
        &mut self,
        target: RobotPose,
        kind: RobotMotionKind,
        speed_percent: f64,
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
        let distance = self.current_pose.maximum_scaled_delta(target);
        self.begin(
            kind,
            speed_percent,
            duration(distance, self.max_linear_speed_mm_s, speed_percent),
        );
        Ok(())
    }

    pub fn command_joints(
        &mut self,
        target: RobotJoints,
        speed_percent: f64,
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
        let distance = self.current_joints.maximum_delta(target);
        self.begin(
            RobotMotionKind::Joint,
            speed_percent,
            duration(distance, self.max_joint_speed_deg_s, speed_percent),
        );
        Ok(())
    }

    pub fn jog_cartesian(
        &mut self,
        axis: RobotCartesianAxis,
        increment: f64,
        speed_percent: f64,
    ) -> Result<(), RobotMotionError> {
        let mut target = self.current_pose;
        match axis {
            RobotCartesianAxis::X => target.x += increment,
            RobotCartesianAxis::Y => target.y += increment,
            RobotCartesianAxis::Z => target.z += increment,
            RobotCartesianAxis::W => target.w += increment,
            RobotCartesianAxis::P => target.p += increment,
            RobotCartesianAxis::R => target.r += increment,
        }
        self.command_pose(target, RobotMotionKind::Jog, speed_percent)
    }

    pub fn jog_joint(
        &mut self,
        axis: usize,
        increment: f64,
        speed_percent: f64,
    ) -> Result<(), RobotMotionError> {
        let mut target = self.current_joints;
        let Some(value) = target.axes.get_mut(axis) else {
            return Err(RobotMotionError::OutsideWorkspace);
        };
        *value += increment;
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
        let progress = self.progress();
        self.current_pose = self.start_pose.interpolate(self.target_pose, progress);
        self.current_joints = self.start_joints.interpolate(self.target_joints, progress);
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
        self.speed_percent = 0.0;
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

    pub const fn speed_percent(&self) -> f64 {
        self.speed_percent
    }

    pub const fn active(&self) -> bool {
        self.active
    }

    pub fn progress(&self) -> f64 {
        if !self.active || self.duration_ms == 0 {
            return 1.0;
        }
        self.elapsed_ms as f64 / self.duration_ms as f64
    }

    fn begin(&mut self, kind: RobotMotionKind, speed_percent: f64, duration_ms: u64) {
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

fn validate_speed(speed_percent: f64) -> Result<(), RobotMotionError> {
    if speed_percent.is_finite() && speed_percent > 0.0 && speed_percent <= 100.0 {
        Ok(())
    } else {
        Err(RobotMotionError::InvalidSpeed)
    }
}

fn duration(distance: f64, maximum_per_second: f64, speed_percent: f64) -> u64 {
    if distance <= f64::EPSILON {
        return 0;
    }
    let units_per_second = maximum_per_second * speed_percent / 100.0;
    ((distance / units_per_second * 1_000.0) as u64).max(100)
}

fn projected_joints(pose: RobotPose, workspace: RobotWorkspace) -> RobotJoints {
    let values = [pose.y, pose.z, pose.x, pose.w, pose.p, pose.r];
    let pose_minimum = [
        workspace.minimum.y,
        workspace.minimum.z,
        workspace.minimum.x,
        workspace.minimum.w,
        workspace.minimum.p,
        workspace.minimum.r,
    ];
    let pose_maximum = [
        workspace.maximum.y,
        workspace.maximum.z,
        workspace.maximum.x,
        workspace.maximum.w,
        workspace.maximum.p,
        workspace.maximum.r,
    ];
    let mut axes = [0.0; 6];
    let mut index = 0;
    while index < axes.len() {
        let ratio = normalized(values[index], pose_minimum[index], pose_maximum[index]);
        axes[index] = lerp(
            workspace.joint_minimum.axes[index],
            workspace.joint_maximum.axes[index],
            ratio,
        );
        index += 1;
    }
    RobotJoints { axes }
}

fn projected_pose(joints: RobotJoints, workspace: RobotWorkspace) -> RobotPose {
    let ratio = |index| {
        normalized(
            joints.axes[index],
            workspace.joint_minimum.axes[index],
            workspace.joint_maximum.axes[index],
        )
    };
    RobotPose {
        x: lerp(workspace.minimum.x, workspace.maximum.x, ratio(2)),
        y: lerp(workspace.minimum.y, workspace.maximum.y, ratio(0)),
        z: lerp(workspace.minimum.z, workspace.maximum.z, ratio(1)),
        w: lerp(workspace.minimum.w, workspace.maximum.w, ratio(3)),
        p: lerp(workspace.minimum.p, workspace.maximum.p, ratio(4)),
        r: lerp(workspace.minimum.r, workspace.maximum.r, ratio(5)),
    }
}

fn normalized(value: f64, minimum: f64, maximum: f64) -> f64 {
    ((value - minimum) / (maximum - minimum)).clamp(0.0, 1.0)
}

fn in_range(value: f64, minimum: f64, maximum: f64) -> bool {
    value.is_finite() && value >= minimum && value <= maximum
}

fn lerp(start: f64, target: f64, progress: f64) -> f64 {
    start + (target - start) * progress.clamp(0.0, 1.0)
}
