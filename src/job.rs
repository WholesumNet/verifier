use std::error::Error;

use libp2p::PeerId;
use home;

#[derive(Debug)]
pub struct Residue {
    pub receipt_cid: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    DockerWarmingUp = 0,     // docker initializing
    Negotiating,             // making offers
    Running,       
    VerificationFailed,      
    VerificationSucceeded,
}

#[derive(Debug)]
pub struct JobId {
    pub local_id: String,       // allows compute(prove) and verification of a job on the same machine    
    pub network_id: String,     // the actual job known to the clients
}

// maintain lifecycle of a job
#[derive(Debug)]
pub struct Job {
    pub id: JobId,
    pub owner: PeerId,                      // the client
    pub status: Status,
    pub residue: Residue,                   // cids for stderr, output, receipt, ...
}

// get base residue path of the host
pub fn get_residue_path() -> Result<String, Box<dyn Error>> {
    let home_dir = home::home_dir()
        .ok_or_else(|| Box::<dyn Error>::from("Home dir is not available."))?
        .into_os_string().into_string()
        .or_else(|_| Err(Box::<dyn Error>::from("OS_String conversion failed.")))?;
    Ok(format!("{home_dir}/.wholesum/jobs"))
}
