use crate::transforms::quadratic::{
    QuadraticTransformKMeansBuilder, QuadraticTransformKMeansBuilderError,
};
use crate::transforms::traits::BuildVQS;
use crate::transforms::StretchingFunction;
use schismrs_mesh::hgrid::Hgrid;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use thiserror::Error;

pub struct VQS {
    transform: Box<dyn BuildVQS>,
}

impl VQS {
    pub fn write_to_file(&self, filename: &PathBuf) -> std::io::Result<()> {
        let mut file = File::create(filename)?;
        write!(file, "{}", self)?;
        Ok(())
    }

    pub fn ivcor(&self) -> u8 {
        1
    }

    pub fn nvrt(&self) -> u8 {
        self.transform.as_ref().nvrt()
    }

    fn bottom_level_indices(&self) -> Vec<u8> {
        self.transform.as_ref().bottom_level_indices()
    }
}

impl fmt::Display for VQS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ! ivcor\n", self.ivcor())?;
        write!(f, "{} ! nvrt\n", self.nvrt())?;
        write!(
            f,
            "{} ! bottom level indices \n",
            self.bottom_level_indices()
                .iter()
                .map(|&index| index.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        )?;
        for (level, values) in self.transform.as_ref().iter_level_values() {
            write!(f, "{} {:?}\n", level, values)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct VQSKMeansBuilder<'a> {
    hgrid: Option<&'a Hgrid>,
    nclusters: Option<u8>,
    stretching: Option<StretchingFunction>,
}

impl<'a> VQSKMeansBuilder<'a> {
    pub fn build(&self) -> Result<VQS, VQSKMeansBuilderError> {
        let hgrid = self
            .hgrid
            .ok_or_else(|| VQSKMeansBuilderError::UninitializedFieldError("hgrid".to_string()))?;
        let nclusters = self.nclusters.ok_or_else(|| {
            VQSKMeansBuilderError::UninitializedFieldError("nclusters".to_string())
        })?;
        let stretching = self.stretching.clone().ok_or_else(|| {
            VQSKMeansBuilderError::UninitializedFieldError("stretching".to_string())
        })?;
        let transform = match stretching {
            StretchingFunction::Quadratic(opts) => {
                let mut builder = QuadraticTransformKMeansBuilder::default();
                if opts.etal.is_some() {
                    builder.etal(opts.etal.unwrap());
                }
                builder.build()?
            }
        };
        Ok(VQS {
            transform: Box::new(transform),
        })
    }

    pub fn hgrid(&mut self, hgrid: &'a Hgrid) -> &mut Self {
        self.hgrid = Some(hgrid);
        self
    }
    pub fn nclusters(&mut self, nclusters: u8) -> &mut Self {
        self.nclusters = Some(nclusters);
        self
    }
    pub fn stretching(&mut self, stretching: StretchingFunction) -> &mut Self {
        self.stretching = Some(stretching);
        self
    }
}

#[derive(Error, Debug)]
pub enum VQSKMeansBuilderError {
    #[error("Unitialized field on VQSKMeansBuilder: {0}")]
    UninitializedFieldError(String),
    #[error(transparent)]
    QuadraticTransformKMeansBuilderError(#[from] QuadraticTransformKMeansBuilderError),
}
