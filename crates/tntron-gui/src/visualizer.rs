use std::f32::consts::TAU;

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Fill, Frame, Path, Stroke};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::theme::ThemeSpec;

#[derive(Debug, Clone, Copy)]
pub struct VizFrameData {
    pub phase: f32,
    pub bands: [f32; 16],
    pub beat: f32,
}

impl Default for VizFrameData {
    fn default() -> Self {
        Self {
            phase: 0.0,
            bands: [0.0; 16],
            beat: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VisualizerProgram {
    pub mode: String,
    pub viz: VizFrameData,
    pub playing: bool,
    pub volume: f32,
    pub speed: f32,
    pub mini: bool,
    pub accent: Color,
    pub text: Color,
    pub muted: Color,
    pub base: Color,
}

impl VisualizerProgram {
    fn has_live_audio(&self) -> bool {
        self.viz.bands.iter().any(|&v| v > 0.002)
    }

    pub fn band_at(&self, idx: usize, count: usize) -> f32 {
        let vol_scale = (self.volume / 100.0).clamp(0.2, 1.0);
        if self.playing && self.has_live_audio() {
            let u = (idx as f32 / count.max(2).saturating_sub(1) as f32).clamp(0.0, 1.0);
            let f_idx = u * 15.0;
            let lo = (f_idx.floor() as usize).min(15);
            let hi = (lo + 1).min(15);
            let frac = f_idx - lo as f32;
            let val = self.viz.bands[lo] * (1.0 - frac) + self.viz.bands[hi] * frac;
            return (0.04 + val * 0.96) * vol_scale;
        }
        let active = if self.playing { 0.72 } else { 0.16 };
        let norm = idx as f32 / count.max(1) as f32;
        let t = self.viz.phase * self.speed.clamp(0.25, 3.0);
        let w1 = (t * 2.3 + norm * 6.5).sin() * 0.5 + 0.5;
        let w2 = (t * 3.8 - norm * 11.0).cos() * 0.5 + 0.5;
        (0.08 + (w1 * 0.55 + w2 * 0.45) * 0.65) * vol_scale * active
    }

    fn bass(&self) -> f32 {
        if !self.playing || !self.has_live_audio() {
            return if self.playing { 0.35 } else { 0.08 };
        }
        (self.viz.bands[0] + self.viz.bands[1] + self.viz.bands[2] + self.viz.bands[3]) * 0.25
    }

    fn mid(&self) -> f32 {
        if !self.playing || !self.has_live_audio() {
            return if self.playing { 0.28 } else { 0.06 };
        }
        self.viz.bands[4..10].iter().sum::<f32>() / 6.0
    }

    fn high(&self) -> f32 {
        if !self.playing || !self.has_live_audio() {
            return if self.playing { 0.22 } else { 0.05 };
        }
        self.viz.bands[10..16].iter().sum::<f32>() / 6.0
    }
}

pub fn visualizer_canvas<'a, Message: 'a>(
    mode: &str,
    viz: &VizFrameData,
    playing: bool,
    volume: f64,
    speed: f64,
    theme: &ThemeSpec,
) -> Element<'a, Message> {
    let prog = VisualizerProgram {
        mode: mode.trim().to_ascii_lowercase(),
        viz: *viz,
        playing,
        volume: volume as f32,
        speed: speed as f32,
        mini: false,
        accent: theme.accent,
        text: theme.text,
        muted: theme.muted,
        base: theme.base,
    };
    Canvas::new(prog)
        .width(Length::Fill)
        .height(Length::Fixed(220.0))
        .into()
}

pub fn mini_visualizer_canvas<'a, Message: 'a>(
    mode: &str,
    viz: &VizFrameData,
    playing: bool,
    theme: &ThemeSpec,
) -> Element<'a, Message> {
    let prog = VisualizerProgram {
        mode: mode.trim().to_ascii_lowercase(),
        viz: *viz,
        playing,
        volume: 100.0,
        speed: 1.0,
        mini: true,
        accent: theme.accent,
        text: theme.text,
        muted: theme.muted,
        base: theme.card,
    };
    Canvas::new(prog)
        .width(Length::Fixed(108.0))
        .height(Length::Fixed(34.0))
        .into()
}

impl<Message> canvas::Program<Message> for VisualizerProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        if self.mini {
            draw_mini(&mut frame, bounds.size(), self);
        } else {
            draw_backdrop(&mut frame, bounds.size(), self);
            match self.mode.as_str() {
                "wave" => draw_wave(&mut frame, bounds.size(), self),
                "circle" => draw_circle(&mut frame, bounds.size(), self),
                "particles" => draw_particles(&mut frame, bounds.size(), self),
                "snow" => draw_snow(&mut frame, bounds.size(), self),
                "flame" => draw_flame(&mut frame, bounds.size(), self),
                "runner" => draw_runner(&mut frame, bounds.size(), self),
                "boulder" => draw_boulder(&mut frame, bounds.size(), self),
                "bicycle" => draw_bicycle(&mut frame, bounds.size(), self),
                _ => draw_bars(&mut frame, bounds.size(), self),
            }
        }
        vec![frame.into_geometry()]
    }
}

fn tint(col: Color, alpha: f32) -> Color {
    Color {
        a: alpha.clamp(0.0, 1.0),
        ..col
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let u = t.clamp(0.0, 1.0);
    Color {
        r: a.r * (1.0 - u) + b.r * u,
        g: a.g * (1.0 - u) + b.g * u,
        b: a.b * (1.0 - u) + b.b * u,
        a: a.a * (1.0 - u) + b.a * u,
    }
}

fn draw_mini(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let bg = Path::rounded_rectangle(Point::ORIGIN, size, 8.0.into());
    frame.fill(&bg, tint(p.base, 0.92));
    if p.playing && p.viz.beat > 0.08 {
        frame.stroke(
            &bg,
            Stroke::default()
                .with_color(tint(p.accent, 0.20 + p.viz.beat * 0.45))
                .with_width(1.2),
        );
    }

    match p.mode.as_str() {
        "wave" => {
            let mid_y = size.height * 0.5;
            let amp = size.height * 0.38;
            let path = Path::new(|b| {
                let steps = 28;
                for i in 0..=steps {
                    let u = i as f32 / steps as f32;
                    let x = 5.0 + u * (size.width - 10.0);
                    let e = p.band_at(i, steps + 1);
                    let ripple = (u * TAU * 3.0 + p.viz.phase * 5.0).sin();
                    let y = mid_y + ripple * amp * (0.2 + e * 0.9);
                    let pt = Point::new(x, y);
                    if i == 0 {
                        b.move_to(pt);
                    } else {
                        b.line_to(pt);
                    }
                }
            });
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(p.accent)
                    .with_width(2.0),
            );
        }
        "circle" => {
            let c = Point::new(size.width * 0.5, size.height * 0.5);
            let r = 7.0 + p.bass() * 5.5 + p.viz.beat * 2.2;
            frame.fill(&Path::circle(c, r * 0.55), tint(p.accent, 0.35 + p.bass() * 0.5));
            frame.stroke(
                &Path::circle(c, r),
                Stroke::default().with_color(p.accent).with_width(1.8),
            );
            for i in 0..16 {
                let e = p.band_at(i, 16);
                let a = (i as f32 / 16.0) * TAU + p.viz.phase * 0.6;
                let r1 = r + 1.5 + e * 6.5;
                frame.stroke(
                    &Path::line(
                        Point::new(c.x + a.cos() * r, c.y + a.sin() * r),
                        Point::new(c.x + a.cos() * r1, c.y + a.sin() * r1),
                    ),
                    Stroke::default().with_color(p.text).with_width(1.6),
                );
            }
        }
        "flame" => {
            let count = 14;
            let base_y = size.height - 4.0;
            let usable = size.width - 10.0;
            let step = usable / count as f32;
            let warm = Color::from_rgb8(0xf9, 0x73, 0x16);
            for i in 0..count {
                let e = p.band_at(i, count);
                let h = (3.0 + e * (size.height - 9.0)).min(size.height - 6.0);
                let x = 5.0 + i as f32 * step;
                let rect = Path::rounded_rectangle(
                    Point::new(x, base_y - h),
                    Size::new((step - 1.5).max(2.0), h),
                    1.5.into(),
                );
                frame.fill(&rect, tint(mix(warm, p.accent, i as f32 / 14.0), 0.72 + 0.28 * e));
            }
        }
        _ => {
            let count = 16;
            let pad_x = 6.0;
            let usable = size.width - pad_x * 2.0;
            let step = usable / count as f32;
            let bw = (step * 0.66).max(2.5);
            let base_y = size.height - 5.0;
            let max_h = size.height - 11.0;
            for i in 0..count {
                let e = p.band_at(i, count);
                let h = (2.5 + e * max_h).min(max_h);
                let x = pad_x + i as f32 * step + (step - bw) * 0.5;
                let y = base_y - h;
                let col = mix(p.accent, p.text, (i as f32 / count as f32) * 0.38);
                let r = Path::rounded_rectangle(Point::new(x, y), Size::new(bw, h), 1.5.into());
                frame.fill(&r, tint(col, 0.65 + e * 0.35));
            }
        }
    }
}

fn draw_backdrop(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let bg = Path::rounded_rectangle(Point::ORIGIN, size, 12.0.into());
    frame.fill(&bg, tint(p.base, 0.88));
    let beat_glow = if p.playing {
        0.04 + p.bass() * 0.08 + p.viz.beat * 0.08
    } else {
        0.02
    };
    let glow = Path::circle(
        Point::new(size.width * 0.5, size.height * 0.52),
        size.height * (0.58 + p.viz.beat * 0.16),
    );
    frame.fill(&glow, tint(p.accent, beat_glow));
}

fn draw_bars(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let count = 32;
    let pad_x = 18.0;
    let usable_w = (size.width - pad_x * 2.0).max(40.0);
    let step = usable_w / count as f32;
    let bar_w = (step * 0.64).max(4.0);
    let base_y = size.height - 26.0;
    let max_h = (size.height - 50.0).max(24.0);

    // Subtle mirror floor glow
    frame.stroke(
        &Path::line(
            Point::new(pad_x, base_y + 2.0),
            Point::new(size.width - pad_x, base_y + 2.0),
        ),
        Stroke::default()
            .with_color(tint(p.accent, 0.25 + p.viz.beat * 0.35))
            .with_width(1.2),
    );

    for i in 0..count {
        let e = p.band_at(i, count);
        let u = i as f32 / (count - 1) as f32;
        let h = (e * max_h).max(5.0);
        let x = pad_x + i as f32 * step + (step - bar_w) * 0.5;
        let y = base_y - h;
        let col = mix(p.accent, p.text, u * 0.35);

        let rect = Path::rounded_rectangle(Point::new(x, y), Size::new(bar_w, h), 3.5.into());
        frame.fill(&rect, tint(col, 0.55 + 0.45 * e));

        // Subtle reflection underneath
        let refl_h = (h * 0.18).min(16.0);
        let refl = Path::rounded_rectangle(
            Point::new(x, base_y + 4.0),
            Size::new(bar_w, refl_h),
            2.0.into(),
        );
        frame.fill(&refl, tint(col, 0.14 * e));

        // Floating peak cap
        let cap_y = (y - 5.0 - p.viz.beat * 3.0).max(8.0);
        let cap = Path::rounded_rectangle(
            Point::new(x, cap_y),
            Size::new(bar_w, 2.5),
            1.2.into(),
        );
        frame.fill(&cap, tint(p.text, 0.7 + 0.3 * e));
    }
}

fn draw_wave(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let mid_y = size.height * 0.52;
    let base_amp = size.height * 0.36;
    let t = p.viz.phase * p.speed.clamp(0.25, 3.0);

    for (layer, alpha, mul) in [(2usize, 0.26, 0.55), (1usize, 0.52, 0.8), (0usize, 0.96, 1.0)] {
        let path = Path::new(|b| {
            let steps = 64;
            for i in 0..=steps {
                let u = i as f32 / steps as f32;
                let x = 14.0 + u * (size.width - 28.0);
                let env = (u * std::f32::consts::PI).sin();
                let band_amp = p.band_at(i, steps + 1);
                let osc = (u * TAU * (2.5 + layer as f32 * 1.1) + t * 3.2 + layer as f32).sin();
                let y = mid_y + osc * base_amp * mul * env * (0.18 + band_amp * 0.95);
                let pt = Point::new(x, y);
                if i == 0 {
                    b.move_to(pt);
                } else {
                    b.line_to(pt);
                }
            }
        });
        let col = if layer == 0 { p.accent } else { p.text };
        frame.stroke(
            &path,
            Stroke::default()
                .with_color(tint(col, alpha))
                .with_width(if layer == 0 { 2.8 + p.viz.beat * 1.4 } else { 1.4 }),
        );
    }
}

fn draw_circle(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let center = Point::new(size.width * 0.5, size.height * 0.5);
    let base_r = size.height.min(size.width) * 0.23;
    let spokes = 48;
    let rot = p.viz.phase * 0.35 * p.speed.clamp(0.25, 3.0);
    let bass = p.bass();

    // Expanding beat shockwave ring
    if p.playing && p.viz.beat > 0.05 {
        let wave_r = base_r * (1.2 + (1.0 - p.viz.beat) * 0.9);
        frame.stroke(
            &Path::circle(center, wave_r),
            Stroke::default()
                .with_color(tint(p.accent, p.viz.beat * 0.45))
                .with_width(1.6),
        );
    }

    frame.fill(
        &Path::circle(center, base_r * (0.48 + bass * 0.42 + p.viz.beat * 0.16)),
        tint(p.accent, 0.25 + bass * 0.35),
    );
    frame.stroke(
        &Path::circle(center, base_r),
        Stroke::default()
            .with_color(tint(p.accent, 0.65))
            .with_width(2.0),
    );

    for i in 0..spokes {
        let half = spokes / 2;
        let sym = if i < half { i } else { spokes - 1 - i };
        let e = p.band_at(sym, half);
        let a = (i as f32 / spokes as f32) * TAU + rot;
        let r0 = base_r * 1.05;
        let r1 = r0 + e * (base_r * 1.05);
        let p0 = Point::new(center.x + a.cos() * r0, center.y + a.sin() * r0);
        let p1 = Point::new(center.x + a.cos() * r1, center.y + a.sin() * r1);
        frame.stroke(
            &Path::line(p0, p1),
            Stroke::default()
                .with_color(tint(mix(p.accent, p.text, e * 0.5), 0.5 + 0.5 * e))
                .with_width(2.6),
        );
        frame.fill(&Path::circle(p1, 1.8), tint(p.text, 0.4 + e * 0.6));
    }
}

fn draw_particles(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let count = 36;
    let t = p.viz.phase * 0.7 * p.speed.clamp(0.25, 3.0);
    let push = p.viz.beat * 14.0;
    let cx = size.width * 0.5;
    let cy = size.height * 0.5;
    let mut pts = Vec::with_capacity(count);

    for i in 0..count {
        let fi = i as f32;
        let e = p.band_at(i, count);
        let raw_x = ((fi * 0.37 + (t * (0.3 + (i % 5) as f32 * 0.11))).sin() * 0.42 + 0.5)
            * size.width;
        let raw_y = ((fi * 0.71 + (t * (0.25 + (i % 4) as f32 * 0.13))).cos() * 0.40 + 0.5)
            * size.height;
        let dx = raw_x - cx;
        let dy = raw_y - cy;
        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
        let pt = Point::new(raw_x + (dx / dist) * push * e, raw_y + (dy / dist) * push * e);
        pts.push(pt);
        let r = 2.0 + e * 6.5;
        let col = if i % 3 == 0 { p.text } else { p.accent };
        frame.fill(&Path::circle(pt, r), tint(col, 0.42 + 0.58 * e));
    }
    for i in 0..count {
        let next = pts[(i + 1) % count];
        let dist = pts[i].distance(next);
        if dist < 125.0 {
            frame.stroke(
                &Path::line(pts[i], next),
                Stroke::default()
                    .with_color(tint(p.accent, (1.0 - dist / 125.0) * (0.28 + p.mid() * 0.35)))
                    .with_width(1.1),
            );
        }
    }
}

fn draw_snow(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let count = 48;
    let gust = 1.0 + p.high() * 1.8 + p.viz.beat * 0.8;
    let t = p.viz.phase * p.speed.clamp(0.25, 3.0) * gust;
    for i in 0..count {
        let fi = i as f32;
        let e = p.band_at(i, count);
        let layer_spd = 18.0 + (i % 4) as f32 * 10.0;
        let y = ((fi * 31.7 + t * layer_spd) % size.height).max(0.0);
        let drift = (t * 1.1 + fi * 0.9).sin() * (8.0 + p.mid() * 16.0);
        let x = (((fi * 73.1) % size.width) + drift).rem_euclid(size.width);
        let r = 1.6 + (i % 4) as f32 * 0.8 + e * 2.2;
        let col = if i % 5 == 0 { p.accent } else { p.text };
        frame.fill(&Path::circle(Point::new(x, y), r), tint(col, 0.65 + e * 0.35));
    }
}

fn draw_flame(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let tongues = 20;
    let base_y = size.height - 12.0;
    let warm = Color::from_rgb8(0xf9, 0x73, 0x16);
    let ember = Color::from_rgb8(0xef, 0x44, 0x44);
    let core = Color::from_rgb8(0xfd, 0xe0, 0x47);

    for i in 0..tongues {
        let e = p.band_at(i, tongues);
        let u = (i as f32 + 0.5) / tongues as f32;
        let center_bias = 1.0 - ((u - 0.5) * 1.5).abs().clamp(0.0, 0.72);
        let h = (26.0 + (e + p.viz.beat * 0.25) * (size.height - 46.0)) * center_bias;
        let sway = (p.viz.phase * 4.2 + i as f32 * 0.8).sin() * (6.0 + p.mid() * 10.0);
        let x = 24.0 + u * (size.width - 48.0);
        let w = (size.width / tongues as f32) * 0.95;

        let flame_path = Path::new(|b| {
            b.move_to(Point::new(x - w * 0.5, base_y));
            b.quadratic_curve_to(
                Point::new(x - w * 0.25 + sway * 0.5, base_y - h * 0.62),
                Point::new(x + sway, base_y - h),
            );
            b.quadratic_curve_to(
                Point::new(x + w * 0.25 + sway * 0.5, base_y - h * 0.56),
                Point::new(x + w * 0.5, base_y),
            );
            b.close();
        });
        let col = if i % 3 == 0 { ember } else { warm };
        frame.fill(&flame_path, Fill::from(tint(col, 0.38 + 0.58 * e)));

        // Hot inner flame tongue
        let inner = Path::new(|b| {
            let ih = h * 0.52;
            let iw = w * 0.45;
            b.move_to(Point::new(x - iw * 0.5, base_y));
            b.quadratic_curve_to(
                Point::new(x + sway * 0.4, base_y - ih * 0.6),
                Point::new(x + sway * 0.6, base_y - ih),
            );
            b.quadratic_curve_to(
                Point::new(x + iw * 0.4, base_y - ih * 0.5),
                Point::new(x + iw * 0.5, base_y),
            );
            b.close();
        });
        frame.fill(&inner, Fill::from(tint(core, 0.22 + 0.45 * e)));
    }
}

fn draw_runner(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let ground_y = size.height * 0.76;
    let t = p.viz.phase * (3.6 + p.bass() * 2.2) * p.speed.clamp(0.25, 3.0);

    let horizon = Path::new(|b| {
        for i in 0..=40 {
            let u = i as f32 / 40.0;
            let x = u * size.width;
            let e = p.band_at(i, 41);
            let y = ground_y - e * 18.0 + (u * 10.0 + t * 0.7).sin() * 3.0;
            if i == 0 {
                b.move_to(Point::new(x, y));
            } else {
                b.line_to(Point::new(x, y));
            }
        }
    });
    frame.stroke(
        &horizon,
        Stroke::default().with_color(tint(p.accent, 0.8)).with_width(2.2),
    );

    let cx = size.width * 0.48 + (t * 0.22).sin() * 28.0;
    let bounce = (t * 2.0).sin().abs() * 5.0 + p.viz.beat * 12.0;
    let hip = Point::new(cx, ground_y - 34.0 - bounce);
    let shoulder = Point::new(cx + 8.0, hip.y - 26.0);
    let head = Point::new(shoulder.x + 4.0, shoulder.y - 12.0);

    let stride = t.sin();
    let leg_a = Point::new(hip.x + stride * 20.0, ground_y - bounce * 0.35);
    let leg_b = Point::new(hip.x - stride * 20.0, ground_y - bounce * 0.55);
    let arm_a = Point::new(shoulder.x - stride * 16.0, shoulder.y + 14.0);
    let arm_b = Point::new(shoulder.x + stride * 16.0, shoulder.y + 10.0);

    let stroke = Stroke::default().with_color(p.text).with_width(3.2);
    frame.fill(&Path::circle(head, 7.5 + p.viz.beat * 1.5), p.accent);
    frame.stroke(&Path::line(hip, shoulder), stroke);
    frame.stroke(&Path::line(hip, leg_a), stroke);
    frame.stroke(&Path::line(hip, leg_b), stroke);
    frame.stroke(&Path::line(shoulder, arm_a), stroke);
    frame.stroke(&Path::line(shoulder, arm_b), stroke);
}

fn draw_boulder(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let left = Point::new(24.0, size.height * 0.84);
    let top = Point::new(size.width - 24.0, size.height * 0.28);
    frame.stroke(
        &Path::line(left, top),
        Stroke::default().with_color(tint(p.muted, 0.7)).with_width(3.0),
    );

    let prog = ((p.viz.phase * 0.14 * p.speed.clamp(0.25, 3.0)) % 1.0 + 0.15).clamp(0.15, 0.85);
    let slope_pt = Point::new(
        left.x + (top.x - left.x) * prog,
        left.y + (top.y - left.y) * prog,
    );
    let radius = 22.0 + p.bass() * 10.0 + p.viz.beat * 4.0;
    let rock_center = Point::new(slope_pt.x + 12.0, slope_pt.y - radius - 2.0);
    frame.fill(&Path::circle(rock_center, radius), tint(p.accent, 0.88));
    frame.stroke(
        &Path::circle(rock_center, radius),
        Stroke::default().with_color(p.text).with_width(2.0),
    );

    let feet = Point::new(slope_pt.x - 22.0, slope_pt.y + 4.0);
    let torso = Point::new(feet.x + 12.0, feet.y - 26.0);
    let head = Point::new(torso.x + 5.0, torso.y - 9.0);
    let s = Stroke::default().with_color(p.text).with_width(3.0);
    frame.fill(&Path::circle(head, 6.5), p.text);
    frame.stroke(&Path::line(feet, torso), s);
    frame.stroke(
        &Path::line(torso, Point::new(rock_center.x - radius * 0.8, rock_center.y + 4.0)),
        s,
    );
}

fn draw_bicycle(frame: &mut Frame, size: Size, p: &VisualizerProgram) {
    let ground_y = size.height * 0.76;
    let t = p.viz.phase * (3.0 + p.bass() * 1.5) * p.speed.clamp(0.25, 3.0);
    let wave_road = Path::new(|b| {
        for i in 0..=48 {
            let u = i as f32 / 48.0;
            let x = u * size.width;
            let e = p.band_at(i, 49);
            let y = ground_y + (u * 8.0 + t * 0.8).sin() * 4.0 - e * 10.0;
            if i == 0 {
                b.move_to(Point::new(x, y));
            } else {
                b.line_to(Point::new(x, y));
            }
        }
    });
    frame.stroke(
        &wave_road,
        Stroke::default().with_color(tint(p.muted, 0.75)).with_width(2.2),
    );

    let hop = p.viz.beat * 6.0;
    let cx = size.width * 0.5;
    let wheel_r = 24.0;
    let rear = Point::new(cx - 42.0, ground_y - wheel_r - hop);
    let front = Point::new(cx + 42.0, ground_y - wheel_r - hop);
    let crank = Point::new(cx - 8.0, ground_y - wheel_r - hop);
    let seat = Point::new(cx - 24.0, ground_y - wheel_r - 28.0 - hop);
    let stem = Point::new(cx + 28.0, ground_y - wheel_r - 30.0 - hop);

    let wheel_stroke = Stroke::default().with_color(p.accent).with_width(2.8);
    let frame_stroke = Stroke::default().with_color(p.text).with_width(2.6);
    frame.stroke(&Path::circle(rear, wheel_r), wheel_stroke);
    frame.stroke(&Path::circle(front, wheel_r), wheel_stroke);

    for w in [rear, front] {
        for sp in 0..4 {
            let a = t * 1.4 + sp as f32 * (TAU / 4.0);
            frame.stroke(
                &Path::line(
                    Point::new(w.x + a.cos() * wheel_r, w.y + a.sin() * wheel_r),
                    Point::new(w.x - a.cos() * wheel_r, w.y - a.sin() * wheel_r),
                ),
                Stroke::default().with_color(tint(p.accent, 0.55)).with_width(1.3),
            );
        }
    }

    frame.stroke(&Path::line(rear, crank), frame_stroke);
    frame.stroke(&Path::line(rear, seat), frame_stroke);
    frame.stroke(&Path::line(crank, seat), frame_stroke);
    frame.stroke(&Path::line(crank, stem), frame_stroke);
    frame.stroke(&Path::line(seat, stem), frame_stroke);
    frame.stroke(&Path::line(front, stem), frame_stroke);
}
