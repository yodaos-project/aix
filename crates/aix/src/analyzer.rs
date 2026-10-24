use crate::xml::{self, Node};
use alloc::{format, string::String, string::ToString, vec::Vec};
#[cfg(not(feature = "std"))]
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct PageConstraint {
    pub width: f64,
    pub height: f64,
}

impl Default for PageConstraint {
    fn default() -> Self {
        Self {
            width: 480.0,
            height: 168.0,
        }
    }
}

pub struct PageAnalyzer;

impl PageAnalyzer {
    fn parse_px_value(val: &str) -> Option<f64> {
        let val = val.trim();
        if val.ends_with("px") {
            val.trim_end_matches("px").parse::<f64>().ok()
        } else {
            None
        }
    }

    fn parse_style_block(style: &str) -> (Option<f64>, Option<f64>) {
        let mut width = None;
        let mut height = None;
        for prop in style.split(';') {
            if let Some((key, val)) = prop.split_once(':') {
                match key.trim() {
                    "width" => width = Self::parse_px_value(val),
                    "height" => height = Self::parse_px_value(val),
                    _ => {}
                }
            }
        }
        (width, height)
    }

    fn parse_wcss(wcss: &str) -> HashMap<String, (Option<f64>, Option<f64>)> {
        let mut rules = HashMap::new();
        for rule_block in wcss.split('}') {
            if let Some((selector_part, properties_part)) = rule_block.split_once('{') {
                for selector in selector_part.split(',') {
                    let selector = selector.trim();
                    if !selector.is_empty() {
                        let (width, height) = Self::parse_style_block(properties_part);
                        if width.is_some() || height.is_some() {
                            rules.insert(selector.to_string(), (width, height));
                        }
                    }
                }
            }
        }
        rules
    }

    pub fn analyze(wxml: Option<&str>, wcss: Option<&str>) -> PageConstraint {
        let mut max_width: f64 = 0.0;
        let mut max_height: f64 = 0.0;
        let mut found_fixed = false;

        let css_rules = wcss.map(Self::parse_wcss).unwrap_or_default();

        if let Some(wxml_content) = wxml {
            if let Ok(nodes) = xml::parse_xml(wxml_content) {
                for node in nodes {
                    if let Node::Element { attributes, .. } = node {
                        let mut width = None;
                        let mut height = None;
                        let id = attributes.get("id");
                        let class_attr = attributes.get("class");
                        let inline_style = attributes.get("style");

                        let classes: Vec<String> = class_attr
                            .map(|s| s.split_whitespace().map(|c| c.to_string()).collect())
                            .unwrap_or_default();

                        // 1. Check external stylesheet
                        if let Some(id_val) = id {
                            let id_selector = format!("#{}", id_val);
                            if let Some(&(w, h)) = css_rules.get(&id_selector) {
                                if width.is_none() {
                                    width = w;
                                }
                                if height.is_none() {
                                    height = h;
                                }
                            }
                        }

                        for class in &classes {
                            let class_selector = format!(".{}", class);
                            if let Some(&(w, h)) = css_rules.get(&class_selector) {
                                if width.is_none() {
                                    width = w;
                                }
                                if height.is_none() {
                                    height = h;
                                }
                            }
                        }

                        // 2. Check inline style (overrides external)
                        if let Some(style_str) = inline_style {
                            let (w, h) = Self::parse_style_block(style_str);
                            if let Some(val) = w {
                                width = Some(val);
                            }
                            if let Some(val) = h {
                                height = Some(val);
                            }
                        }

                        if let Some(w) = width {
                            if w > max_width {
                                max_width = w;
                            }
                            found_fixed = true;
                        }
                        if let Some(h) = height {
                            max_height += h;
                            found_fixed = true;
                        }
                    }
                }
            }
        }

        if found_fixed {
            PageConstraint {
                width: if max_width > 0.0 { max_width } else { 480.0 },
                height: if max_height > 0.0 { max_height } else { 168.0 },
            }
        } else {
            PageConstraint::default()
        }
    }

    pub fn analyze_sfc(sfc_content: &str) -> PageConstraint {
        Self::analyze_sfc_blocks(&xml::parse_sfc_blocks(sfc_content))
    }

    /// Analyze a shared SFC parse result without rescanning or copying block text.
    pub fn analyze_sfc_blocks(blocks: &[xml::SfcBlock<'_>]) -> PageConstraint {
        let mut wxml = None;
        let mut wcss = None;
        for block in blocks {
            match block.name {
                "page" | "template" => wxml = Some(block.body),
                "style" => wcss = Some(block.body),
                _ => {}
            }
        }
        Self::analyze(wxml, wcss)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_dimensions_override_classes_and_partial_inline_sizes_preserve_other_dimensions() {
        let size = PageAnalyzer::analyze(Some(r#"<view id="card" class="first second" style="height: 40px"/>"#), Some("#card {width:320px;height:60px} .first,.second, {width:200px;height:20px} .ignored {display:flex}"));
        assert_eq!(
            size,
            PageConstraint {
                width: 320.0,
                height: 40.0
            }
        );
        assert_eq!(
            PageAnalyzer::analyze(Some("<view style='height: 0px'/>"), None),
            PageConstraint::default()
        );
        assert_eq!(
            PageAnalyzer::analyze(Some("<view"), None),
            PageConstraint::default()
        );
    }

    #[test]
    fn test_analyze_inline_style() {
        let wxml = r#"<view style="width: 100px; height: 200px;"></view>"#;
        let size = PageAnalyzer::analyze(Some(wxml), None);
        assert_eq!(
            size,
            PageConstraint {
                width: 100.0,
                height: 200.0
            }
        );
    }

    #[test]
    fn test_analyze_external_wcss() {
        let wxml = r#"<view class="container"></view>"#;
        let wcss = r#".container { width: 300px; height: 150px; }"#;
        let size = PageAnalyzer::analyze(Some(wxml), Some(wcss));
        assert_eq!(
            size,
            PageConstraint {
                width: 300.0,
                height: 150.0
            }
        );
    }

    #[test]
    fn test_analyze_multi_root_nodes() {
        let wxml = r#"
            <view style="width: 100px; height: 100px;"></view>
            <view style="width: 200px; height: 50px;"></view>
        "#;
        let size = PageAnalyzer::analyze(Some(wxml), None);
        // Max width 200, Total height 100 + 50 = 150
        assert_eq!(
            size,
            PageConstraint {
                width: 200.0,
                height: 150.0
            }
        );
    }

    #[test]
    fn test_analyze_relative_units() {
        let wxml = r#"<view style="width: 100%; height: 100vh;"></view>"#;
        let size = PageAnalyzer::analyze(Some(wxml), None);
        // Should fallback to default
        assert_eq!(size, PageConstraint::default());
    }

    #[test]
    fn test_analyze_mixed_units() {
        let wxml = r#"<view style="width: 100px; height: 100%;"></view>"#;
        let size = PageAnalyzer::analyze(Some(wxml), None);
        // width is 100, height falls back to default 168
        assert_eq!(
            size,
            PageConstraint {
                width: 100.0,
                height: 168.0
            }
        );
    }

    #[test]
    fn test_analyze_empty_tag() {
        let wxml = r#"<view style="width: 100px; height: 100px;" />"#;
        let size = PageAnalyzer::analyze(Some(wxml), None);
        assert_eq!(
            size,
            PageConstraint {
                width: 100.0,
                height: 100.0
            }
        );
    }

    #[test]
    fn test_analyze_sfc() {
        let sfc = r#"
<script def>
{}
</script>
<page>
<view class="container"></view>
</page>
<style>
.container { width: 400px; height: 300px; }
</style>
        "#;
        let size = PageAnalyzer::analyze_sfc(sfc);
        assert_eq!(
            size,
            PageConstraint {
                width: 400.0,
                height: 300.0
            }
        );
    }

    #[test]
    fn test_analyze_id_selector() {
        let wxml = r#"<view id="main"></view>"#;
        let wcss = r#"#main { width: 480px; height: 240px; }"#;
        let size = PageAnalyzer::analyze(Some(wxml), Some(wcss));
        assert_eq!(
            size,
            PageConstraint {
                width: 480.0,
                height: 240.0
            }
        );
    }

    #[test]
    fn test_analyze_no_content() {
        let size = PageAnalyzer::analyze(None, None);
        assert_eq!(size, PageConstraint::default());
    }

    #[test]
    fn test_analyze_inline_overrides_css() {
        let wxml = r#"<view class="container" style="width: 200px;"></view>"#;
        let wcss = r#".container { width: 300px; height: 150px; }"#;
        let size = PageAnalyzer::analyze(Some(wxml), Some(wcss));
        // inline style width overrides CSS width; CSS height still applies
        assert_eq!(
            size,
            PageConstraint {
                width: 200.0,
                height: 150.0
            }
        );
    }

    #[test]
    fn test_analyze_sfc_inline_style() {
        let sfc = r#"
<script def>
{}
</script>
<page>
<view style="width: 360px; height: 200px;"></view>
</page>
<style>
</style>
        "#;
        let size = PageAnalyzer::analyze_sfc(sfc);
        assert_eq!(
            size,
            PageConstraint {
                width: 360.0,
                height: 200.0
            }
        );
    }

    #[test]
    fn test_analyze_sfc_template_tag() {
        let sfc = r#"
<script def>
{}
</script>
<template>
<view class="container"></view>
</template>
<style>
.container { width: 320px; height: 160px; }
</style>
        "#;
        let size = PageAnalyzer::analyze_sfc(sfc);
        assert_eq!(
            size,
            PageConstraint {
                width: 320.0,
                height: 160.0
            }
        );
    }

    #[test]
    fn test_analyze_sfc_no_dimensions() {
        let sfc = r#"
<script def>
{}
</script>
<page>
<view class="container"></view>
</page>
<style>
.container { color: red; }
</style>
        "#;
        let size = PageAnalyzer::analyze_sfc(sfc);
        assert_eq!(size, PageConstraint::default());
    }

    #[test]
    fn test_analyze_sfc_multiple_root_nodes() {
        let sfc = r#"
<script def>
{}
</script>
<page>
<view class="header"></view>
<view class="body"></view>
</page>
<style>
.header { width: 480px; height: 60px; }
.body { width: 480px; height: 300px; }
</style>
        "#;
        let size = PageAnalyzer::analyze_sfc(sfc);
        // Max width across roots: 480px; heights sum (header 60px + body 300px) = 360px
        assert_eq!(
            size,
            PageConstraint {
                width: 480.0,
                height: 360.0
            }
        );
    }
}
