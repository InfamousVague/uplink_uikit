#!/bin/bash

# Read the values from ./warp/peerID
local_peer_id=$(grep -o 'Local PeerID: [^[:space:]]*' ./warp/peerID.txt | awk '{print $NF}')

# Update the values in ./common/src/warp_runner/mod.rs and store in a temporary file
sed -e "s#/ip4/159.65.41.31/tcp/8848/p2p/12D3KooWRF2bz3KDRPvBs1FASRDRk7BfdYc1RUcfwKsz7UBEu7mL#/ip4/127.0.0.1/tcp/4444/p2p/$local_peer_id#" ./common/src/warp_runner/mod.rs > temp_file

# Replace the original mod.rs with the modified content
mv temp_file ./common/src/warp_runner/mod.rs
