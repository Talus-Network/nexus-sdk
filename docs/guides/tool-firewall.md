# Restricting access to Leader nodes (optional)

## Overview

Nexus Tools authenticate `POST /invoke` requests via **signed HTTP** (Ed25519 signatures) and are typically deployed behind **HTTPS**.
In most deployments, this cryptographic authentication is the primary access control mechanism and you do **not** need to rely on IP allowlists.

However, you may still want a firewall as defense-in-depth to reduce generic internet traffic to your Tool’s ingress (for example, only allowing traffic from your reverse proxy / load balancer, or from a fixed set of Leader node egress IPs if Nexus provides them).

This guide provides step-by-step instructions to restrict inbound traffic using UFW.

See also:

- [Tool Communication (HTTPS + Signed HTTP)](tool-communication.md)

## Leader Node IP Addresses

If Nexus provides fixed egress IP addresses for its Leader nodes, ensure that only those IPs are permitted.

{% hint style="warning" %}
Do not assume Leader node IPs are permanent. If IPs change, your Tool will become unreachable even if signed HTTP is configured correctly.
{% endhint %}

## Prerequisites

- Ubuntu server with [UFW](https://help.ubuntu.com/community/UFW) installed.
- Administrative privileges (`sudo` access).

## Step-by-Step Instructions

### 1. Enable UFW

If UFW is not already active, enable it:

```bash
sudo ufw enable
```

### 2. Set Default Policies

Configure UFW to deny all incoming connections by default and allow all outgoing connections:

```bash
sudo ufw default deny incoming
sudo ufw default allow outgoing
```

### 3. Allow Connections from Leader Nodes

Permit incoming connections from each allowed IP address:

```bash
sudo ufw allow from <LEADER_IP_1>
sudo ufw allow from <LEADER_IP_2>
```

{% hint style="info" %}

If you wish to restrict access to specific ports (e.g., SSH on port 22), modify the commands as follows:

```bash
sudo ufw allow from <LEADER_IP_1> to any port 22 proto tcp
sudo ufw allow from <LEADER_IP_2> to any port 22 proto tcp
```

{% endhint %}

### 4. Verify UFW Rules

Check the current UFW status and rules to confirm the configuration:

```bash
sudo ufw status verbose
```

You should see entries indicating that connections from the specified IP addresses are allowed.

## Additional Resources

- [How to Set Up a Firewall with UFW on Ubuntu](https://www.digitalocean.com/community/tutorials/how-to-set-up-a-firewall-with-ufw-on-ubuntu)
- [UFW Essentials: Common Firewall Rules and Commands](https://www.digitalocean.com/community/tutorials/ufw-essentials-common-firewall-rules-and-commands)

---

By following this guide, your Tool will be configured to accept connections only from the specified Leader nodes, enhancing security as defense-in-depth.
