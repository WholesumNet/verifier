#![doc = include_str!("../README.md")]

use futures::{
    select,
    stream::{
        FuturesUnordered,
        StreamExt,
    },
};
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;

use clap::Parser;
use reqwest;
use libp2p::{
    gossipsub, mdns, request_response,
    swarm::{SwarmEvent},
    PeerId,
};

use bollard::Docker;
use jocker::exec::{
    // import_docker_image,
    run_docker_job,
};

use comms::{
    p2p::{LocalBehaviourEvent}, notice, compute
};
use dstorage::dfs;

mod job;

// CLI
#[derive(Parser, Debug)]
#[command(name = "Verifier CLI for Wholesum: p2p verifiable computing marketplace.")]
#[command(author = "Wholesum team")]
#[command(version = "0.1")]
#[command(about = "Yet another verifiable compute marketplace.", long_about = None)]
struct Cli {
    #[arg(short, long)]
    dfs_config_file: Option<String>,
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("<-> `Verifier` agent for Wholesum network <->");
    
    let cli = Cli::parse();
    
    
    // FairOS-dfs http client
    let dfs_config_file = cli.dfs_config_file
        .ok_or_else(|| "FairOS-dfs config file is missing.")?;
    let dfs_config = toml::from_str(&std::fs::read_to_string(dfs_config_file)?)?;

    let dfs_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60)) //@ how much timeout is enough?
        .build()
        .expect("FairOS-dfs server should be available and be running to continue.");
    let dfs_cookie = dfs::login(
        &dfs_client, 
        &dfs_config
    ).await
    .expect("Login failed, shutting down.");
    assert_ne!(
        dfs_cookie, String::from(""),
        "Cookie from FairOS-dfs cannot be empty."
    );

    println!("Connecting to docker daemon...");
    let docker_con = Docker::connect_with_socket_defaults()?;

    let mut jobs = HashMap::<String, job::Job>::new();
    let mut job_execution_futures = FuturesUnordered::new();
        
    // Libp2p swarm 
    let mut swarm = comms::p2p::setup_local_swarm();

    // read full lines from stdin
    // let mut input = io::BufReader::new(io::stdin()).lines().fuse();

    // listen on all interfaces and whatever port the os assigns
    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // oh shit here we go again
    loop {
        select! {
            // line = input.select_next_some() => {
            //   if let Err(e) = swarm
            //     .behaviour_mut().gossipsub
            //     .publish(topic.clone(), line.expect("Stdin not to close").as_bytes()) {
            //       println!("Publish error: {e:?}")
            //     }
            // },
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(LocalBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discovered a new peer: {peer_id}");
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },

                SwarmEvent::Behaviour(LocalBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discovered peer has expired: {peer_id}");
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    }
                },

                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Local node is listening on {address}");
                },

                SwarmEvent::Behaviour(LocalBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message,
                    ..
                })) => {
                    // let msg_str = String::from_utf8_lossy(&message.data);
                    // println!("Got message: '{}' with id: {id} from peer: {peer_id}",
                    //          msg_str);
                    // println!("received gossip message: {:#?}", message);
                    // first byte is message identifier                
                    let notice_req = notice::Notice::try_from(message.data[0])?;
                    match notice_req {
                        
                        notice::Notice::Verification => {                            
                            println!("`need verification` request from client: `{peer_id}`");
                            // engage with the client through a direct p2p channel
                            let sw_req_id = swarm
                                .behaviour_mut().req_resp
                                .send_request(
                                    &peer_id,
                                    notice::Request::VerificationOffer,
                                );
                            println!("verification offer was sent, id: {sw_req_id}");
                        },

                        notice::Notice::JobStatus => {
                            // job status inquiry
                            // servers are lazy with job updates so clients need to query for their job's status every so often

                            // bytes [1-16] determine the job id 
                            // let bytes_id = match message.data[1..=17].try_into() {
                            //     Ok(b) => b,
                            //     Err(e) => {
                            //         println!("Invalid job id for `job-status` request, {e:?}");
                            //         continue;
                            //     },
                            // };
                            // let job_id = Uuid::from_bytes(bytes_id).to_string();
                            // println!("`job-status` request from client: `{}`",
                            //     peer_id);
                            let updates = job_status_of_peer(
                                &jobs,
                                peer_id
                            ); 
                            if updates.len() > 0 {
                                let sw_req_id = swarm
                                    .behaviour_mut().req_resp
                                    .send_request(
                                        &peer_id,
                                        notice::Request::UpdateForJobs(updates),
                                    );
                                // println!("jobs' status was sent to the client. req_id: `{sw_req_id}`");                            
                            }
                        },

                        _ => (),
                    };
                },
                
                // incoming response to an earlier compute/verify offer
                SwarmEvent::Behaviour(LocalBehaviourEvent::ReqResp(request_response::Event::Message{
                    peer: peer_id,
                    message: request_response::Message::Response {
                        response,
                        ..
                    }
                })) => {                    
                    match response {
                        notice::Response::DeclinedOffer => {
                            println!("Offer decliend by the client: `{peer_id}`");
                        },                        
                        
                        notice::Response::VerificationJob(verification_details) => {                           
                            println!("Received `verification job` request from client: `{}`, job: `{:#?}`",
                                peer_id, verification_details);                           
                            // no duplicate jobs are allowed
                            if jobs.contains_key(&verification_details.job_id) {
                                println!("Duplicate verification job, ignored.");
                                continue;
                            }
                            // bring in the receipt                            
                            let v_job_id = match prepare_verification_job(
                                &dfs_client, &dfs_config, &dfs_cookie,
                                &verification_details,
                            ).await {
                                Ok(new_job_id) => new_job_id,
                                Err(e) => {
                                    println!("Failed to prepare the receipt for verification: `{e:?}`");
                                    continue;
                                }
                            };
                            println!("Receipt is ready to be verified: `{v_job_id}`");
                            // run the job
                            let verification_image = String::from("rezahsnz/risc0-warrant");
                            let local_receipt_path = String::from("/home/prince/residue/receipt");
                            let command = vec![
                                String::from("/bin/sh"),
                                String::from("-c"),
                                format!(
                                    "/home/prince/warrant --image-id {} --receipt-file {}",
                                    verification_details.risc0_image_id,
                                    local_receipt_path,
                                )
                            ]; 
                            let residue_path = format!(
                                "{}/verify/{}/residue",
                                job::get_residue_path()?,
                                v_job_id
                            );                          
                            job_execution_futures.push(
                                run_docker_job(
                                    &docker_con,
                                    v_job_id.clone(),
                                    verification_image,
                                    command,
                                    residue_path.clone()
                                )
                            );                          
                            // keep track of running jobs
                            jobs.insert(
                                v_job_id.clone(),
                                job::Job {
                                    id: job::JobId {
                                        local_id: v_job_id,
                                        network_id: verification_details.job_id.clone(),
                                    },
                                    owner: peer_id,
                                    status: job::Status::DockerWarmingUp,
                                    residue: job::Residue {
                                        receipt_cid: Some(verification_details.receipt_cid.clone()),
                                    },
                                },
                            );
                        },

                        _ => (),
                    }
                },

                _ => {}

            },

            // verification job is finished
            job_exec_res = job_execution_futures.select_next_some() => {                
                if let Err(failed) = job_exec_res {
                    println!("Failed to run the job: `{:#?}`", failed);       
                    //@ what to do with job_id?                    
                    let _job_id = failed.who;
                    //@ imply verification_failed?
                    continue;
                }
                let result = job_exec_res.unwrap();
                if false == jobs.contains_key(&result.job_id) {
                    println!("Critical error: job `{}` is missing.", result.job_id);
                    //@ what to do here?
                    continue;
                }
                let job = jobs.get_mut(&result.job_id).unwrap();                
                if result.exit_status_code != 0 { 
                    job.status = job::Status::VerificationFailed;
                    println!("Job `{}`'s execution finished with error: `{}`",
                        result.job_id,
                        result.error_message.unwrap_or_else(|| String::from("")),
                    );
                } else {
                    job.status = job::Status::VerificationSucceeded;
                }
                println!("verification result: {:#?}", job.status);
            },            
        }
    }
}

// retrieve all status of jobs owned by the peer_id
fn job_status_of_peer(
    jobs: &HashMap::<String, job::Job>,
    peer_id: PeerId
) -> Vec<compute::JobUpdate> {
    let mut updates = Vec::<compute::JobUpdate>::new();
    let iter = jobs.values().filter(|&j| j.owner == peer_id);
    for job in iter {
        let status = match job.status {
            
            job::Status::VerificationFailed => {
                compute::JobStatus::VerificationFailed(
                    job.residue.receipt_cid.clone().unwrap()
                )
            },

            job::Status::VerificationSucceeded => {
                compute::JobStatus::VerificationSucceeded(
                    job.residue.receipt_cid.clone().unwrap()
                )
            },

            // all the rest are trivial status
            _ => compute::JobStatus::Running,
        };
        updates.push(compute::JobUpdate {
            id: job.id.network_id.clone(),
            status: status,
        });
    }
    updates
}

async fn prepare_verification_job(
    dfs_client: &reqwest::Client,
    dfs_config: &dfs::Config,
    dfs_cookie: &String,
    verification_details: &compute::VerificationDetails,
) -> Result<String, Box<dyn Error>> {
    // download receipt from the dfs pod and put it into the docker volume
    dfs::fork_pod(
        dfs_client, dfs_config, dfs_cookie,
        verification_details.receipt_cid.clone(),
    ).await?;
    dfs::open_pod(
        dfs_client, dfs_config, dfs_cookie,
        verification_details.pod_name.clone(),
    ).await?;
    // put the receipt inside a docker volume
    let v_job_id = format!("v_{}", verification_details.job_id);    
    let residue_path = format!(
        "{}/verify/{}/residue",
        job::get_residue_path()?,
        v_job_id
    );
    std::fs::create_dir_all(residue_path.clone())?;
    dfs::download_file(
        dfs_client, dfs_config, dfs_cookie,
        verification_details.pod_name.clone(),
        format!("/receipt"),
        format!("{residue_path}/receipt")
    ).await?;
    Ok(v_job_id)
}