pub mod syzoj;
use anyhow::Result;

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OnlineJudge {
    Syzoj,
}

pub trait Exporter {
    fn export(
        problem: &crate::tool::Problem,
        work_dir: &std::path::Path,
        data_dir: &std::path::Path,
        output_dir: &std::path::Path,
    ) -> Result<()>;
}
