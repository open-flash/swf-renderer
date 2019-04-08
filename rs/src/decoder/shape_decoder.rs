use std::collections::vec_deque::VecDeque;

use swf_tree::{FillStyle, LineStyle, Shape as SwfShape, ShapeRecord, ShapeStyles, Vector2D};
use swf_tree::shape_records::{CurvedEdge, StraightEdge, StyleChange};

#[derive(Debug, Clone)]
pub struct Shape {
  pub paths: Vec<StyledPath>,
}

#[derive(Debug, Clone)]
pub struct StyledPath {
  pub path: lyon::path::Path,
  pub fill: Option<FillStyle>,
  pub line: Option<LineStyle>,
}

pub fn decode_shape(swf_shape: &SwfShape) -> Shape {
  let mut decoder = ShapeDecoder::new(&swf_shape.initial_styles);

  for record in swf_shape.records.iter() {
    match record {
      ShapeRecord::CurvedEdge(ref record) => {
        decoder.apply_curved_edge(record);
      }
      ShapeRecord::StraightEdge(ref record) => {
        decoder.apply_straight_edge(record);
      }
      ShapeRecord::StyleChange(ref record) => {
        decoder.apply_style_change(record);
      }
    }
  }

  decoder.get_shape()
}

fn vec_to_point(vec: Vector2D) -> Option<lyon::math::Point> {
  // TODO: Catch precision errors and return `None`.
  let x: f32 = vec.x as f32;
  let y: f32 = vec.y as f32;
  Some(lyon::math::Point::new(x, y))
}

fn segments_to_path(mut open_set: VecDeque<Segment>) -> lyon::path::Path {
  let mut builder = lyon::path::Path::builder();
  while open_set.len() > 0 {
    let (next_open_set, continuous) = extract_continuous(open_set);
    open_set = next_open_set;
    let mut first: bool = true;
    for segment in continuous.into_iter() {
      if first {
        builder.move_to(vec_to_point(segment.start).unwrap());
        first = false;
      }
      builder.line_to(vec_to_point(segment.end).unwrap());
    }
  }
  builder.build()
}

fn extract_continuous(mut open_set: VecDeque<Segment>) -> (VecDeque<Segment>, VecDeque<Segment>) {
  let first = open_set.pop_front().unwrap();
  let mut start: Vector2D = first.start;
  let mut end: Vector2D = first.end;
  let mut remaining: VecDeque<Segment> = VecDeque::new();
  let mut result: VecDeque<Segment> = VecDeque::new();
  result.push_front(first);
  for segment in open_set.into_iter() {
    if segment.start == end {
      end = segment.end;
      result.push_back(segment);
    } else if segment.end == start {
      start = segment.start;
      result.push_front(segment);
    } else {
      remaining.push_back(segment);
    }
  }
  (remaining, result)
}

const fn add_vec2(left: Vector2D, right: Vector2D) -> Vector2D {
  Vector2D {
    x: left.x + right.x,
    y: left.y + right.y,
  }
}

struct ShapeDecoder {
  layers: Vec<StyleLayer>,
  top_layer: StyleLayerBuilder,
  pos: Vector2D,
}

impl ShapeDecoder {
  pub fn new(styles: &ShapeStyles) -> Self {
    Self {
      layers: Vec::new(),
      top_layer: StyleLayerBuilder::new(styles),
      pos: Vector2D { x: 0, y: 0 },
    }
  }

  pub fn apply_curved_edge(&mut self, record: &CurvedEdge) -> () {
    let control = add_vec2(self.pos, record.control_delta);
    let end = add_vec2(control, record.anchor_delta);
    self.top_layer.add_segment(Segment::new(self.pos, end, Some(control)));
    self.pos = end;
  }

  pub fn apply_straight_edge(&mut self, record: &StraightEdge) -> () {
    let end = add_vec2(self.pos, record.delta);
    self.top_layer.add_segment(Segment::new(self.pos, end, None));
    self.pos = end;
  }

  pub fn apply_style_change(&mut self, record: &StyleChange) -> () {
    if let Some(ref new_styles) = record.new_styles {
      self.set_new_styles(new_styles);
    }
    if let Some(left_fill) = record.left_fill {
      self.top_layer.set_left_fill(left_fill);
    }
    if let Some(right_fill) = record.right_fill {
      self.top_layer.set_right_fill(right_fill);
    }
    if let Some(line_fill) = record.line_style {
      self.top_layer.set_line_fill(line_fill);
    }
    if let Some(move_to) = record.move_to {
      self.pos = move_to;
    }
  }

  pub fn get_shape(self) -> Shape {
    let (top_layer, mut layers) = (self.top_layer, self.layers);
    layers.push(top_layer.build());
    let mut paths: Vec<StyledPath> = Vec::new();
    for layer in layers.into_iter() {
      let (fills, lines) = (layer.fills, layer.lines);
      for segment_set in fills.into_iter() {
        let (style, segments) = (segment_set.style, segment_set.segments);
        let path = segments_to_path(segments);
        paths.push(StyledPath { path, fill: Some(style), line: None });
      }
      for segment_set in lines.into_iter() {
        let (style, segments) = (segment_set.style, segment_set.segments);
        let path = segments_to_path(segments);
        paths.push(StyledPath { path, fill: None, line: Some(style) });
      }
    }
    Shape { paths }
  }

  fn set_new_styles(&mut self, styles: &ShapeStyles) -> () {
    let mut layer = StyleLayerBuilder::new(styles);
    ::std::mem::swap(&mut layer, &mut self.top_layer);
    self.layers.push(layer.build());
  }
}

struct StyleLayer {
  pub fills: Vec<SegmentSet<FillStyle>>,
  pub lines: Vec<SegmentSet<LineStyle>>,
}

struct StyleLayerBuilder {
  fills: Vec<SegmentSet<FillStyle>>,
  lines: Vec<SegmentSet<LineStyle>>,
  left_fill: usize,
  right_fill: usize,
  line_fill: usize,
}

impl StyleLayerBuilder {
  pub fn new(styles: &ShapeStyles) -> Self {
    let fills: Vec<SegmentSet<FillStyle>> = styles.fill.iter()
      .map(|style| SegmentSet { style: style.clone(), segments: VecDeque::new() })
      .collect();
    let lines: Vec<SegmentSet<LineStyle>> = styles.line.iter()
      .map(|style| SegmentSet { style: style.clone(), segments: VecDeque::new() })
      .collect();

    Self { fills, lines, left_fill: 0, right_fill: 0, line_fill: 0 }
  }

  pub fn add_segment(&mut self, segment: Segment) {
    if self.left_fill != 0 {
      self.fills[self.left_fill - 1].segments.push_back(segment);
    }
    if self.right_fill != 0 {
      self.fills[self.right_fill - 1].segments.push_back(segment.reverse());
    }
    if self.line_fill != 0 {
      self.lines[self.line_fill - 1].segments.push_back(segment);
    }
  }

  pub fn build(self) -> StyleLayer {
    StyleLayer { fills: self.fills, lines: self.lines }
  }

  pub fn set_left_fill(&mut self, id: usize) -> () {
    debug_assert!(id < self.fills.len() + 1);
    self.left_fill = id;
  }

  pub fn set_right_fill(&mut self, id: usize) -> () {
    debug_assert!(id < self.fills.len() + 1);
    self.right_fill = id;
  }

  pub fn set_line_fill(&mut self, id: usize) -> () {
    debug_assert!(id < self.lines.len() + 1);
    self.line_fill = id;
  }
}

/**
 * For a given style, the corresponding segments in their order of definition.
 */
struct SegmentSet<S> {
  pub style: S,
  pub segments: VecDeque<Segment>,
}

// (start, control, end)
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
struct Segment {
  start: Vector2D,
  end: Vector2D,
  control: Option<Vector2D>,
}

impl Segment {
  pub fn new(start: Vector2D, end: Vector2D, control: Option<Vector2D>) -> Self {
    Self { start, end, control }
  }

  pub fn reverse(&self) -> Self {
    Self { start: self.end, end: self.start, control: self.control }
  }
}
