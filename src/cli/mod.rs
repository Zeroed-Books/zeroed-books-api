use structopt::StructOpt;

mod migrate;

#[derive(StructOpt)]
pub enum Opts {
    Migrate {},
    Serve {},
}

pub async fn run_with_sys_args() -> Result<(), ()> {
    let matches = Opts::from_args();

    match matches {
        Opts::Migrate {} => migrate::run_migrations().map_err(|_| ()),
        Opts::Serve {} => crate::server::rocket()
            .ignite()
            .await
            .expect("Rocket ignition failure")
            .launch()
            .await
            .map(|_| ())
            .map_err(|_| ()),
    }
}
