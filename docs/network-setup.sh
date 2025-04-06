#!/bin/bash

BRIDGE_NAME=msbr0

# create bridge
sudo ip link add ${BRIDGE_NAME} type bridge

# create netns (one for each microservice)
sudo ip netns add ns1
sudo ip netns add ns2

# bring up the bridge
sudo ip link set dev ${BRIDGE_NAME} up

# create veth pairs
sudo ip link add veth_ns1 type veth peer name veth_ns1_${BRIDGE_NAME}
sudo ip link add veth_ns2 type veth peer name veth_ns2_${BRIDGE_NAME}

# connect the netns to bridge
sudo ip link set veth_ns1 netns ns1
sudo ip link set veth_ns1_${BRIDGE_NAME} master ${BRIDGE_NAME}

sudo ip link set veth_ns2 netns ns2
sudo ip link set veth_ns2_${BRIDGE_NAME} master ${BRIDGE_NAME}

# bring up all interfaces
sudo ip netns exec ns1 ip link set veth_ns1 up
sudo ip link set veth_ns1_${BRIDGE_NAME} up

sudo ip netns exec ns2 ip link set veth_ns2 up
sudo ip link set veth_ns2_${BRIDGE_NAME} up

# configure ip's
sudo ip netns exec ns1 ip addr add 10.1.1.2/24 dev veth_ns1
sudo ip netns exec ns2 ip addr add 10.1.1.3/24 dev veth_ns2

# check connectivity
sudo ip netns exec ns1 ping 10.1.1.2
sudo ip netns exec ns2 ping 10.1.1.3

# configure bridge ip
sudo ip addr add 10.1.1.1/24 dev ${BRIDGE_NAME}

# enable ip forwarding
sudo sysctl -w net.ipv4.ip_forward=1

# test bridge and each ns
ping 10.1.1.1
ping 10.1.1.2
ping 10.1.1.2

# list netns
sudo ip netns

# delete ns
sudo ip netns delete ns1
sudo ip netns delete ns2

# delete bridge
sudo ip link delete ${BRIDGE_NAME}


