use std::collections::HashMap;
use tiny_skia::Pixmap;
use aether_core::{
    RenderBackend, CompositionGraph, RefRegistry, AetherError, NodeKind,
    BlendMode, FilterKind, NodeId
};

pub struct CpuBackend;

impl RenderBackend for CpuBackend {
    fn render(
        &self,
        graph: &CompositionGraph,
        width: u32,
        height: u32,
        registry: &RefRegistry,
    ) -> Result<Vec<u8>, AetherError> {
        let sorted = graph.topological_sort()?;
        let mut node_pixmaps: HashMap<NodeId, Pixmap> = HashMap::new();
        
        for node_id in sorted {
            let node = graph.nodes.get(&node_id)
                .ok_or_else(|| AetherError::OperationFailed(format!("Node {} not found", node_id)))?;
                
            match &node.kind {
                NodeKind::Source(r) => {
                    let asset = registry.resolve(r)?;
                    let src_pixmap = Pixmap::load_png(&asset.path)
                        .map_err(|e| AetherError::MediaError(format!("Failed to load source image: {}", e)))?;
                        
                    let mut scaled = Pixmap::new(width, height)
                        .ok_or_else(|| AetherError::MediaError("Failed to allocate scaled pixmap".to_string()))?;
                        
                    let scale_x = width as f32 / src_pixmap.width() as f32;
                    let scale_y = height as f32 / src_pixmap.height() as f32;
                    let paint = tiny_skia::PixmapPaint::default();
                    scaled.draw_pixmap(
                        0, 0,
                        src_pixmap.as_ref(),
                        &paint,
                        tiny_skia::Transform::from_scale(scale_x, scale_y),
                        None,
                    );
                    
                    node_pixmaps.insert(node_id, scaled);
                }
                NodeKind::Blend { mode, opacity } => {
                    let mut base_node_id = None;
                    let mut overlay_node_id = None;
                    for conn in &graph.connections {
                        if conn.to_node == node_id {
                            if conn.to_port == 0 {
                                base_node_id = Some(conn.from_node);
                            } else if conn.to_port == 1 {
                                overlay_node_id = Some(conn.from_node);
                            }
                        }
                    }
                    
                    let base_pixmap = base_node_id.and_then(|id| node_pixmaps.get(&id))
                        .ok_or_else(|| AetherError::OperationFailed(format!("Missing base input for Blend node {}", node_id)))?;
                    let overlay_pixmap = overlay_node_id.and_then(|id| node_pixmaps.get(&id))
                        .ok_or_else(|| AetherError::OperationFailed(format!("Missing overlay input for Blend node {}", node_id)))?;
                        
                    let mut blended = base_pixmap.clone();
                    let skia_blend = match mode {
                        BlendMode::Normal => tiny_skia::BlendMode::SourceOver,
                        BlendMode::Multiply => tiny_skia::BlendMode::Multiply,
                        BlendMode::Screen => tiny_skia::BlendMode::Screen,
                        BlendMode::Overlay => tiny_skia::BlendMode::Overlay,
                        BlendMode::SoftLight => tiny_skia::BlendMode::SoftLight,
                    };
                    
                    let mut paint = tiny_skia::PixmapPaint::default();
                    paint.blend_mode = skia_blend;
                    paint.opacity = *opacity;
                    blended.draw_pixmap(
                        0, 0,
                        overlay_pixmap.as_ref(),
                        &paint,
                        tiny_skia::Transform::identity(),
                        None,
                    );
                    
                    node_pixmaps.insert(node_id, blended);
                }
                NodeKind::Transition { kind: _, duration_ms: _ } => {
                    let mut base_node_id = None;
                    let mut overlay_node_id = None;
                    for conn in &graph.connections {
                        if conn.to_node == node_id {
                            if conn.to_port == 0 {
                                base_node_id = Some(conn.from_node);
                            } else if conn.to_port == 1 {
                                overlay_node_id = Some(conn.from_node);
                            }
                        }
                    }
                    
                    let base_pixmap = base_node_id.and_then(|id| node_pixmaps.get(&id))
                        .ok_or_else(|| AetherError::OperationFailed(format!("Missing base input for Transition node {}", node_id)))?;
                    let overlay_pixmap = overlay_node_id.and_then(|id| node_pixmaps.get(&id))
                        .ok_or_else(|| AetherError::OperationFailed(format!("Missing overlay input for Transition node {}", node_id)))?;
                        
                    let mut blended = base_pixmap.clone();
                    let mut paint = tiny_skia::PixmapPaint::default();
                    paint.blend_mode = tiny_skia::BlendMode::SourceOver;
                    paint.opacity = 0.5; // Default 50% midpoint mix
                    blended.draw_pixmap(
                        0, 0,
                        overlay_pixmap.as_ref(),
                        &paint,
                        tiny_skia::Transform::identity(),
                        None,
                    );
                    
                    node_pixmaps.insert(node_id, blended);
                }
                NodeKind::Filter { kind } => {
                    let input_node_id = graph.connections.iter()
                        .find(|c| c.to_node == node_id && c.to_port == 0)
                        .map(|c| c.from_node)
                        .ok_or_else(|| AetherError::OperationFailed(format!("Missing input for Filter node {}", node_id)))?;
                        
                    let input_pixmap = node_pixmaps.get(&input_node_id)
                        .ok_or_else(|| AetherError::OperationFailed(format!("Input pixmap not found for Filter node {}", node_id)))?;
                        
                    let mut filtered = input_pixmap.clone();
                    match kind {
                        FilterKind::GaussianBlur { radius } => {
                            apply_box_blur(&mut filtered, *radius);
                        }
                        FilterKind::Contrast { factor } => {
                            let pixels = filtered.pixels_mut();
                            for p in pixels {
                                let r = p.red() as f32 / 255.0;
                                let g = p.green() as f32 / 255.0;
                                let b = p.blue() as f32 / 255.0;
                                let a = p.alpha() as f32 / 255.0;
                                
                                let r_new = (((r - 0.5) * factor + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                                let g_new = (((g - 0.5) * factor + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                                let b_new = (((b - 0.5) * factor + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                                
                                *p = tiny_skia::ColorU8::from_rgba(r_new, g_new, b_new, (a * 255.0) as u8).premultiply();
                            }
                        }
                        FilterKind::Brightness { delta } => {
                            let pixels = filtered.pixels_mut();
                            for p in pixels {
                                let r = p.red() as f32 / 255.0;
                                let g = p.green() as f32 / 255.0;
                                let b = p.blue() as f32 / 255.0;
                                let a = p.alpha() as f32 / 255.0;
                                
                                let r_new = ((r + delta).clamp(0.0, 1.0) * 255.0) as u8;
                                let g_new = ((g + delta).clamp(0.0, 1.0) * 255.0) as u8;
                                let b_new = ((b + delta).clamp(0.0, 1.0) * 255.0) as u8;
                                
                                *p = tiny_skia::ColorU8::from_rgba(r_new, g_new, b_new, (a * 255.0) as u8).premultiply();
                            }
                        }
                    }
                    
                    node_pixmaps.insert(node_id, filtered);
                }
                NodeKind::Output => {
                    let input_node_id = graph.connections.iter()
                        .find(|c| c.to_node == node_id && c.to_port == 0)
                        .map(|c| c.from_node)
                        .ok_or_else(|| AetherError::OperationFailed(format!("Missing input for Output node {}", node_id)))?;
                        
                    let input_pixmap = node_pixmaps.get(&input_node_id)
                        .ok_or_else(|| AetherError::OperationFailed(format!("Input pixmap not found for Output node {}", node_id)))?;
                        
                    return Ok(input_pixmap.data().to_vec());
                }
            }
        }
        
        Err(AetherError::OperationFailed("Graph had no output node".to_string()))
    }
}

// Optimizing apply_box_blur:
// 1. Replaced O(W * H * R) naive box kernel sampling with an O(W * H) sliding window algorithm.
// 2. Replaced floating point additions inside the inner loop with integer channel accumulators.
// 3. Allocated single temporary pixel buffer to avoid redundant vector allocations/clones.
fn apply_box_blur(pixmap: &mut Pixmap, radius: f32) {
    let r = radius.round() as i32;
    if r <= 0 { return; }
    
    let w = pixmap.width() as usize;
    let h = pixmap.height() as usize;
    if w == 0 || h == 0 { return; }

    let count = (2 * r + 1) as f32;
    let r_i32 = r;
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    let src = pixmap.pixels();
    let mut temp = vec![tiny_skia::PremultipliedColorU8::TRANSPARENT; w * h];

    // Horizontal blur pass: read from src, write to temp
    for y in 0..h {
        let row_offset = y * w;

        let mut r_sum: u32 = 0;
        let mut g_sum: u32 = 0;
        let mut b_sum: u32 = 0;
        let mut a_sum: u32 = 0;

        for dx in -r_i32..=r_i32 {
            let nx = dx.clamp(0, w_i32 - 1) as usize;
            let p = src[row_offset + nx];
            r_sum += p.red() as u32;
            g_sum += p.green() as u32;
            b_sum += p.blue() as u32;
            a_sum += p.alpha() as u32;
        }

        for x in 0..w {
            let r_avg = (r_sum as f32 / count) as u8;
            let g_avg = (g_sum as f32 / count) as u8;
            let b_avg = (b_sum as f32 / count) as u8;
            let a_avg = (a_sum as f32 / count) as u8;

            temp[row_offset + x] = tiny_skia::ColorU8::from_rgba(r_avg, g_avg, b_avg, a_avg).premultiply();

            let x_i32 = x as i32;
            let leave_x = (x_i32 - r_i32).max(0) as usize;
            let enter_x = (x_i32 + 1 + r_i32).min(w_i32 - 1) as usize;

            let p_leave = src[row_offset + leave_x];
            let p_enter = src[row_offset + enter_x];

            r_sum = r_sum + p_enter.red() as u32 - p_leave.red() as u32;
            g_sum = g_sum + p_enter.green() as u32 - p_leave.green() as u32;
            b_sum = b_sum + p_enter.blue() as u32 - p_leave.blue() as u32;
            a_sum = a_sum + p_enter.alpha() as u32 - p_leave.alpha() as u32;
        }
    }

    // Vertical blur pass: read from temp, write to pixmap.pixels_mut()
    let dst = pixmap.pixels_mut();
    for x in 0..w {
        let mut r_sum: u32 = 0;
        let mut g_sum: u32 = 0;
        let mut b_sum: u32 = 0;
        let mut a_sum: u32 = 0;

        for dy in -r_i32..=r_i32 {
            let ny = dy.clamp(0, h_i32 - 1) as usize;
            let p = temp[ny * w + x];
            r_sum += p.red() as u32;
            g_sum += p.green() as u32;
            b_sum += p.blue() as u32;
            a_sum += p.alpha() as u32;
        }

        for y in 0..h {
            let r_avg = (r_sum as f32 / count) as u8;
            let g_avg = (g_sum as f32 / count) as u8;
            let b_avg = (b_sum as f32 / count) as u8;
            let a_avg = (a_sum as f32 / count) as u8;

            dst[y * w + x] = tiny_skia::ColorU8::from_rgba(r_avg, g_avg, b_avg, a_avg).premultiply();

            let y_i32 = y as i32;
            let leave_y = (y_i32 - r_i32).max(0) as usize;
            let enter_y = (y_i32 + 1 + r_i32).min(h_i32 - 1) as usize;

            let p_leave = temp[leave_y * w + x];
            let p_enter = temp[enter_y * w + x];

            r_sum = r_sum + p_enter.red() as u32 - p_leave.red() as u32;
            g_sum = g_sum + p_enter.green() as u32 - p_leave.green() as u32;
            b_sum = b_sum + p_enter.blue() as u32 - p_leave.blue() as u32;
            a_sum = a_sum + p_enter.alpha() as u32 - p_leave.alpha() as u32;
        }
    }
}
