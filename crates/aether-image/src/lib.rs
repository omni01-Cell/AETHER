pub mod cpu;
pub mod gpu;

use aether_core::{AetherError, Asset, AssetKind, Ref};
use image::GenericImageView;
use resvg::usvg;
use std::fs;
use std::path::Path;
use tiny_skia::{Color, Pixmap, Transform};

/// Helper to parse standard hex colors (e.g. "#FF0000" or "red") into tiny-skia Color.
pub fn parse_color(color_str: &str) -> Color {
    let s = color_str.trim().to_lowercase();
    match s.as_str() {
        "red" => Color::from_rgba8(255, 0, 0, 255),
        "green" => Color::from_rgba8(0, 255, 0, 255),
        "blue" => Color::from_rgba8(0, 0, 255, 255),
        "white" => Color::from_rgba8(255, 255, 255, 255),
        "black" => Color::from_rgba8(0, 0, 0, 255),
        "yellow" => Color::from_rgba8(255, 255, 0, 255),
        "cyan" => Color::from_rgba8(0, 255, 255, 255),
        "magenta" => Color::from_rgba8(255, 0, 255, 255),
        hex if hex.starts_with('#') => {
            let hex_val = hex.trim_start_matches('#');
            if hex_val.len() == 6 {
                let r = u8::from_str_radix(&hex_val[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex_val[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex_val[4..6], 16).unwrap_or(0);
                Color::from_rgba8(r, g, b, 255)
            } else if hex_val.len() == 8 {
                let r = u8::from_str_radix(&hex_val[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex_val[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex_val[4..6], 16).unwrap_or(0);
                let a = u8::from_str_radix(&hex_val[6..8], 16).unwrap_or(255);
                Color::from_rgba8(r, g, b, a)
            } else {
                Color::from_rgba8(128, 128, 128, 255)
            }
        }
        _ => Color::from_rgba8(128, 128, 128, 255),
    }
}

/// Imports a raster image using Content-Addressable Storage (CAS) with Blake3 hashing.
pub fn import_image<P: AsRef<Path>>(
    src_path: P,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let src = src_path.as_ref();
    if !src.exists() {
        return Err(AetherError::IoError(
            src.to_string_lossy().to_string(),
            "Source image file does not exist".to_string(),
        ));
    }

    // 1. Calculate Blake3 hash
    let mut file = fs::File::open(src)
        .map_err(|e| AetherError::IoError(src.to_string_lossy().to_string(), e.to_string()))?;
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| AetherError::IoError(src.to_string_lossy().to_string(), e.to_string()))?;
    let hash = hasher.finalize().to_hex().to_string();

    // 2. Fetch image metadata
    let img = image::ImageReader::open(src)
        .map_err(|e| AetherError::MediaError(format!("Failed to open image file: {}", e)))?
        .decode()
        .map_err(|e| AetherError::MediaError(format!("Failed to decode image metadata: {}", e)))?;

    let (width, height) = img.dimensions();
    let metadata = serde_json::json!({
        "width": width,
        "height": height,
        "color_type": format!("{:?}", img.color()),
    });

    // 3. Move/Copy to the cache directory
    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir).map_err(|e| {
            AetherError::IoError(cache_dir.to_string_lossy().to_string(), e.to_string())
        })?;
    }
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let cache_file_name = format!("{}.{}", hash, ext);
    let cache_file_path = cache_dir.join(cache_file_name);

    if !cache_file_path.exists() {
        fs::copy(src, &cache_file_path)
            .map_err(|e| AetherError::IoError(src.to_string_lossy().to_string(), e.to_string()))?;
    }

    Ok(Asset {
        r,
        kind: AssetKind::Image,
        path: cache_file_path,
        hash,
        metadata,
    })
}

/// Creates a blank colored canvas Pixmap and registers it as an Asset.
pub fn create_canvas(
    width: u32,
    height: u32,
    color_str: &str,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
        AetherError::MediaError(format!(
            "Failed to create canvas pixmap {}x{}",
            width, height
        ))
    })?;

    let color = parse_color(color_str);
    pixmap.fill(color);

    // Save Pixmap to cache under a unique hash
    let png_bytes = pixmap
        .encode_png()
        .map_err(|e| AetherError::MediaError(format!("Failed to encode canvas PNG: {}", e)))?;

    let hash = blake3::hash(&png_bytes).to_hex().to_string();

    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir).map_err(|e| {
            AetherError::IoError(cache_dir.to_string_lossy().to_string(), e.to_string())
        })?;
    }
    let file_path = cache_dir.join(format!("{}.png", hash));
    fs::write(&file_path, png_bytes).map_err(|e| {
        AetherError::IoError(file_path.to_string_lossy().to_string(), e.to_string())
    })?;

    Ok(Asset {
        r,
        kind: AssetKind::Image,
        path: file_path,
        hash,
        metadata: serde_json::json!({
            "width": width,
            "height": height,
            "background": color_str.to_string(),
        }),
    })
}

/// Draws styled text onto an existing image asset using high-fidelity resvg rendering.
pub fn draw_text(
    asset: &Asset,
    text: &str,
    font: &str,
    size: f32,
    x: f32,
    y: f32,
    color_str: &str,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let original_pixmap = Pixmap::load_png(&asset.path)
        .map_err(|e| AetherError::MediaError(format!("Failed to load PNG asset: {}", e)))?;

    let width = original_pixmap.width();
    let height = original_pixmap.height();

    // Create a new empty pixmap for rendering the text
    let mut text_pixmap = Pixmap::new(width, height).ok_or_else(|| {
        AetherError::MediaError("Failed to create temporary text pixmap".to_string())
    })?;

    // Build usvg Font Database
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts();

    // Construct usvg Options
    let mut opt = usvg::Options::default();
    opt.fontdb = std::sync::Arc::new(fontdb);

    // Construct complete SVG string
    let svg_str = format!(
        r##"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">
            <text x="{}" y="{}" font-family="{}" font-size="{}" fill="{}">{}</text>
        </svg>"##,
        width, height, x, y, font, size, color_str, text
    );

    let tree = usvg::Tree::from_str(&svg_str, &opt)
        .map_err(|e| AetherError::MediaError(format!("Failed to parse SVG text: {}", e)))?;

    // Render text on top of the text_pixmap
    resvg::render(&tree, Transform::default(), &mut text_pixmap.as_mut());

    // Composite original pixmap and text pixmap
    let mut final_pixmap = original_pixmap;
    final_pixmap.draw_pixmap(
        0,
        0,
        text_pixmap.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        Transform::default(),
        None,
    );

    // Save final Pixmap to cache under a unique hash
    let png_bytes = final_pixmap
        .encode_png()
        .map_err(|e| AetherError::MediaError(format!("Failed to encode combined PNG: {}", e)))?;

    let hash = blake3::hash(&png_bytes).to_hex().to_string();

    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir).map_err(|e| {
            AetherError::IoError(cache_dir.to_string_lossy().to_string(), e.to_string())
        })?;
    }
    let file_path = cache_dir.join(format!("{}.png", hash));
    fs::write(&file_path, png_bytes).map_err(|e| {
        AetherError::IoError(file_path.to_string_lossy().to_string(), e.to_string())
    })?;

    Ok(Asset {
        r,
        kind: AssetKind::Image,
        path: file_path,
        hash,
        metadata: serde_json::json!({
            "width": width,
            "height": height,
            "parent_hash": asset.hash.clone(),
            "text": text.to_string(),
        }),
    })
}

/// Exports an image asset by copying it to the designated destination path.
pub fn export_image<P: AsRef<Path>>(asset: &Asset, dest_path: P) -> Result<(), AetherError> {
    let dest = dest_path.as_ref();
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string())
        })?;
    }
    fs::copy(&asset.path, dest)
        .map_err(|e| AetherError::IoError(dest.to_string_lossy().to_string(), e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::RefKind;
    use std::path::PathBuf;

    fn temp_test_dir() -> PathBuf {
        let unique_dir = format!(
            "test_image_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        std::env::temp_dir().join(unique_dir)
    }

    #[test]
    fn test_color_parsing() {
        let red = parse_color("red");
        assert_eq!(red, Color::from_rgba8(255, 0, 0, 255));

        let hex6 = parse_color("#00ff00");
        assert_eq!(hex6, Color::from_rgba8(0, 255, 0, 255));

        let hex8 = parse_color("#0000ff80");
        assert_eq!(hex8, Color::from_rgba8(0, 0, 255, 128));

        let default_color = parse_color("invalid_color");
        assert_eq!(default_color, Color::from_rgba8(128, 128, 128, 255));
    }

    #[test]
    fn test_canvas_creation() {
        let dir = temp_test_dir();
        let cache_dir = dir.join("cache");
        let r = Ref {
            kind: RefKind::Image,
            id: 1,
        };
        let asset = create_canvas(400, 300, "blue", r, &cache_dir).unwrap();

        assert_eq!(asset.r, r);
        assert_eq!(asset.kind, AssetKind::Image);
        assert!(asset.path.exists());
        assert_eq!(asset.metadata["width"].as_u64().unwrap(), 400);
        assert_eq!(asset.metadata["height"].as_u64().unwrap(), 300);

        // Test export_image
        let export_path = dir.join("exported.png");
        export_image(&asset, &export_path).unwrap();
        assert!(export_path.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_image_import_and_text_overlay() {
        let dir = temp_test_dir();
        fs::create_dir_all(&dir).unwrap();
        let cache_dir = dir.join("cache");

        // 1. Generate a source png on the fly
        let src_png = dir.join("src_image.png");
        let mut pixmap = Pixmap::new(200, 100).unwrap();
        pixmap.fill(Color::from_rgba8(255, 255, 255, 255));
        pixmap.save_png(&src_png).unwrap();

        // 2. Import the source image
        let r1 = Ref {
            kind: RefKind::Image,
            id: 1,
        };
        let asset1 = import_image(&src_png, r1, &cache_dir).unwrap();

        assert_eq!(asset1.r, r1);
        assert!(asset1.path.exists());
        assert_eq!(asset1.metadata["width"].as_u64().unwrap(), 200);

        // 3. Draw text overlay on imported image
        let r2 = Ref {
            kind: RefKind::Image,
            id: 2,
        };
        let asset2 = draw_text(
            &asset1,
            "Hello",
            "sans-serif",
            24.0,
            20.0,
            50.0,
            "red",
            r2,
            &cache_dir,
        )
        .unwrap();

        assert_eq!(asset2.r, r2);
        assert!(asset2.path.exists());
        assert_eq!(
            asset2.metadata["parent_hash"].as_str().unwrap(),
            asset1.hash
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_render_backends_cpu_and_gpu() {
        use aether_core::{
            CompositionGraph, Connection, FilterKind, Node, NodeKind, RefRegistry, RenderBackend,
        };

        let dir = temp_test_dir();
        let cache_dir = dir.join("cache");

        let r1 = Ref {
            kind: RefKind::Image,
            id: 1,
        };
        let asset1 = create_canvas(100, 100, "red", r1, &cache_dir).unwrap();

        let registry = RefRegistry::new();
        registry.register(r1, asset1).unwrap();

        let mut graph = CompositionGraph::new();
        graph.add_node(Node {
            id: 1,
            kind: NodeKind::Source(r1),
        });
        graph.add_node(Node {
            id: 2,
            kind: NodeKind::Filter {
                kind: FilterKind::Brightness { delta: 0.1 },
            },
        });
        graph.add_node(Node {
            id: 3,
            kind: NodeKind::Output,
        });

        graph
            .connect(Connection {
                from_node: 1,
                from_port: 0,
                to_node: 2,
                to_port: 0,
            })
            .unwrap();
        graph
            .connect(Connection {
                from_node: 2,
                from_port: 0,
                to_node: 3,
                to_port: 0,
            })
            .unwrap();
        graph.output_node = Some(3);

        let cpu = cpu::CpuBackend;
        let cpu_data = cpu.render(&graph, 50, 50, &registry).unwrap();
        assert_eq!(cpu_data.len(), 50 * 50 * 4);

        let gpu = gpu::GpuBackend;
        let gpu_res = gpu.render(&graph, 50, 50, &registry);

        #[cfg(feature = "gpu")]
        {
            let gpu_data = gpu_res.unwrap();
            assert_eq!(gpu_data.len(), 50 * 50 * 4);
        }
        #[cfg(not(feature = "gpu"))]
        {
            assert!(gpu_res.is_err());
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
