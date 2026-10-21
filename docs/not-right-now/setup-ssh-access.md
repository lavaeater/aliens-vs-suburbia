# Setting up SSH access to this machine

A friendly, step-by-step guide to reach this computer (`size-isnt-everything`,
EndeavourOS / Arch-based, user `tommie`) over SSH — first on your **home network**,
then safely **from the internet**.

> **Read this first — the one-paragraph summary.**
> Setting up SSH on your local network is easy and safe. Exposing it directly to the
> internet by forwarding a port on your router is *possible* but genuinely risky —
> bots scan the whole internet for open SSH ports within minutes. **The recommended
> approach for remote access is a VPN (Tailscale is the easiest by far), which needs
> zero router configuration and is dramatically more secure.** Port-forwarding
> instructions are included at the end for completeness, hardened as much as possible.

Everything you type at a terminal is shown as `$ command`. Run them **on this machine**
unless a step says "on your laptop/phone".

---

## Part 0 — What we're building & key concepts (30-second read)

- **SSH** = Secure SHell. An encrypted way to get a terminal (and copy files) on this
  machine from another device.
- **`sshd`** = the SSH *server/daemon* — the background program that listens for
  incoming connections. It must be running here for anyone to connect *to* this machine.
- **SSH keys** vs **passwords**: keys are a pair of files — a *private* key (a secret you
  keep on your laptop/phone) and a *public* key (safe to share, installed on this
  machine). Key auth is both more convenient and far more secure than passwords. We'll
  disable password login entirely once keys work.
- **LAN** (Local Area Network) = your home Wi‑Fi/ethernet. Reaching the machine here is
  low-risk.
- **WAN / the internet** = everything outside your router. Reaching the machine from
  here is where the risk lives, and where the VPN comes in.

---

## Part 1 — Turn on the SSH server (do this once)

### 1.1 Install OpenSSH

Arch/EndeavourOS ship the server in the `openssh` package. Check and install:

```bash
$ pacman -Qi openssh        # is it already installed? (no error = yes)
$ sudo pacman -S openssh    # install it if the above said "package not found"
```

### 1.2 Start it and enable it at boot

```bash
$ sudo systemctl enable --now sshd
$ systemctl status sshd      # should say "active (running)". Press q to exit.
```

`enable` = start automatically on every boot. `--now` = also start it right now.

### 1.3 Find this machine's local address

You'll need this to connect from another device on your network.

```bash
$ ip -4 addr show | grep -oP '(?<=inet\s)\d+(\.\d+){3}' | grep -v 127.0.0.1
```

You'll see something like `192.168.1.42`. That's your **LAN IP**. Note it down.
(Addresses starting `192.168.`, `10.`, or `172.16–31.` are private/LAN addresses.)

> **Tip:** LAN IPs can change when leases expire. To make it stable, either set a
> **DHCP reservation** in your router (bind this machine's MAC address to a fixed IP —
> see Part 6), or just use the hostname: many networks let you connect to
> `size-isnt-everything.local` (via mDNS/Avahi) instead of the raw number.

### 1.4 Test from another device on the same Wi‑Fi

On your laptop (macOS/Linux have `ssh` built in; on Windows use PowerShell or
Windows Terminal, which also have it):

```bash
$ ssh tommie@192.168.1.42        # use the IP from step 1.3
```

Type your normal login password when prompted. If you land at a shell prompt on
`size-isnt-everything`, **the server works.** Type `exit` to disconnect.

If it hangs or is refused, jump to **Troubleshooting** at the bottom.

---

## Part 2 — Switch to SSH keys (strongly recommended)

Passwords over SSH can be brute-forced. Keys effectively can't. Do this now, while
you still have password login as a fallback.

### 2.1 Create a key pair — **on the device you'll connect FROM** (your laptop/phone)

Not on this machine. On your **laptop**:

```bash
$ ssh-keygen -t ed25519 -C "tommie-laptop"
```

- Press Enter to accept the default location (`~/.ssh/id_ed25519`).
- **Set a passphrase** when asked. This encrypts the private key so a stolen laptop
  doesn't hand over your access. (Your OS keychain/`ssh-agent` will remember it so you
  don't retype it constantly.)

This creates two files on your laptop:
- `~/.ssh/id_ed25519` — **private**. Never share, never copy off the laptop.
- `~/.ssh/id_ed25519.pub` — **public**. This is what goes on the server.

### 2.2 Install your public key onto this machine

Easiest, from your **laptop**:

```bash
$ ssh-copy-id tommie@192.168.1.42
```

It'll ask for your password one last time, then append your public key to
`~/.ssh/authorized_keys` on this machine.

<details>
<summary>Manual alternative if <code>ssh-copy-id</code> isn't available (e.g. on Windows)</summary>

On your laptop, print the public key:

```bash
$ cat ~/.ssh/id_ed25519.pub     # (PowerShell: type $env:USERPROFILE\.ssh\id_ed25519.pub)
```

Copy the whole line. Then **on this machine**:

```bash
$ mkdir -p ~/.ssh && chmod 700 ~/.ssh
$ nano ~/.ssh/authorized_keys   # paste the line, save with Ctrl-O Enter, exit Ctrl-X
$ chmod 600 ~/.ssh/authorized_keys
```
</details>

### 2.3 Verify key login works

From your **laptop**:

```bash
$ ssh tommie@192.168.1.42
```

If it logs in **without asking for your account password** (it may ask for your *key
passphrase* — that's fine and different), keys are working. **Do not proceed to Part 3
until this works**, or you can lock yourself out.

---

## Part 3 — Harden the SSH server

Edit the server config **on this machine**:

```bash
$ sudo nano /etc/ssh/sshd_config
```

Prefer dropping a small override file instead of hand-editing the big one — it's cleaner
and survives package updates:

```bash
$ sudo nano /etc/ssh/sshd_config.d/10-hardening.conf
```

Put these lines in it:

```
# Only allow this user to log in over SSH
AllowUsers tommie

# Keys only — no passwords, no keyboard-interactive
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes

# Never allow root to log in directly
PermitRootLogin no

# Small quality-of-life / safety tweaks
MaxAuthTries 3
LoginGraceTime 20
X11Forwarding no
```

Save, then check the config is valid and reload:

```bash
$ sudo sshd -t                 # prints nothing if the config is OK
$ sudo systemctl reload sshd
```

> **Safety net:** keep your current SSH session open while you open a *second* new
> session to confirm you can still get in. If the new one fails, you can fix the config
> in the still-open first session. Never reload-and-close your only connection.

### 3.1 (Optional) Change the port

Moving off port 22 doesn't add real security, but it hugely cuts log noise from bots.
If you want it, add to the same file:

```
Port 2222
```

Then reload, and remember to use `ssh -p 2222 tommie@...` and open `2222` in the
firewall (Part 4) instead of `22`. **If you also set up the firewall, change the port
there too, or you'll lock yourself out.**

---

## Part 4 — Firewall (recommended)

A firewall ensures *only* the ports you intend are reachable. EndeavourOS often ships
with `ufw` or `firewalld`; pick whichever you have (or install one).

### Option A — `ufw` (simplest)

```bash
$ sudo pacman -S ufw
$ sudo ufw default deny incoming
$ sudo ufw default allow outgoing
$ sudo ufw allow 22/tcp          # or 2222/tcp if you changed the port
$ sudo ufw enable
$ sudo systemctl enable --now ufw
$ sudo ufw status verbose
```

To be stricter and only allow SSH *from your LAN*:

```bash
$ sudo ufw allow from 192.168.1.0/24 to any port 22 proto tcp
```

### Option B — `firewalld`

```bash
$ sudo pacman -S firewalld
$ sudo systemctl enable --now firewalld
$ sudo firewall-cmd --permanent --add-service=ssh
$ sudo firewall-cmd --reload
```

---

## Part 5 — Block brute-force attempts with fail2ban (recommended if ever exposed)

`fail2ban` watches the SSH log and temporarily bans IPs that fail to log in repeatedly.

```bash
$ sudo pacman -S fail2ban
$ sudo cp /etc/fail2ban/jail.conf /etc/fail2ban/jail.local  # if jail.local doesn't exist
```

Edit `/etc/fail2ban/jail.local` and make sure the `[sshd]` section is enabled:

```
[sshd]
enabled = true
backend = systemd
maxretry = 3
bantime  = 1h
findtime = 10m
```

(`backend = systemd` is correct on Arch/EndeavourOS since logs go to the journal.)

```bash
$ sudo systemctl enable --now fail2ban
$ sudo fail2ban-client status sshd     # see current bans
```

---

## Part 6 — Reaching the machine FROM THE INTERNET

This is the part to be careful with. Two paths — I **strongly** recommend the first.

### 6A — RECOMMENDED: a mesh VPN (Tailscale) — no router changes, very secure

Tailscale builds a private encrypted network between *your* devices only. Nothing is
exposed to the public internet, there are **no ports to forward**, and it even works
behind carrier-grade NAT where port forwarding is impossible. This is what I'd use.

**On this machine:**

```bash
$ sudo pacman -S tailscale
$ sudo systemctl enable --now tailscaled
$ sudo tailscale up
```

It prints a URL — open it in a browser, log in (Google/GitHub/etc.), and this machine
joins your private "tailnet". Find its Tailscale IP (a `100.x.y.z` address):

```bash
$ tailscale ip -4
```

**On your laptop/phone:** install the Tailscale app (from tailscale.com, your app store,
or your distro's package manager) and log in with the *same account*.

Now, from anywhere in the world:

```bash
$ ssh tommie@100.x.y.z            # the Tailscale IP
# or, with MagicDNS enabled in the Tailscale admin console:
$ ssh tommie@size-isnt-everything
```

That's it. Your SSH server never needs to be internet-facing, so keep the firewall
(Part 4) allowing SSH only from your LAN and the Tailscale interface. Optional extras
worth enabling in the Tailscale admin console: **MagicDNS** (nice hostnames),
**key expiry** and **ACLs** (limit which devices can reach this one).

> **WireGuard alternative:** Tailscale is WireGuard under the hood with the hard parts
> automated. If you want a self-hosted, no-third-party option, you can run raw
> **WireGuard** (`wg-quick`) yourself, or self-host the Tailscale control plane with
> **Headscale**. Both are more setup than Tailscale but keep everything under your
> control. Say the word and I'll write that up separately.

### 6B — Only if you truly need it: port-forwarding on your router

This exposes SSH to the entire internet. Do it **only** after Parts 2–5 are done
(keys-only, root disabled, firewall on, fail2ban running). Never expose password auth.

**Concept:** your router has one public IP. "Port forwarding" tells it: *"traffic
arriving on port X from outside → send it to this machine's LAN IP on port Y."*

**Steps:**

1. **Give this machine a fixed LAN IP.** In your router's admin page (usually
   `http://192.168.1.1` or `http://192.168.0.1` in a browser — check the sticker on the
   router for the address and login), find **DHCP reservation** / **Static lease** and
   bind this machine's MAC address to a fixed IP (e.g. `192.168.1.42`). Get the MAC
   with:
   ```bash
   $ ip link show | grep -A1 'state UP' | grep link/ether
   ```

2. **Create the port-forward rule.** In the router, find **Port Forwarding** /
   **Virtual Server** / **NAT** (naming varies by brand — Asus, TP-Link, Netgear,
   Fritz!Box all differ). Add a rule:
   - **External/WAN port:** pick a high, non-obvious number, e.g. `52200` (do **not**
     use 22 — it's hammered constantly).
   - **Internal/LAN IP:** `192.168.1.42` (from step 1).
   - **Internal/LAN port:** `22` (or `2222` if you changed it in Part 3).
   - **Protocol:** TCP.
   - Save/apply. Some routers require a reboot.

3. **Allow the port through the machine's firewall** (whatever you forwarded *to*,
   e.g. `22` or `2222`). You did this in Part 4.

4. **Find your public IP** (run on this machine):
   ```bash
   $ curl -4 ifconfig.me ; echo
   ```
   Home IPs usually change over time. Fix that with **Dynamic DNS**: services like
   DuckDNS, No‑IP, or Cloudflare give you a stable name (e.g. `tommie.duckdns.org`)
   that auto-updates to your current IP. Many routers have a built-in DDNS client;
   otherwise run the provider's small updater on this machine.

5. **Connect from outside** (e.g. off Wi‑Fi, on mobile data):
   ```bash
   $ ssh -p 52200 tommie@your.public.ip.or.ddns.name
   ```

**Extra hardening if you go this route:**
- Keep `fail2ban` running (Part 5) — non-optional here.
- Consider a non-22 external port purely to cut scan noise.
- Watch the logs: `journalctl -u sshd -f` shows live login attempts (you'll be
  shocked how many bots try within an hour).
- **Best of both:** even with a forward, prefer connecting via Tailscale (6A) and treat
  the forward as an emergency backdoor you can disable.

> **Bottom line:** if 6A (Tailscale) covers your needs — and for personal remote SSH it
> almost always does — **skip 6B entirely.** An unforwarded machine simply cannot be
> attacked from the internet.

---

## Part 7 — Quality-of-life: an SSH config alias

On your **laptop**, create/edit `~/.ssh/config`:

```
Host home
    HostName 100.x.y.z          # Tailscale IP, or LAN IP, or DDNS name
    User tommie
    Port 22                     # match whatever you set
    IdentityFile ~/.ssh/id_ed25519
```

Now you just type:

```bash
$ ssh home
```

`scp`/`sftp`/`rsync` all understand this alias too, e.g. `scp file home:~/`.

---

## Troubleshooting

| Symptom | Likely cause & fix |
|---|---|
| `Connection refused` | `sshd` not running (`sudo systemctl status sshd`) or firewall blocking the port (Part 4). |
| `Connection timed out` | Wrong IP, wrong network, or (from outside) the port-forward/DDNS isn't set up. On LAN, ping the IP first. |
| `Permission denied (publickey)` | Key not installed correctly. Re-check Part 2.2; ensure `~/.ssh` is `700` and `authorized_keys` is `600`; check `AllowUsers` includes `tommie`. |
| Asked for a password after Part 3 | Key auth failed and fell back — but you disabled passwords, so it just fails. Fix the key (Part 2) from your still-open session. |
| Locked out after editing config | Use a still-open session, or physical access to this machine, to revert `/etc/ssh/sshd_config.d/10-hardening.conf` and `sudo systemctl reload sshd`. |
| Live debugging | Server side: `journalctl -u sshd -f`. Client side: `ssh -vvv tommie@host` prints exactly where it fails. |

---

## Quick reference — the safe recommended path

1. `sudo pacman -S openssh && sudo systemctl enable --now sshd`
2. On laptop: `ssh-keygen -t ed25519` → `ssh-copy-id tommie@<LAN-IP>`
3. Harden: keys-only, `PermitRootLogin no`, `AllowUsers tommie` (Part 3)
4. Firewall on, SSH allowed from LAN only (Part 4)
5. `fail2ban` on (Part 5)
6. Remote access via **Tailscale** (Part 6A) — *not* router port-forwarding
7. `ssh home` alias (Part 7)

Do these and you have secure access from your couch or from another continent, with
nothing exposed to the open internet.
