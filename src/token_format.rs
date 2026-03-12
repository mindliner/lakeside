use clap::ValueEnum;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum TokenFormat {
    CashuA,
    CashuB,
}
