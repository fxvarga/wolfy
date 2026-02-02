//! Layout strategies - Different layout algorithms

use crate::domain::value_objects::Rect;
use crate::ui::widgets::{Constraints, MeasuredSize, Widget};

/// Layout direction
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Vertical,
    Horizontal,
}

/// Layout alignment
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

/// Layout strategy trait
pub trait LayoutStrategy {
    /// Measure children and calculate total size
    fn measure(&self, children: &[Box<dyn Widget>], constraints: Constraints) -> MeasuredSize;

    /// Arrange children within bounds
    fn arrange(&self, children: &mut [Box<dyn Widget>], bounds: Rect);
}

/// Vertical stack layout
#[derive(Clone, Debug, Default)]
pub struct VerticalLayout {
    pub spacing: f32,
    pub alignment: Alignment,
}

impl VerticalLayout {
    pub fn new(spacing: f32) -> Self {
        Self {
            spacing,
            alignment: Alignment::Start,
        }
    }

    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}

impl LayoutStrategy for VerticalLayout {
    fn measure(&self, children: &[Box<dyn Widget>], constraints: Constraints) -> MeasuredSize {
        let mut total_height: f32 = 0.0;
        let mut max_width: f32 = 0.0;

        for (i, child) in children.iter().enumerate() {
            let child_size = child.measure(Constraints {
                min_width: 0.0,
                min_height: 0.0,
                max_width: constraints.max_width,
                max_height: f32::INFINITY,
            });

            total_height += child_size.height;
            if i > 0 {
                total_height += self.spacing;
            }
            max_width = max_width.max(child_size.width);
        }

        MeasuredSize::new(
            max_width.clamp(constraints.min_width, constraints.max_width),
            total_height.clamp(constraints.min_height, constraints.max_height),
        )
    }

    fn arrange(&self, children: &mut [Box<dyn Widget>], bounds: Rect) {
        let mut y = bounds.top;

        for child in children {
            let child_size = child.measure(Constraints {
                min_width: 0.0,
                min_height: 0.0,
                max_width: bounds.width(),
                max_height: f32::INFINITY,
            });

            let x = match self.alignment {
                Alignment::Start => bounds.left,
                Alignment::Center => bounds.left + (bounds.width() - child_size.width) / 2.0,
                Alignment::End => bounds.right - child_size.width,
                Alignment::Stretch => bounds.left,
            };

            let width = match self.alignment {
                Alignment::Stretch => bounds.width(),
                _ => child_size.width,
            };

            child.arrange(Rect::from_pos_size(x, y, width, child_size.height));
            y += child_size.height + self.spacing;
        }
    }
}

/// Horizontal stack layout
#[derive(Clone, Debug, Default)]
pub struct HorizontalLayout {
    pub spacing: f32,
    pub alignment: Alignment,
}

impl HorizontalLayout {
    pub fn new(spacing: f32) -> Self {
        Self {
            spacing,
            alignment: Alignment::Start,
        }
    }

    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}

impl LayoutStrategy for HorizontalLayout {
    fn measure(&self, children: &[Box<dyn Widget>], constraints: Constraints) -> MeasuredSize {
        let mut total_width: f32 = 0.0;
        let mut max_height: f32 = 0.0;

        for (i, child) in children.iter().enumerate() {
            let child_size = child.measure(Constraints {
                min_width: 0.0,
                min_height: 0.0,
                max_width: f32::INFINITY,
                max_height: constraints.max_height,
            });

            total_width += child_size.width;
            if i > 0 {
                total_width += self.spacing;
            }
            max_height = max_height.max(child_size.height);
        }

        MeasuredSize::new(
            total_width.clamp(constraints.min_width, constraints.max_width),
            max_height.clamp(constraints.min_height, constraints.max_height),
        )
    }

    fn arrange(&self, children: &mut [Box<dyn Widget>], bounds: Rect) {
        let mut x = bounds.left;

        for child in children {
            let child_size = child.measure(Constraints {
                min_width: 0.0,
                min_height: 0.0,
                max_width: f32::INFINITY,
                max_height: bounds.height(),
            });

            let y = match self.alignment {
                Alignment::Start => bounds.top,
                Alignment::Center => bounds.top + (bounds.height() - child_size.height) / 2.0,
                Alignment::End => bounds.bottom - child_size.height,
                Alignment::Stretch => bounds.top,
            };

            let height = match self.alignment {
                Alignment::Stretch => bounds.height(),
                _ => child_size.height,
            };

            child.arrange(Rect::from_pos_size(x, y, child_size.width, height));
            x += child_size.width + self.spacing;
        }
    }
}

/// Grid layout
#[derive(Clone, Debug)]
pub struct GridLayout {
    pub columns: usize,
    pub row_spacing: f32,
    pub column_spacing: f32,
}

impl GridLayout {
    pub fn new(columns: usize) -> Self {
        Self {
            columns: columns.max(1),
            row_spacing: 0.0,
            column_spacing: 0.0,
        }
    }

    pub fn with_spacing(mut self, row_spacing: f32, column_spacing: f32) -> Self {
        self.row_spacing = row_spacing;
        self.column_spacing = column_spacing;
        self
    }
}

impl LayoutStrategy for GridLayout {
    fn measure(&self, children: &[Box<dyn Widget>], constraints: Constraints) -> MeasuredSize {
        let rows = (children.len() + self.columns - 1) / self.columns;
        let cell_width =
            (constraints.max_width - self.column_spacing * (self.columns - 1) as f32) / self.columns as f32;

        let mut total_height: f32 = 0.0;

        for row in 0..rows {
            let mut row_height: f32 = 0.0;
            for col in 0..self.columns {
                let idx = row * self.columns + col;
                if idx < children.len() {
                    let child_size = children[idx].measure(Constraints {
                        min_width: 0.0,
                        min_height: 0.0,
                        max_width: cell_width,
                        max_height: f32::INFINITY,
                    });
                    row_height = row_height.max(child_size.height);
                }
            }
            total_height += row_height;
            if row > 0 {
                total_height += self.row_spacing;
            }
        }

        MeasuredSize::new(constraints.max_width, total_height)
    }

    fn arrange(&self, children: &mut [Box<dyn Widget>], bounds: Rect) {
        let cell_width =
            (bounds.width() - self.column_spacing * (self.columns - 1) as f32) / self.columns as f32;

        let rows = (children.len() + self.columns - 1) / self.columns;
        let mut y = bounds.top;

        for row in 0..rows {
            let mut row_height: f32 = 0.0;

            // First pass: measure row height
            for col in 0..self.columns {
                let idx = row * self.columns + col;
                if idx < children.len() {
                    let child_size = children[idx].measure(Constraints {
                        min_width: 0.0,
                        min_height: 0.0,
                        max_width: cell_width,
                        max_height: f32::INFINITY,
                    });
                    row_height = row_height.max(child_size.height);
                }
            }

            // Second pass: arrange
            for col in 0..self.columns {
                let idx = row * self.columns + col;
                if idx < children.len() {
                    let x = bounds.left + col as f32 * (cell_width + self.column_spacing);
                    children[idx].arrange(Rect::from_pos_size(x, y, cell_width, row_height));
                }
            }

            y += row_height + self.row_spacing;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock widget for testing
    struct MockWidget {
        size: MeasuredSize,
        bounds: Rect,
    }

    impl MockWidget {
        fn new(width: f32, height: f32) -> Box<dyn Widget> {
            Box::new(Self {
                size: MeasuredSize::new(width, height),
                bounds: Rect::zero(),
            })
        }
    }

    impl Widget for MockWidget {
        fn measure(&self, _constraints: Constraints) -> MeasuredSize {
            self.size
        }

        fn arrange(&mut self, bounds: Rect) {
            self.bounds = bounds;
        }

        fn render(&self, _renderer: &mut dyn crate::application::ports::RenderPort) {}

        fn bounds(&self) -> Rect {
            self.bounds
        }

        fn state(&self) -> crate::ui::widgets::WidgetState {
            crate::ui::widgets::WidgetState::new()
        }

        fn set_state(&mut self, _state: crate::ui::widgets::WidgetState) {}
    }

    #[test]
    fn test_vertical_layout_measure() {
        let layout = VerticalLayout::new(10.0);
        let children: Vec<Box<dyn Widget>> = vec![
            MockWidget::new(100.0, 50.0),
            MockWidget::new(80.0, 50.0),
            MockWidget::new(120.0, 50.0),
        ];

        let size = layout.measure(&children, Constraints::unbounded());

        assert_eq!(size.width, 120.0); // Max child width
        assert_eq!(size.height, 170.0); // 50 + 10 + 50 + 10 + 50
    }

    #[test]
    fn test_grid_layout_measure() {
        let layout = GridLayout::new(2).with_spacing(10.0, 10.0);
        let children: Vec<Box<dyn Widget>> = vec![
            MockWidget::new(100.0, 50.0),
            MockWidget::new(100.0, 50.0),
            MockWidget::new(100.0, 50.0),
        ];

        let size = layout.measure(
            &children,
            Constraints::new(0.0, 0.0, 210.0, f32::INFINITY),
        );

        assert_eq!(size.height, 110.0); // 50 + 10 + 50 (2 rows)
    }
}
