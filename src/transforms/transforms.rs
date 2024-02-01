use super::quadratic::QuadraticTransformBuilder;
use super::quadratic::QuadraticTransformBuilderError;
use super::quadratic::QuadraticTransformOpts;
use super::s::STransformBuilder;
use super::s::STransformBuilderError;
use super::s::STransformOpts;
use super::traits::Transform;
use schismrs_hgrid::Hgrid;
use std::rc::Rc;
use thiserror::Error;

#[derive(Clone, Debug)]
pub enum StretchingFunction<'a> {
    Quadratic(QuadraticTransformOpts<'a>),
    S(STransformOpts<'a>),
}

impl<'a> StretchingFunction<'a> {
    pub fn etal(&self) -> &f64 {
        match self {
            StretchingFunction::Quadratic(opts) => opts.etal,
            StretchingFunction::S(opts) => opts.etal,
        }
    }
    pub fn transform(
        &self,
        hgrid: &Hgrid,
        depths: &Vec<f64>,
        nlevels: &Vec<usize>,
    ) -> Result<Rc<dyn Transform>, StretchingFunctionError> {
        match self {
            StretchingFunction::Quadratic(opts) => Ok(Rc::new(
                QuadraticTransformBuilder::default()
                    .hgrid(hgrid)
                    .depths(depths)
                    .nlevels(nlevels)
                    .etal(opts.etal)
                    .skew_decay_rate(opts.skew_decay_rate)
                    .a_vqs0(opts.a_vqs0)
                    .build()?,
            )),
            StretchingFunction::S(opts) => Ok(Rc::new(
                STransformBuilder::default()
                    .hgrid(hgrid)
                    .depths(depths)
                    .nlevels(nlevels)
                    .etal(opts.etal)
                    .a_vqs0(opts.a_vqs0)
                    .theta_f(opts.theta_f)
                    .theta_b(opts.theta_b)
                    .build()?,
            )),
        }
    }
}

#[derive(Error, Debug)]
pub enum StretchingFunctionError {
    #[error(transparent)]
    STransformBuilderError(#[from] STransformBuilderError),
    #[error(transparent)]
    QuadraticTransformBuilderError(#[from] QuadraticTransformBuilderError),
}
