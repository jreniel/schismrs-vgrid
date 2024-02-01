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
            StretchingFunction::Quadratic(opts) => {
                let mut builder = QuadraticTransformBuilder::default();
                builder.hgrid(hgrid);
                builder.depths(depths);
                builder.nlevels(nlevels);
                builder.etal(opts.etal);
                builder.skew_decay_rate(opts.skew_decay_rate);
                builder.a_vqs0(opts.a_vqs0);
                Ok(Rc::new(builder.build()?))
            }
            StretchingFunction::S(opts) => {
                let mut builder = STransformBuilder::default();
                builder.hgrid(hgrid);
                builder.depths(depths);
                builder.nlevels(nlevels);
                builder.etal(opts.etal);
                builder.a_vqs0(opts.a_vqs0);
                builder.theta_f(opts.theta_f);
                builder.theta_b(opts.theta_b);
                Ok(Rc::new(builder.build()?))
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum StretchingFunctionError {
    #[error(transparent)]
    STransformBuilderError(#[from] STransformBuilderError),
    #[error(transparent)]
    QuadraticTransformBuilderError(#[from] QuadraticTransformBuilderError),
    // UninitializedFieldError(String),
    // #[error(transparent)]
    // VQSBuilderError(#[from] VQSBuilderError),
    // #[error(transparent)]
    // KMeansHSMCreateError(#[from] KMeansHSMCreateError),
    // #[error("shallow_levels must be >= 2")]
    // InvalidShallowLevels,
}
// impl StretchingFunction {
//     pub fn nvrt(&self) {
//         // match
//     }
// }

// a_vqs(m)=max(-1.d0,a_vqs0-(m-1)*0.03)
// tmp=a_vqs(m)*sigma*sigma+(1+a_vqs(m))*sigma !transformed sigma
// z_mas(k,m)=tmp*(etal+hsm(m))+etal
