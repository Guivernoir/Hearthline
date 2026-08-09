# Customer LAN

The Customer LAN is the private residential network inside the customer
premises. It owns the endpoints, access switch, and router inside interface.

![Customer LAN physical view](screenshot.png)

![Customer LAN logical view](logical-screenshot.png)

![Customer PC-01 session network state](workstation-screenshot.png)

## Implementation Status

The physical and logical LAN diagrams are implemented. Addressing and
per-appliance interfaces are now represented in parsed YAML and available from
each device inspector. Customer PC-01 and PC-02 are enterable as responsive
workstations whose profiles, terminals, browsers, and activity traces are
supplied by the local Rust API. Each endpoint has independent DNS, public
HTTPS, and perimeter-denied SSH scenarios. Both can also run `ping` against a
destination covered by one of their validated route templates; the terminal
resolves names when required, constructs up to four ICMP probes in Rust, and
reports verified replies, loss, timing, and per-probe traces. Arbitrary
topology discovery remains unimplemented. The local API now retains a separate
DNS client cache for each workstation, expires entries after 60 seconds of
deterministic session time, and exposes `ipconfig /displaydns` and
`ipconfig /flushdns`. Browser, `curl`, `ping`, and SSH resolution consult this
cache; `nslookup` intentionally queries the configured DNS server every time.
Each API workstation also owns one mutable union of its compatible baseline
scenario appliances and media. DNS, HTTPS, ICMP, and SSH therefore reuse the
same endpoint ARP cache and customer-edge PAT table. Browser details expose ARP
and PAT counts, while terminal `arp -a` reads the actual simulated endpoint
table. The workstation Network State application enumerates the appliances in
that isolated compatible runtime and presents their active switch CAM,
neighbor, PAT, and firewall-session tables. Its capability-gated simulator
console executes bounded read-only `show` commands in Rust. This interface is
session instrumentation; it does not grant the customer endpoint privileged
device access and does not represent a global management plane.
Customer PC-01 can also run configured traversal,
disallowed-method, and SQL-injection request-body probes with `curl`; all three
are prevented by the business DMZ WAF and create distinct defensive evidence
for the Central SOC session.

## Scope

```text
Customer PC-01 --+
                 +-- Customer SW-01 -- Customer RTR-01 Gi0/0
Customer PC-02 --+                         |
                                      Customer Edge
```

The modem, access network, and provider next hop belong to
[Customer Edge](../customer-edge/README.md).

## Inventory

| Asset | Role | Address |
| --- | --- | --- |
| `Customer PC-01` | Customer workstation | `192.168.0.2/24` |
| `Customer PC-02` | Customer workstation | `192.168.0.3/24` |
| `Customer SW-01` | Layer 2 access switch | Layer 2 |
| `Customer RTR-01` | Default gateway and routed boundary | `192.168.0.1/24` inside |

Both workstations use `192.168.0.1` as their default gateway and
`198.51.100.50` as their public DNS resolver.

## Physical Connectivity

| Local endpoint | Remote endpoint | Purpose |
| --- | --- | --- |
| `Customer PC-01 FastEthernet0` | `Customer SW-01 FastEthernet0/1` | Workstation access |
| `Customer PC-02 FastEthernet0` | `Customer SW-01 FastEthernet0/2` | Workstation access |
| `Customer SW-01 GigabitEthernet0/1` | `Customer RTR-01 GigabitEthernet0/0` | Default-gateway handoff |

The access links share one private Layer 2 domain. No trunk or inter-VLAN
routing is required for the baseline.

## Validation Targets

- Both workstations can reach the default gateway.
- Both workstations can communicate through the access switch.
- Non-local traffic is sent to `Customer RTR-01`.
- The LAN has no direct provider or business attachment.
- Translation and public reachability are evaluated at the next levels.

The four assets, their baseline interfaces, and the private prefix are parsed
from canonical YAML. The selected DNS and public-service scenarios construct
the originating workstation, access switch, and router from those records and
exercise their selected links. PC-01 and PC-02 terminal and browser actions
use independent source addresses and scenario documents for `nslookup`,
HTTPS, and denied SSH. Interactive `ping` replaces a compatible path's
application packet with ICMP while retaining its validated participants and
media; the endpoint and DNS-server YAML `respond_to_icmp` value controls
whether Rust emits an echo reply. LAN-only reachability scenarios remain
planned. The DNS cache is local API-session state: it is isolated by
workstation and reset by configuration changes or API restart. The same reset
boundary applies to the retained network runtime. Controlled outage, recovery,
continuity, HA-isolation, and autonomy scenarios remain fresh simulation
workspace runs and cannot be inserted into an interactive workstation session.

PC-01 additionally selects a security exercise only for its exact configured
method, path, and request body. `curl -I` follows the normal configured HTTPS
scenario, whereas the traversal URL, `curl -X DELETE` request, and quoted
`curl --data "username=admin' OR '1'='1"` payload select their respective
controlled denials. A benign quoted POST uses the normal HTTPS path. Ordinary
browsing continues to use GET on the successful public HTTPS scenario.
