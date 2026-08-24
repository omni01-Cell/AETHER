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

/// Applies a two-pass separable box blur to the given pixmap.
///
/// OPTIMIZATION: Uses an O(1) per-pixel sliding window algorithm with integer accumulators.
/// Previously, for every pixel, the filter iterated `2r + 1` times per channel across both passes,
/// yielding O(W * H * r) complexity. The sliding window reduces per-pixel updates to a single
/// subtraction and addition, achieving O(W * H) complexity independent of blur radius `r`.
fn apply_box_blur(pixmap: &mut Pixmap, radius: f32) {
    let r = radius.round() as i32;
    if r <= 0 { return; }
    
    let w = pixmap.width() as i32;
    let h = pixmap.height() as i32;
    if w <= 0 || h <= 0 { return; }

    let count = (2 * r + 1) as u32;
    let pixels = pixmap.pixels().to_vec();
    let mut temp = pixels.clone();
    
    // Horizontal blur pass - O(1) per pixel sliding window
    for y in 0..h {
        let row_offset = (y * w) as usize;

        // Initialize sliding window accumulator for x = 0
        let mut r_sum: u32 = 0;
        let mut g_sum: u32 = 0;
        let mut b_sum: u32 = 0;
        let mut a_sum: u32 = 0;

        for dx in -r..=r {
            let nx = dx.clamp(0, w - 1) as usize;
            let p = pixels[row_offset + nx];
            r_sum += p.red() as u32;
            g_sum += p.green() as u32;
            b_sum += p.blue() as u32;
            a_sum += p.alpha() as u32;
        }

        for x in 0..w {
            temp[row_offset + x as usize] = tiny_skia::ColorU8::from_rgba(
                (r_sum / count) as u8,
                (g_sum / count) as u8,
                (b_sum / count) as u8,
                (a_sum / count) as u8,
            ).premultiply();

            // Advance window to x + 1 by subtracting leaving pixel and adding entering pixel
            let leaving_x = (x - r).clamp(0, w - 1) as usize;
            let entering_x = (x + 1 + r).clamp(0, w - 1) as usize;

            let p_leave = pixels[row_offset + leaving_x];
            let p_enter = pixels[row_offset + entering_x];

            r_sum = r_sum + p_enter.red() as u32 - p_leave.red() as u32;
            g_sum = g_sum + p_enter.green() as u32 - p_leave.green() as u32;
            b_sum = b_sum + p_enter.blue() as u32 - p_leave.blue() as u32;
            a_sum = a_sum + p_enter.alpha() as u32 - p_leave.alpha() as u32;
        }
    }
    
    // Vertical blur pass - O(1) per pixel sliding window
    let pixels_h = temp.clone();
    let pixels_mut = pixmap.pixels_mut();
    let w_usize = w as usize;

    for x in 0..w {
        let col_offset = x as usize;

        // Initialize sliding window accumulator for y = 0
        let mut r_sum: u32 = 0;
        let mut g_sum: u32 = 0;
        let mut b_sum: u32 = 0;
        let mut a_sum: u32 = 0;

        for dy in -r..=r {
            let ny = dy.clamp(0, h - 1) as usize;
            let p = pixels_h[ny * w_usize + col_offset];
            r_sum += p.red() as u32;
            g_sum += p.green() as u32;
            b_sum += p.blue() as u32;
            a_sum += p.alpha() as u32;
        }

        for y in 0..h {
            let cur_y = y as usize;
            pixels_mut[cur_y * w_usize + col_offset] = tiny_skia::ColorU8::from_rgba(
                (r_sum / count) as u8,
                (g_sum / count) as u8,
                (b_sum / count) as u8,
                (a_sum / count) as u8,
            ).premultiply();

            // Advance window to y + 1 by subtracting leaving pixel and adding entering pixel
            let leaving_y = (y - r).clamp(0, h - 1) as usize;
            let entering_y = (y + 1 + r).clamp(0, h - 1) as usize;

            let p_leave = pixels_h[leaving_y * w_usize + col_offset];
            let p_enter = pixels_h[entering_y * w_usize + col_offset];

            r_sum = r_sum + p_enter.red() as u32 - p_leave.red() as u32;
            g_sum = g_sum + p_enter.green() as u32 - p_leave.green() as u32;
            b_sum = b_sum + p_enter.blue() as u32 - p_leave.blue() as u32;
            a_sum = a_sum + p_enter.alpha() as u32 - p_leave.alpha() as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_blur_correctness() {
        let mut pixmap = Pixmap::new(5, 5).unwrap();
        // Fill center pixel (2, 2) with white (255, 255, 255, 255)
        let white = tiny_skia::ColorU8::from_rgba(255, 255, 255, 255).premultiply();
        pixmap.pixels_mut()[2 * 5 + 2] = white;

        // Apply blur with radius 1
        apply_box_blur(&mut pixmap, 1.0);

        // Radius 1 means 3x3 box window, box count = 3 * 3 = 9.
        // Expected center pixel value after two 1D box passes = 255 / 9 = 28 (truncated integer arithmetic)
        let center_pixel = pixmap.pixels()[2 * 5 + 2];
        assert!(
            center_pixel.red() > 0 && center_pixel.red() <= 30,
            "Center pixel red channel expected around 28, got {}",
            center_pixel.red()
        );

        // Check corner pixel (0, 0) remains black/transparent
        let corner_pixel = pixmap.pixels()[0];
        assert_eq!(corner_pixel.red(), 0);
    }
}
