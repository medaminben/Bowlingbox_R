//! The [`PhysicsWorld`] of the game, on Rapier, ported from the C++ `BulletWorld.cpp`.
//!
//! Rapier types stay inside this module; the trait speaks `[f32; 3]` and `[f32; 4]`.

use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use rapier3d::math::{Pose as RPose, Real, Vector};
use rapier3d::prelude::{
    ActiveEvents, CoefficientCombineRule, ColliderBuilder, ColliderHandle, ColliderSet,
    CollisionEvent, ContactPair, EventHandler, PhysicsWorld as Engine, RigidBody, RigidBodyBuilder,
    RigidBodyHandle, RigidBodySet, SharedShape, SoftBodySet, SoftBodyTearEvent,
};

use crate::types::{BALL_START_Z, Body, Impact, LANE, Pose};
use crate::world::PhysicsWorld;

/// Lateral ball offset in metres of a throw that usually strikes, with
/// [`POCKET_ANGLE_DEG`] and [`POCKET_POWER`]. The middle of the offsets 0.040 to 0.065 m
/// that strike at every power from 0.6 to 1.0.
pub const POCKET_OFFSET: f32 = 0.05;
/// Throw angle in degrees of a throw that usually strikes.
pub const POCKET_ANGLE_DEG: f32 = 0.0;
/// Throw power in `[0, 1]` of a throw that usually strikes.
pub const POCKET_POWER: f32 = 0.8;

const SUB_STEP: f32 = 1.0 / 120.0;
const MAX_SUB_STEPS: u32 = 4;

const PIT_DEPTH: f32 = 0.8;
const PIT_LENGTH: f32 = 2.4;
const BALL_MASS: f32 = 7.26;
const PIN_MASS: f32 = 1.5;
/// Below these a body counts as at rest (m/s, rad/s), as in C++.
const SLOW_LINEAR: f32 = 0.05;
const SLOW_ANGULAR: f32 = 0.2;
/// Rapier puts a body to sleep below these (m/s, rad/s), as Bullet's sleeping thresholds.
const SLEEP_LINEAR: f32 = 0.05;
const SLEEP_ANGULAR: f32 = 0.1;
/// cos 45°: a pin whose up axis leans less than this is standing.
const STANDING_COS: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// N·s; a resting pin pushes about 0.12 per sub-step.
const IMPACT_THRESHOLD: f32 = 0.3;
const MAX_IMPACTS: usize = 256;
/// The rack bounces after spawning; those pin contacts are not a hit. Pin contacts count
/// once the ball is this close to the head pin.
const PIN_IMPACT_MIN_BALL_Z: f32 = LANE.head_pin_z - 2.0;
/// Below this height the ball is in the pit.
const PIT_BALL_Y: f32 = -0.3;

// Surfaces combine by multiplying, as in Bullet: static surfaces use 1.0 and the dynamic
// bodies carry the real value.
const STATIC_FRICTION: f32 = 1.0;
const STATIC_RESTITUTION: f32 = 1.0;
const PIT_RESTITUTION: f32 = 0.2;
// Tuned on a strike map over the pocket (offset 0.02 to 0.13 m, power 0.6 to 1.0) with the
// pin centre of mass below; C++ values in brackets.
/// As C++.
const BALL_FRICTION: f32 = 0.1;
/// [0.1] Ball-pin 0.3 · 0.7 = 0.21: the pins fly a little more, widening the pocket.
const BALL_RESTITUTION: f32 = 0.3;
/// As C++: pin-pin 0.09 and pin-deck 0.3.
const PIN_FRICTION: f32 = 0.3;
/// [0.5] Livelier pin-pin hits (0.49) carry the knockdown through the rack.
const PIN_RESTITUTION: f32 = 0.7;
/// Rapier has no rolling friction: the ball loses this much speed per second while it rolls
/// (m/s², C++ none). A 4 m/s throw stops after about 11 m, a 7 m/s throw reaches the pins.
const BALL_ROLLING_DECEL: f32 = 0.7;
/// Stands in for Bullet's rolling friction on the pins (1/s); 0 to 1.5 made no difference.
const PIN_ANGULAR_DAMPING: f32 = 0.5;

/// Collider tag of each body kind, kept in the collider's `user_data`.
fn tag(body: Body) -> u128 {
    match body {
        Body::Ball => 0,
        Body::Pin => 1,
        Body::Lane => 2,
        Body::Gutter => 3,
        Body::Pit => 4,
        Body::Other => 5,
    }
}

fn body_of(tag: u128) -> Body {
    match tag {
        0 => Body::Ball,
        1 => Body::Pin,
        2 => Body::Lane,
        3 => Body::Gutter,
        4 => Body::Pit,
        _ => Body::Other,
    }
}

/// The ten pin spots: head pin first, then row by row from left to right.
fn pin_spots() -> [(f32, f32); 10] {
    let s = LANE.pin_spacing;
    let row = s * 0.866_025_4; // s · cos 30°
    let z0 = LANE.head_pin_z;
    [
        (0.0, z0),
        (-s / 2.0, z0 + row),
        (s / 2.0, z0 + row),
        (-s, z0 + 2.0 * row),
        (0.0, z0 + 2.0 * row),
        (s, z0 + 2.0 * row),
        (-1.5 * s, z0 + 3.0 * row),
        (-s / 2.0, z0 + 3.0 * row),
        (s / 2.0, z0 + 3.0 * row),
        (1.5 * s, z0 + 3.0 * row),
    ]
}

fn to_pose(p: &RPose) -> Pose {
    let t = p.translation;
    let r = p.rotation;
    Pose {
        position: [t.x, t.y, t.z],
        rotation: [r.x, r.y, r.z, r.w],
    }
}

/// A touching pair of colliders, in a fixed order.
type Pair = (ColliderHandle, ColliderHandle);

fn pair(a: ColliderHandle, b: ColliderHandle) -> Pair {
    if a.0 <= b.0 { (a, b) } else { (b, a) }
}

#[derive(Default)]
struct Recorded {
    impacts: Vec<Impact>,
    /// Touching pairs that already had their one force event; dropped when they separate.
    seen: HashSet<Pair>,
}

/// Records each new contact once, from the first contact-force event of a touching pair.
///
/// Rapier's `ContactForceEvent::started` marks the force crossing the threshold, which a
/// continuous contact repeats whenever its force dips and rises; collision events tell when
/// the pair separates and may report again.
struct ImpactCollector {
    ball: RigidBodyHandle,
    recorded: Mutex<Recorded>,
}

impl ImpactCollector {
    fn recorded(&mut self) -> &mut Recorded {
        self.recorded
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn take(&mut self) -> Vec<Impact> {
        std::mem::take(&mut self.recorded().impacts)
    }

    /// Drops pending impacts and forgets the pairs that involve `collider`.
    fn forget(&mut self, collider: ColliderHandle) {
        let recorded = self.recorded();
        recorded.impacts.clear();
        recorded
            .seen
            .retain(|&(a, b)| a != collider && b != collider);
    }
}

/// World position of the contact point with the largest impulse.
fn strongest_point(pair: &ContactPair, colliders: &ColliderSet) -> [f32; 3] {
    let Some(c1) = colliders.get(pair.collider1) else {
        return [0.0; 3];
    };
    let mut best: Option<(Real, Vector)> = None;
    for m in pair.manifolds() {
        let sub = m.subshape_pos1().copied().unwrap_or(RPose::IDENTITY);
        for pt in &m.points {
            if best.is_none_or(|(impulse, _)| pt.data.impulse > impulse) {
                best = Some((pt.data.impulse, *c1.position() * (sub * pt.local_p1)));
            }
        }
    }
    best.map_or([0.0; 3], |(_, p)| [p.x, p.y, p.z])
}

impl EventHandler for ImpactCollector {
    fn handle_collision_event(
        &self,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        event: CollisionEvent,
        _contact_pair: Option<&ContactPair>,
    ) {
        if let CollisionEvent::Stopped(a, b, _) = event {
            let mut recorded = self.recorded.lock().unwrap_or_else(PoisonError::into_inner);
            recorded.seen.remove(&pair(a, b));
        }
    }

    fn handle_contact_force_event(
        &self,
        dt: Real,
        bodies: &RigidBodySet,
        colliders: &ColliderSet,
        contact_pair: &ContactPair,
        total_force_magnitude: Real,
    ) {
        let impulse = total_force_magnitude * dt;
        if impulse < IMPACT_THRESHOLD {
            return;
        }
        let mut recorded = self.recorded.lock().unwrap_or_else(PoisonError::into_inner);
        if !recorded
            .seen
            .insert(pair(contact_pair.collider1, contact_pair.collider2))
        {
            return;
        }
        let kind = |h| {
            colliders
                .get(h)
                .map_or(Body::Other, |c| body_of(c.user_data))
        };
        let (a, b) = (kind(contact_pair.collider1), kind(contact_pair.collider2));
        let ball_z = bodies.get(self.ball).map_or(0.0, |b| b.translation().z);
        if (a == Body::Pin || b == Body::Pin) && ball_z < PIN_IMPACT_MIN_BALL_Z {
            return;
        }
        if recorded.impacts.len() >= MAX_IMPACTS {
            return;
        }
        recorded.impacts.push(Impact {
            a: a.min(b),
            b: a.max(b),
            impulse,
            position: strongest_point(contact_pair, colliders),
        });
    }

    fn handle_soft_body_tear_event(&self, _soft_bodies: &SoftBodySet, _event: &SoftBodyTearEvent) {}
}

/// Rigid-body world on Rapier: lane, gutters with outer walls, pit, one ball and ten pins.
///
/// [`PhysicsWorld::step`] runs fixed sub-steps of 1/120 s, at most four per call, like
/// Bullet's `stepSimulation(dt, 4, 1/120)`: the remainder carries over, time beyond four
/// sub-steps is dropped.
pub struct RapierWorld {
    engine: Engine,
    ball: RigidBodyHandle,
    pins: [RigidBodyHandle; 10],
    in_play: [bool; 10],
    pending: f32,
    collector: ImpactCollector,
}

impl Default for RapierWorld {
    fn default() -> Self {
        Self::new()
    }
}

fn surface(
    builder: ColliderBuilder,
    body: Body,
    friction: f32,
    restitution: f32,
) -> ColliderBuilder {
    builder
        .user_data(tag(body))
        .friction(friction)
        .friction_combine_rule(CoefficientCombineRule::Multiply)
        .restitution(restitution)
        .restitution_combine_rule(CoefficientCombineRule::Multiply)
        .active_events(ActiveEvents::CONTACT_FORCE_EVENTS | ActiveEvents::COLLISION_EVENTS)
        .contact_force_event_threshold(IMPACT_THRESHOLD / SUB_STEP)
}

fn set_sleep_thresholds(body: &mut RigidBody) {
    let activation = body.activation_mut();
    activation.normalized_linear_threshold = SLEEP_LINEAR;
    activation.angular_threshold = SLEEP_ANGULAR;
}

/// Two stacked cylinders as in C++, origin `pin_origin_height` above the base.
fn pin_collider() -> ColliderBuilder {
    let d = LANE;
    let lower = SharedShape::cylinder(0.08, d.pin_max_radius);
    let upper = SharedShape::cylinder(0.09, 0.035);
    let lower_y = 0.10 - d.pin_origin_height;
    let upper_y = 0.29 - d.pin_origin_height;
    let shape = SharedShape::compound(vec![
        (RPose::translation(0.0, lower_y, 0.0), lower),
        (RPose::translation(0.0, upper_y, 0.0), upper),
    ]);
    // The centre of mass follows from the shape, about 3 cm above the origin. Bullet put it
    // at the origin; the higher one topples like a real pin and doubles the strikes.
    surface(
        ColliderBuilder::new(shape),
        Body::Pin,
        PIN_FRICTION,
        PIN_RESTITUTION,
    )
    .mass(PIN_MASS)
}

impl RapierWorld {
    #[must_use]
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.gravity = Vector::new(0.0, -9.81, 0.0);
        engine.integration_parameters.dt = SUB_STEP;

        let d = LANE;
        let outer_x = d.half_width + d.gutter_width;
        let add_static = |engine: &mut Engine, body, half: Vector, at: Vector, restitution| {
            let collider = ColliderBuilder::cuboid(half.x, half.y, half.z).translation(at);
            engine.insert_collider(surface(collider, body, STATIC_FRICTION, restitution), None);
        };

        // Lane: approach + lane, top surface at y = 0.
        let full = d.length + d.approach;
        let mid_z = (d.length - d.approach) / 2.0;
        add_static(
            &mut engine,
            Body::Lane,
            Vector::new(d.half_width, 0.05, full / 2.0),
            Vector::new(0.0, -0.05, mid_z),
            STATIC_RESTITUTION,
        );
        // Gutters, gutter_depth below the lane, along the approach too so an edge ball on
        // the approach does not drop. Outer walls: inner face at outer_x + 0.06 = 0.825,
        // beyond MAX_AIM_OFFSET + ball radius (0.809).
        for sx in [-1.0, 1.0] {
            add_static(
                &mut engine,
                Body::Gutter,
                Vector::new(d.gutter_width / 2.0, 0.05, full / 2.0),
                Vector::new(
                    sx * (d.half_width + d.gutter_width / 2.0),
                    -d.gutter_depth - 0.05,
                    mid_z,
                ),
                STATIC_RESTITUTION,
            );
            add_static(
                &mut engine,
                Body::Gutter,
                Vector::new(0.02, 0.15, full / 2.0),
                Vector::new(sx * (outer_x + 0.08), 0.05, mid_z),
                STATIC_RESTITUTION,
            );
        }
        // Pit behind the deck: floor, back wall, side walls.
        let pit_z = d.length + PIT_LENGTH / 2.0;
        add_static(
            &mut engine,
            Body::Pit,
            Vector::new(outer_x + 0.12, 0.05, PIT_LENGTH / 2.0),
            Vector::new(0.0, -PIT_DEPTH - 0.05, pit_z),
            PIT_RESTITUTION,
        );
        add_static(
            &mut engine,
            Body::Pit,
            Vector::new(outer_x + 0.12, 0.6, 0.05),
            Vector::new(0.0, -0.3, d.length + PIT_LENGTH + 0.05),
            PIT_RESTITUTION,
        );
        for sx in [-1.0, 1.0] {
            add_static(
                &mut engine,
                Body::Pit,
                Vector::new(0.05, 0.6, PIT_LENGTH / 2.0),
                Vector::new(sx * (outer_x + 0.17), -0.3, pit_z),
                PIT_RESTITUTION,
            );
        }

        let mut ball_body = RigidBodyBuilder::dynamic()
            .translation(Vector::new(0.0, d.ball_radius, BALL_START_Z))
            .ccd_enabled(true)
            .build();
        set_sleep_thresholds(&mut ball_body);
        let ball_collider = surface(
            ColliderBuilder::ball(d.ball_radius),
            Body::Ball,
            BALL_FRICTION,
            BALL_RESTITUTION,
        )
        .mass(BALL_MASS);
        let (ball, _) = engine.insert(ball_body, ball_collider);

        let spots = pin_spots();
        let pins = std::array::from_fn(|i| {
            let (x, z) = spots[i];
            let mut body = RigidBodyBuilder::dynamic()
                .translation(Vector::new(x, d.pin_origin_height, z))
                .ccd_enabled(true)
                .angular_damping(PIN_ANGULAR_DAMPING)
                .build();
            set_sleep_thresholds(&mut body);
            engine.insert(body, pin_collider()).0
        });

        Self {
            engine,
            ball,
            pins,
            in_play: [true; 10],
            pending: 0.0,
            collector: ImpactCollector {
                ball,
                recorded: Mutex::default(),
            },
        }
    }

    fn body(&self, h: RigidBodyHandle) -> &RigidBody {
        &self.engine.bodies[h]
    }

    fn teleport(&mut self, h: RigidBodyHandle, at: Vector) {
        let body = &mut self.engine.bodies[h];
        body.set_position(RPose::from_translation(at), true);
        body.set_linvel(Vector::ZERO, true);
        body.set_angvel(Vector::ZERO, true);
        body.reset_forces(true);
        body.reset_torques(true);
    }

    /// Rolling resistance: slows the ball's speed and spin together while it is on a surface.
    fn roll_ball(&mut self) {
        let body = &mut self.engine.bodies[self.ball];
        let y = body.translation().y;
        if y > LANE.ball_radius + 0.01 || y < PIT_BALL_Y {
            return;
        }
        let v = body.linvel();
        let speed = v.length();
        if speed <= 0.0 {
            return;
        }
        let k = (1.0 - BALL_ROLLING_DECEL * SUB_STEP / speed).max(0.0);
        let w = body.angvel();
        body.set_linvel(v * k, false);
        body.set_angvel(w * k, false);
    }

    fn sub_step(&mut self) {
        self.roll_ball();
        self.engine.step_with_events(&(), &self.collector);
    }

    fn slow(body: &RigidBody) -> bool {
        body.is_sleeping()
            || !body.is_enabled()
            || (body.linvel().length() < SLOW_LINEAR && body.angvel().length() < SLOW_ANGULAR)
    }
}

impl PhysicsWorld for RapierWorld {
    fn step(&mut self, dt: f32) {
        // Also rejects NaN, which would stop the world for good.
        if dt.is_nan() || dt <= 0.0 {
            return;
        }
        self.pending += dt;
        let due = (self.pending / SUB_STEP).floor();
        self.pending -= due * SUB_STEP;
        // `due` is a small non-negative whole number here.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let due = due.max(0.0) as u32;
        for _ in 0..due.min(MAX_SUB_STEPS) {
            self.sub_step();
        }
    }

    fn place_ball(&mut self, offset: f32) {
        self.teleport(
            self.ball,
            Vector::new(offset, LANE.ball_radius, BALL_START_Z),
        );
    }

    fn launch_ball(&mut self, offset: f32, angle_deg: f32, speed: f32) {
        self.place_ball(offset);
        let a = angle_deg.to_radians();
        let dir = Vector::new(a.sin(), 0.0, a.cos());
        // Rolling without slipping: omega = (v / r) * (up x dir). The C++ used dir x up,
        // which starts the ball with backspin.
        let spin = Vector::Y.cross(dir) * (speed / LANE.ball_radius);
        let body = &mut self.engine.bodies[self.ball];
        body.set_linvel(dir * speed, true);
        body.set_angvel(spin, true);
    }

    fn reset_rack(&mut self) {
        let spots = pin_spots();
        for (i, (x, z)) in spots.into_iter().enumerate() {
            self.engine.bodies[self.pins[i]].set_enabled(true);
            self.in_play[i] = true;
            self.teleport(self.pins[i], Vector::new(x, LANE.pin_origin_height, z));
        }
    }

    /// Recreates the world from scratch and drops pending impacts.
    fn rebuild(&mut self) {
        *self = Self::new();
    }

    fn remove_pin(&mut self, i: usize) {
        if i >= 10 || !self.in_play[i] {
            return;
        }
        let pin = &mut self.engine.bodies[self.pins[i]];
        pin.set_enabled(false);
        let colliders = pin.colliders().to_vec();
        self.in_play[i] = false;
        for collider in colliders {
            self.collector.forget(collider);
        }
    }

    fn ball_pose(&self) -> Pose {
        to_pose(self.body(self.ball).position())
    }

    fn pin_pose(&self, i: usize) -> Pose {
        to_pose(self.body(self.pins[i]).position())
    }

    fn is_pin_standing(&self, i: usize) -> bool {
        if !self.in_play[i] {
            return false;
        }
        let p = self.body(self.pins[i]).position();
        let up = p.rotation * Vector::Y;
        let o = p.translation;
        up.y > STANDING_COS
            && o.x.abs() < LANE.half_width + 0.1
            && o.z < LANE.length + 0.05
            && o.y > 0.0
    }

    fn is_pin_in_play(&self, i: usize) -> bool {
        self.in_play[i]
    }

    fn ball_in_pit(&self) -> bool {
        self.body(self.ball).translation().y < PIT_BALL_Y
    }

    fn ball_speed(&self) -> f32 {
        self.body(self.ball).linvel().length()
    }

    fn all_slow(&self) -> bool {
        Self::slow(self.body(self.ball))
            && (0..10).all(|i| !self.in_play[i] || Self::slow(self.body(self.pins[i])))
    }

    fn drain_impacts(&mut self) -> Vec<Impact> {
        self.collector.take()
    }
}
