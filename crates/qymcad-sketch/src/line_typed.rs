//! THE TYPED SIZES OF THE LINE IN HAND.
//!
//! While a chain of lines is being drawn, the length and the angle of the segment under the pointer are
//! typed as a number or a formula (`100-20`, `35/4`). The characters are filtered on the way in - only
//! what a formula of one real number may hold - and at Enter the value must READ as one real number
//! (`parse_num`): anything else is refused in words, and nothing is laid. TAB moves the keyboard between
//! the length and the angle; Esc gives the typing up and keeps the tool.

use egui::Context;
use qymcad_ui_state::{LineField, Notice, SketchCtx, SketchShape};
use super::*;

/// THE CHARACTERS A FORMULA OF A SIZE MAY HOLD: digits, the decimal point and comma, the signs and
/// parentheses of an expression, the space between them. Everything else is dropped before it reaches
/// the field.
const FORMULA_CHARS: &str = "0123456789.,+-*/() ";

/// READ THE KEYBOARD of the line in hand: characters filtered into the active field, Backspace taking
/// the last one, TAB moving between the length and the angle. Enter is the frame's - it arrives at
/// `apply`, where the segment is laid out of the values that read as real numbers.
pub fn typing_events(sk: &mut SketchCtx, ctx: &Context) {
    // THE FIELDS LIVE ONLY WHILE A CHAIN IS UNDER WAY: another tool, or a chain not begun, means no
    // keyboard of its own
    if sk.armed.drawing() != Some(SketchShape::Line) || sk.tool.pts.is_empty() {
        sk.tool.line_typing = None;
        return;
    }
    // THE RING STAYS ON THE SHEET: whatever egui's focus navigation left in the sidebars is given up
    // while the fields hold the keyboard, so TAB never lights a button of a panel
    ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
    let events = ctx.input(|i| i.events.clone());
    for ev in events {
        match ev {
            egui::Event::Text(t) => {
                let field = *sk.tool.line_typing.get_or_insert(LineField::Length);
                let buf = match field {
                    LineField::Length => &mut sk.tool.line_len,
                    LineField::Angle => &mut sk.tool.line_ang,
                };
                for c in t.chars().filter(|c| FORMULA_CHARS.contains(*c)) {
                    buf.push(c);
                }
                // THE PICTURE WAITS FOR THE HAND: the preview follows the typed values only after the
                // pause since the last character (TYPED_SETTLE of the preview)
                sk.tool.typed_at = Some(std::time::Instant::now());
            }
            egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                let buf = match sk.tool.line_typing {
                    Some(LineField::Length) => &mut sk.tool.line_len,
                    Some(LineField::Angle) => &mut sk.tool.line_ang,
                    None => continue,
                };
                buf.pop();
                sk.tool.typed_at = Some(std::time::Instant::now());
            }
            _ => {}
        }
    }
}

/// The active field's text and the field itself, or none while the keyboard is not in the line.
pub fn typed_texts(sk: &SketchCtx) -> Option<(LineField, String, String)> {
    let field = sk.tool.line_typing?;
    Some((field, sk.tool.line_len.clone(), sk.tool.line_ang.clone()))
}

/// TAB, SEEN AT THE RAW INPUT: the keyboard moves between the length and the angle of the line in hand.
/// The key is filtered out before egui reads it (`App::raw_input_hook`), so its focus navigation never
/// carries the ring away into the sidebars - the key works only on the two fields.
pub fn toggle_line_field(tool: &mut qymcad_ui_state::SketchTool) {
    let now = tool.line_typing.unwrap_or(LineField::Length);
    tool.line_typing = Some(match now {
        LineField::Length => LineField::Angle,
        LineField::Angle => LineField::Length,
    });
}

/// THE SEGMENT LAID BY ENTER out of the typed length and angle: each value must read as one real number,
/// and what is not typed is taken from where the pointer stands. The endpoint goes through THE SAME step
/// a click goes through (`line_step`), as one undo step of its own.
pub fn apply_typed(sk: &mut SketchCtx, si: usize) {
    let Some(&last) = sk.tool.pts.last() else { return };
    let len = parse_num(&*sk.project, &sk.tool.line_len);
    let ang = parse_num(&*sk.project, &sk.tool.line_ang);
    // the direction and the length as the pointer holds them now - what the typed values replace
    let (cursor_len, cursor_dir) = match sk.tool.cursor {
        Some(c) => {
            let (dx, dy) = (c.x - last.x, c.y - last.y);
            let l = (dx * dx + dy * dy).sqrt();
            (l, if l > 1e-9 { Point2::new(dx / l, dy / l) } else { Point2::new(1.0, 0.0) })
        }
        None => (0.0, Point2::new(1.0, 0.0)),
    };
    // THE LENGTH: typed, or the pointer's
    let Some(len) = len.or((cursor_len > 1e-9).then_some(cursor_len)).filter(|v| *v > 0.0) else {
        *sk.status = Notice::refusal(qymcad_i18n::tr("sk-typed-not-a-number"));
        return;
    };
    // THE DIRECTION: the typed angle is THE ANGLE BETWEEN THE OLD SEGMENT AND THE CURRENT ONE - the ray
    // from the corner back along the old segment stands where the arc stands, and the horizontal X axis
    // stands in for the old segment of the first one - turned to the side the pointer stands on; or just
    // the pointer's direction
    let dir = match ang {
        Some(deg) => {
            let base = match sk.tool.pts.len() {
                n if n >= 2 => {
                    let p = sk.tool.pts[n - 2];
                    let (dx, dy) = (p.x - last.x, p.y - last.y);
                    let l = (dx * dx + dy * dy).sqrt();
                    if l > 1e-9 {
                        Point2::new(dx / l, dy / l)
                    } else {
                        Point2::new(-1.0, 0.0)
                    }
                }
                _ => Point2::new(1.0, 0.0),
            };
            // THE SIDE the smaller arc stands on, as the pointer showed it
            let s = match sk.tool.cursor {
                Some(c) => {
                    let cross = base.x * (c.y - last.y) - base.y * (c.x - last.x);
                    if cross < 0.0 {
                        -1.0
                    } else {
                        1.0
                    }
                }
                None => 1.0,
            };
            let rad = deg.to_radians() * s;
            Point2::new(base.x * rad.cos() - base.y * rad.sin(), base.x * rad.sin() + base.y * rad.cos())
        }
        None if cursor_len > 1e-9 => cursor_dir,
        None => {
            *sk.status = Notice::refusal(qymcad_i18n::tr("sk-typed-not-a-number"));
            return;
        }
    };
    let w = Point2::new(last.x + dir.x * len, last.y + dir.y * len);
    line_step(sk, sk.canvas, si, w);
    sk.tool.line_typing = None;
    sk.tool.line_len.clear();
    sk.tool.line_ang.clear();
    // THE CHAIN GOES ON: the segment laid out of its typed sizes is a step of it like a clicked one -
    // the next click, or the next pair of typed sizes, adds to it from the corner the segment ended at
}
