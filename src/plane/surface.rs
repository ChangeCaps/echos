use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;
use bevy_inspector_egui::Inspectable;
use bevy_prototype_debug_lines::DebugLines;
use serde::{Deserialize, Serialize};

#[derive(Inspectable, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceInputKind {
    Pitch,
    Yaw,
    Roll,
    Flap,
    None,
}

impl Default for SurfaceInputKind {
    fn default() -> Self {
        Self::Flap
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SurfaceInputState {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

#[derive(Inspectable, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceSide {
    Left,
    Right,
    Center,
}

impl Default for SurfaceSide {
    fn default() -> Self {
        Self::Center
    }
}

#[derive(Clone, Debug, Default)]
pub struct SurfaceForces {
    pub linear: Vec3,
    pub angular: Vec3,
}

const fn default_lift() -> f32 {
    1.0
}

#[derive(Inspectable, Clone, Debug, Serialize, Deserialize)]
pub struct PlaneSurface {
    pub input_kind: SurfaceInputKind,
    pub side: SurfaceSide,
    pub position: Vec3,
    pub rotation: Vec3,
    #[serde(default = "default_lift")]
    pub lift: f32,
    pub span: f32,
    pub chord: f32,
    pub lift_slope: f32,
    pub skin_friction: f32,
    pub zero_lift_aoa: f32,
    pub stall_angle_high: f32,
    pub stall_angle_low: f32,
    pub flap_fraction: f32,
}

impl Default for PlaneSurface {
    fn default() -> Self {
        Self {
            input_kind: SurfaceInputKind::None,
            side: SurfaceSide::Center,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            lift: 1.0,
            span: 0.0,
            chord: 0.0,
            lift_slope: 6.28,
            skin_friction: 0.02,
            zero_lift_aoa: 0.0,
            stall_angle_high: 20.0,
            stall_angle_low: -15.0,
            flap_fraction: 0.2,
        }
    }
}

impl PlaneSurface {
    pub fn rotation_quat(&self) -> Quat {
        Quat::from_euler(
            EulerRot::YXZ,
            self.rotation.y.to_radians(),
            self.rotation.x.to_radians(),
            self.rotation.z.to_radians(),
        )
    }

    pub fn input_flap_angle(&self, input: &SurfaceInputState) -> f32 {
        match self.input_kind {
            SurfaceInputKind::Pitch => input.pitch * 6.0,
            SurfaceInputKind::Yaw => -input.yaw * 24.0,
            SurfaceInputKind::Roll => match self.side {
                SurfaceSide::Left => -input.roll * 6.0,
                SurfaceSide::Right => input.roll * 6.0,
                SurfaceSide::Center => 0.0,
            },
            SurfaceInputKind::Flap => -input.pitch * 6.0,
            SurfaceInputKind::None => 0.0,
        }
    }

    pub fn calculate_forces(
        &self,
        world_air_velocity: Vec3,
        air_density: f32,
        relative_position: Vec3,
        position: Vec3,
        rotation: Quat,
        flap_angle: f32,
        lines: &mut DebugLines,
    ) -> SurfaceForces {
        let corrected_lift_slope = self.lift_slope * self.aspect()
            / (self.aspect() + 2.0 * (self.aspect() + 4.0) / (self.aspect() + 2.0));

        let theta = f32::acos(2.0 * self.flap_fraction - 1.0);
        let flap_effectiveness = 1.0 - (theta - theta.sin()) / PI;
        let delta_lift = corrected_lift_slope
            * flap_effectiveness
            * Self::flap_effectiveness_correction(flap_angle)
            * flap_angle;

        let zero_lift_aoa_base = self.zero_lift_aoa.to_radians();
        let zero_lift_aoa = zero_lift_aoa_base - delta_lift / corrected_lift_slope;

        let stall_angle_high_base = self.stall_angle_high.to_radians();
        let stall_angle_low_base = self.stall_angle_low.to_radians();

        let cl_max_high = corrected_lift_slope * (stall_angle_high_base - zero_lift_aoa_base)
            + delta_lift * Self::lift_coefficient_max_fraction(self.flap_fraction);
        let cl_max_low = corrected_lift_slope * (stall_angle_low_base - zero_lift_aoa_base)
            + delta_lift * Self::lift_coefficient_max_fraction(self.flap_fraction);

        let stall_angle_high = zero_lift_aoa + cl_max_high / corrected_lift_slope;
        let stall_angle_low = zero_lift_aoa + cl_max_low / corrected_lift_slope;

        let mut air_velocity = rotation.conjugate() * world_air_velocity;
        air_velocity.x = 0.0;
        let drag_direction = rotation * air_velocity.normalize_or_zero();
        let local_x = rotation * Vec3::X;
        let lift_direction = Vec3::cross(drag_direction, -local_x);

        let area = self.chord * self.span;
        let dynamic_pressure = 0.5 * air_density * air_velocity.length_squared();
        let angle_of_attack = f32::atan2(air_velocity.y, -air_velocity.z);

        let mut color = Color::BLUE;

        let coefficients = self.calculate_coefficients(
            angle_of_attack,
            corrected_lift_slope,
            zero_lift_aoa,
            stall_angle_high,
            stall_angle_low,
            flap_angle,
            &mut color,
        );

        let lift = lift_direction * coefficients.x * dynamic_pressure * area * self.lift;
        let drag = drag_direction * coefficients.y * dynamic_pressure * area * self.lift;
        let torque = local_x * coefficients.z * dynamic_pressure * area * self.chord * self.lift;

        if cfg!(feature = "debug") {
            lines.line_colored(position, position + lift * 0.01, 0.0, color);
            lines.line_colored(position, position + drag * 0.01, 0.0, Color::GREEN);
        }

        let linear = lift + drag;
        let angular = Vec3::cross(relative_position, linear) + torque;

        SurfaceForces { linear, angular }
    }

    fn calculate_coefficients(
        &self,
        angle_of_attack: f32,
        corrected_lift_slope: f32,
        zero_lift_aoa: f32,
        stall_angle_high: f32,
        stall_angle_low: f32,
        flap_angle: f32,
        color: &mut Color,
    ) -> Vec3 {
        let coefficients;

        let padding_angle_high =
            Self::lerp(15.0, 5.0, (flap_angle.to_degrees() + 50.0) / 100.0).to_radians();
        let padding_angle_low =
            Self::lerp(15.0, 5.0, (-flap_angle.to_degrees() + 50.0) / 100.0).to_radians();
        let padding_stall_angle_high = stall_angle_high + padding_angle_high;
        let padding_stall_angle_low = stall_angle_low - padding_angle_low;

        if angle_of_attack < stall_angle_high && angle_of_attack > stall_angle_low {
            coefficients = self.calculate_coefficients_at_low_aoa(
                angle_of_attack,
                corrected_lift_slope,
                zero_lift_aoa,
            );
        } else {
            if angle_of_attack > padding_stall_angle_high
                || angle_of_attack < padding_stall_angle_low
            {
                *color = Color::ORANGE_RED;

                coefficients = self.calculate_coefficients_at_stall(
                    angle_of_attack,
                    corrected_lift_slope,
                    zero_lift_aoa,
                    stall_angle_high,
                    stall_angle_low,
                    flap_angle,
                );
            } else {
                let coefficients_low;
                let coefficients_stall;
                let lerp_param;

                if angle_of_attack > stall_angle_high {
                    coefficients_low = self.calculate_coefficients_at_low_aoa(
                        stall_angle_high,
                        corrected_lift_slope,
                        zero_lift_aoa,
                    );
                    coefficients_stall = self.calculate_coefficients_at_stall(
                        padding_stall_angle_high,
                        corrected_lift_slope,
                        zero_lift_aoa,
                        stall_angle_high,
                        stall_angle_low,
                        flap_angle,
                    );
                    lerp_param = (angle_of_attack - stall_angle_high)
                        / (padding_stall_angle_high - stall_angle_high)
                } else {
                    coefficients_low = self.calculate_coefficients_at_low_aoa(
                        stall_angle_low,
                        corrected_lift_slope,
                        zero_lift_aoa,
                    );
                    coefficients_stall = self.calculate_coefficients_at_stall(
                        padding_stall_angle_low,
                        corrected_lift_slope,
                        zero_lift_aoa,
                        stall_angle_high,
                        stall_angle_low,
                        flap_angle,
                    );
                    lerp_param = (angle_of_attack - stall_angle_low)
                        / (padding_stall_angle_low - stall_angle_low)
                }

                *color =
                    Vec4::lerp(Color::BLUE.into(), Color::ORANGE_RED.into(), lerp_param).into();

                coefficients = Vec3::lerp(coefficients_low, coefficients_stall, lerp_param);
            }
        }

        coefficients
    }

    fn calculate_coefficients_at_low_aoa(
        &self,
        angle_of_attack: f32,
        corrected_lift_slope: f32,
        zero_lift_aoa: f32,
    ) -> Vec3 {
        let lift_coefficient = corrected_lift_slope * (angle_of_attack - zero_lift_aoa);
        let induced_angle = lift_coefficient / (PI * self.aspect());
        let effective_angle = angle_of_attack - zero_lift_aoa - induced_angle;

        let tangential_coefficient = self.skin_friction * effective_angle.cos();

        let normal_coefficient = (lift_coefficient
            + effective_angle.sin() * tangential_coefficient)
            / effective_angle.cos();
        let drag_coefficient = normal_coefficient * effective_angle.sin()
            + tangential_coefficient * effective_angle.cos();
        let torque_coefficient =
            -normal_coefficient + Self::torq_coefficient_proportion(effective_angle);

        Vec3::new(lift_coefficient, drag_coefficient, torque_coefficient)
    }

    fn calculate_coefficients_at_stall(
        &self,
        angle_of_attack: f32,
        corrected_lift_slope: f32,
        zero_lift_aoa: f32,
        stall_angle_high: f32,
        stall_angle_low: f32,
        flap_angle: f32,
    ) -> Vec3 {
        let lift_coefficient_low_aoa = if angle_of_attack > stall_angle_high {
            corrected_lift_slope * (stall_angle_high - zero_lift_aoa)
        } else {
            corrected_lift_slope * (stall_angle_low - zero_lift_aoa)
        };

        let mut induced_angle = lift_coefficient_low_aoa / (self.aspect() * PI);

        let lerp_param = if angle_of_attack > stall_angle_high {
            (FRAC_PI_2 - f32::clamp(angle_of_attack, -FRAC_PI_2, FRAC_PI_2))
                / (FRAC_PI_2 - stall_angle_high)
        } else {
            (-FRAC_PI_2 - f32::clamp(angle_of_attack, -FRAC_PI_2, FRAC_PI_2))
                / (-FRAC_PI_2 - stall_angle_low)
        };

        induced_angle = Self::lerp(0.0, induced_angle, lerp_param);
        let effective_angle = angle_of_attack - zero_lift_aoa - induced_angle;

        let normal_coefficient = Self::friction_at_90_degrees(flap_angle)
            * effective_angle.sin()
            * (1.0 / (0.56 + 0.44 * effective_angle.sin().abs()))
            - 0.41 * (1.0 - f32::exp(-17.0 / self.aspect()));
        let tangent_coefficient = 0.5 * self.skin_friction * effective_angle.cos();

        let lift_coefficient = normal_coefficient * effective_angle.cos()
            - tangent_coefficient * effective_angle.sin();
        let drag_coefficient = normal_coefficient * effective_angle.sin()
            + tangent_coefficient * effective_angle.cos();
        let torque_coefficient =
            -normal_coefficient * Self::torq_coefficient_proportion(effective_angle);

        Vec3::new(lift_coefficient, drag_coefficient, torque_coefficient)
    }

    fn aspect(&self) -> f32 {
        self.span / self.chord
    }

    fn torq_coefficient_proportion(effective_angle: f32) -> f32 {
        0.25 - 0.175 * (1.0 - 2.0 * effective_angle.abs() / PI)
    }

    fn friction_at_90_degrees(flap_angle: f32) -> f32 {
        1.98 - 4.26e-2 * flap_angle * flap_angle + 2.1e-1 * flap_angle
    }

    fn flap_effectiveness_correction(flap_angle: f32) -> f32 {
        Self::lerp(0.8, 0.4, (flap_angle.abs().to_degrees() - 10.0) / 50.0)
    }

    fn lift_coefficient_max_fraction(flap_fraction: f32) -> f32 {
        f32::clamp(1.0 - 0.5 * (flap_fraction - 0.1) / 0.3, 0.0, 1.0)
    }

    fn lerp(a: f32, b: f32, x: f32) -> f32 {
        (1.0 - x) * a + x * b
    }
}
