
# Wholesum network `verifier` CLI

## Overview

Wholesum network is a p2p verifiable computing network. It builds on top of [Risc0](https://risczero.com/), [Swarm](https://ethswarm.org), [FairOS-dfs](https://github.com/fairDataSociety/fairOS-dfs), and [Libp2p](https://libp2p.io) to facilitate verifiable computing at scale.  

## How to run

Running a verifier is no easy task and involves getting some dependencies up and running.

### Prerequisites

You would need to get certain environments ready for the verifier to function properly.

#### Swarm

You would need a swarm node(configured as a LightNode) to enable decentralized storage. Head to the [docs](https://docs.ethswarm.org/docs/bee/installation/quick-start) and get it up and running. We recommend using the Swarm Desktop app as it is very easy to config and use. Remember, your verifier is going to store things on Swarm so your wallet needs to be funded before storage is possible.

#### FairOS-dfs server and its wallet

Once Swarm is up and running, you should setup `FairOS-dfs` server. dfs server makes it possible to create advanced objects on top of Swarm. Head to the [docs](https://docs.fairos.fairdatasociety.org/docs/fairOS-dfs/quickstart) and get it up.

Once dfs server is up, you need to create a wallet and put the credentials in a file structured as below:

<pre>
# a fairos-dfs client
endpoint = "http://localhost:9090"
username = "foo"
password = "password for foo"
</pre>

Save it, say, to `dfs-user.toml` and setup is complete. To create a wallet with the above credentials, go to the [registration](https://create.fairdatasociety.org) web app and create your desired username. Pleae note that you would need Sepolia(an Ethereum testnet) eth to register users.

#### Docker 

Docker is needed as it is going to run `Risc0` images. Make sure docker runtime is properly setup. This awesome [guide](https://www.digitalocean.com/community/tutorials/how-to-install-and-use-docker-on-ubuntu-20-04) is also helpful.

### How to run

To run a verifier agent, you would first need to fork the [comms](https://github.com/WholesumNet/comms) library and put it in the parent("..") directory of the verifier directory.

Now everything is ready to serve clients. Hit<br>
`cargo run -- -d dfs-user.toml`

and wait for verification requests!

### USAGE

<pre>
Usage: verifier [OPTIONS]

Options:
  -d, --dfs-config-file <DFS_CONFIG_FILE>  
  -h, --help                               Print help
  -V, --version                            Print version
</pre>
