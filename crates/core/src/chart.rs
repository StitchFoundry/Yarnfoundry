//! The colorwork chart: a grid of palette indices plus palette, technique and gauge.

use serde::{Deserialize, Serialize};

use crate::gauge::{Gauge, MAX_SIDE};

/// Most colours a palette may hold.
pub const MAX_PALETTE: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Craft {
    Knit,
    Crochet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Technique {
    Tapestry,
    Intarsia,
    FairIsle,
    Filet,
}

/// Worked back and forth (flat) or in the round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Construction {
    Flat,
    Round,
}

/// One yarn colour: a generic name (never a brand) and its sRGB value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteColor {
    pub name: String,
    pub rgb: [u8; 3],
}

/// Why a chart was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChartError {
    #[error("chart size {0} x {1} is not between 1 and {MAX_SIDE}")]
    BadSize(u32, u32),
    #[error("the grid has {found} cells but {width} x {height} needs {expected}")]
    CellCount { width: u32, height: u32, expected: usize, found: usize },
    #[error("the palette must have between 1 and {MAX_PALETTE} colours (has {0})")]
    BadPalette(usize),
    #[error("cell {index} uses colour {colour} but the palette has {len}")]
    ColourOutOfRange { index: usize, colour: u8, len: usize },
    #[error("gauge: {0}")]
    Gauge(#[from] crate::gauge::GaugeError),
}

/// A colorwork chart. `cells` is row-major, row 0 at the bottom of the piece (the first row worked).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chart {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<u8>,
    pub palette: Vec<PaletteColor>,
    pub craft: Craft,
    pub technique: Technique,
    pub construction: Construction,
    pub gauge: Gauge,
}

impl Chart {
    /// A chart filled with palette colour 0.
    pub fn blank(
        width: u32,
        height: u32,
        palette: Vec<PaletteColor>,
        craft: Craft,
        technique: Technique,
        construction: Construction,
        gauge: Gauge,
    ) -> Result<Self, ChartError> {
        check_size(width, height)?;
        let cells = vec![0; (width as usize) * (height as usize)];
        Self::new(width, height, cells, palette, craft, technique, construction, gauge)
    }

    /// A validated chart.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: u32,
        height: u32,
        cells: Vec<u8>,
        palette: Vec<PaletteColor>,
        craft: Craft,
        technique: Technique,
        construction: Construction,
        gauge: Gauge,
    ) -> Result<Self, ChartError> {
        let chart = Self { width, height, cells, palette, craft, technique, construction, gauge };
        chart.validate()?;
        Ok(chart)
    }

    /// Checks the invariants: sizes in range, cell count right, every cell a palette index, gauge sane.
    pub fn validate(&self) -> Result<(), ChartError> {
        check_size(self.width, self.height)?;
        let expected = (self.width as usize) * (self.height as usize);
        if self.cells.len() != expected {
            return Err(ChartError::CellCount { width: self.width, height: self.height, expected, found: self.cells.len() });
        }
        if self.palette.is_empty() || self.palette.len() > MAX_PALETTE {
            return Err(ChartError::BadPalette(self.palette.len()));
        }
        if let Some((index, &colour)) = self.cells.iter().enumerate().find(|&(_, &c)| usize::from(c) >= self.palette.len()) {
            return Err(ChartError::ColourOutOfRange { index, colour, len: self.palette.len() });
        }
        self.gauge.validate()?;
        Ok(())
    }

    /// The colour index at column `x`, row `y` (row 0 = first row worked), or `None` outside the chart.
    pub fn get(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells.get((y as usize) * (self.width as usize) + (x as usize)).copied()
    }

    /// Sets a cell; `false` if outside the chart or the colour is not in the palette.
    pub fn set(&mut self, x: u32, y: u32, colour: u8) -> bool {
        if x >= self.width || y >= self.height || usize::from(colour) >= self.palette.len() {
            return false;
        }
        match self.cells.get_mut((y as usize) * (self.width as usize) + (x as usize)) {
            Some(cell) => {
                *cell = colour;
                true
            }
            None => false,
        }
    }

    /// Finished size in the gauge's unit.
    pub fn finished_size(&self) -> (f64, f64) {
        self.gauge.size_of(self.width, self.height, self.gauge.unit)
    }
}

fn check_size(width: u32, height: u32) -> Result<(), ChartError> {
    if (1..=MAX_SIDE).contains(&width) && (1..=MAX_SIDE).contains(&height) { Ok(()) } else { Err(ChartError::BadSize(width, height)) }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::gauge::Unit;

    pub fn two_colours() -> Vec<PaletteColor> {
        vec![PaletteColor { name: "cream".into(), rgb: [250, 244, 230] }, PaletteColor { name: "red".into(), rgb: [200, 30, 40] }]
    }

    pub fn gauge() -> Gauge {
        Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap()
    }

    pub fn sample() -> Chart {
        let mut c = Chart::blank(4, 3, two_colours(), Craft::Knit, Technique::Intarsia, Construction::Flat, gauge()).unwrap();
        c.set(1, 1, 1);
        c.set(3, 2, 1);
        c
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn a_blank_chart_is_all_colour_zero() {
        let c = sample();
        assert_eq!((c.width, c.height, c.cells.len()), (4, 3, 12));
        assert_eq!(c.get(0, 0), Some(0));
        assert_eq!(c.get(1, 1), Some(1));
        assert_eq!(c.get(4, 0), None);
        assert_eq!(c.get(0, 3), None);
    }

    #[test]
    fn set_refuses_outside_cells_and_unknown_colours() {
        let mut c = sample();
        assert!(!c.set(9, 0, 1));
        assert!(!c.set(0, 0, 2));
        assert!(c.set(0, 0, 1));
    }

    #[test]
    fn invalid_charts_are_errors() {
        let g = gauge();
        let mk = |w, h, cells: Vec<u8>, pal| Chart::new(w, h, cells, pal, Craft::Knit, Technique::Tapestry, Construction::Flat, g);
        assert!(matches!(mk(0, 3, vec![], two_colours()), Err(ChartError::BadSize(0, 3))));
        assert!(matches!(mk(2, 2, vec![0; 3], two_colours()), Err(ChartError::CellCount { .. })));
        assert!(matches!(mk(2, 1, vec![0, 0], vec![]), Err(ChartError::BadPalette(0))));
        assert!(matches!(mk(2, 1, vec![0, 5], two_colours()), Err(ChartError::ColourOutOfRange { index: 1, colour: 5, len: 2 })));
        assert!(matches!(mk(MAX_SIDE + 1, 1, vec![], two_colours()), Err(ChartError::BadSize(..))));
    }

    #[test]
    fn finished_size_follows_the_gauge() {
        let c = Chart::blank(40, 56, two_colours(), Craft::Knit, Technique::FairIsle, Construction::Round, gauge()).unwrap();
        let (w, h) = c.finished_size();
        assert!((w - 8.0).abs() < 1e-9 && (h - 8.0).abs() < 1e-9);
    }
}
