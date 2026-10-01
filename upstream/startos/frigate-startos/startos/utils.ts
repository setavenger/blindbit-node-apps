// Host id Frigate binds its Electrum interface on, and the container port it
// listens on. Exported so dependents resolve Frigate over the bridge without a
// literal.
export const electrumHostId = 'electrum'
export const electrumPort = 50001
