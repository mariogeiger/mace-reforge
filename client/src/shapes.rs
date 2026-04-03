use leptos::prelude::*;
use mace_reforge_shared::Shape;

pub const ALL_SHAPES: &[Shape] = &[
    Shape::Circle,
    Shape::Square,
    Shape::Triangle,
    Shape::Diamond,
    Shape::Star,
    Shape::Hexagon,
];

pub const PALETTE: &[&str] = &[
    "#c0392b", "#e67e22", "#f1c40f", "#27ae60", "#2980b9", "#8e44ad", "#e84393", "#2d3436",
];

pub fn shape_svg(shape: Shape, color: String, size: f64) -> impl IntoView {
    let s = size;
    let half = s / 2.0;
    let vb = format!("0 0 {s} {s}");
    let inner = match &shape {
        Shape::Circle => format!(
            r#"<circle cx="{half}" cy="{half}" r="{}" fill="{color}"/>"#,
            half * 0.85
        ),
        Shape::Square => {
            let inset = s * 0.12;
            let side = s - inset * 2.0;
            format!(
                r#"<rect x="{inset}" y="{inset}" width="{side}" height="{side}" rx="{}" fill="{color}"/>"#,
                s * 0.08
            )
        }
        Shape::Triangle => {
            let top = s * 0.1;
            let bot = s * 0.9;
            format!(
                r#"<polygon points="{half},{top} {bot},{bot} {top},{bot}" fill="{color}"/>"#
            )
        }
        Shape::Diamond => {
            let m = s * 0.08;
            let e = s - m;
            format!(
                r#"<polygon points="{half},{m} {e},{half} {half},{e} {m},{half}" fill="{color}"/>"#
            )
        }
        Shape::Star => {
            let cx = half;
            let cy = half;
            let ro = half * 0.9;
            let ri = half * 0.35;
            let mut pts = String::new();
            for i in 0..10 {
                let angle =
                    std::f64::consts::FRAC_PI_2 * -1.0 + std::f64::consts::PI * i as f64 / 5.0;
                let r = if i % 2 == 0 { ro } else { ri };
                if !pts.is_empty() {
                    pts.push(' ');
                }
                pts.push_str(&format!(
                    "{:.1},{:.1}",
                    cx + r * angle.cos(),
                    cy + r * angle.sin()
                ));
            }
            format!(r#"<polygon points="{pts}" fill="{color}"/>"#)
        }
        Shape::Hexagon => {
            let cx = half;
            let cy = half;
            let r = half * 0.88;
            let mut pts = String::new();
            for i in 0..6 {
                let angle = std::f64::consts::PI / 3.0 * i as f64 - std::f64::consts::FRAC_PI_2;
                if !pts.is_empty() {
                    pts.push(' ');
                }
                pts.push_str(&format!(
                    "{:.1},{:.1}",
                    cx + r * angle.cos(),
                    cy + r * angle.sin()
                ));
            }
            format!(r#"<polygon points="{pts}" fill="{color}"/>"#)
        }
    };

    view! {
        <svg
            viewBox=vb
            xmlns="http://www.w3.org/2000/svg"
            inner_html=inner
        />
    }
}

pub fn shape_name(shape: &Shape) -> &'static str {
    match shape {
        Shape::Circle => "Circle",
        Shape::Square => "Square",
        Shape::Triangle => "Triangle",
        Shape::Diamond => "Diamond",
        Shape::Star => "Star",
        Shape::Hexagon => "Hexagon",
    }
}
