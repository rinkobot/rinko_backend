///! Satellite status renderer - Generate images from data
use super::types::{ReportStatus, SatelliteInfo};
use anyhow::{Context, Result};
use chrono::{DateTime, Timelike, Utc};
use std::path::{Path, PathBuf};

/// Map time difference to color gradient (green -> yellow -> red)
/// Based on hours difference between target time and now
fn map_time_to_color(target_time: &str, now_utc: &DateTime<Utc>, min_hours: f64, max_hours: f64) -> Result<String> {
    // Parse target time
    let target = DateTime::parse_from_rfc3339(target_time)
        .context("Failed to parse target time")?;
    let target_utc = target.with_timezone(&Utc);

    // Calculate hour difference
    let delta_hours = (now_utc.signed_duration_since(target_utc)).num_seconds().abs() as f64 / 3600.0;

    // Color anchors
    let green = (125u8, 227u8, 61u8);   // #7de33d - fresh
    let yellow = (255u8, 255u8, 0u8);   // #ffff00 - medium
    let red = (255u8, 0u8, 0u8);        // #ff0000 - stale

    let (r, g, b) = if delta_hours <= min_hours {
        green
    } else if delta_hours >= max_hours {
        red
    } else {
        let mid = (min_hours + max_hours) / 2.0;
        if delta_hours <= mid {
            // Interpolate green -> yellow
            let t = (delta_hours - min_hours) / (mid - min_hours);
            lerp_color(green, yellow, t)
        } else {
            // Interpolate yellow -> red
            let t = (delta_hours - mid) / (max_hours - mid);
            lerp_color(yellow, red, t)
        }
    };

    Ok(format!("#{:02X}{:02X}{:02X}", r, g, b))
}

/// Linear color interpolation
fn lerp_color(c1: (u8, u8, u8), c2: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let r = (c1.0 as f64 + (c2.0 as f64 - c1.0 as f64) * t).round() as u8;
    let g = (c1.1 as f64 + (c2.1 as f64 - c1.1 as f64) * t).round() as u8;
    let b = (c1.2 as f64 + (c2.2 as f64 - c1.2 as f64) * t).round() as u8;
    (r, g, b)
}

const SVG_TEMPLATE: &str = include_str!("../../../resources/sat_template.svg");
const BLOCK_TITLE_HEIGHT: f32 = 45.0;
const HEADER_HEIGHT: f32 = 40.0;
const ROW_HEIGHT: f32 = 38.0;
const BLOCK_SPACING: f32 = 30.0;
const TOP_PADDING: f32 = 20.0;
const FOOTER_HEIGHT: f32 = 32.0;
const MAX_REPORTS_PER_SATELLITE: usize = 5;

// Layout positions
const X_CALLSIGN: f32 = 20.0;
const X_GRIDS: f32 = 170.0;
const X_REPORT: f32 = 280.0;
const X_TIME: f32 = 540.0;
const COLOR_BLOCK_WIDTH: f32 = 12.0;
const COLOR_BLOCK_HEIGHT: f32 = 18.0;
const COLOR_BLOCK_TEXT_SPACING: f32 = 8.0;

/// Satellite status renderer
pub struct SatelliteRenderer {
    output_dir: PathBuf,
}

impl SatelliteRenderer {
    /// Create a new renderer
    pub fn new(output_dir: impl AsRef<Path>) -> Self {
        Self {
            output_dir: output_dir.as_ref().to_path_buf(),
        }
    }

    /// Render satellite status to image
    /// 
    /// # Arguments
    /// * `satellites` - List of satellites to render
    /// 
    /// # Returns
    /// Path to the generated image file
    pub async fn render_satellites(&self, satellites: &[SatelliteInfo]) -> Result<PathBuf> {
        // Ensure output directory exists
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .context("Failed to create output directory")?;

        // Generate filename based on satellites and current time
        let filename = self.generate_filename(satellites);
        let output_path = self.output_dir.join(&filename);

        // Check if image already exists (cache hit)
        if output_path.exists() {
            tracing::debug!("Using cached image: {:?}", output_path);
            return Ok(output_path);
        }

        // Generate the image
        self.generate_image(satellites, &output_path).await?;

        tracing::info!("Generated satellite status image: {:?}", output_path);

        Ok(output_path)
    }

    /// Generate filename for the rendered image
    fn generate_filename(&self, satellites: &[SatelliteInfo]) -> String {
        let now = chrono::Utc::now();
        
        // Floor to 15-minute blocks for caching
        let minute = (now.minute() / 15) * 15;
        let floored = now
            .with_minute(minute)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap();

        let time_str = floored.format("%Y%m%d_%H%M").to_string();

        // Generate satellite names part
        let sat_names: Vec<String> = satellites
            .iter()
            .take(5) // Limit to avoid too long filenames
            .map(|s| Self::normalize_sat_name(&s.name))
            .collect();

        let sat_part = if sat_names.is_empty() {
            "empty".to_string()
        } else if sat_names.len() > 3 {
            format!("{}_and_{}_more", sat_names[..2].join("_"), sat_names.len() - 2)
        } else {
            sat_names.join("_")
        };

        format!("sat_{}_{}.png", time_str, sat_part)
    }

    /// Normalize satellite name for filename (remove special characters)
    fn normalize_sat_name(name: &str) -> String {
        name.chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase()
    }

    /// Generate the actual image
    /// 
    /// Uses SVG template and renders to PNG
    async fn generate_image(&self, satellites: &[SatelliteInfo], output_path: &Path) -> Result<()> {
        // Generate SVG content
        let svg_content = self.generate_svg(satellites)?;

        // Render to PNG
        self.render_svg_to_png(&svg_content, output_path).await?;

        Ok(())
    }

    /// Generate SVG content from template
    fn generate_svg(&self, satellites: &[SatelliteInfo]) -> Result<String> {
        let mut current_y = TOP_PADDING;
        let mut content = String::new();
        let now_utc = Utc::now();

        if satellites.is_empty() {
            content.push_str(&format!(
                r#"<text x="410" y="100" text-anchor="middle" class="table-text">No satellite data available.</text>"#
            ));
            current_y = 120.0;
        } else {
            for sat in satellites {
                content.push_str(&self.generate_satellite_block(sat, &mut current_y, &now_utc)?);
            }
        }

        // Generate footer
        let footer = self.generate_footer(current_y);
        let total_height = current_y + FOOTER_HEIGHT;

        // Replace placeholders in template
        let svg = SVG_TEMPLATE
            .replace("{{SVG_HEIGHT}}", &total_height.to_string())
            .replace("{{CONTENT}}", &content)
            .replace("{{FOOTER}}", &footer);

        Ok(svg)
    }

    /// Generate a single satellite block
    fn generate_satellite_block(
        &self,
        sat: &SatelliteInfo,
        current_y: &mut f32,
        now_utc: &DateTime<Utc>,
    ) -> Result<String> {
        let mut block = String::new();

        // Satellite title
        block.push_str(&format!(
            r#"<text x="{}" y="{}" class="satellite-title">{}</text>"#,
            X_CALLSIGN,
            *current_y + BLOCK_TITLE_HEIGHT / 2.0,
            Self::escape_xml(&sat.name)
        ));
        block.push('\n');
        *current_y += BLOCK_TITLE_HEIGHT;

        // AMSAT update status
        let (status_class, status_text) = if sat.amsat_update_status {
            ("amsat-update-success", "AMSAT Update: Success")
        } else {
            ("amsat-update-failure", "AMSAT Update: Failed")
        };

        let logo_size = ROW_HEIGHT * 0.6;
        let logo_x = X_TIME;
        let logo_y = *current_y + (ROW_HEIGHT - logo_size) / 2.0;

        block.push_str(&format!(
            r#"<image x="{}" y="{}" width="{}" height="{}" href="resources/amsat.png"/><text x="{}" y="{}" class="{}">{}</text>
"#,
            logo_x,
            logo_y,
            logo_size,
            logo_size,
            logo_x + logo_size + 10.0,
            *current_y + ROW_HEIGHT / 2.0,
            status_class,
            status_text
        ));

        // Last update time
        let last_update_str = sat.last_updated.format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let hours_ago = (now_utc.signed_duration_since(sat.last_updated)).num_hours();
        
        block.push_str(&format!(
            r#"<text x="{}" y="{}" class="table-text">Last update: {} ({}h ago)</text>"#,
            X_CALLSIGN,
            *current_y + ROW_HEIGHT / 2.0,
            last_update_str,
            hours_ago
        ));
        block.push('\n');
        *current_y += ROW_HEIGHT;

        // Check if we have data
        if sat.data_blocks.is_empty() {
            block.push_str(&format!(
                r#"<text x="410" y="{}" text-anchor="middle" class="table-text">No reports available.</text>"#,
                *current_y + 40.0
            ));
            block.push('\n');
            *current_y += 80.0;
            *current_y += BLOCK_SPACING;
            return Ok(block);
        }

        // Table header
        block.push_str(&format!(
            r##"<g class="header">
<rect x="0" y="{}" width="100%" height="{}" fill="#f0f2f5" />
<text x="{}" y="{}" class="table-text header-text">Callsign</text>
<text x="{}" y="{}" class="table-text header-text">Grids</text>
<text x="{}" y="{}" class="table-text header-text">Report</text>
<text x="{}" y="{}" class="table-text header-text">Time</text>
</g>
"##,
            *current_y,
            HEADER_HEIGHT,
            X_CALLSIGN,
            *current_y + HEADER_HEIGHT / 2.0,
            X_GRIDS,
            *current_y + HEADER_HEIGHT / 2.0,
            X_REPORT,
            *current_y + HEADER_HEIGHT / 2.0,
            X_TIME,
            *current_y + HEADER_HEIGHT / 2.0,
        ));
        *current_y += HEADER_HEIGHT;

        // Data rows (limit to MAX_REPORTS_PER_SATELLITE)
        let mut row_count = 0;
        'outer: for data_block in &sat.data_blocks {
            for report in &data_block.reports {
                if row_count >= MAX_REPORTS_PER_SATELLITE {
                    break 'outer;
                }

                let y_pos = *current_y + ROW_HEIGHT / 2.0;
                let report_color = ReportStatus::string_to_color_hex(&report.report);
                let report_text = ReportStatus::from_string(&report.report).to_string();
                
                // Calculate time difference
                let report_time = DateTime::parse_from_rfc3339(&report.reported_time)
                    .unwrap_or_else(|_| Utc::now().into());
                let hours_ago = (now_utc.signed_duration_since(report_time)).num_hours();

                // Calculate time color (0-12 hours gradient: green -> yellow -> red)
                let time_color = map_time_to_color(&report.reported_time, now_utc, 0.0, 12.0)
                    .unwrap_or_else(|e| {
                        tracing::warn!("Failed to map time to color: {}", e);
                        "#808080".to_string() // Default gray
                    });

                block.push_str(&format!(
                    r##"<g class="data-row">
   <text x="{}" y="{}" class="table-text">{}</text>
   <text x="{}" y="{}" class="table-text">{}</text>
   <rect x="{}" y="{}" width="{}" height="{}" fill="{}" rx="1" />
   <text x="{}" y="{}" class="table-text">{}</text>
   <rect x="{}" y="{}" width="{}" height="{}" fill="{}" rx="1" />
   <text x="{}" y="{}" class="table-text">{} ({}h ago)</text>
</g>
"##,
                    X_CALLSIGN,
                    y_pos,
                    Self::escape_xml(&report.callsign),
                    X_GRIDS,
                    y_pos,
                    Self::escape_xml(&report.grid_square),
                    X_REPORT,
                    y_pos - COLOR_BLOCK_HEIGHT / 2.0,
                    COLOR_BLOCK_WIDTH,
                    COLOR_BLOCK_HEIGHT,
                    report_color,
                    X_REPORT + COLOR_BLOCK_WIDTH + COLOR_BLOCK_TEXT_SPACING,
                    y_pos,
                    report_text,
                    X_TIME,
                    y_pos - COLOR_BLOCK_HEIGHT / 2.0,
                    COLOR_BLOCK_WIDTH,
                    COLOR_BLOCK_HEIGHT,
                    time_color,
                    X_TIME + COLOR_BLOCK_WIDTH + COLOR_BLOCK_TEXT_SPACING,
                    y_pos,
                    report.reported_time,
                    hours_ago
                ));

                *current_y += ROW_HEIGHT;
                row_count += 1;
            }
        }

        *current_y += BLOCK_SPACING;
        Ok(block)
    }

    /// Generate footer
    fn generate_footer(&self, footer_y: f32) -> String {
        let render_time = chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        
        format!(
            r##"    <g id="footer">
<rect x="0" y="{}" width="100%" height="{}" fill="#f0f2f5" />
<text x="50%" y="{}" class="table-text footer-text" text-anchor="middle">
    Powered by Rinko, rendered at {} BJT
</text>
</g>
"##,
            footer_y,
            FOOTER_HEIGHT,
            footer_y + FOOTER_HEIGHT / 2.0,
            render_time
        )
    }

    /// Escape XML special characters
    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    /// Render SVG to PNG using resvg
    async fn render_svg_to_png(&self, svg_content: &str, output_path: &Path) -> Result<()> {
        use resvg::render;
        use usvg::{Options, Transform, Tree};
        use tiny_skia::Pixmap;
        use fontdb::Database;

        // Load fonts from fonts directory
        let mut fontdb = Database::new();
        fontdb.load_fonts_dir("fonts");
        tracing::debug!("Loaded {} font faces from fonts directory", fontdb.len());

        // Parse SVG with font database
        let mut options = Options::default();
        options.font_family = "Consolas".to_string();
        options.fontdb = std::sync::Arc::new(fontdb);
        
        let tree = Tree::from_str(svg_content, &options)
            .context("Failed to parse SVG")?;

        // Create pixmap
        let size = tree.size().to_int_size();
        let mut pixmap = Pixmap::new(size.width(), size.height())
            .context("Failed to create pixmap")?;

        // Render
        render(&tree, Transform::default(), &mut pixmap.as_mut());

        // Encode to PNG
        let png_data = pixmap.encode_png()
            .context("Failed to encode PNG")?;

        // Write to file
        tokio::fs::write(output_path, png_data)
            .await
            .context("Failed to write PNG file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_sat_name() {
        assert_eq!(SatelliteRenderer::normalize_sat_name("AO-91"), "ao91");
        assert_eq!(SatelliteRenderer::normalize_sat_name("ISS-FM"), "issfm");
        assert_eq!(SatelliteRenderer::normalize_sat_name("PO-101[FM]"), "po101fm");
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(SatelliteRenderer::escape_xml("Test & <tag>"), "Test &amp; &lt;tag&gt;");
    }

    #[tokio::test]
    async fn test_render_empty() {
        let temp_dir = std::env::temp_dir().join("rinko_render_test");
        let renderer = SatelliteRenderer::new(&temp_dir);
        
        let result = renderer.render_satellites(&[]).await;
        assert!(result.is_ok());

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
