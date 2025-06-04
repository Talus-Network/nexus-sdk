# Restricting access to Leader nodes

## Overview

To enhance security on your Tool, it's essential to configure your server to accept incoming connections solely from the authorized Leader nodes. This guide provides step-by-step instructions to achieve this using UFW.

## Leader Node IP Addresses

Ensure that only the following IP addresses are permitted:

- `35.211.137.107`
- `35.207.42.45`

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

Permit incoming connections from each Leader node IP address:

```bash
sudo ufw allow from 35.211.137.107
sudo ufw allow from 35.207.42.45
```

{% hint style="info" %}

If you wish to restrict access to specific ports (e.g., SSH on port 22), modify the commands as follows:

```bash
sudo ufw allow from 35.211.137.107 to any port 22 proto tcp
sudo ufw allow from 35.207.42.45 to any port 22 proto tcp
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

By following this guide, your Tool will be configured to accept connections only from the specified Leader nodes, enhancing the security of your deployment.
