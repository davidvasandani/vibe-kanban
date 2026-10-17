# Remote machine management

Tags: `vk/24ed-remote-machine-m`

## Paired identity survives unavailable discovery

Remote Access manages relay pairings, independently of cluster membership.
Render paired records independently of the available-host selector and role
setup. A machine that cannot be discovered may be the one the user needs to
remove; an empty pairing-candidate list must never hide existing pairings.

Join live metadata to paired records by host ID. Successful discovery with a
missing host means offline; failed or not-yet-completed discovery means unknown.
Unpaired is distinct from both. Only online machines enable Open workspaces.
Keep Remove available when the host is offline or status cannot be checked.
Use native buttons, not a clickable row div with nested action buttons.

## Ownership and errors

Local paired inventory comes from relayApi.listPairedRelayHosts; remote-web
inventory comes from browser relay pairing storage. Gate these queries by
runtime. Do not turn storage/network exceptions into successful empty lists.
Settings uses a strict local inventory query while preserving the app-bar hook's
existing behavior. Pair/remove refresh both inventory and live discovery.

Removal forgets this client's pairing, not remote workspaces. Confirmation names
the machine; removing the route's active host closes settings and navigates home.
No SSH, power, service or cluster operation belongs to this flow.

## Verification

Rendered tests cover both runtimes for empty discovery with offline pairings,
unknown connectivity, failed inventory and retry, online navigation, unpaired
status, cancellation/failure of removal, active-host removal, and initial target
selection. Landing tests ensure inventory is present without choosing a role,
Host/Client setup stays available and sign-in gates inventory queries.
