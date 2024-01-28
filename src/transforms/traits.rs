use ndarray::Axis;
use ndarray::{Array1, Array2};
use ndarray_stats::errors::MinMaxError;
use ndarray_stats::QuantileExt;
use plotly::color::NamedColor;
use plotly::common::{Line, Marker, Mode};
use plotly::{Plot, Scatter};
use thiserror::Error;

pub trait Transform {
    fn zmas(&self) -> &Array2<f64>;
    fn etal(&self) -> &f64;
    fn a_vqs0(&self) -> &f64;

    fn make_zmas_plot(&self) -> Result<Plot, TransformPlotterError> {
        let z_mas = self.zmas();
        let mut plot = Plot::new();
        for master_grid in z_mas.axis_iter(Axis(1)) {
            let master_grid = master_grid
                .iter()
                .filter(|&&x| !x.is_nan())
                .cloned()
                .collect::<Array1<f64>>();
            let min_value = *master_grid.min()?;
            let trace = Scatter::new(vec![min_value; master_grid.len()], master_grid.to_vec())
                .mode(Mode::LinesMarkers)
                .line(Line::new().color(NamedColor::Blue))
                .marker(Marker::new().color(NamedColor::Black));
            plot.add_trace(trace);
        }
        Ok(plot)
    }
}

#[derive(Error, Debug)]
pub enum TransformPlotterError {
    #[error("Unreachable: Could not find a minimum value for master grid")]
    MinMaxError(#[from] MinMaxError),
}
