//! Alacritty Chart Decorations are drawings or effects over drawings that
//! are not tied to metrics, these could be reference points, alarms/etc.

// Example config:
// - name: load
//   decorations:
//   - type: reference             # Draw a reference line
//     value: 1.0                  # At metrics value 1.0
//     color: "0x00ff00"
//
// TODO: There are several RFCs in rust to allow enum variants to impl a specific Trait but they
// haven't been merged

use crate::*;
use tracing::{event, span, Level};
/// `Decoration` contains several types of decorations to add to a chart
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type")]
pub enum Decoration {
    #[serde(rename = "reference")]
    Reference(ReferencePointDecoration),
    #[serde(rename = "alert")]
    Alert(ActiveAlertUnderLineDecoration),
    None,
    /* Maybe add Average, threshold coloring (turn line red after a certain
     * point) */
}

impl Default for Decoration {
    fn default() -> Decoration {
        Decoration::None
    }
}

impl Decoration {
    /// Calls the internal methods to get the top_value
    pub fn init(&mut self, display_size: SizeInfo) {
        match self {
            Decoration::Reference(ref mut d) => d.init(display_size),
            Decoration::Alert(ref mut d) => d.init(display_size),
            Decoration::None => (),
        };
    }
    /// Calls the internal methods to update the opengl values
    pub fn update_opengl_vecs(
        &mut self,
        display_size: SizeInfo,
        offset: Value2D,
        stats: TimeSeriesStats,
    ) {
        match self {
            Decoration::Reference(ref mut d) => d.update_opengl_vecs(display_size, offset, stats),
            Decoration::Alert(ref mut d) => d.update_opengl_vecs(display_size, offset, stats),
            Decoration::None => (),
        };
    }

    /// Calls the internal methods to get the width
    pub fn width(&self) -> f32 {
        match self {
            Decoration::Reference(d) => d.width(),
            Decoration::Alert(d) => d.width(),
            Decoration::None => Decoration::default_width(),
        }
    }

    /// Calls the internal methods to get the opengl_vertices
    pub fn opengl_vertices(&self) -> Vec<f32> {
        match self {
            Decoration::Reference(d) => d.opengl_vertices(),
            Decoration::Alert(d) => d.opengl_vertices(),
            Decoration::None => Decoration::default_opengl_vertices(),
        }
    }

    /// Calls the internal methods to get the color
    pub fn color(&self) -> Rgb {
        match self {
            Decoration::Reference(d) => d.color(),
            Decoration::Alert(d) => d.color(),
            Decoration::None => Decoration::default_color(),
        }
    }

    /// Calls the internal methods to get the alpha
    pub fn alpha(&self) -> f32 {
        match self {
            Decoration::Reference(d) => d.alpha(),
            Decoration::Alert(d) => d.alpha(),
            Decoration::None => Decoration::default_alpha(),
        }
    }

    /// Calls the internal methods to get the bottom_value
    pub fn bottom_value(&self) -> f64 {
        match self {
            Decoration::Reference(d) => d.bottom_value(),
            Decoration::Alert(d) => d.bottom_value(),
            Decoration::None => Decoration::default_bottom_value(),
        }
    }

    /// Calls the internal methods to get the top_value
    pub fn top_value(&self) -> f64 {
        match self {
            Decoration::Reference(d) => d.top_value(),
            Decoration::Alert(d) => d.top_value(),
            Decoration::None => Decoration::default_top_value(),
        }
    }

    /// Default width
    fn default_width() -> f32 {
        0f32
    }

    /// Default opengl_vertices
    fn default_opengl_vertices() -> Vec<f32> {
        vec![]
    }

    /// Default color
    fn default_color() -> Rgb {
        Rgb::default()
    }

    /// Default alpha
    fn default_alpha() -> f32 {
        0.0f32
    }

    /// Default top value
    fn default_top_value() -> f64 {
        0f64
    }

    /// Default bottom value
    fn default_bottom_value() -> f64 {
        0f64
    }
}

/// `Decorate` defines functions that a struct must implement to be drawable
pub trait Decorate {
    fn init(&mut self, _display_size: SizeInfo) {}
    /// Every decoration will implement a different update_opengl_vecs
    /// This method is called every time it needs to be redrawn.
    fn update_opengl_vecs(
        &mut self,
        _display_size: SizeInfo,
        _offset: Value2D,
        stats: TimeSeriesStats,
    ) {
        let span = span!(Level::TRACE, "update_opengl_vecs: default Trait function");
    }

    /// `width` of the Decoration as it may need space to be drawn, otherwise
    /// the decoration and the data itself would overlap, these are pixels
    fn width(&self) -> f32 {
        debug!("Using default Decorate trait method.");
        Decoration::default_width()
    }

    /// `opengl_vertices` returns the representation of the decoration in
    /// opengl. These are for now GL_LINES and 2D only
    fn opengl_vertices(&self) -> Vec<f32> {
        Decoration::default_opengl_vertices()
    }

    /// `color` returns the Rgb for the decoration
    fn color(&self) -> Rgb {
        Decoration::default_color()
    }

    /// `alpha` returns the transparency for the decoration
    fn alpha(&self) -> f32 {
        Decoration::default_alpha()
    }

    /// `bottom_value` returns a value in the range of the collected metrics, this helps
    /// visuallize a point of reference on the actual metrics (the metrics being below or above it)
    fn bottom_value(&self) -> f64 {
        Decoration::default_bottom_value()
    }

    /// `top_value` is the Y value of the decoration, it needs to be
    /// in the range of the metrics that have been collected, thus f64
    /// this is the highest point the Decoration will use
    fn top_value(&self) -> f64 {
        Decoration::default_top_value()
    }
}

/// `ReferencePointDecoration` draws a fixed point to give a reference point
/// of what a drawn value may mean
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ReferencePointDecoration {
    /// The value at which to draw the reference point
    pub value: f64,

    /// The reference point will use additional height for the axis line
    /// this makes it fit in the configured space, basically the value
    /// will be incremented by this additional percentage to give more
    /// space to draw the axis tick
    #[serde(default)]
    pub height_multiplier: f64,

    /// RGB color
    #[serde(default)]
    pub color: Rgb,

    /// Transparency
    #[serde(default)]
    pub alpha: f32,

    /// The pixels to separate from the left and right
    #[serde(default)]
    pub padding: Value2D,

    /// The opengl vertices is stored in this vector
    #[serde(default)]
    pub opengl_data: Vec<f32>,

    /// The capacity is always 12, see opengl_vertices()
    #[serde(default)]
    pub opengl_vec_capacity: usize,
}

impl Default for ReferencePointDecoration {
    fn default() -> ReferencePointDecoration {
        ReferencePointDecoration {
            value: 1.0,
            height_multiplier: 0.05,
            color: Rgb::default(),
            alpha: 0.5,
            padding: Value2D {
                x: 1f32,
                y: 0f32, // No top/bottom padding
            },
            opengl_data: vec![],
            opengl_vec_capacity: 12,
        }
    }
}

impl Decorate for ReferencePointDecoration {
    fn width(&self) -> f32 {
        debug!("Using custom width from ReferencePointDecoration");
        self.padding.x * 2. // Reserve space left and right
    }

    fn opengl_vertices(&self) -> Vec<f32> {
        self.opengl_data.clone()
    }

    /// `update_opengl_vecs` Draws a marker at a fixed position for
    /// reference.
    fn update_opengl_vecs(
        &mut self,
        display_size: SizeInfo,
        offset: Value2D,
        stats: TimeSeriesStats,
    ) {
        debug!("ReferencePointDecoration:update_opengl_vecs: Starting");
        if self.opengl_vec_capacity != self.opengl_data.capacity() {
            self.opengl_data = vec![0.; self.opengl_vec_capacity];
        }
        // The vertexes of the above marker idea can be represented as
        // connecting lines for these coordinates:
        //         |Actual Draw Metric Data|
        // x1,y2   |                       |   x2,y2
        // x1,y1 --|-----------------------|-- x2,y1
        // x1,y3   |                       |   x2,y3
        // |- 10% -|-         80%         -|- 10% -|
        // TODO: Call only when max or min have changed in collected metrics
        //
        // Calculate X coordinates:
        let x1 = display_size.scale_x(offset.x);
        let x2 = display_size.scale_x(offset.x + display_size.chart_width);

        // Calculate Y, the marker hints are 10% of the current values
        // This means that the
        let y1 = display_size.scale_y(stats.max, self.value);
        let y2 = display_size.scale_y(stats.max, self.top_value());
        let y3 = display_size.scale_y(stats.max, self.bottom_value());

        // Build the left most axis "tick" mark.
        self.opengl_data[0] = x1;
        self.opengl_data[1] = y2;
        self.opengl_data[2] = x1;
        self.opengl_data[3] = y3;

        // Create the line to the other side
        self.opengl_data[4] = x1;
        self.opengl_data[5] = y1;
        self.opengl_data[6] = x2;
        self.opengl_data[7] = y1;
        // Finish the axis "tick" on the other side
        self.opengl_data[8] = x2;
        self.opengl_data[9] = y3;
        self.opengl_data[10] = x2;
        self.opengl_data[11] = y2;
        debug!(
            "ReferencePointDecoration:update_opengl_vecs: Finished: {:?}",
            self.opengl_data
        );
    }

    /// `bottom_value` decrements the reference point value by a percentage
    /// to account for space to draw the axis tick
    fn bottom_value(&self) -> f64 {
        self.value - self.value * self.height_multiplier
    }
    /// `top_value` is the Y value of the decoration, it needs to be
    /// in the range of the metrics that have been collected, thus f64
    /// this is the highest point the Decoration will use
    fn top_value(&self) -> f64 {
        self.value + self.value * self.height_multiplier
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum AlertComparator {
    #[serde(rename = ">")]
    GreaterThan,
    #[serde(rename = ">=")]
    GreaterThanOrEqual,
    #[serde(rename = "<")]
    LessThan,
    #[serde(rename = "<=")]
    LessThanOrEqual,
    #[serde(rename = "=")]
    Equal,
}

impl Default for AlertComparator {
    fn default() -> Self {
        AlertComparator::GreaterThan
    }
}

/// `ActiveAlertUnderLineDecoration` draws an underlined series of
/// red triangles below a portion of the screen to denote alert below a
/// chart
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ActiveAlertUnderLineDecoration {
    /// The threshold of the alert, wether is active or not.
    pub threshold: f64,

    #[serde(default)]
    pub target: String,

    /// A mathematical operator to compare
    #[serde(default)]
    pub comparator: AlertComparator,

    /// A target TimeSeries name that we will compare with
    /// Must be in the current chart item
    #[serde(default)]
    pub color: Rgb,

    /// Transparency
    #[serde(default)]
    pub alpha: f32,

    /// The pixels to separate from the left and right
    #[serde(default)]
    pub padding: Value2D,

    /// The opengl vertices is stored in this vector
    /// The capacity is static, one triangle on the left and one on the right
    #[serde(default)]
    pub opengl_data: Vec<f32>,

    /// The capacity is always 12, see opengl_vertices()
    #[serde(default)]
    pub opengl_vec_capacity: usize,
}

impl Default for ActiveAlertUnderLineDecoration {
    fn default() -> ActiveAlertUnderLineDecoration {
        ActiveAlertUnderLineDecoration {
            threshold: 1f64, // the value to compare with
            comparator: AlertComparator::default(),
            target: String::from(""),
            color: Rgb::default(),
            alpha: 0.5,
            padding: Value2D {
                x: 1f32,
                y: 1f32, // XXX: figure out how to reserve space vertically
            },
            opengl_data: vec![],
            opengl_vec_capacity: 12usize, // Minimum to draw left bar to right bar and whiskers
        }
    }
}

impl Decorate for ActiveAlertUnderLineDecoration {
    fn opengl_vertices(&self) -> Vec<f32> {
        self.opengl_data.clone()
    }

    /// `update_opengl_vecs` Draws a series of triangles at the bottom of
    /// a metric to show an alarm
    fn update_opengl_vecs(
        &mut self,
        display_size: SizeInfo,
        offset: Value2D,
        stats: TimeSeriesStats,
    ) {
        debug!("ActiveAlertUnderLineDecoration:update_opengl_vecs: Starting");
        // TODO: This needs to be calculated only at the start, perhaps an init() method.
        // TODO: Depending on the number of alarms, the transparency should become 0.
        if self.opengl_vec_capacity != self.opengl_data.capacity() {
            self.opengl_data = vec![0.; self.opengl_vec_capacity];
        }
        // The vertexes of the above marker idea can be represented as
        // connecting lines for these coordinates:
        //         |Actual Draw Metric Data|
        //         |                       |
        //         |                       |
        // x1,y1   ||\                   /||   x4,y1
        // x1,y2   |--+-----------------+--|   x4,y2
        // |- 5 % -|-         90%         -|- 5 % -|
        //          x2,y2             x3,y2
        //
        // Calculate X coordinates:
        let x1 = display_size.scale_x(offset.x);
        let x2 = display_size.scale_x(offset.x + 0.1 * display_size.chart_width);
        let x3 = display_size
            .scale_x(offset.x + display_size.chart_width - 0.1 * display_size.chart_width);
        let x4 = display_size.scale_x(offset.x + display_size.chart_width);

        // Calculate Y, the marker hints are by default 10% of the chart height
        // Same as the chart_width to have the same amount of pixels.
        let y1 = -1.0 - (0.1 * display_size.chart_width);
        let y2 = -1.0;

        // TODO: Fix this part in a for loop overwriting the allocated vector
        // Build the left most triangle
        self.opengl_data[0] = x2;
        self.opengl_data[1] = y2;
        self.opengl_data[2] = x1;
        self.opengl_data[3] = y1;
        self.opengl_data[4] = x1;
        self.opengl_data[5] = y2;

        // Create the line to the other side
        self.opengl_data[6] = x4;
        self.opengl_data[7] = y2;

        // Build the right most triangle
        self.opengl_data[8] = x4;
        self.opengl_data[9] = y2;
        self.opengl_data[10] = x3;
        self.opengl_data[11] = y2;

        debug!(
            "ActiveAlertUnderLineDecoration:update_opengl_vecs: Finished: {:?}",
            self.opengl_data
        );
    }
}
