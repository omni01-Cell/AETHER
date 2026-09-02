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
                            // Optimization (Bolt): Precompute a 256-entry lookup table (LUT) for 8-bit channel transformation.
                            // This elides millions of floating-point conversions, divisions, and multiplications per frame.
                            let mut lut = [0u8; 256];
                            for i in 0..256 {
                                let c_f = i as f32 / 255.0;
                                lut[i] = (((c_f - 0.5) * factor + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                            }
                            for p in filtered.pixels_mut() {
                                let r_new = lut[p.red() as usize];
                                let g_new = lut[p.green() as usize];
                                let b_new = lut[p.blue() as usize];
                                *p = tiny_skia::ColorU8::from_rgba(r_new, g_new, b_new, p.alpha()).premultiply();
                            }
                        }
                        FilterKind::Brightness { delta } => {
                            // Optimization (Bolt): Precompute a 256-entry lookup table (LUT) for 8-bit channel transformation.
                            // This elides millions of floating-point conversions, divisions, and multiplications per frame.
                            let mut lut = [0u8; 256];
                            for i in 0..256 {
                                let c_f = i as f32 / 255.0;
                                lut[i] = ((c_f + delta).clamp(0.0, 1.0) * 255.0) as u8;
                            }
                            for p in filtered.pixels_mut() {
                                let r_new = lut[p.red() as usize];
                                let g_new = lut[p.green() as usize];
                                let b_new = lut[p.blue() as usize];
                                *p = tiny_skia::ColorU8::from_rgba(r_new, g_new, b_new, p.alpha()).premultiply();
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

fn apply_box_blur(pixmap: &mut Pixmap, radius: f32) {
    let r = radius.round() as i32;
    if r <= 0 { return; }
    
    let w = pixmap.width() as i32;
    let h = pixmap.height() as i32;
    let pixels = pixmap.pixels().to_vec();
    let mut temp = pixels.clone();
    
    // Horizontal blur pass
    for y in 0..h {
        for x in 0..w {
            let mut r_sum = 0.0;
            let mut g_sum = 0.0;
            let mut b_sum = 0.0;
            let mut a_sum = 0.0;
            let mut count = 0.0;
            
            for dx in -r..=r {
                let nx = (x + dx).clamp(0, w - 1);
                let p = pixels[(y * w + nx) as usize];
                r_sum += p.red() as f32;
                g_sum += p.green() as f32;
                b_sum += p.blue() as f32;
                a_sum += p.alpha() as f32;
                count += 1.0;
            }
            
            let dest = &mut temp[(y * w + x) as usize];
            *dest = tiny_skia::ColorU8::from_rgba(
                (r_sum / count) as u8,
                (g_sum / count) as u8,
                (b_sum / count) as u8,
                (a_sum / count) as u8,
            ).premultiply();
        }
    }
    
    // Vertical blur pass
    let pixels_h = temp.clone();
    let pixels_mut = pixmap.pixels_mut();
    for x in 0..w {
        for y in 0..h {
            let mut r_sum = 0.0;
            let mut g_sum = 0.0;
            let mut b_sum = 0.0;
            let mut a_sum = 0.0;
            let mut count = 0.0;
            
            for dy in -r..=r {
                let ny = (y + dy).clamp(0, h - 1);
                let p = pixels_h[(ny * w + x) as usize];
                r_sum += p.red() as f32;
                g_sum += p.green() as f32;
                b_sum += p.blue() as f32;
                a_sum += p.alpha() as f32;
                count += 1.0;
            }
            
            pixels_mut[(y * w + x) as usize] = tiny_skia::ColorU8::from_rgba(
                (r_sum / count) as u8,
                (g_sum / count) as u8,
                (b_sum / count) as u8,
                (a_sum / count) as u8,
            ).premultiply();
        }
    }
}
