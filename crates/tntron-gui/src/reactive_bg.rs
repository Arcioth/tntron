use std::f32::consts::TAU;
use std::time::Instant;

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Frame, Path, Stroke};
use iced::widget::image;
use iced::{Color, Element, Length, Point, Radians, Rectangle, Renderer, Size, Theme, Vector};
use tntron_core::ReactiveUiConfig;

#[derive(Debug, Clone, Copy)]
struct ReactiveParticle {
    angle: f32,
    dist: f32,
    speed: f32,
    size: f32,
    hue_offset: f32,
}

#[derive(Debug, Clone, Copy)]
struct GlowOrb {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    radius: f32,
    twinkle_phase: f32,
    twinkle_speed: f32,
    hue_offset: f32,
}

#[derive(Debug, Clone)]
pub struct ReactiveBgState {
    pub hue_deg: f32,
    pub bass: f32,
    pub vocal: f32,
    pub drum_pulse: f32,
    pub bands: [f32; 16],
    pub phase: f32,
    prev_drum: f32,
    last_tick: Instant,
    particles: Vec<ReactiveParticle>,
    glow_orbs: Vec<GlowOrb>,
}

impl ReactiveBgState {
    pub fn new() -> Self {
        let mut particles = Vec::with_capacity(140);
        let mut glow_orbs = Vec::with_capacity(48);
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next_f32 = || -> f32 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as f32) / ((1u64 << 31) as f32)
        };
        for _ in 0..140 {
            particles.push(ReactiveParticle {
                angle: next_f32() * TAU,
                dist: 0.05 + next_f32() * 1.15,
                speed: 0.055 + next_f32() * 0.14,
                size: 1.4 + next_f32() * 3.0,
                hue_offset: (next_f32() - 0.5) * 70.0,
            });
        }
        for _ in 0..48 {
            glow_orbs.push(GlowOrb {
                x: next_f32(),
                y: next_f32(),
                vx: (next_f32() - 0.5) * 0.032,
                vy: (next_f32() - 0.5) * 0.028,
                radius: 8.0 + next_f32() * 24.0,
                twinkle_phase: next_f32() * TAU,
                twinkle_speed: 1.4 + next_f32() * 3.2,
                hue_offset: (next_f32() - 0.5) * 95.0,
            });
        }
        Self {
            hue_deg: 210.0,
            bass: 0.0,
            vocal: 0.0,
            drum_pulse: 0.0,
            bands: [0.0; 16],
            phase: 0.0,
            prev_drum: 0.0,
            last_tick: Instant::now(),
            particles,
            glow_orbs,
        }
    }

    /// Advances reactive hue, bass/vocal envelopes, drum-hit impulse, and zooming particles.
    pub fn on_frame(
        &mut self,
        playing: bool,
        bands: &[f32; 16],
        raw_beat: f32,
        cfg: &ReactiveUiConfig,
    ) {
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.last_tick)
            .as_secs_f32()
            .clamp(0.001, 0.08);
        self.last_tick = now;

        if !cfg.enabled {
            self.bass *= 0.8;
            self.vocal *= 0.8;
            self.drum_pulse *= 0.8;
            return;
        }

        self.phase = (self.phase + dt * 1.8) % TAU;
        for (dst, &src) in self.bands.iter_mut().zip(bands.iter()) {
            *dst = if playing {
                *dst * 0.42 + src * 0.58
            } else {
                *dst * 0.85
            };
        }

        let raw_bass =
            (bands[0] * 1.3 + bands[1] * 1.15 + bands[2] * 0.95 + bands[3] * 0.7) / 4.1;
        let raw_vocal = (bands[4] * 0.8
            + bands[5]
            + bands[6] * 1.25
            + bands[7] * 1.25
            + bands[8] * 1.15
            + bands[9]
            + bands[10] * 0.7)
            / 7.15;

        if playing {
            self.bass = self.bass * 0.45 + raw_bass * 0.55;
            self.vocal = self.vocal * 0.55 + raw_vocal * 0.45;
        } else {
            self.bass = (self.bass * 0.9 + 0.04 * 0.1).min(0.15);
            self.vocal *= 0.88;
        }

        // Drum hit onset (beat flux + kick/snare crest).
        let drum_now = if playing {
            (raw_beat * 0.75 + ((bands[1] + bands[11] + bands[12]) / 3.0) * 0.48).min(1.0)
        } else {
            0.0
        };
        let drum_rise = (drum_now - self.prev_drum).max(0.0);
        self.prev_drum = self.prev_drum * 0.68 + drum_now * 0.32;
        if playing && (drum_now > 0.34 && drum_rise > 0.06 || raw_beat > 0.55) {
            self.drum_pulse = (self.drum_pulse + drum_rise * 1.6 + raw_beat * 0.35).min(1.65);
        } else {
            self.drum_pulse = (self.drum_pulse * (1.0 - dt * 5.2)).max(0.0);
        }

        // Continuous hue rotation whose velocity reacts dynamically to vocal presence.
        if cfg.hue_loop {
            let vocal_boost = if playing {
                self.vocal * cfg.hue_vocal_react * 260.0
            } else {
                0.0
            };
            let deg_per_sec = cfg.hue_speed * 34.0 + vocal_boost;
            self.hue_deg = (self.hue_deg + dt * deg_per_sec) % 360.0;
        }

        // Zoom particles outward radially, accelerating sharply on drum hits.
        let active_count = cfg.particle_count.min(self.particles.len());
        let zoom_mul = if playing {
            1.0 + self.drum_pulse * cfg.particle_zoom * 4.4
        } else {
            0.32
        };
        for (idx, pt) in self.particles.iter_mut().take(active_count).enumerate() {
            pt.dist += pt.speed * dt * zoom_mul;
            if pt.dist > 1.24 {
                pt.dist = 0.03 + ((idx % 17) as f32) * 0.011;
            }
        }

        // Advance floating luminous glow orbs / bubbles / sparkles.
        if cfg.glow_particles_enabled || cfg.custom_particle_enabled {
            let drift_mul = if playing {
                1.0 + self.vocal * 1.6 + self.drum_pulse * 1.4
            } else {
                0.35
            };
            let is_bubbles = cfg.glow_particle_style.eq_ignore_ascii_case("bubbles");
            for orb in &mut self.glow_orbs {
                orb.twinkle_phase = (orb.twinkle_phase + dt * orb.twinkle_speed) % TAU;
                if is_bubbles {
                    let sway = (self.phase * 1.2 + orb.twinkle_phase).sin() * 0.018;
                    let rise = (0.045 + orb.radius * 0.0022) * drift_mul;
                    orb.x = (orb.x + (orb.vx * 0.4 + sway) * dt).rem_euclid(1.08) - 0.04;
                    orb.y = (orb.y - rise * dt).rem_euclid(1.10) - 0.05;
                } else {
                    let wobble_x = (self.phase + orb.twinkle_phase).cos() * 0.006;
                    let wobble_y = (self.phase * 0.8 + orb.twinkle_phase).sin() * 0.006;
                    orb.x = (orb.x + (orb.vx + wobble_x) * dt * drift_mul).rem_euclid(1.08) - 0.04;
                    orb.y = (orb.y + (orb.vy + wobble_y) * dt * drift_mul).rem_euclid(1.08) - 0.04;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReactiveBgProgram {
    pub state: ReactiveBgState,
    pub cfg: ReactiveUiConfig,
    pub accent: Color,
    pub base: Color,
    pub has_custom_bg: bool,
    pub custom_sprite: Option<image::Handle>,
    pub playing: bool,
}

pub fn reactive_bg_canvas<'a, Message: 'a>(
    state: &ReactiveBgState,
    cfg: &ReactiveUiConfig,
    accent: Color,
    base: Color,
    has_custom_bg: bool,
    custom_sprite: Option<image::Handle>,
    playing: bool,
) -> Element<'a, Message> {
    Canvas::new(ReactiveBgProgram {
        state: state.clone(),
        cfg: cfg.clone(),
        accent,
        base,
        has_custom_bg,
        custom_sprite,
        playing,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

impl<Message> canvas::Program<Message> for ReactiveBgProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let size = bounds.size();
        let mut frame = Frame::new(renderer, size);

        // 1. Base backdrop behind reactive layers (let custom image show through when present).
        if !self.has_custom_bg {
            frame.fill_rectangle(Point::ORIGIN, size, self.base);
        } else if self.base.a > 0.02 {
            frame.fill_rectangle(Point::ORIGIN, size, tint(self.base, self.base.a * 0.35));
        }

        let primary = if self.cfg.hue_loop {
            hsl_to_color(self.state.hue_deg, self.cfg.hue_saturation, 0.56)
        } else {
            self.accent
        };
        let secondary = if self.cfg.hue_loop {
            hsl_to_color(
                (self.state.hue_deg + 54.0) % 360.0,
                self.cfg.hue_saturation,
                0.52,
            )
        } else {
            Color {
                r: (self.accent.r * 0.7 + 0.3).min(1.0),
                g: (self.accent.g * 0.7 + 0.15).min(1.0),
                b: (self.accent.b * 0.85 + 0.15).min(1.0),
                a: 1.0,
            }
        };

        let blur = self.cfg.blur.clamp(0.0, 1.0);

        // 2. Bass Glow around edges and corners (softened with blur passes).
        if self.cfg.bass_glow {
            let intensity = (0.10 + self.state.bass * 0.90) * self.cfg.bass_intensity;
            let spread = (size.width.min(size.height) * 0.22) * self.cfg.bass_radius * (0.7 + blur * 0.85);
            let passes = (3 + (blur * 5.0).round() as usize).clamp(3, 8);
            for pass in 0..passes {
                let u = 1.0 - (pass as f32 / passes as f32);
                let thickness = spread * (0.28 + 0.72 * u);
                let a = (intensity * (0.045 + blur * 0.025) * (1.0 - u * 0.45)).clamp(0.0, 0.45);
                let col = tint(if pass % 2 == 0 { primary } else { secondary }, a);
                // Left/Right/Top/Bottom soft edge glow strips
                frame.fill_rectangle(Point::ORIGIN, Size::new(thickness, size.height), col);
                frame.fill_rectangle(
                    Point::new(size.width - thickness, 0.0),
                    Size::new(thickness, size.height),
                    col,
                );
                frame.fill_rectangle(Point::ORIGIN, Size::new(size.width, thickness * 0.65), col);
                frame.fill_rectangle(
                    Point::new(0.0, size.height - thickness * 0.75),
                    Size::new(size.width, thickness * 0.75),
                    col,
                );
            }

            // Corner bass radial blooms
            let r_base = size.width.min(size.height) * 0.36 * self.cfg.bass_radius * (0.75 + blur * 0.7);
            let corners = [
                (Point::new(0.0, 0.0), primary),
                (Point::new(size.width, 0.0), secondary),
                (Point::new(0.0, size.height), secondary),
                (Point::new(size.width, size.height), primary),
            ];
            for (pt, c) in corners {
                for ring in 0..4 {
                    let frac = 1.0 - ring as f32 * 0.22;
                    let a = (intensity * (0.055 + ring as f32 * 0.022)).clamp(0.0, 0.38);
                    frame.fill(&Path::circle(pt, r_base * frac), tint(c, a));
                }
            }
        }

        // 3. Zooming Particles on Drum Hits
        if self.cfg.particles_enabled && self.cfg.particle_count > 0 {
            let cx = size.width * 0.5;
            let cy = size.height * 0.5;
            let max_span = (cx * cx + cy * cy).sqrt();
            let count = self.cfg.particle_count.min(self.state.particles.len());
            let blur_passes = if blur > 0.2 { 2 } else { 1 };

            for pt in self.state.particles.iter().take(count) {
                let r = pt.dist * max_span;
                let px = cx + pt.angle.cos() * r;
                let py = cy + pt.angle.sin() * r;
                if px < -40.0 || px > size.width + 40.0 || py < -40.0 || py > size.height + 40.0 {
                    continue;
                }
                let pcol = if self.cfg.hue_loop {
                    hsl_to_color(
                        (self.state.hue_deg + pt.hue_offset).rem_euclid(360.0),
                        self.cfg.hue_saturation,
                        0.68,
                    )
                } else {
                    primary
                };

                let zoom_scale = 1.0 + self.state.drum_pulse * self.cfg.particle_zoom * 0.9;
                let radius = pt.size * (0.45 + pt.dist * 1.45) * zoom_scale * (1.0 + blur * 1.25);
                let alpha = ((0.18 + pt.dist * 0.55) * (0.55 + self.state.drum_pulse * 0.45) * (1.0 - blur * 0.32)).clamp(0.04, 0.9);

                // Radial motion streak during drum hits
                if self.state.drum_pulse > 0.12 {
                    let tail_len = (10.0 + self.state.drum_pulse * 28.0 * self.cfg.particle_zoom) * pt.dist;
                    let tail = Point::new(px - pt.angle.cos() * tail_len, py - pt.angle.sin() * tail_len);
                    frame.stroke(
                        &Path::line(tail, Point::new(px, py)),
                        Stroke::default()
                            .with_color(tint(pcol, alpha * 0.55))
                            .with_width((radius * 0.7).max(1.0)),
                    );
                }

                for b in 0..blur_passes {
                    let mul = if b == 0 { 1.0 } else { 1.0 + blur * 1.9 };
                    let a_ring = if b == 0 { alpha } else { alpha * 0.25 };
                    frame.fill(&Path::circle(Point::new(px, py), radius * mul), tint(pcol, a_ring));
                }

                // Luminous glow halo around zooming drum sparks
                if self.cfg.glow_particles_enabled && self.cfg.glow_particle_intensity > 0.01 {
                    let glow_i = self.cfg.glow_particle_intensity.clamp(0.0, 2.5);
                    let glow_s = self.cfg.glow_particle_size.clamp(0.4, 2.5);
                    let halo_r = radius * (2.6 + self.state.drum_pulse * 1.2) * glow_s;
                    frame.fill(
                        &Path::circle(Point::new(px, py), halo_r),
                        tint(pcol, (alpha * 0.14 * glow_i).clamp(0.0, 0.32)),
                    );
                    frame.fill(
                        &Path::circle(Point::new(px, py), halo_r * 0.52),
                        tint(pcol, (alpha * 0.28 * glow_i).clamp(0.0, 0.50)),
                    );
                }
            }
        }

        // 3b. Dedicated Luminous Glow Particles & Special Effect Modes
        let style = self.cfg.glow_particle_style.trim().to_ascii_lowercase();
        if self.cfg.aurora_enabled || (self.cfg.glow_particles_enabled && style == "aurora") {
            draw_aurora_curtains(&mut frame, size, self, primary, secondary, blur);
        }

        if self.cfg.glow_particles_enabled && self.cfg.glow_particle_intensity > 0.01 {
            let glow_i = self.cfg.glow_particle_intensity.clamp(0.0, 2.5);
            let glow_s = self.cfg.glow_particle_size.clamp(0.4, 2.5);
            match style.as_str() {
                "spiral" => draw_spiral_particles(&mut frame, size, self, primary, secondary, glow_i, glow_s),
                "infinity8" | "infinity" => draw_infinity8_particles(&mut frame, size, self, primary, secondary, glow_i, glow_s),
                "anime_speed_lines" | "speed_lines" => draw_anime_speed_lines(&mut frame, size, self, primary, secondary, glow_i, glow_s),
                "bubbles" => draw_bubbles(&mut frame, size, self, primary, secondary, glow_i, glow_s),
                "flare" => draw_lens_flares(&mut frame, size, self, primary, secondary, glow_i, glow_s),
                "glamour" => draw_glamour_sparkles(&mut frame, size, self, primary, secondary, glow_i, glow_s),
                "aurora" | "custom_sprite" => {}
                _ => {
                    // "bokeh", "glowy", "fireflies", "starburst", "sparks"
                    let orb_count = match style.as_str() {
                        "fireflies" | "sparks" | "glowy" => 44usize,
                        _ => 34usize,
                    }
                    .min(self.state.glow_orbs.len());

                    for (idx, orb) in self.state.glow_orbs.iter().take(orb_count).enumerate() {
                        let px = orb.x * size.width;
                        let py = orb.y * size.height;
                        let twinkle = 0.55 + 0.45 * orb.twinkle_phase.sin();
                        let energy_boost = 0.55
                            + self.state.bass * 0.65
                            + self.state.vocal * 0.45
                            + self.state.drum_pulse * 0.55;
                        let base_col = orb_color(self, idx, orb_count, orb.hue_offset, primary, secondary);
                        let white_core = lighten_white(base_col);
                        let r = orb.radius
                            * glow_s
                            * (0.72 + self.state.bass * 0.45 + blur * 0.45)
                            * match style.as_str() {
                                "fireflies" => 0.52,
                                "sparks" => 0.48,
                                "starburst" => 0.72,
                                "glowy" => 0.90,
                                _ => 1.0,
                            };
                        let alpha = (0.14 * glow_i * twinkle * energy_boost).clamp(0.02, 0.58);

                        for (ring_i, (rmul, amul)) in [(2.35, 0.22), (1.55, 0.42), (0.95, 0.72)].iter().enumerate() {
                            let ring_r = (r * rmul).max(2.0);
                            let ring_col = if ring_i == 2 && style != "bokeh" {
                                tint(white_core, (alpha * amul * 1.15).clamp(0.0, 0.85))
                            } else {
                                tint(base_col, (alpha * amul).clamp(0.0, 0.65))
                            };
                            frame.fill(&Path::circle(Point::new(px, py), ring_r), ring_col);
                        }
                        frame.fill(
                            &Path::circle(Point::new(px, py), (r * 0.26).clamp(1.6, 8.0)),
                            tint(white_core, (alpha * 1.45).clamp(0.0, 0.95)),
                        );

                        if style == "starburst" {
                            let arm = r * (1.55 + self.state.drum_pulse * 0.9);
                            let ray_col = tint(white_core, (alpha * 0.95).clamp(0.0, 0.75));
                            frame.stroke(
                                &Path::line(Point::new(px - arm, py), Point::new(px + arm, py)),
                                Stroke::default().with_color(ray_col).with_width(1.6),
                            );
                            frame.stroke(
                                &Path::line(Point::new(px, py - arm), Point::new(px, py + arm)),
                                Stroke::default().with_color(ray_col).with_width(1.6),
                            );
                        } else if style == "sparks" {
                            let tail = Point::new(px - orb.vx * 520.0, py - orb.vy * 520.0);
                            frame.stroke(
                                &Path::line(tail, Point::new(px, py)),
                                Stroke::default()
                                    .with_color(tint(base_col, (alpha * 0.75).clamp(0.0, 0.65)))
                                    .with_width((r * 0.32).max(1.2)),
                            );
                        }
                    }
                }
            }
        }

        // 3c. Custom .svg / .png / .jxl Sprite Particles with configurable size & beat pulse
        if (self.cfg.custom_particle_enabled || style == "custom_sprite")
            && self.custom_sprite.is_some()
        {
            if let Some(handle) = &self.custom_sprite {
                draw_custom_sprites(&mut frame, size, self, handle);
            }
        }

        // 4. Side Background EQ Styles
        if self.cfg.eq_enabled {
            draw_side_eq(&mut frame, size, self, primary, secondary, blur);
        }

        // 5. Configurable Frosted Softness / Dimming Veil so UI foreground stays legible
        let frost_alpha = (1.0 - self.cfg.bg_dim) * 0.42;
        if (frost_alpha > 0.01 || blur > 0.05) && !self.has_custom_bg {
            let mist = tint(
                self.base,
                (0.10 + blur * 0.24 + (1.0 - self.cfg.bg_dim) * 0.12).clamp(0.0, 0.62),
            );
            frame.fill_rectangle(Point::ORIGIN, size, mist);
        }

        vec![frame.into_geometry()]
    }
}

fn draw_side_eq(
    frame: &mut Frame,
    size: Size,
    p: &ReactiveBgProgram,
    primary: Color,
    secondary: Color,
    blur: f32,
) {
    let draw_left = !p.cfg.eq_sides.eq_ignore_ascii_case("right");
    let draw_right = !p.cfg.eq_sides.eq_ignore_ascii_case("left");
    let reach = p.cfg.eq_width.clamp(16.0, size.width * 0.35);
    let opacity = p.cfg.eq_opacity.clamp(0.05, 1.0);
    let bands_n = 28usize;
    let pad_y = 28.0;
    let usable_h = (size.height - pad_y * 2.0).max(40.0);
    let step = usable_h / bands_n as f32;
    let style = p.cfg.eq_style.trim().to_ascii_lowercase();

    let band_energy = |idx: usize| -> f32 {
        let u = idx as f32 / (bands_n.saturating_sub(1).max(1) as f32);
        let mirror_u = if style == "mirrored" {
            1.0 - (u * 2.0 - 1.0).abs()
        } else {
            u
        };
        let f_idx = mirror_u * 15.0;
        let lo = (f_idx.floor() as usize).min(15);
        let hi = (lo + 1).min(15);
        let frac = f_idx - lo as f32;
        let val = p.state.bands[lo] * (1.0 - frac) + p.state.bands[hi] * frac;
        if p.playing {
            (0.08 + val * 0.92).clamp(0.05, 1.0)
        } else {
            0.08 + 0.05 * (p.state.phase + u * 4.0).sin().abs()
        }
    };

    match style.as_str() {
        "wave" => {
            for &is_left in &[true, false] {
                if (is_left && !draw_left) || (!is_left && !draw_right) {
                    continue;
                }
                let path = Path::new(|b| {
                    for i in 0..=bands_n {
                        let u = i as f32 / bands_n as f32;
                        let y = pad_y + u * usable_h;
                        let e = band_energy(i.min(bands_n - 1));
                        let x_off = 6.0 + e * reach * (0.65 + 0.35 * (p.state.phase * 2.2 + u * 7.0).sin().abs());
                        let x = if is_left { x_off } else { size.width - x_off };
                        let pt = Point::new(x, y);
                        if i == 0 {
                            b.move_to(pt);
                        } else {
                            b.line_to(pt);
                        }
                    }
                });
                if blur > 0.1 {
                    frame.stroke(
                        &path,
                        Stroke::default()
                            .with_color(tint(secondary, opacity * 0.32))
                            .with_width(4.0 + blur * 12.0),
                    );
                }
                frame.stroke(
                    &path,
                    Stroke::default()
                        .with_color(tint(primary, opacity * 0.92))
                        .with_width(2.4),
                );
            }
        }
        "minimal" => {
            let pillar_w = (reach * 0.35).max(8.0);
            let pillar_h = usable_h * (0.28 + p.state.bass * 0.55);
            let y = (size.height - pillar_h) * 0.5;
            if draw_left {
                let col = tint(primary, opacity * (0.25 + p.state.bass * 0.55));
                frame.fill(
                    &Path::rounded_rectangle(Point::new(8.0, y), Size::new(pillar_w, pillar_h), (pillar_w * 0.5).into()),
                    col,
                );
            }
            if draw_right {
                let col = tint(secondary, opacity * (0.25 + p.state.bass * 0.55));
                frame.fill(
                    &Path::rounded_rectangle(
                        Point::new(size.width - pillar_w - 8.0, y),
                        Size::new(pillar_w, pillar_h),
                        (pillar_w * 0.5).into(),
                    ),
                    col,
                );
            }
        }
        _ => {
            // "bars", "mirrored", "blocks"
            let bar_h = (step * 0.64).max(3.0);
            for i in 0..bands_n {
                let u = i as f32 / bands_n as f32;
                let e = band_energy(i);
                let len = (6.0 + e * reach).min(reach);
                let y = pad_y + i as f32 * step + (step - bar_h) * 0.5;
                let col = mix_color(primary, secondary, u);

                let draw_side = |frame: &mut Frame, x: f32, bw: f32, bh: f32, c: Color| {
                    if style == "blocks" {
                        let seg_w = 8.0;
                        let gap = 3.0;
                        let count = ((bw / (seg_w + gap)).floor() as usize).max(1);
                        for s in 0..count {
                            let sx = if x < size.width * 0.5 {
                                x + s as f32 * (seg_w + gap)
                            } else {
                                x + bw - (s + 1) as f32 * (seg_w + gap)
                            };
                            frame.fill(
                                &Path::rounded_rectangle(Point::new(sx, y), Size::new(seg_w, bh), 2.0.into()),
                                tint(c, opacity * (0.55 + 0.45 * e)),
                            );
                        }
                    } else {
                        if blur > 0.12 {
                            let halo = 3.0 + blur * 9.0;
                            frame.fill(
                                &Path::rounded_rectangle(
                                    Point::new((x - halo * 0.4).max(0.0), y - halo * 0.35),
                                    Size::new(bw + halo * 0.8, bh + halo * 0.7),
                                    4.0.into(),
                                ),
                                tint(c, opacity * 0.16 * (0.4 + e)),
                            );
                        }
                        frame.fill(
                            &Path::rounded_rectangle(Point::new(x, y), Size::new(bw, bh), (bh * 0.45).into()),
                            tint(c, opacity * (0.45 + e * 0.55)),
                        );
                    }
                };

                if draw_left {
                    draw_side(frame, 4.0, len, bar_h, col);
                }
                if draw_right {
                    draw_side(frame, size.width - len - 4.0, len, bar_h, col);
                }
            }
        }
    }
}

fn orb_color(
    p: &ReactiveBgProgram,
    idx: usize,
    total: usize,
    hue_offset: f32,
    primary: Color,
    secondary: Color,
) -> Color {
    if p.cfg.hue_loop {
        hsl_to_color(
            (p.state.hue_deg + hue_offset).rem_euclid(360.0),
            p.cfg.hue_saturation,
            0.66,
        )
    } else {
        mix_color(primary, secondary, (idx as f32) / (total.max(1) as f32))
    }
}

fn lighten_white(c: Color) -> Color {
    Color {
        r: (c.r * 0.35 + 0.65).min(1.0),
        g: (c.g * 0.35 + 0.65).min(1.0),
        b: (c.b * 0.35 + 0.65).min(1.0),
        a: 1.0,
    }
}

fn draw_spiral_particles(
    frame: &mut Frame,
    size: Size,
    p: &ReactiveBgProgram,
    primary: Color,
    secondary: Color,
    glow_i: f32,
    glow_s: f32,
) {
    let cx = size.width * 0.5;
    let cy = size.height * 0.5;
    let max_r = (cx * cx + cy * cy).sqrt() * 0.92;
    let arms = 4usize;
    let per_arm = (p.cfg.particle_count.clamp(12, 120) / arms).max(6);
    let spin = p.state.phase * (1.2 + p.state.vocal * 0.8);

    for arm in 0..arms {
        let arm_base = (arm as f32 / arms as f32) * TAU;
        for step in 0..per_arm {
            let u = (step as f32 + 0.5) / per_arm as f32;
            let theta = arm_base + spin + u * 5.8 + p.state.drum_pulse * 0.45;
            let r = u * max_r * (0.85 + p.state.bass * 0.25);
            let px = cx + theta.cos() * r;
            let py = cy + theta.sin() * r;
            let col = orb_color(p, step, per_arm, u * 90.0 + arm as f32 * 35.0, primary, secondary);
            let rad = (3.5 + u * 9.0) * glow_s * (0.8 + p.state.drum_pulse * 0.5);
            let alpha = ((0.16 + 0.36 * u) * glow_i).clamp(0.03, 0.72);
            frame.fill(&Path::circle(Point::new(px, py), rad * 1.9), tint(col, alpha * 0.28));
            frame.fill(&Path::circle(Point::new(px, py), rad), tint(col, alpha));
            frame.fill(&Path::circle(Point::new(px, py), rad * 0.4), tint(lighten_white(col), alpha));
        }
    }
}

fn draw_infinity8_particles(
    frame: &mut Frame,
    size: Size,
    p: &ReactiveBgProgram,
    primary: Color,
    secondary: Color,
    glow_i: f32,
    glow_s: f32,
) {
    let cx = size.width * 0.5;
    let cy = size.height * 0.5;
    let rx = size.width * (0.36 + p.state.bass * 0.06);
    let ry = size.height * (0.34 + p.state.drum_pulse * 0.06);
    let count = p.cfg.particle_count.clamp(16, 96);

    for i in 0..count {
        let lane = i % 2;
        let t = p.state.phase * (1.1 + lane as f32 * 0.35) + (i as f32 / count as f32) * TAU;
        let denom = 1.0 + t.sin().powi(2);
        let x_n = t.cos() / denom;
        let y_n = (t.sin() * t.cos()) / denom;
        let px = cx + x_n * rx;
        let py = cy + y_n * ry * 1.7 + if lane == 1 { 18.0 } else { -18.0 };
        let col = orb_color(p, i, count, (i as f32) * 8.0, primary, secondary);
        let rad = (4.5 + (i % 5) as f32 * 1.6) * glow_s * (0.85 + p.state.vocal * 0.45);
        let alpha = (0.34 * glow_i).clamp(0.04, 0.85);
        frame.fill(&Path::circle(Point::new(px, py), rad * 2.2), tint(col, alpha * 0.24));
        frame.fill(&Path::circle(Point::new(px, py), rad), tint(col, alpha));
        frame.fill(&Path::circle(Point::new(px, py), rad * 0.35), tint(lighten_white(col), alpha));
    }
}

fn draw_anime_speed_lines(
    frame: &mut Frame,
    size: Size,
    p: &ReactiveBgProgram,
    primary: Color,
    secondary: Color,
    glow_i: f32,
    glow_s: f32,
) {
    let cx = size.width * 0.5;
    let cy = size.height * 0.5;
    let max_r = (cx * cx + cy * cy).sqrt() * 1.1;
    let lines = (p.cfg.particle_count.clamp(24, 140) * 3) / 4;
    let pulse = 0.25 + p.state.bass * 0.75 + p.state.drum_pulse * 0.9;

    for i in 0..lines {
        let jitter = ((i as f32 * 1.618 + p.state.phase * 4.5).sin()) * 0.04;
        let angle = (i as f32 / lines as f32) * TAU + jitter;
        let inner_frac = 0.28 + ((i * 37) % 100) as f32 / 100.0 * 0.34 - (pulse * 0.10);
        let inner = max_r * inner_frac.clamp(0.18, 0.78);
        let outer = max_r;
        let p0 = Point::new(cx + angle.cos() * inner, cy + angle.sin() * inner);
        let p1 = Point::new(cx + angle.cos() * outer, cy + angle.sin() * outer);
        let col = orb_color(p, i, lines, (i % 7) as f32 * 18.0, primary, secondary);
        let alpha = ((0.18 + 0.38 * (pulse / 2.0).min(1.0)) * glow_i).clamp(0.04, 0.85);
        let width = (1.2 + ((i % 4) as f32) * 0.9) * glow_s * (0.7 + p.state.drum_pulse * 0.9);
        frame.stroke(
            &Path::line(p0, p1),
            Stroke::default().with_color(tint(col, alpha)).with_width(width),
        );
    }
}

fn draw_bubbles(
    frame: &mut Frame,
    size: Size,
    p: &ReactiveBgProgram,
    primary: Color,
    secondary: Color,
    glow_i: f32,
    glow_s: f32,
) {
    let count = p.cfg.particle_count.clamp(10, 64).min(p.state.glow_orbs.len());
    for (idx, orb) in p.state.glow_orbs.iter().take(count).enumerate() {
        let px = orb.x * size.width;
        let py = orb.y * size.height;
        let col = orb_color(p, idx, count, orb.hue_offset, primary, secondary);
        let r = (10.0 + orb.radius * 0.95) * glow_s * (0.9 + p.state.bass * 0.25);
        let alpha = (0.38 * glow_i).clamp(0.05, 0.80);

        frame.fill(&Path::circle(Point::new(px, py), r), tint(col, alpha * 0.16));
        frame.stroke(
            &Path::circle(Point::new(px, py), r),
            Stroke::default()
                .with_color(tint(lighten_white(col), alpha * 0.85))
                .with_width(1.6),
        );
        // Glossy specular bubble highlight dot near upper-left rim
        let hl = Point::new(px - r * 0.36, py - r * 0.36);
        frame.fill(
            &Path::circle(hl, (r * 0.22).max(2.0)),
            Color::from_rgba(1.0, 1.0, 1.0, (alpha * 0.9).clamp(0.1, 0.85)),
        );
    }
}

fn draw_lens_flares(
    frame: &mut Frame,
    size: Size,
    p: &ReactiveBgProgram,
    primary: Color,
    secondary: Color,
    glow_i: f32,
    glow_s: f32,
) {
    let sx = size.width * (0.5 + 0.28 * (p.state.phase * 0.55).cos());
    let sy = size.height * (0.42 + 0.18 * (p.state.phase * 0.45).sin());
    let cx = size.width * 0.5;
    let cy = size.height * 0.5;
    let vx = cx - sx;
    let vy = cy - sy;
    let burst = (0.45 + p.state.bass * 0.7 + p.state.drum_pulse * 0.9) * glow_i;
    let white = lighten_white(primary);

    // Anamorphic horizontal flare streak
    let streak_w = size.width * (0.42 + p.state.drum_pulse * 0.28);
    let streak_h = (4.0 + p.state.bass * 12.0) * glow_s;
    frame.fill(
        &Path::rounded_rectangle(
            Point::new(sx - streak_w * 0.5, sy - streak_h * 0.5),
            Size::new(streak_w, streak_h),
            streak_h.into(),
        ),
        tint(primary, (0.22 * burst).clamp(0.03, 0.55)),
    );
    frame.fill(
        &Path::circle(Point::new(sx, sy), 28.0 * glow_s * (0.8 + p.state.drum_pulse * 0.4)),
        tint(white, (0.35 * burst).clamp(0.05, 0.75)),
    );

    // Iris ghost discs along the optical axis
    for (idx, t) in [-0.35_f32, 0.45, 0.95, 1.45, 1.95].iter().enumerate() {
        let gx = sx + vx * *t;
        let gy = sy + vy * *t;
        let gr = (14.0 + (idx as f32) * 9.0) * glow_s;
        let col = orb_color(p, idx, 5, idx as f32 * 45.0, primary, secondary);
        frame.fill(&Path::circle(Point::new(gx, gy), gr), tint(col, (0.14 * burst).clamp(0.02, 0.36)));
        frame.stroke(
            &Path::circle(Point::new(gx, gy), gr),
            Stroke::default().with_color(tint(white, (0.20 * burst).clamp(0.02, 0.42))).with_width(1.2),
        );
    }
}

fn draw_glamour_sparkles(
    frame: &mut Frame,
    size: Size,
    p: &ReactiveBgProgram,
    primary: Color,
    secondary: Color,
    glow_i: f32,
    glow_s: f32,
) {
    let count = p.cfg.particle_count.clamp(12, 64).min(p.state.glow_orbs.len());
    for (idx, orb) in p.state.glow_orbs.iter().take(count).enumerate() {
        let px = orb.x * size.width;
        let py = orb.y * size.height;
        let twinkle = (0.35 + 0.65 * orb.twinkle_phase.sin().abs()).powf(1.4);
        let col = orb_color(p, idx, count, orb.hue_offset, primary, secondary);
        let white = lighten_white(col);
        let r = (7.0 + orb.radius * 0.7) * glow_s * (0.8 + p.state.drum_pulse * 0.5) * twinkle;
        let alpha = (0.52 * glow_i * twinkle).clamp(0.04, 0.92);

        frame.fill(&Path::circle(Point::new(px, py), r * 1.4), tint(col, alpha * 0.22));
        // 4-point diamond star
        let star = Path::new(|b| {
            let inn = r * 0.22;
            b.move_to(Point::new(px, py - r * 1.35));
            b.line_to(Point::new(px + inn, py - inn));
            b.line_to(Point::new(px + r * 1.35, py));
            b.line_to(Point::new(px + inn, py + inn));
            b.line_to(Point::new(px, py + r * 1.35));
            b.line_to(Point::new(px - inn, py + inn));
            b.line_to(Point::new(px - r * 1.35, py));
            b.line_to(Point::new(px - inn, py - inn));
            b.close();
        });
        frame.fill(&star, tint(white, alpha));
    }
}

fn draw_aurora_curtains(
    frame: &mut Frame,
    size: Size,
    p: &ReactiveBgProgram,
    primary: Color,
    secondary: Color,
    blur: f32,
) {
    let curtains = 3usize;
    let ribbons = 36usize;
    let intensity = (0.35 + p.state.vocal * 0.75 + p.state.bass * 0.45) * p.cfg.glow_particle_intensity.clamp(0.2, 2.5);
    let col_a = hsl_to_color((p.state.hue_deg + 150.0) % 360.0, 0.85, 0.56);
    let col_b = mix_color(primary, secondary, 0.5);

    for c in 0..curtains {
        let cf = c as f32;
        let base_y = size.height * (0.14 + cf * 0.16);
        let curtain_h = size.height * (0.28 + blur * 0.14 + p.state.bass * 0.12);
        let col = if c % 2 == 0 { col_a } else { col_b };
        let col_top = mix_color(col, secondary, cf * 0.35);

        for i in 0..ribbons {
            let u = i as f32 / ribbons as f32;
            let x = u * size.width;
            let w = (size.width / ribbons as f32) + 6.0;
            let wave = (u * 6.5 + p.state.phase * (0.8 + cf * 0.25) + cf * 2.1).sin();
            let y = base_y + wave * 34.0;
            let shimmer = 0.45 + 0.55 * (u * 11.0 + p.state.phase * 1.4).cos().abs();
            let a = (0.085 * intensity * shimmer).clamp(0.015, 0.34);
            frame.fill(
                &Path::rounded_rectangle(Point::new(x, y), Size::new(w, curtain_h), 12.0.into()),
                tint(col_top, a),
            );
        }
    }
}

fn draw_custom_sprites(
    frame: &mut Frame,
    size: Size,
    p: &ReactiveBgProgram,
    handle: &image::Handle,
) {
    let count = p
        .cfg
        .custom_particle_count
        .clamp(1, 64)
        .min(p.state.glow_orbs.len());
    let base_sz = p.cfg.custom_particle_size.clamp(8.0, 160.0);
    for (idx, orb) in p.state.glow_orbs.iter().take(count).enumerate() {
        let px = orb.x * size.width;
        let py = orb.y * size.height;
        let pulse = 1.0 + p.state.drum_pulse * 0.28 * p.cfg.particle_zoom.clamp(0.1, 2.0);
        let sz = (base_sz * (0.72 + (idx % 4) as f32 * 0.16) * pulse).clamp(6.0, 220.0);
        let rot = orb.twinkle_phase * 0.35 + p.state.phase * (0.4 + (idx % 3) as f32 * 0.2);
        let alpha = (0.55 + 0.35 * orb.twinkle_phase.sin().abs()).clamp(0.15, 0.98);
        let half = sz * 0.5;
        frame.with_save(|f| {
            f.translate(Vector::new(px, py));
            f.rotate(Radians(rot));
            f.draw_image(
                Rectangle::new(Point::new(-half, -half), Size::new(sz, sz)),
                canvas::Image::new(handle.clone()).opacity(alpha),
            );
        });
    }
}

fn tint(col: Color, alpha: f32) -> Color {
    Color {
        a: alpha.clamp(0.0, 1.0),
        ..col
    }
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    let u = t.clamp(0.0, 1.0);
    Color {
        r: a.r * (1.0 - u) + b.r * u,
        g: a.g * (1.0 - u) + b.g * u,
        b: a.b * (1.0 - u) + b.b * u,
        a: 1.0,
    }
}

pub fn hsl_to_color(h_deg: f32, s: f32, l: f32) -> Color {
    let h = h_deg.rem_euclid(360.0) / 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s <= 0.001 {
        return Color::from_rgb(l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue_to_rgb = |mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 0.5 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    };
    Color::from_rgb(hue_to_rgb(h + 1.0 / 3.0), hue_to_rgb(h), hue_to_rgb(h - 1.0 / 3.0))
}

#[derive(Debug, Clone)]
pub struct CustomBgProgram {
    pub gradient: bool,
    pub c1: Color,
    pub c2: Color,
}

/// Renders either a solid tinted backdrop or a smooth multi-step vertical gradient background.
pub fn custom_bg_canvas<'a, Message: 'a>(
    gradient: bool,
    c1: Color,
    c2: Color,
) -> Element<'a, Message> {
    Canvas::new(CustomBgProgram { gradient, c1, c2 })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

impl<Message> canvas::Program<Message> for CustomBgProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let size = bounds.size();
        let mut frame = Frame::new(renderer, size);
        if !self.gradient {
            frame.fill_rectangle(Point::ORIGIN, size, self.c1);
        } else {
            let steps = 48usize;
            let strip_h = (size.height / steps as f32) + 1.0;
            for i in 0..steps {
                let u = i as f32 / (steps.saturating_sub(1).max(1) as f32);
                let col = Color {
                    r: self.c1.r * (1.0 - u) + self.c2.r * u,
                    g: self.c1.g * (1.0 - u) + self.c2.g * u,
                    b: self.c1.b * (1.0 - u) + self.c2.b * u,
                    a: self.c1.a * (1.0 - u) + self.c2.a * u,
                };
                frame.fill_rectangle(
                    Point::new(0.0, i as f32 * (size.height / steps as f32)),
                    Size::new(size.width, strip_h),
                    col,
                );
            }
        }
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reactive_bg_vocal_hue_and_drum_pulse() {
        let mut st = ReactiveBgState::new();
        st.last_tick = Instant::now() - std::time::Duration::from_millis(50);
        let cfg = ReactiveUiConfig {
            enabled: true,
            ..Default::default()
        };
        let initial_hue = st.hue_deg;

        let mut bands = [0.05f32; 16];
        bands[0] = 0.85;
        bands[6] = 0.90;
        bands[7] = 0.88;
        st.on_frame(true, &bands, 0.80, &cfg);

        assert!(st.bass > 0.15, "bass envelope should rise");
        assert!(st.vocal > 0.15, "vocal envelope should rise");
        assert!(st.drum_pulse > 0.25, "drum pulse should fire on beat hit");
        assert!(
            (st.hue_deg - initial_hue).abs() > 0.1,
            "hue loop should advance with vocal energy"
        );
    }
}
